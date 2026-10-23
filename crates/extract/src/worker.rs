//! Fil qui possède l'unique instance de PDFium du processus.
//!
//! PDFium ne peut être chargé qu'une fois par processus et n'est pas sûr
//! entre fils : toutes les opérations (lecture, rendu, assemblage) passent par
//! ce fil, une par une. Une panique de `pdfium-render` est rattrapée et
//! devient une erreur « document endommagé ». Un crash du code C++ de PDFium,
//! lui, arrête le processus : l'isolation dans un processus séparé est prévue
//! au prompt 13a.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::OnceLock;
use std::sync::mpsc::{self, Sender, SyncSender};
use std::thread;

use minim_core::ports::{ConvertError, DocumentKind, RawDocument, ReadError, RenderOptions};
use pdfium_render::prelude::Pdfium;

use crate::pdf::LibraryUnavailable;
use crate::{convert, pdf};

/// Opération confiée au fil PDFium.
pub(crate) enum Job {
    /// Lecture du texte et des métadonnées d'un PDF.
    Read {
        bytes: Vec<u8>,
        max_pages: u32,
        reply: Sender<Result<RawDocument, ReadError>>,
    },
    /// Rendu des pages d'un PDF en images, envoyées une par une.
    Render {
        bytes: Vec<u8>,
        max_pages: u32,
        options: RenderOptions,
        events: SyncSender<RenderEvent>,
    },
    /// Assemblage d'images en PDF.
    Compose {
        images: Vec<(DocumentKind, Vec<u8>)>,
        reply: Sender<Result<Vec<u8>, ConvertError>>,
    },
}

/// Avancement d'un rendu.
pub(crate) enum RenderEvent {
    /// Image d'une page (numéro à partir de 1).
    Page(u32, Vec<u8>),
    /// Rendu terminé, avec le nombre de pages.
    Done(u32),
    /// Rendu interrompu.
    Failed(ConvertError),
}

static WORKER: OnceLock<Result<Sender<Job>, LibraryUnavailable>> = OnceLock::new();

/// File des opérations du fil PDFium, démarré au premier appel avec la
/// bibliothèque du dossier `library_dir`.
pub(crate) fn jobs(library_dir: &Path) -> Result<Sender<Job>, LibraryUnavailable> {
    WORKER.get_or_init(|| start(library_dir)).clone()
}

fn start(library_dir: &Path) -> Result<Sender<Job>, LibraryUnavailable> {
    let library = Pdfium::pdfium_platform_library_name_at_path(library_dir);
    let (ready_tx, ready_rx) = mpsc::channel();
    let (jobs_tx, jobs_rx) = mpsc::channel::<Job>();
    thread::Builder::new()
        .name("minim-pdfium".to_owned())
        .spawn(move || {
            let pdfium = match Pdfium::bind_to_library(library) {
                Ok(bindings) => Pdfium::new(bindings),
                Err(_) => {
                    let _ = ready_tx.send(false);
                    return;
                }
            };
            let _ = ready_tx.send(true);
            for job in jobs_rx {
                run(&pdfium, job);
            }
        })
        .map_err(|_| LibraryUnavailable)?;
    match ready_rx.recv() {
        Ok(true) => Ok(jobs_tx),
        _ => Err(LibraryUnavailable),
    }
}

/// Exécute une opération. Le demandeur a pu abandonner (délai dépassé) :
/// l'envoi de la réponse échoue alors sans effet.
fn run(pdfium: &Pdfium, job: Job) {
    match job {
        Job::Read {
            bytes,
            max_pages,
            reply,
        } => {
            let result = catch_unwind(AssertUnwindSafe(|| pdf::parse(pdfium, &bytes, max_pages)))
                .unwrap_or(Err(ReadError::Corrupted));
            let _ = reply.send(result);
        }
        Job::Render {
            bytes,
            max_pages,
            options,
            events,
        } => {
            let event = catch_unwind(AssertUnwindSafe(|| {
                convert::render(pdfium, &bytes, max_pages, options, &events)
            }))
            .unwrap_or(Err(ReadError::Corrupted.into()))
            .map_or_else(RenderEvent::Failed, RenderEvent::Done);
            let _ = events.send(event);
        }
        Job::Compose { images, reply } => {
            let result = catch_unwind(AssertUnwindSafe(|| convert::compose(pdfium, &images)))
                .unwrap_or(Err(ReadError::Corrupted.into()));
            let _ = reply.send(result);
        }
    }
}
