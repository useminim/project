//! Lecteurs et convertisseurs de documents de minim : implémentations de
//! `DocumentReader` et `DocumentConverter`.
//!
//! - PDF : [`PdfReader`] et [`PdfConverter`], avec PDFium (D-34). Les binaires
//!   de PDFium sont fournis par `scripts/fetch-pdfium.mjs` dans les ressources
//!   de l'application.

pub mod convert;
pub mod pdf;
mod worker;

pub use convert::PdfConverter;
pub use pdf::{LibraryUnavailable, PdfLimits, PdfReader};
