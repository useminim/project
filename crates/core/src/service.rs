//! Services applicatifs : ce que l'interface peut demander, orchestré au-dessus
//! des ports (dépôt, horloge, identifiants). Les commandes Tauri n'en sont que
//! des adaptateurs.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ulid::Ulid;

use crate::action::Action;
use crate::id::{UserId, WorkflowId};
use crate::ports::{Clock, IdGenerator, PRIORITY_STEP, StoreError, WorkflowRepository};
use crate::trigger::Trigger;
use crate::workflow::{CURRENT_SCHEMA_VERSION, Owner, ValidationError, Workflow};

/// Propriétaire des workflows créés tant que les comptes n'existent pas
/// (prompt 19) : un utilisateur local unique.
pub const LOCAL_OWNER: Owner = Owner::User(UserId::from_ulid(Ulid::nil()));

/// Ce que l'utilisateur saisit dans le wizard. Identifiant, version, priorité,
/// propriétaire et dates sont gérés par le service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct WorkflowDraft {
    /// Nom affiché.
    pub name: String,
    /// Le workflow est évalué.
    pub enabled: bool,
    /// Le workflow suivant est aussi évalué après une correspondance (D-10).
    pub continue_on_match: bool,
    /// Déclencheur.
    pub trigger: Trigger,
    /// Actions, dans l'ordre.
    pub actions: Vec<Action>,
}

/// Échec d'une opération sur les workflows.
#[derive(Debug, Error)]
pub enum WorkflowServiceError {
    /// Le workflow n'existe pas (ou a été supprimé).
    #[error("workflow introuvable")]
    NotFound,
    /// Le workflow est invalide.
    #[error("workflow invalide ({} problème(s))", .0.len())]
    Invalid(Vec<ValidationError>),
    /// Le workflow a été modifié depuis sa lecture, ou la liste a changé
    /// avant un réordonnancement.
    #[error("le workflow a été modifié entre-temps")]
    Conflict,
    /// Le stockage a échoué.
    #[error(transparent)]
    Store(StoreError),
}

impl From<StoreError> for WorkflowServiceError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::VersionConflict { .. }
            | StoreError::AlreadyExists { .. }
            | StoreError::InvalidOrder => Self::Conflict,
            other => Self::Store(other),
        }
    }
}

type Result<T> = std::result::Result<T, WorkflowServiceError>;

/// Gestion des workflows de l'utilisateur.
#[derive(Clone)]
pub struct WorkflowService {
    repository: Arc<dyn WorkflowRepository>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdGenerator>,
}

impl WorkflowService {
    /// Service au-dessus de `repository`.
    pub fn new(
        repository: Arc<dyn WorkflowRepository>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdGenerator>,
    ) -> Self {
        Self {
            repository,
            clock,
            ids,
        }
    }

    /// Workflows dans l'ordre d'évaluation.
    ///
    /// # Erreurs
    ///
    /// [`WorkflowServiceError::Store`] si le stockage échoue.
    pub fn list(&self) -> Result<Vec<Workflow>> {
        Ok(self.repository.list()?)
    }

    /// Un workflow.
    ///
    /// # Erreurs
    ///
    /// [`WorkflowServiceError::NotFound`] s'il n'existe pas.
    pub fn get(&self, id: WorkflowId) -> Result<Workflow> {
        self.repository
            .get(id)?
            .ok_or(WorkflowServiceError::NotFound)
    }

    /// Problèmes de validation de `draft` (vide s'il est valide), sans rien
    /// enregistrer.
    #[must_use]
    pub fn validate(&self, draft: &WorkflowDraft) -> Vec<ValidationError> {
        self.candidate(draft).validate().err().unwrap_or_default()
    }

    /// Workflow non enregistré construit à partir de `draft`, pour le valider
    /// ou l'essayer (identifiant nul, version 1).
    #[must_use]
    pub fn candidate(&self, draft: &WorkflowDraft) -> Workflow {
        let now = self.clock.now();
        Workflow {
            schema_version: CURRENT_SCHEMA_VERSION,
            id: WorkflowId::from_ulid(Ulid::nil()),
            name: draft.name.clone(),
            version: 1,
            enabled: draft.enabled,
            priority: PRIORITY_STEP,
            continue_on_match: draft.continue_on_match,
            trigger: draft.trigger.clone(),
            actions: draft.actions.clone(),
            owner: LOCAL_OWNER,
            created_at: now,
            updated_at: now,
        }
    }

