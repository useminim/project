//! Horloge et génération d'identifiants, remplaçables dans les tests pour
//! obtenir des résultats déterministes.

use chrono::{DateTime, Utc};
use ulid::Ulid;

use crate::id::{DocumentId, RunId, WorkflowId};

/// Source de l'heure courante.
pub trait Clock: Send + Sync {
    /// Heure courante, en UTC.
    fn now(&self) -> DateTime<Utc>;
}

/// Horloge du système.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Source d'identifiants. Les méthodes typées enveloppent l'ULID produit par
/// [`IdGenerator::next_ulid`].
pub trait IdGenerator: Send + Sync {
    /// Produit un nouvel ULID.
    fn next_ulid(&self) -> Ulid;

    /// Produit un identifiant de workflow.
    fn workflow_id(&self) -> WorkflowId {
        WorkflowId::from_ulid(self.next_ulid())
    }

    /// Produit un identifiant de document.
    fn document_id(&self) -> DocumentId {
        DocumentId::from_ulid(self.next_ulid())
    }

    /// Produit un identifiant d'exécution.
    fn run_id(&self) -> RunId {
        RunId::from_ulid(self.next_ulid())
    }
}

/// Générateur d'ULID à partir de l'horloge du système et d'aléa.
#[derive(Debug, Clone, Copy, Default)]
pub struct UlidGenerator;

impl IdGenerator for UlidGenerator {
    fn next_ulid(&self) -> Ulid {
        Ulid::generate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_distinct_identifiers() {
        let ids = UlidGenerator;

        assert_ne!(ids.workflow_id(), ids.workflow_id());
    }

    #[test]
    fn reads_the_system_clock() {
        let before = Utc::now();
        let now = SystemClock.now();

        assert!(now >= before);
    }
}
