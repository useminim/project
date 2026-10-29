//! Essai d'un workflow sur un fichier choisi par l'utilisateur, avant que les
//! documents n'arrivent par le glisser-déposer : évaluation du déclencheur
//! avec ses explications, simulation des actions (rien n'est écrit), puis
//! exécution réelle à la demande, inscrite au journal.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;

use crate::actions::{ExecutionRequest, Tools, WorkflowRun, run_workflow};
use crate::execution::ActionRecord;
use crate::id::WorkflowId;
use crate::matching::{Evaluation, FileFacts, evaluate};
use crate::ports::{
    Clock, DocumentConverter, DryRunFileSystem, ExecutionLogRepository, FileSystem, IdGenerator,
    StdFileSystem,
};
use crate::service::{WorkflowDraft, WorkflowService, WorkflowServiceError};
use crate::trigger::MatchResult;
use crate::workflow::Workflow;

/// Résultat d'un essai.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct TrialOutcome {
    /// Le déclencheur, condition par condition.
    pub evaluation: Evaluation,
    /// Les actions ont été exécutées pour de vrai (sinon : simulées, ou pas
    /// lancées parce que le déclencheur ne reconnaît pas le fichier).
    pub executed: bool,
    /// Actions, simulées ou faites, avec les fichiers produits.
    pub actions: Vec<ActionRecord>,
    /// Code de l'échec (`FailureCode`), si une action a échoué.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub error: Option<String>,
    /// Emplacement du fichier après les actions.
    pub location: PathBuf,
}

/// Date locale d'un instant, dans le fuseau de l'utilisateur.
pub type LocalDate = Arc<dyn Fn(DateTime<Utc>) -> NaiveDate + Send + Sync>;

/// Essais de workflows sur des fichiers.
#[derive(Clone)]
pub struct TrialService {
    workflows: WorkflowService,
    logs: Arc<dyn ExecutionLogRepository>,
    converter: Arc<dyn DocumentConverter>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdGenerator>,
    local_date: LocalDate,
}

impl TrialService {
    /// Service d'essai : `workflows` pour lire et valider, `logs` pour le
    /// journal des exécutions réelles.
    pub fn new(
        workflows: WorkflowService,
        logs: Arc<dyn ExecutionLogRepository>,
        converter: Arc<dyn DocumentConverter>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdGenerator>,
        local_date: LocalDate,
    ) -> Self {
        Self {
            workflows,
            logs,
            converter,
            clock,
            ids,
            local_date,
        }
    }

    /// Ce que ferait le workflow `draft` (pas forcément enregistré) sur le
    /// fichier `path` : rien n'est écrit sur le disque ni au journal. Les
    /// actions sont simulées même si le déclencheur ne reconnaît pas le
    /// fichier, pour montrer leur effet.
    ///
    /// # Erreurs
    ///
    /// [`WorkflowServiceError::Invalid`] si le brouillon est invalide,
    /// [`TrialError::File`] si le fichier ne peut pas être lu.
    pub fn preview(&self, draft: &WorkflowDraft, path: &Path) -> Result<TrialOutcome, TrialError> {
        let issues = self.workflows.validate(draft);
        if !issues.is_empty() {
            return Err(WorkflowServiceError::Invalid(issues).into());
        }
        let workflow = self.workflows.candidate(draft);
        let simulation = DryRunFileSystem::new(StdFileSystem);
        self.trial(&workflow, path, &simulation, false)
    }

    /// Exécute pour de vrai le workflow enregistré `id` sur le fichier
    /// `path`, si son déclencheur reconnaît le fichier, et l'inscrit au
    /// journal.
    ///
    /// # Erreurs
    ///
    /// [`WorkflowServiceError::NotFound`] si le workflow n'existe pas,
    /// [`TrialError::File`] si le fichier ne peut pas être lu,
    /// [`WorkflowServiceError::Store`] si le journal ne peut pas être écrit.
    pub fn run(&self, id: WorkflowId, path: &Path) -> Result<TrialOutcome, TrialError> {
        let workflow = self.workflows.get(id)?;
        self.trial(&workflow, path, &StdFileSystem, true)
    }

    fn trial(
        &self,
        workflow: &Workflow,
        path: &Path,
        fs: &dyn FileSystem,
        for_real: bool,
    ) -> Result<TrialOutcome, TrialError> {
        let file = FileFacts::from_path(path).map_err(TrialError::File)?;
        let evaluation = evaluate(&workflow.trigger, &file);
        let matched = evaluation.result == MatchResult::Match;
        if for_real && !matched {
            return Ok(TrialOutcome {
                evaluation,
                executed: false,
                actions: Vec::new(),
                error: None,
                location: path.to_owned(),
            });
        }
        let now = self.clock.now();
        let request = ExecutionRequest {
            document_id: self.ids.document_id(),
            location: path,
            file: &file,
            file_date: file.modified_at().map(|date| (self.local_date)(date)),
            today: (self.local_date)(now),
            started_at: now,
        };
        let tools = Tools {
            fs,
            converter: self.converter.as_ref(),
        };
        let WorkflowRun { log, location, .. } =
            run_workflow(tools, workflow, self.ids.run_id(), &request, true);
        if for_real {
            self.logs
                .append(&log)
                .map_err(|error| TrialError::Workflow(WorkflowServiceError::Store(error)))?;
        }
        Ok(TrialOutcome {
            evaluation,
            executed: for_real,
            actions: log.actions,
            error: log.error,
            location,
        })
    }
}

