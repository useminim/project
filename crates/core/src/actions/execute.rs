//! Exécution des actions d'un workflow sur un fichier (D-10).
//!
//! - Avant d'agir, minim vérifie que le fichier n'a pas changé depuis son
//!   analyse (taille et date de modification).
//! - Les actions portent sur le fichier d'origine. Aucune n'écrase un fichier
//!   existant : en cas de collision, le nom reçoit un suffixe (`Facture
//!   (2).pdf`). Chaque opération est inscrite au journal avec l'empreinte du
//!   fichier produit.
//! - Si une action échoue, celles déjà faites par le workflow sont annulées
//!   dans l'ordre inverse, à condition que le fichier produit n'ait pas changé
//!   et que l'emplacement d'origine soit libre. Une action qui n'a pas pu être
//!   annulée reste `success` dans le journal.
//! - Dans une chaîne `continue_on_match`, le premier workflow en échec arrête
//!   la chaîne ; les précédents restent faits.
//!
//! Aucun chemin n'est journalisé : les erreurs ne portent qu'un code stable.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::{DateTime, NaiveDate, Utc};

use crate::action::Action;
use crate::actions::render::{RenderContext, RenderError, render};
use crate::actions::sanitize::{sanitize_file_name, with_collision_suffix};
use crate::execution::{
    ActionRecord, ActionStatus, ExecutionLog, ExecutionStatus, FileOperation, OperationKind,
};
use crate::id::{DocumentId, RunId, WorkflowId};
use crate::matching::FileFacts;
use crate::matching::text::comparable;
use crate::ports::{FileSystem, IdGenerator};
use crate::workflow::Workflow;

/// Nombre maximal de suffixes essayés avant d'abandonner (`Facture (999).pdf`).
const MAX_COLLISIONS: u32 = 999;

/// Cause stable de l'échec d'une exécution, enregistrée dans
/// `ExecutionLog.error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureCode {
    /// Le fichier n'est plus à son emplacement.
    SourceMissing,
    /// Le fichier a été modifié depuis son analyse.
    FileChanged,
    /// Le dossier de destination n'existe pas.
    DestinationMissing,
    /// Une variable du modèle de nom n'a pas de valeur pour ce document.
    MissingVariable,
    /// Le modèle de nom contient un format invalide.
    InvalidTemplate,
    /// Le nom produit est vide après assainissement.
    EmptyFileName,
    /// Trop de fichiers portent déjà ce nom.
    TooManyCollisions,
    /// L'action n'est pas encore disponible (conversion, extraction).
    ActionNotAvailable,
    /// La copie ne correspond pas à l'original.
    CopyMismatch,
    /// Lecture ou écriture impossible (droits, disque plein…).
    Io,
}

impl FailureCode {
    /// Code tel qu'enregistré dans le journal.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceMissing => "source_missing",
            Self::FileChanged => "file_changed",
            Self::DestinationMissing => "destination_missing",
            Self::MissingVariable => "missing_variable",
            Self::InvalidTemplate => "invalid_template",
            Self::EmptyFileName => "empty_file_name",
            Self::TooManyCollisions => "too_many_collisions",
            Self::ActionNotAvailable => "action_not_available",
            Self::CopyMismatch => "copy_mismatch",
            Self::Io => "io",
        }
    }
}

impl From<io::Error> for FailureCode {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::NotFound {
            Self::SourceMissing
        } else {
            Self::Io
        }
    }
}

impl From<RenderError> for FailureCode {
    fn from(error: RenderError) -> Self {
        match error {
            RenderError::MissingVariable { .. } => Self::MissingVariable,
            RenderError::UnexpectedFormat { .. } | RenderError::InvalidDateFormat { .. } => {
                Self::InvalidTemplate
            }
        }
    }
}

