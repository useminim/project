//! Bibliothèque PDFium absente ou processus impossible à lancer.

use minim_core::ports::{DocumentKind, DocumentReader as _, ReadError};
use minim_extract::{PdfLimits, PdfReader, PdfWorker, WorkerCommand};

#[test]
fn reports_a_missing_library() {
    let worker = PdfWorker::new(
        WorkerCommand::dedicated(env!("CARGO_BIN_EXE_minim-pdf-worker")),
        "/dossier/absent",
        PdfLimits::default(),
    );

    assert!(matches!(worker.start(), Err(ReadError::Unavailable(_))));
    let reader = PdfReader::new(worker);
    assert!(matches!(
        reader.read(b"%PDF-1.7", DocumentKind::Pdf),
        Err(ReadError::Unavailable(_))
    ));
}

#[test]
fn reports_a_worker_that_cannot_be_launched() {
    let worker = PdfWorker::new(
        WorkerCommand::dedicated("/programme/absent"),
        "/dossier/absent",
        PdfLimits::default(),
    );

    assert!(matches!(worker.start(), Err(ReadError::Unavailable(_))));
}
