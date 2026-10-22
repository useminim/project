use std::fs;
use std::sync::Mutex;

use serde_json::json;

use super::*;
use crate::execution::Fingerprint;
use crate::ports::{DryRunFileSystem, FileMetadata, StdFileSystem};
use crate::testing::{FakeConverter, SequentialIds, samples};

const NAME: &str = "Facture_2026-03-15.pdf";

struct Sandbox {
    _dir: tempfile::TempDir,
    root: PathBuf,
    file: PathBuf,
    facts: FileFacts,
}

fn sandbox() -> Sandbox {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_owned();
    let file = root.join(NAME);
    fs::write(&file, "contenu de la facture").unwrap();
    let facts = FileFacts::from_path(&file).unwrap();
    Sandbox {
        _dir: dir,
        root,
        file,
        facts,
    }
}

impl Sandbox {
    fn request(&self) -> ExecutionRequest<'_> {
        ExecutionRequest {
            document_id: samples::document_id(1),
            location: &self.file,
            file: &self.facts,
            file_date: NaiveDate::from_ymd_opt(2026, 3, 16),
            today: NaiveDate::from_ymd_opt(2026, 10, 16).unwrap(),
            started_at: samples::date(0),
        }
    }

    fn names(&self, folder: &str) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(self.root.join(folder))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn folder(&self, name: &str) -> PathBuf {
        let folder = self.root.join(name);
        fs::create_dir_all(&folder).unwrap();
        folder
    }
}

fn workflow(actions: serde_json::Value) -> Workflow {
    let mut workflow = samples::workflow(samples::workflow_id(1), 10);
    workflow.actions = serde_json::from_value(actions).unwrap();
    workflow
}

fn run(fs: &dyn FileSystem, sandbox: &Sandbox, workflow: &Workflow) -> WorkflowRun {
    run_with(fs, &FakeConverter::default(), sandbox, workflow)
}

fn run_with(
    fs: &dyn FileSystem,
    converter: &FakeConverter,
    sandbox: &Sandbox,
    workflow: &Workflow,
) -> WorkflowRun {
    let tools = Tools { fs, converter };
    run_workflow(
        tools,
        workflow,
        samples::run_id(1),
        &sandbox.request(),
        true,
    )
}

fn statuses(run: &WorkflowRun) -> Vec<ActionStatus> {
    run.log.actions.iter().map(|record| record.status).collect()
}

#[test]
fn renames_the_original_file() {
    let sandbox = sandbox();
    let workflow = workflow(json!([
        { "type": "rename", "template": "{date_in_name:YYYYMMDD} Facture.{extension}" }
    ]));

    let run = run(&StdFileSystem, &sandbox, &workflow);

    assert_eq!(run.failure, None);
    assert_eq!(run.log.status, ExecutionStatus::Success);
    assert_eq!(sandbox.names(""), ["20260315 Facture.pdf"]);
    assert_eq!(run.location, sandbox.root.join("20260315 Facture.pdf"));
    let [operation] = run.log.actions[0].operations.as_slice() else {
        panic!("une seule opération attendue");
    };
    assert_eq!(operation.kind, OperationKind::Rename);
    assert_eq!(operation.from, sandbox.file);
    assert_eq!(
        operation.fingerprint.to_string(),
        "d75bef5392f0d66d769dc67518c0768935a891e9b4e3e187f56f2b304d45b4ba"
    );
}

#[test]
fn never_overwrites_and_adds_a_suffix() {
    let sandbox = sandbox();
    fs::write(sandbox.root.join("Facture.pdf"), "autre").unwrap();
    fs::write(sandbox.root.join("Facture (2).pdf"), "autre").unwrap();

    let run = run(
        &StdFileSystem,
        &sandbox,
        &workflow(json!([{ "type": "rename", "template": "Facture.pdf" }])),
    );

    assert_eq!(run.failure, None);
    assert_eq!(
        sandbox.names(""),
        ["Facture (2).pdf", "Facture (3).pdf", "Facture.pdf"]
    );
    assert_eq!(
        fs::read_to_string(sandbox.root.join("Facture.pdf")).unwrap(),
        "autre"
    );
}

#[test]
fn keeps_a_name_that_does_not_change() {
    let sandbox = sandbox();

    let run = run(
        &StdFileSystem,
        &sandbox,
        &workflow(json!([{ "type": "rename", "template": "{original_name}.{extension}" }])),
    );

    assert_eq!(run.failure, None);
    assert!(run.log.actions[0].operations.is_empty());
    assert_eq!(sandbox.names(""), [NAME]);
}