/// Document sur lequel exécuter des workflows.
#[derive(Debug, Clone, Copy)]
pub struct ExecutionRequest<'a> {
    /// Document concerné.
    pub document_id: DocumentId,
    /// Emplacement actuel du fichier.
    pub location: &'a Path,
    /// Propriétés relevées à l'analyse : le fichier doit encore y
    /// correspondre, et elles alimentent les modèles de nom.
    pub file: &'a FileFacts,
    /// Date de modification du fichier, dans le fuseau de l'utilisateur.
    pub file_date: Option<NaiveDate>,
    /// Date du jour, dans le fuseau de l'utilisateur.
    pub today: NaiveDate,
    /// Début de l'exécution.
    pub started_at: DateTime<Utc>,
}

/// Résultat de l'exécution d'un workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRun {
    /// Entrée du journal.
    pub log: ExecutionLog,
    /// Emplacement du fichier après l'exécution (inchangé si elle a échoué et
    /// a été entièrement annulée).
    pub location: PathBuf,
    /// Cause de l'échec, si l'exécution a échoué.
    pub failure: Option<FailureCode>,
}

/// Résultat d'une chaîne de workflows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainRun {
    /// Exécutions faites, dans l'ordre ; la dernière a échoué si la chaîne
    /// s'est arrêtée.
    pub runs: Vec<WorkflowRun>,
    /// Workflows qui ne se sont pas exécutés à cause d'un échec.
    pub not_run: Vec<WorkflowId>,
    /// Emplacement final du fichier.
    pub location: PathBuf,
}

impl ChainRun {
    /// La chaîne s'est arrêtée sur un échec.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.runs.iter().any(|run| run.failure.is_some())
    }
}

/// Exécute `workflows` dans l'ordre (chaîne `continue_on_match`), en arrêtant
/// au premier échec.
pub fn run_chain(
    fs: &dyn FileSystem,
    workflows: &[&Workflow],
    ids: &dyn IdGenerator,
    request: &ExecutionRequest<'_>,
) -> ChainRun {
    let mut runs = Vec::new();
    let mut location = request.location.to_owned();
    let mut remaining = workflows.iter();
    for workflow in remaining.by_ref() {
        // Seul le premier workflow trouve le fichier tel qu'il a été analysé.
        let check_unchanged = runs.is_empty();
        let current = ExecutionRequest {
            location: &location,
            ..*request
        };
        let run = run_workflow(fs, workflow, ids.run_id(), &current, check_unchanged);
        location.clone_from(&run.location);
        let failed = run.failure.is_some();
        runs.push(run);
        if failed {
            break;
        }
    }
    ChainRun {
        runs,
        not_run: remaining.map(|workflow| workflow.id).collect(),
        location,
    }
}

/// Exécute les actions de `workflow` sur le fichier, et les annule si l'une
/// échoue.
pub fn run_workflow(
    fs: &dyn FileSystem,
    workflow: &Workflow,
    run_id: RunId,
    request: &ExecutionRequest<'_>,
    check_unchanged: bool,
) -> WorkflowRun {
    let mut records: Vec<ActionRecord> = Vec::new();
    let mut location = request.location.to_owned();
    let mut failure = None;

    if check_unchanged && let Err(code) = check_file_unchanged(fs, request) {
        failure = Some(code);
    }
    if failure.is_none() {
        for action in &workflow.actions {
            let started = Instant::now();
            let outcome = apply(fs, action, &location, request);
            let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            match outcome {
                Ok(operation) => {
                    if let Some(operation) = &operation
                        && operation.kind != OperationKind::Copy
                    {
                        location.clone_from(&operation.to);
                    }
                    records.push(ActionRecord {
                        kind: action.kind(),
                        status: ActionStatus::Success,
                        duration_ms,
                        operation,
                    });
                }
                Err(code) => {
                    records.push(ActionRecord {
                        kind: action.kind(),
                        status: ActionStatus::Failed,
                        duration_ms,
                        operation: None,
                    });
                    failure = Some(code);
                    break;
                }
            }
        }
    }
    if failure.is_some() {
        location = roll_back(fs, &mut records, location);
    }

    WorkflowRun {
        log: ExecutionLog {
            id: run_id,
            document_id: request.document_id,
            workflow_id: workflow.id,
            workflow_version: workflow.version,
            status: if failure.is_some() {
                ExecutionStatus::Failed
            } else {
                ExecutionStatus::Success
            },
            actions: records,
            error: failure.map(|code| code.as_str().to_owned()),
            started_at: request.started_at,
        },
        location,
        failure,
    }
}