    /// Crée un workflow, placé après tous les autres.
    ///
    /// # Erreurs
    ///
    /// [`WorkflowServiceError::Invalid`] si `draft` est invalide ; rien n'est
    /// alors enregistré.
    pub fn create(&self, draft: WorkflowDraft) -> Result<Workflow> {
        let last = self.repository.list()?.iter().map(|w| w.priority).max();
        let now = self.clock.now();
        let workflow = Workflow {
            schema_version: CURRENT_SCHEMA_VERSION,
            id: self.ids.workflow_id(),
            name: draft.name,
            version: 1,
            enabled: draft.enabled,
            priority: last.map_or(PRIORITY_STEP, |p| p.saturating_add(PRIORITY_STEP)),
            continue_on_match: draft.continue_on_match,
            trigger: draft.trigger,
            actions: draft.actions,
            owner: LOCAL_OWNER,
            created_at: now,
            updated_at: now,
        };
        workflow.validate().map_err(WorkflowServiceError::Invalid)?;
        self.repository.save(&workflow)?;
        Ok(workflow)
    }

    /// Remplace le contenu du workflow `id`, lu à la version
    /// `expected_version`.
    ///
    /// # Erreurs
    ///
    /// [`WorkflowServiceError::NotFound`] s'il n'existe plus,
    /// [`WorkflowServiceError::Conflict`] s'il a changé depuis,
    /// [`WorkflowServiceError::Invalid`] si `draft` est invalide.
    pub fn update(
        &self,
        id: WorkflowId,
        expected_version: u32,
        draft: WorkflowDraft,
    ) -> Result<Workflow> {
        let current = self.get(id)?;
        if current.version != expected_version {
            return Err(WorkflowServiceError::Conflict);
        }
        let workflow = Workflow {
            name: draft.name,
            enabled: draft.enabled,
            continue_on_match: draft.continue_on_match,
            trigger: draft.trigger,
            actions: draft.actions,
            version: current.version.saturating_add(1),
            updated_at: self.clock.now(),
            ..current
        };
        workflow.validate().map_err(WorkflowServiceError::Invalid)?;
        // Le dépôt revérifie la version : une modification concurrente entre
        // la lecture et l'écriture donne aussi un conflit.
        self.repository.save(&workflow)?;
        Ok(workflow)
    }

    /// Active ou désactive le workflow `id`.
    ///
    /// # Erreurs
    ///
    /// [`WorkflowServiceError::NotFound`] s'il n'existe pas.
    pub fn set_enabled(&self, id: WorkflowId, enabled: bool) -> Result<Workflow> {
        let current = self.get(id)?;
        if current.enabled == enabled {
            return Ok(current);
        }
        let workflow = Workflow {
            enabled,
            version: current.version.saturating_add(1),
            updated_at: self.clock.now(),
            ..current
        };
        self.repository.save(&workflow)?;
        Ok(workflow)
    }

    /// Supprime le workflow `id`.
    ///
    /// # Erreurs
    ///
    /// [`WorkflowServiceError::NotFound`] s'il n'existe pas.
    pub fn delete(&self, id: WorkflowId) -> Result<()> {
        if self.repository.delete(id)? {
            Ok(())
        } else {
            Err(WorkflowServiceError::NotFound)
        }
    }

