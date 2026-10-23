//! Workflow complet sur le disque avec la vraie bibliothèque PDFium : le
//! moteur d'actions de `minim-core` et le convertisseur de `minim-extract`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "fichier de tests : clippy.toml n'autorise unwrap que dans les fonctions #[test]"
)]

use std::fs;
use std::path::PathBuf;

use chrono::NaiveDate;
use image::GenericImageView as _;
use minim_core::ExecutionStatus;
use minim_core::actions::{ExecutionRequest, Tools, run_workflow};
use minim_core::execution::OperationKind;
use minim_core::matching::FileFacts;
use minim_core::ports::StdFileSystem;
use minim_core::testing::samples;
use minim_extract::{PdfConverter, PdfLimits};
use serde_json::json;

fn converter() -> PdfConverter {
    let dir = std::env::var_os("MINIM_PDFIUM_DIR").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../apps/desktop/src-tauri/resources/pdfium")
        },
        PathBuf::from,
    );
    PdfConverter::start(&dir, PdfLimits::default()).expect("PDFium introuvable")
}

#[test]
fn converts_a_scan_into_images_then_files_the_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let inbox = dir.path().join("Reçus");
    let archive = dir.path().join("Archives");
    fs::create_dir_all(&inbox).unwrap();
    fs::create_dir_all(&archive).unwrap();
    let document = inbox.join("Scan facture.pdf");
    fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pdf/scan.pdf"),
        &document,
    )
    .unwrap();
    let facts = FileFacts::from_path(&document).unwrap();
    let mut workflow = samples::workflow(samples::workflow_id(1), 10);
    workflow.actions = serde_json::from_value(json!([
        { "type": "convert", "to": "jpeg", "dpi": 100 },
        { "type": "move", "destination": archive }
    ]))
    .unwrap();
    assert_eq!(workflow.validate(), Ok(()));
    let converter = converter();
    let request = ExecutionRequest {
        document_id: samples::document_id(1),
        location: &document,
        file: &facts,
        file_date: None,
        today: NaiveDate::from_ymd_opt(2026, 10, 22).unwrap(),
        started_at: samples::date(0),
    };

    let run = run_workflow(
        Tools {
            fs: &StdFileSystem,
            converter: &converter,
        },
        &workflow,
        samples::run_id(1),
        &request,
        true,
    );

    assert_eq!(
        run.log.status,
        ExecutionStatus::Success,
        "{:?}",
        run.failure
    );
    assert_eq!(run.location, archive.join("Scan facture.pdf"));
    assert!(run.location.exists());
    let mut produced: Vec<_> = fs::read_dir(&inbox)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    produced.sort();
    assert_eq!(produced, ["Scan facture_p1.jpg", "Scan facture_p2.jpg"]);
    for name in &produced {
        let page = image::open(inbox.join(name)).unwrap();
        // Format Letter (612 × 792 points) à 100 points par pouce.
        assert_eq!(page.dimensions(), (850, 1100));
    }
    let operations = &run.log.actions[0].operations;
    assert_eq!(operations.len(), 2);
    assert!(
        operations
            .iter()
            .all(|operation| operation.kind == OperationKind::Convert)
    );
}