#[test]
fn changes_only_the_case_of_a_name() {
    let sandbox = sandbox();

    let run = run(
        &StdFileSystem,
        &sandbox,
        &workflow(json!([{ "type": "rename", "template": "FACTURE_2026-03-15.pdf" }])),
    );

    assert_eq!(run.failure, None);
    assert_eq!(sandbox.names(""), ["FACTURE_2026-03-15.pdf"]);
}

#[test]
fn sanitizes_the_rendered_name() {
    let sandbox = sandbox();

    let run = run(
        &StdFileSystem,
        &sandbox,
        &workflow(json!([{ "type": "rename", "template": "CON.pdf" }])),
    );

    assert_eq!(run.failure, None);
    assert_eq!(sandbox.names(""), ["CON_.pdf"]);
}

#[test]
fn moves_and_creates_folders() {
    let sandbox = sandbox();
    let destination = sandbox.root.join("Archives").join("2026");

    let run = run(
        &StdFileSystem,
        &sandbox,
        &workflow(json!([
            { "type": "move", "destination": destination, "create_folders": true }
        ])),
    );

    assert_eq!(run.failure, None);
    assert_eq!(sandbox.names("Archives/2026"), [NAME]);
    assert!(!sandbox.file.exists());
}

#[test]
fn refuses_a_missing_destination() {
    let sandbox = sandbox();

    let run = run(
        &StdFileSystem,
        &sandbox,
        &workflow(json!([{ "type": "move", "destination": sandbox.root.join("absent") }])),
    );

    assert_eq!(run.failure, Some(FailureCode::DestinationMissing));
    assert_eq!(run.log.error.as_deref(), Some("destination_missing"));
    assert!(sandbox.file.exists());
}

#[test]
fn copies_and_keeps_the_original() {
    let sandbox = sandbox();
    let copies = sandbox.folder("Copies");
    fs::write(copies.join(NAME), "autre").unwrap();

    let run = run(
        &StdFileSystem,
        &sandbox,
        &workflow(json!([{ "type": "copy", "destination": copies }])),
    );

    assert_eq!(run.failure, None);
    assert_eq!(
        run.location, sandbox.file,
        "une copie ne déplace pas le document"
    );
    assert_eq!(
        sandbox.names("Copies"),
        ["Facture_2026-03-15 (2).pdf", NAME]
    );
    assert!(sandbox.file.exists());
}

#[test]
fn runs_a_chain_of_three_actions() {
    let sandbox = sandbox();
    let copies = sandbox.folder("Copies");
    let archives = sandbox.root.join("Archives");

    let run = run(
        &StdFileSystem,
        &sandbox,
        &workflow(json!([
            { "type": "rename", "template": "Facture mars.pdf" },
            { "type": "copy", "destination": copies },
            { "type": "move", "destination": archives, "create_folders": true }
        ])),
    );

    assert_eq!(run.failure, None);
    assert_eq!(statuses(&run), [ActionStatus::Success; 3]);
    assert_eq!(sandbox.names("Copies"), ["Facture mars.pdf"]);
    assert_eq!(sandbox.names("Archives"), ["Facture mars.pdf"]);
    assert_eq!(sandbox.names(""), ["Archives", "Copies"]);
}

#[test]
fn rolls_back_the_actions_already_done_when_one_fails() {
    let sandbox = sandbox();
    let copies = sandbox.folder("Copies");

    let run = run(
        &StdFileSystem,
        &sandbox,
        &workflow(json!([
            { "type": "rename", "template": "Facture mars.pdf" },
            { "type": "copy", "destination": copies },
            { "type": "move", "destination": sandbox.root.join("absent") }
        ])),
    );

    assert_eq!(run.failure, Some(FailureCode::DestinationMissing));
    assert_eq!(run.log.status, ExecutionStatus::Failed);
    assert_eq!(
        statuses(&run),
        [
            ActionStatus::RolledBack,
            ActionStatus::RolledBack,
            ActionStatus::Failed
        ]
    );
    assert_eq!(
        sandbox.names(""),
        ["Copies", NAME],
        "le document revient comme avant"
    );
    assert!(sandbox.names("Copies").is_empty());
    assert_eq!(run.location, sandbox.file);
}

