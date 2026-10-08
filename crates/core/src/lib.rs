//! Modèle métier et moteur de workflows : modèle, validation, déclencheurs, chaîne d'actions et ports (traits).
//!
//! Ces types font foi pour le modèle de données ; leur forme JSON est celle
//! échangée avec l'interface et synchronisée avec le cloud.

pub mod action;
pub mod execution;
pub mod field;
pub mod id;
pub mod profile;
pub mod template;
pub mod trigger;
pub mod workflow;

pub use action::{Action, ActionKind, ConvertFormat, ExportFormat};
pub use execution::{ActionRecord, ActionStatus, ExecutionLog, ExecutionStatus};
pub use field::{FieldName, FieldNameError};
pub use id::{DocumentId, IdError, OrganizationId, ProfileId, RunId, UserId, WorkflowId};
pub use profile::{DetectedField, DocumentProfile, FileInfo, TextSource};
pub use template::{Segment, Template, TemplateError};
pub use trigger::{Condition, ConditionCategory, MatchMode, MatchResult, Trigger, TriggerSource};
pub use workflow::{Owner, ValidationError, Workflow};