fn check_file_unchanged(
    fs: &dyn FileSystem,
    request: &ExecutionRequest<'_>,
) -> Result<(), FailureCode> {
    let metadata = fs.metadata(request.location)?;
    let modified = metadata.modified.map(DateTime::<Utc>::from);
    let same_date = request
        .file
        .modified_at()
        .is_none_or(|expected| modified == Some(expected));
    if metadata.is_file && metadata.size == request.file.size() && same_date {
        Ok(())
    } else {
        Err(FailureCode::FileChanged)
    }
}

/// Applique une action ; renvoie l'opération faite, ou `None` si rien n'a
/// changé.
fn apply(
    fs: &dyn FileSystem,
    action: &Action,
    location: &Path,
    request: &ExecutionRequest<'_>,
) -> Result<Option<FileOperation>, FailureCode> {
    let current_name = location
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or(FailureCode::SourceMissing)?;
    match action {
        Action::Rename { template } => {
            let context = RenderContext {
                file: request.file,
                file_date: request.file_date,
                today: request.today,
            };
            let name = sanitize_file_name(&render(template, &context)?)
                .map_err(|_| FailureCode::EmptyFileName)?;
            let folder = location.parent().ok_or(FailureCode::SourceMissing)?;
            place(fs, location, folder, &name, OperationKind::Rename)
        }
        Action::Move {
            destination,
            create_folders,
        } => {
            if *create_folders {
                fs.create_dir_all(destination)?;
            }
            require_folder(fs, destination)?;
            place(
                fs,
                location,
                destination,
                &current_name,
                OperationKind::Move,
            )
        }
        Action::Copy { destination } => {
            require_folder(fs, destination)?;
            place(
                fs,
                location,
                destination,
                &current_name,
                OperationKind::Copy,
            )
        }
        Action::Convert { .. } | Action::ExtractFields { .. } => {
            Err(FailureCode::ActionNotAvailable)
        }
    }
}

fn require_folder(fs: &dyn FileSystem, folder: &Path) -> Result<(), FailureCode> {
    match fs.metadata(folder) {
        Ok(metadata) if !metadata.is_file => Ok(()),
        _ => Err(FailureCode::DestinationMissing),
    }
}

/// Place le fichier `from` dans `folder` sous `name`, ou sous le premier nom
/// libre avec un suffixe.
fn place(
    fs: &dyn FileSystem,
    from: &Path,
    folder: &Path,
    name: &str,
    kind: OperationKind,
) -> Result<Option<FileOperation>, FailureCode> {
    for number in 1..=MAX_COLLISIONS {
        let candidate = if number == 1 {
            name.to_owned()
        } else {
            with_collision_suffix(name, number)
        };
        let to = folder.join(&candidate);
        if to == from {
            // Le fichier porte déjà ce nom, à cet endroit.
            return Ok(None);
        }
        let result = match kind {
            OperationKind::Copy => copy_verified(fs, from, &to),
            OperationKind::Rename | OperationKind::Move => relocate(fs, from, &to),
        };
        match result {
            Ok(()) => {
                return Ok(Some(FileOperation {
                    kind,
                    from: from.to_owned(),
                    fingerprint: fs.fingerprint(&to)?,
                    to,
                }));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                return Err(FailureCode::CopyMismatch);
            }
            Err(error) => return Err(error.into()),
        }
    }
    Err(FailureCode::TooManyCollisions)
}

/// Copie `from` vers `to` (libre) et vérifie l'empreinte de la copie.
fn copy_verified(fs: &dyn FileSystem, from: &Path, to: &Path) -> io::Result<()> {
    fs.copy_new(from, to)?;
    if fs.fingerprint(from)? == fs.fingerprint(to)? {
        Ok(())
    } else {
        let _ = fs.remove_file(to);
        Err(io::ErrorKind::InvalidData.into())
    }
}

