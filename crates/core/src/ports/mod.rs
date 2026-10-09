//! Ports : les interfaces de `minim-core` vers l'extérieur (D-09).
//!
//! Les implémentations réelles (lecteurs PDF, OCR, stockage, cloud,
//! imprimante) vivent dans d'autres crates ; `minim-core` ne connaît que ces
//! traits. Tous sont `Send + Sync` et utilisables derrière `Arc<dyn …>`.

pub mod ocr;
pub mod reader;
pub mod store;
pub mod time;

pub use ocr::{Language, OcrEngine, OcrError, PageImage};
pub use reader::{
    DocumentKind, DocumentMetadata, DocumentReader, Page, PageText, RawDocument, ReadError, Rect,
    TextSpan,
};
pub use store::{ExecutionLogRepository, StoreError, WaitingZoneRepository, WorkflowRepository};
pub use time::{Clock, IdGenerator, SystemClock, UlidGenerator};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    // Vérifie à la compilation que chaque port est utilisable derrière
    // `Arc<dyn …>` et partageable entre fils.
    fn assert_shareable<T: ?Sized + Send + Sync>() {}

    #[test]
    fn every_port_is_object_safe_and_shareable() {
        assert_shareable::<Arc<dyn DocumentReader>>();
        assert_shareable::<Arc<dyn OcrEngine>>();
        assert_shareable::<Arc<dyn WorkflowRepository>>();
        assert_shareable::<Arc<dyn ExecutionLogRepository>>();
        assert_shareable::<Arc<dyn WaitingZoneRepository>>();
        assert_shareable::<Arc<dyn Clock>>();
        assert_shareable::<Arc<dyn IdGenerator>>();
    }
}
