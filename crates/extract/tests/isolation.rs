//! Isolation de PDFium dans un processus séparé : un document qui fait
//! planter, boucler ou gonfler PDFium donne une erreur typée dans le délai,
//! et le document suivant est traité normalement.
//!
//! Les comportements « crash », « boucle » et « mémoire » sont simulés par le
//! processus de test (`%MINIM-TEST:…`, binaires de développement seulement) ;
//! les documents piégés du corpus, eux, sont de vrais PDF.

#![allow(
    clippy::unwrap_used,
    reason = "fichier de tests : clippy.toml n'autorise unwrap que dans les fonctions #[test]"
)]

use std::time::{Duration, Instant};

use minim_core::ports::{
    ConvertError, DocumentConverter as _, DocumentKind, DocumentReader as _, ImageFormat,
    ReadError, RenderOptions,
};
use minim_extract::{PdfConverter, PdfLimits, PdfReader};

mod common;

const FACTURE: &[u8] = include_bytes!("../../../fixtures/pdf/facture.pdf");
const BOUCLE: &[u8] = include_bytes!("../../../fixtures/pdf/boucle.pdf");
const BOMBE: &[u8] = include_bytes!("../../../fixtures/pdf/bombe.pdf");
const GEANTE: &[u8] = include_bytes!("../../../fixtures/pdf/geante.pdf");

fn limits() -> PdfLimits {
    PdfLimits {
        timeout: Duration::from_secs(20),
        ..PdfLimits::default()
    }
}

fn reads_the_invoice(reader: &PdfReader) {
    let document = reader.read(FACTURE, DocumentKind::Pdf).unwrap();
    assert!(document.pages[0].text.text.contains("Dupont SARL"));
}

#[test]
fn survives_a_crash_of_pdfium() {
    let reader = PdfReader::new(common::worker(limits()));

    let result = reader.read(b"%MINIM-TEST:abort", DocumentKind::Pdf);

    assert!(matches!(result, Err(ReadError::Corrupted)), "{result:?}");
    reads_the_invoice(&reader);
}

#[test]
fn stops_a_document_that_never_ends() {
    let reader = PdfReader::new(common::worker(PdfLimits {
        timeout: Duration::from_secs(1),
        ..limits()
    }));

    let started = Instant::now();
    let result = reader.read(b"%MINIM-TEST:hang", DocumentKind::Pdf);

    assert!(matches!(result, Err(ReadError::Timeout)), "{result:?}");
    assert!(started.elapsed() < Duration::from_secs(5));
    reads_the_invoice(&reader);
}

#[test]
fn stops_a_document_that_eats_memory() {
    let limit = 256 * 1024 * 1024;
    let reader = PdfReader::new(common::worker(PdfLimits {
        max_memory: limit,
        ..limits()
    }));

    let started = Instant::now();
    let result = reader.read(b"%MINIM-TEST:eat-memory", DocumentKind::Pdf);

    assert!(
        matches!(result, Err(ReadError::TooLarge { limit_bytes }) if limit_bytes == limit),
        "{result:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(10));
    reads_the_invoice(&reader);
}

#[test]
fn shares_one_process_between_reader_and_converter() {
    let worker = common::worker(limits());
    let reader = PdfReader::new(worker.clone());
    let converter = PdfConverter::new(worker);

    assert!(
        reader
            .read(b"%MINIM-TEST:abort", DocumentKind::Pdf)
            .is_err()
    );
    let mut pages = 0;
    converter
        .pdf_to_images(FACTURE, png(), &mut |_, _| {
            pages += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(pages, 1);
    reads_the_invoice(&reader);
}

fn png() -> RenderOptions {
    RenderOptions {
        format: ImageFormat::Png,
        dpi: 72,
    }
}

/// Lit et rend un document piégé : le résultat doit arriver dans le délai,
/// sous forme de document ou d'erreur typée, sans arrêter l'application.
fn trap(bytes: &[u8]) -> (Result<usize, ReadError>, Result<u32, ConvertError>) {
    let worker = common::worker(limits());
    let reader = PdfReader::new(worker.clone());
    let converter = PdfConverter::new(worker);
    let started = Instant::now();

    let read = reader
        .read(bytes, DocumentKind::Pdf)
        .map(|document| document.pages.len());
    let rendered = converter.pdf_to_images(bytes, png(), &mut |_, _| Ok(()));

    assert!(started.elapsed() < Duration::from_secs(45));
    reads_the_invoice(&reader);
    (read, rendered)
}

#[test]
fn handles_a_page_tree_that_contains_itself() {
    let (read, rendered) = trap(BOUCLE);

    assert!(
        matches!(read, Ok(0) | Err(ReadError::Corrupted)),
        "{read:?}"
    );
    assert!(
        matches!(
            rendered,
            Ok(0) | Err(ConvertError::Read(ReadError::Corrupted))
        ),
        "{rendered:?}"
    );
}

#[test]
fn handles_a_decompression_bomb() {
    let (read, rendered) = trap(BOMBE);

    let typed = |error: &ReadError| {
        matches!(
            error,
            ReadError::Corrupted | ReadError::TooLarge { .. } | ReadError::Timeout
        )
    };
    assert!(
        read.as_ref().map_or_else(typed, |&pages| pages == 1),
        "{read:?}"
    );
    assert!(
        match &rendered {
            Ok(pages) => *pages == 1,
            Err(ConvertError::Read(error)) => typed(error),
            Err(_) => false,
        },
        "{rendered:?}"
    );
}

#[test]
fn refuses_to_render_giant_pages() {
    let (read, rendered) = trap(GEANTE);

    assert_eq!(read.unwrap(), 2);
    assert!(
        matches!(rendered, Err(ConvertError::PageTooLarge)),
        "{rendered:?}"
    );
}
