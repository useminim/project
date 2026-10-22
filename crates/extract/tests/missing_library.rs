//! Bibliothèque PDFium absente (processus de test séparé : PDFium ne se
//! charge qu'une fois par processus).

use minim_core::ports::ReadError;
use minim_extract::{PdfLimits, PdfReader};

#[test]
fn reports_a_missing_library() {
    let result = PdfReader::start(
        std::path::Path::new("/dossier/absent"),
        PdfLimits::default(),
    );

    assert!(matches!(result, Err(ReadError::Unavailable(_))));
}
