//! Modèle métier et moteur de workflows : modèle, validation, déclencheurs, chaîne d'actions et ports (traits).
//!
//! Ces types font foi pour le modèle de données ; leur forme JSON est celle
//! échangée avec l'interface et synchronisée avec le cloud.

pub mod field;
pub mod id;

pub use field::{FieldName, FieldNameError};
pub use id::{DocumentId, IdError, OrganizationId, ProfileId, RunId, UserId, WorkflowId};
