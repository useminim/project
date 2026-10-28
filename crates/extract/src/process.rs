//! Processus PDFium séparé (prompt 13a, D-36).
//!
//! PDFium est du code C++ : un PDF malformé ou malveillant peut le faire
//! planter, boucler ou consommer toute la mémoire. Les documents sont donc
//! traités dans un processus à part, lancé à la demande et gardé pour les
//! demandes suivantes. [`PdfWorker`] lui envoie les demandes une par une par
//! un tube (jamais par un fichier) et surveille la réponse :
//!
//! - **crash** : le processus disparaît, la demande échoue avec
//!   [`ReadError::Corrupted`] ;
//! - **délai** : au-delà de [`PdfLimits::timeout`] (par demande, ou par page
//!   pour un rendu), le processus est tué et la demande échoue avec
//!   [`ReadError::Timeout`] ;
//! - **mémoire** : au-delà de [`PdfLimits::max_memory`], mesurée toutes les
//!   [`POLL`], le processus est tué et la demande échoue avec
//!   [`ReadError::TooLarge`].
//!
//! Un processus tué est relancé à la demande suivante. Le processus n'écrit
//! rien sur le disque et ne journalise rien ; sa sortie d'erreur est ignorée.

use std::ffi::OsString;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use minim_core::ports::{ConvertError, ReadError};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use thiserror::Error;

use crate::pdf::{LibraryUnavailable, PdfLimits};
use crate::wire::{self, Request, Response};

/// Argument qui lance un exécutable en processus PDFium, suivi du dossier de
/// la bibliothèque (voir [`crate::serve_if_requested`]).
pub const WORKER_ARG: &str = "--pdf-worker";

/// Intervalle de mesure de la mémoire du processus.
pub const POLL: Duration = Duration::from_millis(50);

/// Délai de démarrage du processus (chargement de PDFium compris).
const STARTUP_TIMEOUT: Duration = Duration::from_secs(20);

/// Commande qui lance le processus PDFium.
#[derive(Debug, Clone)]
pub struct WorkerCommand {
    program: PathBuf,
    args: Vec<OsString>,
}

impl WorkerCommand {
    /// Lance `program` avec l'argument [`WORKER_ARG`] puis le dossier de la
    /// bibliothèque. Pour l'application : son propre exécutable
    /// (`std::env::current_exe()`).
    #[must_use]
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: vec![WORKER_ARG.into()],
        }
    }

    /// Lance `program` avec seulement le dossier de la bibliothèque en
    /// argument (exécutable dédié `minim-pdf-worker`).
    #[must_use]
    pub fn dedicated(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
        }
    }

    fn spawn(&self, library_dir: &Path) -> std::io::Result<Child> {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .arg(library_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            // Pas de fenêtre de console pour le processus (Windows seulement).
            use std::os::windows::process::CommandExt as _;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command.spawn()
    }
}

/// Le processus PDFium a répondu de façon incompréhensible ou n'a pas pu
/// être lancé.
#[derive(Debug, Clone, Copy, Error)]
#[error("processus PDFium injoignable")]
pub struct WorkerUnreachable;

/// Processus PDFium partagé par [`crate::PdfReader`] et
/// [`crate::PdfConverter`]. Les clones désignent le même processus ; les
/// demandes sont traitées une par une.
#[derive(Debug, Clone)]
pub struct PdfWorker {
    config: Arc<Config>,
    state: Arc<Mutex<State>>,
}

#[derive(Debug)]
struct Config {
    command: WorkerCommand,
    library_dir: PathBuf,
    limits: PdfLimits,
}

#[derive(Debug, Default)]
struct State {
    running: Option<Running>,
    system: Option<System>,
}

#[derive(Debug)]
struct Running {
    child: Child,
    input: BufWriter<ChildStdin>,
    output: Receiver<Vec<u8>>,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Suite donnée à une réponse.
pub(crate) enum Step<T> {
    /// Attendre la réponse suivante (page suivante d'un rendu).
    Continue,
    /// Demande terminée.
    Finish(T),
    /// Abandon par l'appelant : le processus, encore occupé, est arrêté.
    Abort(ConvertError),
}

impl PdfWorker {
    /// Processus lancé par `command` avec la bibliothèque PDFium du dossier
    /// `library_dir`. Il n'est lancé qu'à la première demande (ou par
    /// [`Self::start`]).
    #[must_use]
    pub fn new(command: WorkerCommand, library_dir: impl Into<PathBuf>, limits: PdfLimits) -> Self {
        Self {
            config: Arc::new(Config {
                command,
                library_dir: library_dir.into(),
                limits,
            }),
            state: Arc::new(Mutex::new(State::default())),
        }
    }

    /// Limites appliquées.
    #[must_use]
    pub fn limits(&self) -> PdfLimits {
        self.config.limits
    }