/// Déplace `from` vers `to` sans jamais écraser : lien physique puis
/// suppression de la source, ou, si les deux sont sur des volumes différents
/// ou que le volume ne prend pas en charge les liens, copie vérifiée puis
/// suppression. `ErrorKind::AlreadyExists` si `to` est occupé par un autre
/// fichier.
fn relocate(fs: &dyn FileSystem, from: &Path, to: &Path) -> io::Result<()> {
    if is_case_variant_of_itself(fs, from, to)? {
        return change_case(fs, from, to);
    }
    match fs.link_new(from, to) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Err(error),
        Err(_) => copy_verified(fs, from, to)?,
    }
    if let Err(error) = fs.remove_file(from) {
        // Ne laisse pas deux exemplaires : revient à l'état de départ.
        let _ = fs.remove_file(to);
        return Err(error);
    }
    Ok(())
}

/// `to` ne diffère de `from` que par la casse, et désigne le même fichier
/// (système de fichiers insensible à la casse : Windows, macOS).
fn is_case_variant_of_itself(fs: &dyn FileSystem, from: &Path, to: &Path) -> io::Result<bool> {
    let same_folder = from.parent() == to.parent();
    let (Some(from_name), Some(to_name)) = (from.file_name(), to.file_name()) else {
        return Ok(false);
    };
    let (from_name, to_name) = (from_name.to_string_lossy(), to_name.to_string_lossy());
    if !same_folder
        || from_name == to_name
        || comparable(&from_name, false) != comparable(&to_name, false)
    {
        return Ok(false);
    }
    if !fs.exists(to) {
        return Ok(false);
    }
    let (a, b) = (fs.metadata(from)?, fs.metadata(to)?);
    Ok(a == b && fs.fingerprint(from)? == fs.fingerprint(to)?)
}

/// Renommage qui ne change que la casse, par un nom temporaire.
fn change_case(fs: &dyn FileSystem, from: &Path, to: &Path) -> io::Result<()> {
    let folder = from.parent().unwrap_or_else(|| Path::new(""));
    let mut temporary = None;
    for number in 1..=MAX_COLLISIONS {
        let candidate = folder.join(format!(".minim-renommage-{number}"));
        match fs.link_new(from, &candidate) {
            Ok(()) => {
                temporary = Some(candidate);
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    let temporary = temporary.ok_or_else(|| io::Error::from(io::ErrorKind::AlreadyExists))?;
    fs.remove_file(from)?;
    if let Err(error) = fs.link_new(&temporary, to) {
        // Remet le fichier sous son nom d'origine.
        let _ = fs.link_new(&temporary, from);
        let _ = fs.remove_file(&temporary);
        return Err(error);
    }
    fs.remove_file(&temporary)
}

/// Annule, dans l'ordre inverse, les actions réussies de `records`. Renvoie
/// l'emplacement du fichier après l'annulation.
fn roll_back(fs: &dyn FileSystem, records: &mut [ActionRecord], mut location: PathBuf) -> PathBuf {
    for record in records.iter_mut().rev() {
        if record.status != ActionStatus::Success {
            continue;
        }
        let Some(operation) = &record.operation else {
            // Rien n'avait changé sur le disque.
            record.status = ActionStatus::RolledBack;
            continue;
        };
        if undo(fs, operation).is_ok() {
            if operation.kind != OperationKind::Copy {
                location.clone_from(&operation.from);
            }
            record.status = ActionStatus::RolledBack;
        } else {
            // Une annulation impossible arrête le retour en arrière : les
            // actions plus anciennes dépendent de celle-ci.
            break;
        }
    }
    location
}

/// Annule une opération, si c'est sûr : le fichier produit n'a pas changé et
/// l'emplacement d'origine est libre.
///
/// # Erreurs
///
/// Renvoie une erreur si l'opération ne peut pas être annulée sans risque.
pub fn undo(fs: &dyn FileSystem, operation: &FileOperation) -> io::Result<()> {
    if fs.fingerprint(&operation.to)? != operation.fingerprint {
        return Err(io::Error::other("le fichier produit a changé"));
    }
    match operation.kind {
        OperationKind::Copy => fs.remove_file(&operation.to),
        OperationKind::Rename | OperationKind::Move => relocate(fs, &operation.to, &operation.from),
    }
}

#[cfg(test)]
mod tests;
