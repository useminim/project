//! Lecture du corpus `fixtures/pdf/` avec la vraie bibliothèque PDFium,
//! installée par `scripts/fetch-pdfium.mjs` (ou désignée par
//! `MINIM_PDFIUM_DIR`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "fichier de tests : clippy.toml n'autorise unwrap que dans les fonctions #[test]"
)]

use std::time::Duration;

use minim_core::ports::{DocumentKind, DocumentReader as _, RawDocument, ReadError};
use minim_extract::{PdfLimits, PdfReader};

mod common;

const FACTURE: &[u8] = include_bytes!("../../../fixtures/pdf/facture.pdf");
const POLICE: &[u8] = include_bytes!("../../../fixtures/pdf/police.pdf");
const SCAN: &[u8] = include_bytes!("../../../fixtures/pdf/scan.pdf");
const VIDE: &[u8] = include_bytes!("../../../fixtures/pdf/vide.pdf");
const CHIFFRE: &[u8] = include_bytes!("../../../fixtures/pdf/chiffre.pdf");

fn reader(limits: PdfLimits) -> PdfReader {
    PdfReader::new(common::worker(limits))
}

fn read(bytes: &[u8]) -> Result<RawDocument, ReadError> {
    reader(PdfLimits::default()).read(bytes, DocumentKind::Pdf)
}

fn all_text(document: &RawDocument) -> String {
    document
        .pages
        .iter()
        .map(|page| page.text.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn reads_an_invoice_with_positions() {
    let document = read(FACTURE).unwrap();

    assert_eq!(document.pages.len(), 1);
    let text = all_text(&document);
    for expected in [
        "Facture n° 2026-0042",
        "Dupont SARL",
        "1 250,00",
        "Échéance",
    ] {
        assert!(text.contains(expected), "« {expected} » manque");
    }
    let spans = &document.pages[0].text.spans;
    assert!(!spans.is_empty());
    for span in spans {
        let bounds = span.bounds;
        assert!((0.0..=1.0).contains(&bounds.x) && (0.0..=1.0).contains(&bounds.y));
        assert!(bounds.x + bounds.width <= 1.0 + 1e-4 && bounds.y + bounds.height <= 1.0 + 1e-4);
    }
    // Le titre se trouve en haut de la page.
    let title = spans
        .iter()
        .find(|span| span.text.contains("Facture"))
        .unwrap();
    assert!(title.bounds.y < 0.2, "{:?}", title.bounds);
    assert!(!document.metadata.restricted);
    assert!(document.metadata.created_at.is_some());
}

#[test]
fn reads_embedded_fonts() {
    let text = all_text(&read(POLICE).unwrap());

    assert!(text.contains("Attestation de stage"));
    assert!(text.contains("Élodie Exemple"));
}

#[test]
fn finds_no_text_in_a_scan_or_a_blank_page() {
    let scan = read(SCAN).unwrap();
    assert_eq!(scan.pages.len(), 2);
    assert!(scan.pages.iter().all(|page| page.text.is_empty()));

    let blank = read(VIDE).unwrap();
    assert_eq!(blank.pages.len(), 1);
    assert!(blank.pages[0].text.is_empty());
    assert!(blank.pages[0].text.spans.is_empty());
}

#[test]
fn reports_an_encrypted_document() {
    assert!(matches!(read(CHIFFRE), Err(ReadError::Encrypted)));
}

#[test]
fn reports_damaged_documents_without_panicking() {
    let truncated = &FACTURE[..FACTURE.len() * 6 / 10];
    for bytes in [
        truncated,
        b"ceci n'est pas un PDF".as_slice(),
        b"".as_slice(),
    ] {
        assert!(
            matches!(read(bytes), Err(ReadError::Corrupted)),
            "{} octets",
            bytes.len()
        );
    }
}

#[test]
fn enforces_the_limits() {
    let small = reader(PdfLimits {
        max_bytes: 1024,
        ..PdfLimits::default()
    });
    assert!(matches!(
        small.read(FACTURE, DocumentKind::Pdf),
        Err(ReadError::TooLarge { limit_bytes: 1024 })
    ));

    let short = reader(PdfLimits {
        max_pages: 1,
        ..PdfLimits::default()
    });
    assert!(matches!(
        short.read(SCAN, DocumentKind::Pdf),
        Err(ReadError::TooManyPages { limit: 1 })
    ));

    let hasty = reader(PdfLimits {
        timeout: Duration::ZERO,
        ..PdfLimits::default()
    });
    assert!(matches!(
        hasty.read(SCAN, DocumentKind::Pdf),
        Err(ReadError::Timeout)
    ));
}

#[test]
fn reads_only_pdf() {
    let reader = reader(PdfLimits::default());

    assert!(reader.supports(DocumentKind::Pdf));
    assert!(!reader.supports(DocumentKind::Png));
    assert!(matches!(
        reader.read(FACTURE, DocumentKind::Docx),
        Err(ReadError::Unsupported(DocumentKind::Docx))
    ));
}

#[test]
fn never_prints_the_text_of_a_document() {
    let document = read(FACTURE).unwrap();

    let output = format!("{document:?}");
    assert!(!output.contains("Dupont"), "{output}");
}
