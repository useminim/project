//! Lecteurs de documents de minim : implémentations de `DocumentReader`.
//!
//! - PDF : [`PdfReader`], avec PDFium (D-34). Les binaires de PDFium sont
//!   fournis par `scripts/fetch-pdfium.mjs` dans les ressources de
//!   l'application.

pub mod pdf;

pub use pdf::{LibraryUnavailable, PdfLimits, PdfReader};
