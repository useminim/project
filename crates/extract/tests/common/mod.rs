//! Processus PDFium des tests : l'exécutable `minim-pdf-worker` de la crate,
//! avec la bibliothèque installée par `scripts/fetch-pdfium.mjs` (ou désignée
//! par `MINIM_PDFIUM_DIR`).

#![allow(
    dead_code,
    clippy::expect_used,
    reason = "module partagé par plusieurs fichiers de tests"
)]

use std::path::PathBuf;

use minim_extract::{PdfLimits, PdfWorker, WorkerCommand};

pub fn library_dir() -> PathBuf {
    std::env::var_os("MINIM_PDFIUM_DIR").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../apps/desktop/src-tauri/resources/pdfium")
        },
        PathBuf::from,
    )
}

pub fn command() -> WorkerCommand {
    WorkerCommand::dedicated(env!("CARGO_BIN_EXE_minim-pdf-worker"))
}

/// Processus PDFium démarré, avec `limits`.
pub fn worker_with(command: WorkerCommand, limits: PdfLimits) -> PdfWorker {
    let worker = PdfWorker::new(command, library_dir(), limits);
    worker.start().expect(
        "PDFium introuvable : lancer `node scripts/fetch-pdfium.mjs` (fait par `pnpm test`)",
    );
    worker
}

pub fn worker(limits: PdfLimits) -> PdfWorker {
    worker_with(command(), limits)
}