/// Échec d'un essai.
#[derive(Debug, thiserror::Error)]
pub enum TrialError {
    /// Workflow introuvable ou invalide, ou stockage en échec.
    #[error(transparent)]
    Workflow(#[from] WorkflowServiceError),
    /// Le fichier choisi ne peut pas être lu.
    #[error("fichier illisible")]
    File(#[source] std::io::Error),
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::*;
    use crate::testing::memory::{InMemoryExecutionLogRepository, InMemoryWorkflowRepository};
    use crate::testing::{FakeConverter, FixedClock, SequentialIds, samples};

    struct Fixture {
        trials: TrialService,
        workflows: WorkflowService,
        logs: Arc<InMemoryExecutionLogRepository>,
        dir: tempfile::TempDir,
    }

    fn fixture() -> Fixture {
        let clock = Arc::new(FixedClock::new(samples::date(0)));
        let ids = Arc::new(SequentialIds::default());
        let workflows = WorkflowService::new(
            Arc::new(InMemoryWorkflowRepository::default()),
            clock.clone(),
            ids.clone(),
        );
        let logs = Arc::new(InMemoryExecutionLogRepository::default());
        let trials = TrialService::new(
            workflows.clone(),
            logs.clone(),
            Arc::new(FakeConverter::default()),
            clock,
            ids,
            Arc::new(|date: DateTime<Utc>| date.date_naive()),
        );
        Fixture {
            trials,
            workflows,
            logs,
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn draft(folder: &Path) -> WorkflowDraft {
        serde_json::from_value(json!({
            "name": "Factures",
            "enabled": true,
            "continue_on_match": false,
            "trigger": {
                "match": "all",
                "conditions": [
                    { "type": "file_name_contains", "value": "facture" },
                    { "type": "file_extension_is", "values": ["pdf"] }
                ],
                "source": "manual"
            },
            "actions": [
                { "type": "rename", "template": "{date_in_name:YYYY-MM-DD}_{original_name}.{extension}" },
                { "type": "move", "destination": folder, "create_folders": true }
            ]
        }))
        .unwrap()
    }

    fn names(folder: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(folder)
            .map(|entries| {
                entries
                    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    #[test]
    fn simulates_then_runs_for_real_and_logs_the_run() {
        let Fixture {
            trials,
            workflows,
            logs,
            dir,
        } = fixture();
        let file = dir.path().join("Facture 15.03.2026.pdf");
        fs::write(&file, "contenu").unwrap();
        let archive = dir.path().join("Comptabilité");

        let preview = trials.preview(&draft(&archive), &file).unwrap();

        assert_eq!(preview.evaluation.result, MatchResult::Match);
        assert!(!preview.executed);
        assert_eq!(preview.error, None);
        let expected = archive.join("2026-03-15_Facture 15.03.2026.pdf");
        assert_eq!(preview.location, expected);
        assert!(file.exists(), "la simulation ne touche à rien");
        assert!(!archive.exists());
        assert!(logs.recent(10).unwrap().is_empty());

        let workflow = workflows.create(draft(&archive)).unwrap();
        let run = trials.run(workflow.id, &file).unwrap();

        assert!(run.executed);
        assert_eq!(run.location, expected);
        assert_eq!(names(&archive), ["2026-03-15_Facture 15.03.2026.pdf"]);
        assert!(!file.exists());
        let logged = logs.recent(10).unwrap();
        assert_eq!(logged.len(), 1);
        assert_eq!(logged[0].workflow_id, workflow.id);
        assert_eq!(logged[0].actions, run.actions);
    }

    #[test]
    fn explains_a_file_that_is_not_recognized_and_does_not_run() {
        let Fixture {
            trials,
            workflows,
            logs,
            dir,
        } = fixture();
        let file = dir.path().join("Devis.pdf");
        fs::write(&file, "contenu").unwrap();
        let archive = dir.path().join("Comptabilité");

        let preview = trials.preview(&draft(&archive), &file).unwrap();
        let workflow = workflows.create(draft(&archive)).unwrap();
        let run = trials.run(workflow.id, &file).unwrap();

        assert_eq!(preview.evaluation.result, MatchResult::NoMatch);
        assert_eq!(
            preview.evaluation.conditions[0].result,
            MatchResult::NoMatch
        );
        assert_eq!(preview.evaluation.conditions[1].result, MatchResult::Match);
        // Les actions sont tout de même simulées, pour montrer leur effet :
        // ici le renommage échoue faute de date dans le nom.
        assert_eq!(preview.error.as_deref(), Some("missing_variable"));
        assert!(!run.executed);
        assert!(run.actions.is_empty());
        assert!(file.exists());
        assert!(logs.recent(10).unwrap().is_empty());
    }

    #[test]
    fn reports_a_missing_file_or_an_invalid_draft() {
        let Fixture { trials, dir, .. } = fixture();
        let archive = dir.path().join("Comptabilité");
        let mut invalid = draft(&archive);
        invalid.actions.clear();

        assert!(matches!(
            trials.preview(&draft(&archive), &dir.path().join("absent.pdf")),
            Err(TrialError::File(_))
        ));
        assert!(matches!(
            trials.preview(&invalid, &dir.path().join("absent.pdf")),
            Err(TrialError::Workflow(WorkflowServiceError::Invalid(_)))
        ));
    }
}
