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

use crate::ports::{ExecutionLogRepository, StoreError, WorkflowRepository};
use crate::testing::samples;

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

    // Enregistrer un workflow existant le remplace.
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

    // Supprimer.
    assert!(repo.delete(high.id).unwrap());
    assert_eq!(repo.get(high.id).unwrap(), None);
    assert!(
        !repo.delete(high.id).unwrap(),
        "supprimer deux fois renvoie false"
    );
    assert!(!repo.delete(unknown).unwrap());
    assert_eq!(repo.list().unwrap(), [high_later, renamed]);
}

/// Vérifie le contrat de [`ExecutionLogRepository`] sur un dépôt vide.
///
/// # Panics
///
/// Échoue dès qu'une règle du contrat n'est pas respectée.
pub fn execution_log_repository_contract(repo: &dyn ExecutionLogRepository) {
    let invoice = samples::document_id(1);
    let receipt = samples::document_id(2);
    assert_eq!(repo.recent(10).unwrap(), [], "un journal neuf est vide");
    assert_eq!(repo.for_document(invoice).unwrap(), []);

    let first = samples::execution_log(samples::run_id(1), invoice, samples::date(0));
    let second = samples::execution_log(samples::run_id(2), receipt, samples::date(1));
    let third = samples::execution_log(samples::run_id(3), invoice, samples::date(2));
    // Même date de début que `third` : départagée par l'identifiant.
    let same_time = samples::execution_log(samples::run_id(4), receipt, samples::date(2));
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
    assert_eq!(repo.recent(2).unwrap(), [same_time, third.clone()]);
    assert_eq!(repo.recent(0).unwrap(), []);
    assert_eq!(
        repo.for_document(invoice).unwrap(),
        [first.clone(), third],
        "for_document renvoie les exécutions du document dans l'ordre chronologique"
    );

    assert!(
        matches!(repo.append(&first), Err(StoreError::AlreadyExists { .. })),
        "une exécution n'est jamais remplacée"
    );
    assert_eq!(repo.recent(10).unwrap().len(), 4);
}
