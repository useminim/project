//! Modèle métier et moteur de workflows : modèle, validation, déclencheurs, chaîne d'actions et ports (traits).
//!
//! Ces types font foi pour le modèle de données ; leur forme JSON est celle
//! échangée avec l'interface et synchronisée avec le cloud.

pub mod action;
pub mod execution;
pub mod field;
pub mod id;
pub mod ports;
pub mod profile;
pub mod redacted;
pub mod template;
#[cfg(any(test, feature = "test-support"))]
pub mod testing;
pub mod trigger;
pub mod waiting_zone;
pub mod workflow;

pub use action::{Action, ActionKind, ConvertFormat, ExportFormat};
pub use execution::{ActionRecord, ActionStatus, ExecutionLog, ExecutionStatus};
pub use field::{FieldName, FieldNameError};
pub use id::{DocumentId, IdError, OrganizationId, ProfileId, RunId, UserId, WorkflowId};
pub use profile::{DetectedField, DocumentProfile, FileInfo, TextSource};
pub use redacted::{REDACTED, Redacted};
pub use template::{Segment, Template, TemplateError};
pub use trigger::{Condition, ConditionCategory, MatchMode, MatchResult, Trigger, TriggerSource};
pub use waiting_zone::{WaitingEntry, WaitingReason};
pub use workflow::{
    CURRENT_SCHEMA_VERSION, MAX_CONDITION_DEPTH, MAX_NAME_LENGTH, Owner, ValidationError,
    ValidationErrorKind, Workflow,
};
