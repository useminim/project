//! Processus PDFium de l'application (D-36) : l'exécutable de minim se relance
//! lui-même avec l'argument `--pdf-worker`, et la bibliothèque PDFium vient
//! des ressources de l'application (`pdfium/`).

use std::path::Path;

use minim_core::ports::{ConvertError, DocumentConverter, DocumentKind, ReadError, RenderOptions};
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

/// Convertisseur de l'application : PDFium dans son processus, ou, si le
/// processus ne peut pas être préparé, un convertisseur qui répond
/// « indisponible ».
pub fn converter(worker: Option<&PdfWorker>) -> std::sync::Arc<dyn DocumentConverter> {
    match worker {
        Some(worker) => std::sync::Arc::new(minim_extract::PdfConverter::new(worker.clone())),
        None => std::sync::Arc::new(Unavailable),
    }
}

/// Convertisseur utilisé quand PDFium ne peut pas être lancé.
struct Unavailable;

/// PDFium ne peut pas être lancé.
#[derive(Debug, thiserror::Error)]
#[error("processus PDFium non préparé")]
struct NotPrepared;

impl DocumentConverter for Unavailable {
    fn max_input_bytes(&self) -> u64 {
        u64::MAX
    }

    fn pdf_to_images(
        &self,
        _pdf: &[u8],
        _options: RenderOptions,
        _page: &mut dyn FnMut(u32, Vec<u8>) -> std::io::Result<()>,
    ) -> Result<u32, ConvertError> {
        Err(ReadError::Unavailable(Box::new(NotPrepared)).into())
    }

    fn images_to_pdf(&self, _images: &[(DocumentKind, &[u8])]) -> Result<Vec<u8>, ConvertError> {
        Err(ReadError::Unavailable(Box::new(NotPrepared)).into())
    }
}
