//! Processus PDFium de l'application (D-36) : l'exécutable de minim se relance
//! lui-même avec l'argument `--pdf-worker`, et la bibliothèque PDFium vient
//! des ressources de l'application (`pdfium/`).

use std::path::Path;

use minim_extract::{PdfLimits, PdfWorker, WorkerCommand};

/// Prépare le processus PDFium et le démarre en arrière-plan, pour signaler
/// dans les logs une bibliothèque absente dès le lancement. Renvoie `None` si
/// l'exécutable de l'application est introuvable.
pub fn prepare(resource_dir: &Path) -> Option<PdfWorker> {
    let program = std::env::current_exe().ok()?;
    let worker = PdfWorker::new(
        WorkerCommand::new(program),
        resource_dir.join("pdfium"),
        PdfLimits::default(),
    );
    let starting = worker.clone();
    let spawned = std::thread::Builder::new()
        .name("minim-pdfium-start".to_owned())
        .spawn(move || match starting.start() {
            Ok(()) => tracing::info!("lecteur PDF prêt"),
            Err(error) => tracing::warn!(%error, "lecteur PDF indisponible"),
        });
    if spawned.is_err() {
        tracing::warn!("démarrage du lecteur PDF impossible");
    }
    Some(worker)
}
