//! Lecteurs et convertisseurs de documents de minim : implémentations de
//! `DocumentReader` et `DocumentConverter`.
//!
//! - PDF : [`PdfReader`] et [`PdfConverter`], avec PDFium (D-34). Les binaires
//!   de PDFium sont fournis par `scripts/fetch-pdfium.mjs` dans les ressources
//!   de l'application.
//! - PDFium tourne dans un processus séparé ([`PdfWorker`], D-36) : un
//!   document piégé ne peut ni faire tomber ni bloquer l'application.

pub mod convert;
pub mod pdf;
pub mod process;
mod server;
mod wire;

pub use convert::PdfConverter;
pub use pdf::{LibraryUnavailable, PdfLimits, PdfReader};
pub use process::{PdfWorker, WORKER_ARG, WorkerCommand, WorkerUnreachable};
pub use server::{serve, serve_if_requested};