#[test]
fn refuses_a_file_changed_since_its_analysis() {
    let sandbox = sandbox();
    fs::write(&sandbox.file, "contenu modifié entre-temps").unwrap();

    let run = run(
        &StdFileSystem,
        &sandbox,
        &workflow(json!([{ "type": "rename", "template": "Facture.pdf" }])),
    );

    assert_eq!(run.failure, Some(FailureCode::FileChanged));
    assert!(run.log.actions.is_empty());
    assert_eq!(sandbox.names(""), [NAME]);
}

#[test]
fn fails_on_a_missing_variable_or_an_unavailable_action() {
    let sandbox = sandbox();
    let cases = [
        (
            json!([{ "type": "rename", "template": "{client_name}.pdf" }]),
            FailureCode::MissingVariable,
        ),
        (
            json!([
                { "type": "rename", "template": "Facture.pdf" },
                { "type": "extract_fields", "fields": ["date"], "output": "csv" }
            ]),
            FailureCode::ActionNotAvailable,
        ),
    ];
    for (actions, expected) in cases {
        let run = run(&StdFileSystem, &sandbox, &workflow(actions));

        assert_eq!(run.failure, Some(expected));
        assert_eq!(sandbox.names(""), [NAME]);
    }
}

/// Disque qui refuse les liens physiques, comme entre deux volumes ou sur une
/// clé USB en FAT.
struct NoHardLinks;

impl FileSystem for NoHardLinks {
    fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        StdFileSystem.metadata(path)
    }
    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        StdFileSystem.create_dir_all(path)
    }
    fn link_new(&self, _from: &Path, _to: &Path) -> io::Result<()> {
        Err(io::ErrorKind::CrossesDevices.into())
    }
    fn copy_new(&self, from: &Path, to: &Path) -> io::Result<()> {
        StdFileSystem.copy_new(from, to)
    }
    fn write_new(&self, path: &Path, content: &[u8]) -> io::Result<()> {
        StdFileSystem.write_new(path, content)
    }
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        StdFileSystem.read(path)
    }
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        StdFileSystem.remove_file(path)
    }
    fn fingerprint(&self, path: &Path) -> io::Result<Fingerprint> {
        StdFileSystem.fingerprint(path)
    }
}

#[test]
fn moves_across_volumes_by_a_verified_copy() {
    let sandbox = sandbox();
    let archives = sandbox.folder("Archives");

    let run = run(
        &NoHardLinks,
        &sandbox,
        &workflow(json!([{ "type": "move", "destination": archives }])),
    );

    assert_eq!(run.failure, None);
    assert!(!sandbox.file.exists());
    assert_eq!(
        fs::read_to_string(sandbox.root.join("Archives").join(NAME)).unwrap(),
        "contenu de la facture"
    );
}

/// Disque dont la n-ième suppression échoue (droits retirés entre-temps).
struct FailingRemoval {
    calls: Mutex<u32>,
    fail_on: u32,
}

impl FileSystem for FailingRemoval {
    fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        StdFileSystem.metadata(path)
    }
    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        StdFileSystem.create_dir_all(path)
    }
    fn link_new(&self, from: &Path, to: &Path) -> io::Result<()> {
        StdFileSystem.link_new(from, to)
    }
    fn copy_new(&self, from: &Path, to: &Path) -> io::Result<()> {
        StdFileSystem.copy_new(from, to)
    }
    fn write_new(&self, path: &Path, content: &[u8]) -> io::Result<()> {
        StdFileSystem.write_new(path, content)
    }
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        StdFileSystem.read(path)
    }
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        let mut calls = self.calls.lock().unwrap();
        *calls += 1;
        if *calls == self.fail_on {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        StdFileSystem.remove_file(path)
    }
    fn fingerprint(&self, path: &Path) -> io::Result<Fingerprint> {
        StdFileSystem.fingerprint(path)
    }
}

#[test]
fn reports_what_stays_done_when_a_roll_back_fails() {
    let sandbox = sandbox();
    // Suppression 1 : le renommage ; suppression 2 : son annulation échoue.
    let disk = FailingRemoval {
        calls: Mutex::new(0),
        fail_on: 2,
    };

    let run = run(
        &disk,
        &sandbox,
        &workflow(json!([
            { "type": "rename", "template": "Facture mars.pdf" },
            { "type": "move", "destination": sandbox.root.join("absent") }
        ])),
    );

    assert_eq!(run.failure, Some(FailureCode::DestinationMissing));
    assert_eq!(
        statuses(&run),
        [ActionStatus::Success, ActionStatus::Failed]
    );
    assert_eq!(
        sandbox.names(""),
        ["Facture mars.pdf"],
        "un seul exemplaire, jamais deux"
    );
    assert_eq!(run.location, sandbox.root.join("Facture mars.pdf"));
}

