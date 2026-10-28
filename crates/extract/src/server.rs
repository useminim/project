//! Côté processus PDFium : charge la bibliothèque, puis traite les demandes
//! reçues sur l'entrée standard et répond sur la sortie standard, jusqu'à la
//! fin de l'entrée (application arrêtée).
//!
//! Une panique de `pdfium-render` est rattrapée et devient « document
//! endommagé » ; un crash de PDFium arrête ce processus seulement.

use std::ffi::OsString;
use std::io::{self, BufWriter, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use minim_core::ports::{ConvertError, ReadError};
use pdfium_render::prelude::Pdfium;

use crate::process::WORKER_ARG;
use crate::wire::{self, Request, Response};
use crate::{convert, pdf};

/// Code de sortie : PDFium introuvable.
const EXIT_LIBRARY_UNAVAILABLE: i32 = 3;
/// Code de sortie : arguments ou protocole incorrects.
const EXIT_USAGE: i32 = 2;

/// Si les arguments de l'exécutable (`std::env::args_os()`) demandent le
/// processus PDFium ([`WORKER_ARG`] puis le dossier de la bibliothèque), le
/// fait tourner et renvoie son code de sortie ; sinon renvoie `None`.
///
/// L'application l'appelle au tout début de son démarrage, avant d'ouvrir
/// une fenêtre.
pub fn serve_if_requested(args: impl IntoIterator<Item = OsString>) -> Option<i32> {
    let mut args = args.into_iter().skip(1);
    if args.next()? != WORKER_ARG {
        return None;
    }
    Some(args.next().map_or(EXIT_USAGE, |dir| serve(Path::new(&dir))))
}

/// Fait tourner le processus PDFium avec la bibliothèque du dossier
/// `library_dir` ; renvoie le code de sortie.
#[must_use]
pub fn serve(library_dir: &Path) -> i32 {
    let mut output = BufWriter::new(io::stdout().lock());
    let library = Pdfium::pdfium_platform_library_name_at_path(library_dir);
    let Ok(bindings) = Pdfium::bind_to_library(library) else {
        let _ = wire::write_frame(&mut output, &Response::LibraryUnavailable.encode());
        return EXIT_LIBRARY_UNAVAILABLE;
    };
    let pdfium = Pdfium::new(bindings);
    if wire::write_frame(&mut output, &Response::Ready.encode()).is_err() {
        return 0;
    }
    let mut input = io::stdin().lock();
    loop {
        let frame = match wire::read_frame(&mut input) {
            Ok(Some(frame)) => frame,
            // Fin de l'entrée : l'application s'est arrêtée.
            Ok(None) | Err(_) => return 0,
        };
        let Ok(request) = Request::decode(&frame) else {
            return EXIT_USAGE;
        };
        test_hooks::before(&request);
        if handle(&pdfium, request, &mut output).is_err() {
            return 0;
        }
    }
}

fn handle(pdfium: &Pdfium, request: Request, output: &mut impl Write) -> io::Result<()> {
    let send = |output: &mut dyn Write, response: Response| -> io::Result<()> {
        let mut output = output;
        wire::write_frame(&mut output, &response.encode())
    };
    let response = match request {
        Request::Read { max_pages, bytes } => {
            catch_unwind(AssertUnwindSafe(|| pdf::parse(pdfium, &bytes, max_pages)))
                .unwrap_or(Err(ReadError::Corrupted))
                .map_or_else(|error| Response::Failed(error.into()), Response::Document)
        }
        Request::Render {
            max_pages,
            options,
            bytes,
        } => {
            let mut written = Ok(());
            let result = catch_unwind(AssertUnwindSafe(|| {
                convert::render(pdfium, &bytes, max_pages, options, &mut |number, image| {
                    written = send(output, Response::Page(number, image));
                    written.is_ok()
                })
            }))
            .unwrap_or(Err(ReadError::Corrupted.into()));
            written?;
            result.map_or_else(Response::Failed, Response::Done)
        }
        Request::Compose { images } => {
            catch_unwind(AssertUnwindSafe(|| convert::compose(pdfium, &images)))
                .unwrap_or(Err(ConvertError::from(ReadError::Corrupted)))
                .map_or_else(Response::Failed, Response::Pdf)
        }
    };
    send(output, response)
}

/// Comportements de test, seulement dans les binaires de développement
/// (`debug_assertions`) : un document qui commence par `%MINIM-TEST:` simule
/// un crash (`abort`), une boucle sans fin (`hang`) ou une consommation de
/// mémoire (`eat-memory`).
mod test_hooks {
    use crate::wire::Request;

    #[cfg(debug_assertions)]
    pub(super) fn before(request: &Request) {
        let bytes = match request {
            Request::Read { bytes, .. } | Request::Render { bytes, .. } => bytes.as_slice(),
            Request::Compose { .. } => return,
        };
        let Some(hook) = bytes.strip_prefix(b"%MINIM-TEST:") else {
            return;
        };
        match hook {
            b"abort" => std::process::abort(),
            b"hang" => loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            },
            b"eat-memory" => {
                // 1 Gio écrit octet par octet : réellement occupé.
                let memory = vec![1_u8; 1024 * 1024 * 1024];
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    std::hint::black_box(&memory);
                }
            }
            _ => {}
        }
    }

    #[cfg(not(debug_assertions))]
    pub(super) const fn before(_request: &Request) {}
}