    /// Réordonne les workflows : `ordered` doit contenir chaque workflow une
    /// fois exactement.
    ///
    /// # Erreurs
    ///
    /// [`WorkflowServiceError::Conflict`] si `ordered` ne correspond pas aux
    /// workflows existants (liste modifiée entre-temps).
    pub fn reorder(&self, ordered: &[WorkflowId]) -> Result<Vec<Workflow>> {
        self.repository.reorder(ordered, self.clock.now())?;
        self.list()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use serde_json::json;

    use super::*;
    use crate::testing::memory::InMemoryWorkflowRepository;
    use crate::testing::{FixedClock, SequentialIds, samples};
    use crate::workflow::ValidationErrorKind;

    struct Fixture {
        service: WorkflowService,
        clock: Arc<FixedClock>,
    }

    fn fixture() -> Fixture {
        let clock = Arc::new(FixedClock::new(samples::date(0)));
        let service = WorkflowService::new(
            Arc::new(InMemoryWorkflowRepository::default()),
            clock.clone(),
            Arc::new(SequentialIds::default()),
        );
        Fixture { service, clock }
    }

    fn draft(name: &str) -> WorkflowDraft {
        serde_json::from_value(json!({
            "name": name,
            "enabled": true,
            "continue_on_match": false,
            "trigger": {
                "match": "all",
                "conditions": [{ "type": "file_name_contains", "value": "facture" }],
                "source": "manual"
            },
            "actions": [{ "type": "rename", "template": "{date}.pdf" }]
        }))
        .unwrap()
    }

    #[test]
    fn creates_workflows_one_after_the_other() {
        let Fixture { service, .. } = fixture();

        let first = service.create(draft("Factures")).unwrap();
        let second = service.create(draft("Devis")).unwrap();

        assert_eq!((first.version, first.priority), (1, 10));
        assert_eq!(second.priority, 20);
        assert_eq!(first.owner, LOCAL_OWNER);
        assert_eq!(first.created_at, samples::date(0));
        assert_eq!(
            service
                .list()
                .unwrap()
                .iter()
                .map(|w| w.name.as_str())
                .collect::<Vec<_>>(),
            ["Factures", "Devis"]
        );
    }

    #[test]
    fn refuses_an_invalid_workflow() {
        let Fixture { service, .. } = fixture();
        let mut invalid = draft(" ");
        invalid.actions.clear();

        let issues = service.validate(&invalid);
        let error = service.create(invalid).unwrap_err();

        let kinds: Vec<_> = issues.iter().map(|issue| issue.kind.clone()).collect();
        assert_eq!(
            kinds,
            [
                ValidationErrorKind::EmptyName,
                ValidationErrorKind::NoActions
            ]
        );
        assert!(matches!(error, WorkflowServiceError::Invalid(found) if found == issues));
        assert!(service.list().unwrap().is_empty());
        assert!(service.validate(&draft("Factures")).is_empty());
    }

    #[test]
    fn updates_a_workflow_at_the_expected_version() {
        let Fixture { service, clock } = fixture();
        let created = service.create(draft("Factures")).unwrap();
        clock.advance(Duration::minutes(5));

        let updated = service
            .update(created.id, 1, draft("Factures fournisseurs"))
            .unwrap();

        assert_eq!(updated.version, 2);
        assert_eq!(updated.name, "Factures fournisseurs");
        assert_eq!(updated.created_at, created.created_at);
        assert_eq!(updated.updated_at, samples::date(5));
        assert_eq!(updated.priority, created.priority);
        assert_eq!(service.get(created.id).unwrap(), updated);
    }

    #[test]
    fn reports_a_version_conflict() {
        let Fixture { service, .. } = fixture();
        let created = service.create(draft("Factures")).unwrap();
        service
            .update(created.id, 1, draft("Modifié ailleurs"))
            .unwrap();

        let error = service
            .update(created.id, 1, draft("Ma version"))
            .unwrap_err();

        assert!(matches!(error, WorkflowServiceError::Conflict));
        assert_eq!(service.get(created.id).unwrap().name, "Modifié ailleurs");
    }

    #[test]
    fn enables_disables_and_deletes() {
        let Fixture { service, .. } = fixture();
        let created = service.create(draft("Factures")).unwrap();

        let disabled = service.set_enabled(created.id, false).unwrap();
        let unchanged = service.set_enabled(created.id, false).unwrap();
        service.delete(created.id).unwrap();

        assert!(!disabled.enabled);
        assert_eq!(disabled.version, 2);
        assert_eq!(unchanged.version, 2);
        assert!(matches!(
            service.get(created.id),
            Err(WorkflowServiceError::NotFound)
        ));
        assert!(matches!(
            service.delete(created.id),
            Err(WorkflowServiceError::NotFound)
        ));
        assert!(matches!(
            service.set_enabled(created.id, true),
            Err(WorkflowServiceError::NotFound)
        ));
    }

    #[test]
    fn reorders_workflows() {
        let Fixture { service, .. } = fixture();
        let a = service.create(draft("A")).unwrap();
        let b = service.create(draft("B")).unwrap();
        let c = service.create(draft("C")).unwrap();

        let reordered = service.reorder(&[c.id, a.id, b.id]).unwrap();

        assert_eq!(
            reordered
                .iter()
                .map(|w| (w.name.as_str(), w.priority))
                .collect::<Vec<_>>(),
            [("C", 10), ("A", 20), ("B", 30)]
        );
        assert!(matches!(
            service.reorder(&[a.id, b.id]),
            Err(WorkflowServiceError::Conflict)
        ));
    }
}
