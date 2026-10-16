//! Tests de contrat : le comportement attendu de toute implémentation des
//! dépôts. Chaque implémentation les appelle depuis ses propres tests, avec un
//! dépôt vide :
//!
//! ```ignore
//! #[test]
//! fn sqlite_workflow_repository_passes_the_contract() {
//!     contracts::workflow_repository_contract(&SqliteWorkflowRepository::open_in_memory()?);
//! }
//! ```

#![allow(
    clippy::unwrap_used,
    reason = "code de test exporté : un échec doit interrompre le test"
)]

use crate::execution::{ExecutionLog, ExecutionStatus};
use crate::ports::{ExecutionLogQuery, ExecutionLogRepository, StoreError, WorkflowRepository};
use crate::testing::samples;
use crate::workflow::Workflow;

fn ids(workflows: &[Workflow]) -> Vec<String> {
    workflows
        .iter()
        .map(|workflow| workflow.id.to_string())
        .collect()
}

/// Vérifie le contrat de [`WorkflowRepository`] sur un dépôt vide.
///
/// # Panics
///
/// Échoue dès qu'une règle du contrat n'est pas respectée.
pub fn workflow_repository_contract(repo: &dyn WorkflowRepository) {
    let unknown = samples::workflow_id(999);
    assert_eq!(repo.list().unwrap(), [], "un dépôt neuf est vide");
    assert_eq!(repo.get(unknown).unwrap(), None);

    // Ordre d'évaluation : priorité croissante, puis identifiant croissant.
    let low = samples::workflow(samples::workflow_id(1), 20);
    let high = samples::workflow(samples::workflow_id(2), 10);
    let high_later = samples::workflow(samples::workflow_id(3), 10);
    for workflow in [&low, &high_later, &high] {
        repo.save(workflow).unwrap();
    }
    assert_eq!(
        repo.list().unwrap(),
        [high.clone(), high_later.clone(), low.clone()],
        "list renvoie les workflows par priorité puis par identifiant"
    );
    assert_eq!(repo.get(low.id).unwrap(), Some(low.clone()));

    // Remplacer un workflow exige exactement la version suivante.
    let mut renamed = low.clone();
    renamed.name = "Factures renommées".to_owned();
    renamed.version = 2;
    repo.save(&renamed).unwrap();
    assert_eq!(repo.get(low.id).unwrap(), Some(renamed.clone()));
    assert_eq!(
        repo.list().unwrap().len(),
        3,
        "save remplace sans dupliquer"
    );
    for stale_version in [2, 4] {
        let mut stale = renamed.clone();
        stale.version = stale_version;
        assert!(
            matches!(
                repo.save(&stale),
                Err(StoreError::VersionConflict {
                    stored: 2,
                    attempted,
                    ..
                }) if attempted == stale_version
            ),
            "la version {stale_version} est refusée quand la version 2 est enregistrée"
        );
    }
    assert_eq!(repo.get(low.id).unwrap(), Some(renamed.clone()));

    // Supprimer laisse un tombstone qui garde sa version (2 + 1).
    assert!(repo.delete(high.id).unwrap());
    assert_eq!(repo.get(high.id).unwrap(), None);
    assert!(
        !repo.delete(high.id).unwrap(),
        "supprimer deux fois renvoie false"
    );
    assert!(!repo.delete(unknown).unwrap());
    assert_eq!(repo.list().unwrap(), [high_later.clone(), renamed.clone()]);
    let mut resurrected = high.clone();
    resurrected.version = 1;
    assert!(
        matches!(
            repo.save(&resurrected),
            Err(StoreError::VersionConflict { stored: 2, .. })
        ),
        "un tombstone garde sa version"
    );

    // Réordonner : priorités 10, 20…, version et date mises à jour seulement
    // pour les workflows qui changent de priorité.
    let reordered_at = samples::date(30);
    assert!(matches!(
        repo.reorder(&[renamed.id], reordered_at),
        Err(StoreError::InvalidOrder)
    ));
    assert!(matches!(
        repo.reorder(&[renamed.id, renamed.id, high_later.id], reordered_at),
        Err(StoreError::InvalidOrder)
    ));
    assert!(
        matches!(
            repo.reorder(&[renamed.id, high.id], reordered_at),
            Err(StoreError::InvalidOrder)
        ),
        "un workflow supprimé ne peut pas être réordonné"
    );
    assert_eq!(
        repo.list().unwrap(),
        [high_later.clone(), renamed.clone()],
        "un ordre refusé ne modifie rien"
    );

    repo.reorder(&[renamed.id, high_later.id], reordered_at)
        .unwrap();
    let list = repo.list().unwrap();
    assert_eq!(
        ids(&list),
        [renamed.id.to_string(), high_later.id.to_string()]
    );
    let (first, second) = (&list[0], &list[1]);
    assert_eq!(
        (first.priority, first.version, first.updated_at),
        (10, 3, reordered_at)
    );
    assert_eq!(
        (second.priority, second.version, second.updated_at),
        (20, 2, reordered_at)
    );

    let later = samples::date(40);
    repo.reorder(&[renamed.id, high_later.id], later).unwrap();
    assert_eq!(
        repo.list().unwrap(),
        list,
        "un ordre inchangé ne modifie ni version ni date"
    );
}