#[test]
fn undoes_an_operation_only_if_the_file_is_unchanged() {
    let sandbox = sandbox();
    let run = run(
        &StdFileSystem,
        &sandbox,
        &workflow(json!([{ "type": "rename", "template": "Facture mars.pdf" }])),
    );
    let operation = run.log.actions[0].operations[0].clone();
    fs::write(&operation.to, "modifié depuis").unwrap();

    assert!(undo(&StdFileSystem, &operation).is_err());
    assert_eq!(sandbox.names(""), ["Facture mars.pdf"]);
}

#[test]
fn stops_a_chain_at_the_first_failure() {
    let sandbox = sandbox();
    let mut first = workflow(json!([{ "type": "rename", "template": "Facture mars.pdf" }]));
    first.continue_on_match = true;
    let mut second =
        workflow(json!([{ "type": "move", "destination": sandbox.root.join("absent") }]));
    second.id = samples::workflow_id(2);
    let mut third = workflow(json!([{ "type": "rename", "template": "Jamais.pdf" }]));
    third.id = samples::workflow_id(3);

    let chain = run_chain(
        Tools {
            fs: &StdFileSystem,
            converter: &FakeConverter::default(),
        },
        &[&first, &second, &third],
        &SequentialIds::default(),
        &sandbox.request(),
    );

    assert!(chain.failed());
    assert_eq!(chain.runs.len(), 2);
    assert_eq!(chain.runs[0].log.status, ExecutionStatus::Success);
    assert_eq!(chain.runs[1].failure, Some(FailureCode::DestinationMissing));
    assert_eq!(chain.not_run, [samples::workflow_id(3)]);
    assert_eq!(
        sandbox.names(""),
        ["Facture mars.pdf"],
        "le premier workflow reste fait"
    );
    assert_ne!(chain.runs[0].log.id, chain.runs[1].log.id);
}

#[test]
fn simulates_without_changing_anything() {
    let sandbox = sandbox();
    fs::write(sandbox.root.join("Facture mars.pdf"), "autre").unwrap();
    let archives = sandbox.root.join("Archives");
    let workflow = workflow(json!([
        { "type": "rename", "template": "Facture mars.pdf" },
        { "type": "move", "destination": archives, "create_folders": true }
    ]));

    let run = run(&DryRunFileSystem::new(StdFileSystem), &sandbox, &workflow);

    assert_eq!(run.failure, None);
    assert_eq!(run.location, archives.join("Facture mars (2).pdf"));
    assert_eq!(
        sandbox.names(""),
        ["Facture mars.pdf", NAME],
        "rien n'a changé"
    );
}

#[cfg(windows)]
#[test]
fn produces_names_that_windows_accepts() {
    let sandbox = sandbox();

    let run = run(
        &StdFileSystem,
        &sandbox,
        &workflow(json!([{ "type": "rename", "template": "NUL.txt" }])),
    );

    assert_eq!(run.failure, None);
    assert!(sandbox.root.join("NUL_.txt").exists());
}

#[test]
fn converts_a_pdf_into_one_image_per_page_next_to_it() {
    let sandbox = sandbox();
    fs::write(sandbox.root.join("Facture_2026-03-15_p1.png"), "autre").unwrap();
    let workflow = workflow(json!([
        { "type": "convert", "to": "png", "dpi": 300 },
        { "type": "move", "destination": sandbox.folder("classé") }
    ]));

    let run = run(&StdFileSystem, &sandbox, &workflow);

    assert_eq!(run.failure, None);
    assert_eq!(
        sandbox.names(""),
        [
            "Facture_2026-03-15_p1 (2).png",
            "Facture_2026-03-15_p1.png",
            "Facture_2026-03-15_p2.png",
            "classé",
        ]
    );
    assert_eq!(
        fs::read_to_string(sandbox.root.join("Facture_2026-03-15_p2.png")).unwrap(),
        "image-2-300"
    );
    // Le document reste le PDF : c'est lui que l'action suivante déplace.
    assert_eq!(sandbox.names("classé"), [NAME]);
    let operations = &run.log.actions[0].operations;
    assert_eq!(operations.len(), 2);
    assert!(operations.iter().all(|operation| {
        operation.kind == OperationKind::Convert && operation.from == sandbox.file
    }));
    assert_eq!(operations[1].fingerprint, Fingerprint::of(b"image-2-300"));
}