    /// Lance le processus tout de suite, pour signaler au démarrage de
    /// l'application une bibliothèque absente.
    ///
    /// # Erreurs
    ///
    /// [`ReadError::Unavailable`] si le processus ne démarre pas ou ne trouve
    /// pas PDFium.
    pub fn start(&self) -> Result<(), ReadError> {
        let mut state = self.lock();
        self.ensure_running(&mut state.running).map(|_| ())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Processus en cours, relancé s'il s'est arrêté.
    fn ensure_running<'a>(
        &self,
        running: &'a mut Option<Running>,
    ) -> Result<&'a mut Running, ReadError> {
        let alive = running
            .as_mut()
            .is_some_and(|running| matches!(running.child.try_wait(), Ok(None)));
        if !alive {
            *running = None;
            *running = Some(self.launch()?);
        }
        running
            .as_mut()
            .ok_or_else(|| ReadError::Unavailable(Box::new(WorkerUnreachable)))
    }

    fn launch(&self) -> Result<Running, ReadError> {
        let unreachable = || ReadError::Unavailable(Box::new(WorkerUnreachable));
        let mut child = self
            .config
            .command
            .spawn(&self.config.library_dir)
            .map_err(|_| unreachable())?;
        let (Some(input), Some(mut stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            return Err(unreachable());
        };
        let (frames, output) = mpsc::channel();
        thread::Builder::new()
            .name("minim-pdfium-output".to_owned())
            .spawn(move || {
                // S'arrête à la fin du processus (fin du flux) ou quand plus
                // personne n'écoute.
                while let Ok(Some(frame)) = wire::read_frame(&mut stdout) {
                    if frames.send(frame).is_err() {
                        break;
                    }
                }
            })
            .map_err(|_| unreachable())?;
        let running = Running {
            child,
            input: BufWriter::new(input),
            output,
        };
        match running
            .output
            .recv_timeout(STARTUP_TIMEOUT)
            .map(|frame| Response::decode(&frame))
        {
            Ok(Ok(Response::Ready)) => Ok(running),
            Ok(Ok(Response::LibraryUnavailable)) => {
                Err(ReadError::Unavailable(Box::new(LibraryUnavailable)))
            }
            _ => Err(unreachable()),
        }
    }

    /// Envoie `request` et passe chaque réponse à `handle` jusqu'à la fin de
    /// la demande. Une réponse d'échec termine la demande sans arrêter le
    /// processus ; un délai dépassé, un excès de mémoire, un crash ou un
    /// abandon l'arrêtent.
    pub(crate) fn exchange<T>(
        &self,
        request: &Request,
        mut handle: impl FnMut(Response) -> Step<T>,
    ) -> Result<T, ConvertError> {
        let limits = self.config.limits;
        let mut guard = self.lock();
        let State {
            running: slot,
            system,
        } = &mut *guard;
        let running = self.ensure_running(slot)?;
        let sent = wire::write_frame(&mut running.input, &request.encode());
        if sent.is_err() {
            // Le processus vient de mourir : il sera relancé à la prochaine
            // demande.
            *slot = None;
            return Err(ReadError::Corrupted.into());
        }
        let pid = Pid::from_u32(running.child.id());
        let system = system.get_or_insert_with(System::new);
        let mut deadline = Instant::now() + limits.timeout;
        let outcome = loop {
            let now = Instant::now();
            if now >= deadline {
                break Err(ReadError::Timeout.into());
            }
            let frame = match running.output.recv_timeout(POLL.min(deadline - now)) {
                Ok(frame) => frame,
                Err(RecvTimeoutError::Timeout) => {
                    if memory(system, pid) > limits.max_memory {
                        break Err(ReadError::TooLarge {
                            limit_bytes: limits.max_memory,
                        }
                        .into());
                    }
                    continue;
                }
                // Crash : le processus a disparu au milieu de la demande.
                Err(RecvTimeoutError::Disconnected) => break Err(ReadError::Corrupted.into()),
            };
            let Ok(response) = Response::decode(&frame) else {
                break Err(ReadError::Unavailable(Box::new(WorkerUnreachable)).into());
            };
            if let Response::Failed(error) = response {
                return Err(error);
            }
            match handle(response) {
                Step::Continue => deadline = Instant::now() + limits.timeout,
                Step::Finish(value) => return Ok(value),
                Step::Abort(error) => break Err(error),
            }
        };
        // Le processus est peut-être encore occupé par ce document : il est
        // arrêté, et relancé à la demande suivante.
        *slot = None;
        outcome
    }
}

/// Mémoire utilisée par le processus `pid`, en octets (0 si inconnue).
fn memory(system: &mut System, pid: Pid) -> u64 {
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_memory(),
    );
    system.process(pid).map_or(0, sysinfo::Process::memory)
}

/// Réponse inattendue pour la demande en cours.
pub(crate) fn unexpected<T>() -> Step<T> {
    Step::Abort(ReadError::Unavailable(Box::new(WorkerUnreachable)).into())
}
