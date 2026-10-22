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

use std::path::PathBuf;

use crate::document::{Document, DocumentStatus};
use crate::execution::{ExecutionLog, ExecutionStatus};
use crate::ports::{
    DocumentRepository, ExecutionLogQuery, ExecutionLogRepository, StoreError,
    WaitingZoneRepository, WorkflowRepository,
};
use crate::testing::samples;
use crate::waiting_zone::WaitingReason;
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

/// Vérifie le contrat de [`DocumentRepository`] sur un dépôt vide.
///
/// # Panics
///
/// Échoue dès qu'une règle du contrat n'est pas respectée.
pub fn document_repository_contract(repo: &dyn DocumentRepository) {
    let first = samples::document(samples::document_id(1), 1);
    let second = samples::document(samples::document_id(2), 2);
    // Reçu en même temps que `second` : départagé par l'identifiant.
    let mut same_time = samples::document(samples::document_id(3), 3);
    same_time.received_at = second.received_at;
    assert_eq!(repo.get(first.id).unwrap(), None, "un dépôt neuf est vide");
    assert_eq!(repo.find_by_hash(&first.sha256).unwrap(), None);
    assert_eq!(repo.list_by_status(DocumentStatus::Pending).unwrap(), []);

    for document in [&same_time, &first, &second] {
        repo.save(document).unwrap();
    }
    assert_eq!(repo.get(first.id).unwrap(), Some(first.clone()));
    assert_eq!(
        repo.find_by_hash(&second.sha256).unwrap(),
        Some(second.clone())
    );
    assert_eq!(repo.find_by_hash(&samples::content_hash(99)).unwrap(), None);
    assert_eq!(
        repo.list_by_status(DocumentStatus::Pending).unwrap(),
        [first.clone(), second.clone(), same_time.clone()],
        "list_by_status renvoie les documents par date de réception puis par identifiant"
    );

    // Même contenu sous un autre identifiant : accepté, l'ingestion décide
    // quoi faire d'un doublon. find_by_hash renvoie le premier reçu.
    let mut copy = samples::document(samples::document_id(4), 1);
    copy.path = PathBuf::from("/autre/dossier/copie.pdf");
    repo.save(&copy).unwrap();
    assert_eq!(repo.get(copy.id).unwrap(), Some(copy.clone()));
    assert_eq!(
        repo.find_by_hash(&first.sha256).unwrap(),
        Some(first.clone()),
        "find_by_hash renvoie le premier document reçu qui a ce contenu"
    );

    // Remplacement d'un document existant (nouvel emplacement).
    let mut moved = first.clone();
    moved.path = PathBuf::from("/archives/2026/facture-1.pdf");
    repo.save(&moved).unwrap();
    assert_eq!(repo.get(first.id).unwrap(), Some(moved.clone()));
    assert_eq!(
        repo.find_by_hash(&first.sha256).unwrap(),
        Some(moved.clone())
    );

    // Statuts : seules les transitions permises sont enregistrées.
    assert!(
        repo.update_status(first.id, DocumentStatus::Processing)
            .unwrap()
    );
    assert!(
        repo.update_status(first.id, DocumentStatus::Processed)
            .unwrap()
    );
    assert!(
        matches!(
            repo.update_status(first.id, DocumentStatus::Processing),
            Err(StoreError::InvalidTransition {
                from: DocumentStatus::Processed,
                to: DocumentStatus::Processing,
                ..
            })
        ),
        "un document traité ne repart pas en traitement sans retraitement explicite"
    );
    assert_eq!(
        repo.get(first.id).unwrap().map(|document| document.status),
        Some(DocumentStatus::Processed),
        "une transition refusée ne modifie rien"
    );
    assert!(
        !repo
            .update_status(samples::document_id(99), DocumentStatus::Processing)
            .unwrap(),
        "update_status renvoie false pour un document inconnu"
    );
    assert_eq!(
        repo.list_by_status(DocumentStatus::Processed).unwrap(),
        [Document {
            status: DocumentStatus::Processed,
            ..moved.clone()
        }]
    );
    assert_eq!(
        repo.list_by_status(DocumentStatus::Pending).unwrap(),
        [copy.clone(), second.clone(), same_time.clone()]
    );

    // Retraitement : enregistré par `save`, qui garde le statut tel quel.
    let mut reprocessed = repo.get(first.id).unwrap().unwrap();
    reprocessed.reprocess().unwrap();
    repo.save(&reprocessed).unwrap();
    assert_eq!(
        repo.list_by_status(DocumentStatus::Pending).unwrap(),
        [reprocessed, copy, second, same_time]
    );
}

/// Vérifie le contrat de [`WaitingZoneRepository`] sur un dépôt vide.
///
/// # Panics
///
/// Échoue dès qu'une règle du contrat n'est pas respectée.
pub fn waiting_zone_repository_contract(repo: &dyn WaitingZoneRepository) {
    assert_eq!(
        repo.list().unwrap(),
        [],
        "une zone d'attente neuve est vide"
    );
    assert!(!repo.remove(samples::document_id(1)).unwrap());

    let late = samples::waiting_entry(samples::document_id(1), samples::date(5));
    let early = samples::waiting_entry(samples::document_id(2), samples::date(0));
    // Entré en même temps que `late` : départagé par l'identifiant.
    let mut same_time = samples::waiting_entry(samples::document_id(3), samples::date(5));
    same_time.reason = WaitingReason::ActionFailed {
        workflow_id: samples::workflow_id(1),
        run_id: samples::run_id(1),
    };
    for entry in [&same_time, &late, &early] {
        repo.put(entry).unwrap();
    }
    assert_eq!(
        repo.list().unwrap(),
        [early.clone(), late.clone(), same_time.clone()],
        "list renvoie les documents par date d'entrée puis par identifiant"
    );

    // Une nouvelle entrée pour le même document remplace l'ancienne.
    let mut retried = early.clone();
    retried.reason = WaitingReason::Uncertain {
        candidates: vec![samples::workflow_id(1), samples::workflow_id(2)],
    };
    retried.entered_at = samples::date(10);
    repo.put(&retried).unwrap();
    assert_eq!(
        repo.list().unwrap(),
        [late.clone(), same_time.clone(), retried.clone()]
    );

    assert!(repo.remove(late.document_id).unwrap());
    assert!(!repo.remove(late.document_id).unwrap());
    assert_eq!(repo.list().unwrap(), [same_time, retried]);
}