#[test]
fn converts_with_default_settings_and_to_jpeg() {
    let sandbox = sandbox();

    let run = run(
        &StdFileSystem,
        &sandbox,
        &workflow(json!([{ "type": "convert", "to": "jpeg" }])),
    );

    assert_eq!(run.failure, None);
    assert_eq!(
        sandbox.names(""),
        [
            NAME,
            "Facture_2026-03-15_p1.jpg",
            "Facture_2026-03-15_p2.jpg"
        ]
    );
    assert_eq!(
        fs::read_to_string(sandbox.root.join("Facture_2026-03-15_p1.jpg")).unwrap(),
        "image-1-150"
    );
}

#[test]
fn converts_an_image_into_a_pdf() {
    let sandbox = sandbox();
    let workflow = workflow(json!([
        { "type": "rename", "template": "Scan.jpg" },
        { "type": "convert", "to": "pdf" }
    ]));

    let run = run(&StdFileSystem, &sandbox, &workflow);

    assert_eq!(run.failure, None);
    assert_eq!(sandbox.names(""), ["Scan.jpg", "Scan.pdf"]);
    assert_eq!(
        fs::read_to_string(sandbox.root.join("Scan.pdf")).unwrap(),
        "pdf:1"
    );
    assert_eq!(run.location, sandbox.root.join("Scan.jpg"));
}

#[test]
fn refuses_a_conversion_between_unsupported_formats() {
    let cases = [
        json!([{ "type": "convert", "to": "pdf" }]),
        json!([
            { "type": "rename", "template": "Notes.docx" },
            { "type": "convert", "to": "png" }
        ]),
        json!([
            { "type": "rename", "template": "Photo.png" },
            { "type": "convert", "to": "jpeg" }
        ]),
    ];
    for actions in cases {
        let sandbox = sandbox();

        let run = run(&StdFileSystem, &sandbox, &workflow(actions));

        assert_eq!(run.failure, Some(FailureCode::UnsupportedConversion));
        assert_eq!(sandbox.names(""), [NAME], "tout est annulé");
    }
}

#[test]
fn removes_the_pages_already_written_when_a_conversion_fails() {
    let sandbox = sandbox();
    let converter = FakeConverter {
        pages: 5,
        fail_at_page: Some(3),
        ..FakeConverter::default()
    };

    let run = run_with(
        &StdFileSystem,
        &converter,
        &sandbox,
        &workflow(json!([{ "type": "convert", "to": "png" }])),
    );

    assert_eq!(run.failure, Some(FailureCode::DocumentUnreadable));
    assert_eq!(run.log.error.as_deref(), Some("document_unreadable"));
    assert_eq!(sandbox.names(""), [NAME]);
    assert!(run.log.actions[0].operations.is_empty());
}

#[test]
fn rolls_back_a_conversion_when_a_later_action_fails() {
    let sandbox = sandbox();
    let workflow = workflow(json!([
        { "type": "convert", "to": "png" },
        { "type": "move", "destination": sandbox.root.join("absent") }
    ]));

    let run = run(&StdFileSystem, &sandbox, &workflow);

    assert_eq!(run.failure, Some(FailureCode::DestinationMissing));
    assert_eq!(
        statuses(&run),
        [ActionStatus::RolledBack, ActionStatus::Failed]
    );
    assert_eq!(sandbox.names(""), [NAME]);
}

#[test]
fn refuses_a_document_larger_than_the_converter_accepts() {
    let sandbox = sandbox();
    let converter = FakeConverter {
        max_input_bytes: 4,
        ..FakeConverter::default()
    };

    let run = run_with(
        &StdFileSystem,
        &converter,
        &sandbox,
        &workflow(json!([{ "type": "convert", "to": "png" }])),
    );

    assert_eq!(run.failure, Some(FailureCode::DocumentTooLarge));
    assert_eq!(sandbox.names(""), [NAME]);
}

#[test]
fn simulates_a_conversion_without_writing() {
    let sandbox = sandbox();
    let dry = DryRunFileSystem::new(StdFileSystem);

    let run = run(
        &dry,
        &sandbox,
        &workflow(json!([{ "type": "convert", "to": "png" }])),
    );

    assert_eq!(run.failure, None);
    assert_eq!(run.log.actions[0].operations.len(), 2);
    assert!(dry.exists(&sandbox.root.join("Facture_2026-03-15_p2.png")));
    assert_eq!(sandbox.names(""), [NAME]);
}