fn log(n: u128, document: u128, minutes: i64) -> ExecutionLog {
    samples::execution_log(
        samples::run_id(n),
        samples::document_id(document),
        samples::date(minutes),
    )
}

/// Vérifie le contrat de [`ExecutionLogRepository`] sur un dépôt vide.
///
/// # Panics
///
/// Échoue dès qu'une règle du contrat n'est pas respectée.
pub fn execution_log_repository_contract(repo: &dyn ExecutionLogRepository) {
    let invoice = samples::document_id(1);
    assert_eq!(repo.recent(10).unwrap(), [], "un journal neuf est vide");
    assert_eq!(repo.for_document(invoice).unwrap(), []);
    assert_eq!(repo.search(&ExecutionLogQuery::default()).unwrap(), []);

    let first = log(1, 1, 0);
    let mut second = log(2, 2, 1);
    second.status = ExecutionStatus::Failed;
    second.error = Some("action_failed".to_owned());
    let mut third = log(3, 1, 2);
    third.workflow_id = samples::workflow_id(2);
    // Même date de début que `third` : départagée par l'identifiant.
    let mut same_time = log(4, 2, 2);
    same_time.status = ExecutionStatus::SentToWaitingZone;
    for log in [&third, &first, &same_time, &second] {
        repo.append(log).unwrap();
    }

    assert_eq!(
        repo.recent(10).unwrap(),
        [
            same_time.clone(),
            third.clone(),
            second.clone(),
            first.clone()
        ],
        "recent renvoie les exécutions de la plus récente à la plus ancienne"
    );
    assert_eq!(repo.recent(2).unwrap(), [same_time.clone(), third.clone()]);
    assert_eq!(repo.recent(0).unwrap(), []);
    assert_eq!(
        repo.for_document(invoice).unwrap(),
        [first.clone(), third.clone()],
        "for_document renvoie les exécutions du document dans l'ordre chronologique"
    );

    // Recherche : filtres combinables et pagination.
    let search = |query: ExecutionLogQuery| repo.search(&query).unwrap();
    assert_eq!(
        search(ExecutionLogQuery::default()),
        repo.recent(10).unwrap()
    );
    assert_eq!(
        search(ExecutionLogQuery {
            workflow_id: Some(samples::workflow_id(2)),
            ..ExecutionLogQuery::default()
        }),
        [third.clone()]
    );
    assert_eq!(
        search(ExecutionLogQuery {
            status: Some(ExecutionStatus::Failed),
            ..ExecutionLogQuery::default()
        }),
        [second.clone()]
    );
    assert_eq!(
        search(ExecutionLogQuery {
            started_from: Some(samples::date(1)),
            started_before: Some(samples::date(2)),
            ..ExecutionLogQuery::default()
        }),
        [second.clone()],
        "started_from est inclus, started_before exclu"
    );
    assert_eq!(
        search(ExecutionLogQuery {
            limit: 2,
            offset: 1,
            ..ExecutionLogQuery::default()
        }),
        [third.clone(), second.clone()]
    );
    assert_eq!(
        search(ExecutionLogQuery {
            offset: 10,
            ..ExecutionLogQuery::default()
        }),
        []
    );

    assert!(
        matches!(repo.append(&first), Err(StoreError::AlreadyExists { .. })),
        "une exécution n'est jamais remplacée"
    );
    assert_eq!(repo.recent(10).unwrap().len(), 4);

    // Purge : supprime ce qui a commencé avant la date limite (exclue).
    assert_eq!(repo.purge_before(samples::date(2)).unwrap(), 2);
    assert_eq!(repo.recent(10).unwrap(), [same_time, third]);
    assert_eq!(repo.purge_before(samples::date(0)).unwrap(), 0);
}
