//! Erreur renvoyée par toutes les commandes Tauri.
//!
//! L'interface reçoit un `code` stable, une clé de traduction et, au besoin,
//! des détails structurés. Elle ne reçoit jamais le texte d'une erreur système,
//! qui pourrait contenir un chemin ou un nom de fichier : l'erreur source est
//! journalisée côté Rust, puis remplacée par un message générique.

use std::fmt;
use std::io;

use minim_core::ValidationError;
use serde::Serialize;

/// Catégorie stable d'une erreur, sérialisée en `snake_case`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Les données envoyées par l'interface sont invalides.
    ValidationFailed,
    /// L'élément demandé n'existe pas.
    NotFound,
    /// Une lecture ou une écriture sur le disque a échoué.
    Io,
    /// Erreur inattendue ; le détail n'est que dans les logs.
    Internal,
}

impl ErrorCode {
    /// Clé de traduction du message affiché à l'utilisateur.
    #[must_use]
    pub const fn message_key(self) -> &'static str {
        match self {
            Self::ValidationFailed => "errors.validation_failed",
            Self::NotFound => "errors.not_found",
            Self::Io => "errors.io",
            Self::Internal => "errors.internal",
        }
    }
}

/// Nature d'une erreur d'entrée-sortie, sans le message du système.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum IoErrorKind {
    /// Le fichier ou le dossier n'existe pas.
    NotFound,
    /// Les droits sont insuffisants.
    PermissionDenied,
    /// Le fichier ou le dossier existe déjà.
    AlreadyExists,
    /// Le disque est plein.
    StorageFull,
    /// Autre erreur d'entrée-sortie.
    Other,
}

impl From<io::ErrorKind> for IoErrorKind {
    fn from(kind: io::ErrorKind) -> Self {
        match kind {
            io::ErrorKind::NotFound => Self::NotFound,
            io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            io::ErrorKind::AlreadyExists => Self::AlreadyExists,
            io::ErrorKind::StorageFull => Self::StorageFull,
            _ => Self::Other,
        }
    }
}

/// Détails structurés d'une erreur, selon sa catégorie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ErrorDetails {
    /// Problèmes de validation, à afficher ensemble.
    Validation {
        /// Problèmes trouvés.
        issues: Vec<ValidationError>,
    },
    /// Nature de l'erreur d'entrée-sortie.
    Io {
        /// Nature de l'erreur.
        io_kind: IoErrorKind,
    },
}

/// Erreur renvoyée à l'interface par une commande Tauri.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct CommandError {
    /// Catégorie de l'erreur.
    pub code: ErrorCode,
    /// Clé de traduction du message à afficher.
    pub message_key: String,
    /// Détails structurés, si la catégorie en prévoit.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub details: Option<ErrorDetails>,
}

impl CommandError {
    /// Crée une erreur sans détails.
    #[must_use]
    pub fn new(code: ErrorCode) -> Self {
        Self {
            code,
            message_key: code.message_key().to_owned(),
            details: None,
        }
    }

    /// Erreur inattendue : journalise la source et renvoie un message générique.
    ///
    /// Le texte de `source` est écrit dans les logs : les erreurs des crates ne
    /// doivent donc contenir aucune donnée sensible (voir `minim_core::Redacted`).
    #[must_use]
    pub fn internal(source: &dyn std::error::Error) -> Self {
        tracing::error!(error = %source, "erreur interne");
        Self::new(ErrorCode::Internal)
    }

    fn with_details(mut self, details: ErrorDetails) -> Self {
        self.details = Some(details);
        self
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message_key.as_str())
    }
}

impl std::error::Error for CommandError {}

impl From<Vec<ValidationError>> for CommandError {
    /// Une erreur de validation est attendue : elle n'est journalisée qu'au
    /// niveau `debug`, sans son contenu.
    fn from(issues: Vec<ValidationError>) -> Self {
        tracing::debug!(issues = issues.len(), "validation refusée");
        Self::new(ErrorCode::ValidationFailed).with_details(ErrorDetails::Validation { issues })
    }
}

impl From<io::Error> for CommandError {
    /// Seuls la nature de l'erreur et son code système sont journalisés : le
    /// message d'une erreur d'entrée-sortie peut contenir un chemin.
    fn from(error: io::Error) -> Self {
        let io_kind = IoErrorKind::from(error.kind());
        tracing::error!(
            ?io_kind,
            os_error = error.raw_os_error(),
            "erreur d'entrée-sortie"
        );
        let code = if io_kind == IoErrorKind::NotFound {
            ErrorCode::NotFound
        } else {
            ErrorCode::Io
        };
        Self::new(code).with_details(ErrorDetails::Io { io_kind })
    }
}

#[cfg(test)]
mod tests {
    use minim_core::ValidationErrorKind;
    use serde_json::{Value, json};

    use super::*;

    fn to_json(error: &CommandError) -> Value {
        serde_json::to_value(error).unwrap()
    }

    #[test]
    fn serializes_an_error_without_details() {
        assert_eq!(
            to_json(&CommandError::new(ErrorCode::Internal)),
            json!({ "code": "internal", "message_key": "errors.internal" })
        );
    }

    #[test]
    fn serializes_validation_issues() {
        let error = CommandError::from(vec![
            ValidationError::new("name", ValidationErrorKind::EmptyName),
            ValidationError::new(
                "trigger.conditions[1]",
                ValidationErrorKind::EmptySearchText,
            ),
        ]);

        assert_eq!(
            to_json(&error),
            json!({
                "code": "validation_failed",
                "message_key": "errors.validation_failed",
                "details": {
                    "kind": "validation",
                    "issues": [
                        { "path": "name", "code": "empty_name" },
                        { "path": "trigger.conditions[1]", "code": "empty_search_text" }
                    ]
                }
            })
        );
    }

    #[test]
    fn hides_the_system_message_of_an_io_error() {
        let source = io::Error::new(io::ErrorKind::PermissionDenied, "/home/alice/secret.pdf");
        let error = CommandError::from(source);

        let output = to_json(&error).to_string();
        assert!(!output.contains("alice"));
        assert_eq!(
            to_json(&error),
            json!({
                "code": "io",
                "message_key": "errors.io",
                "details": { "kind": "io", "io_kind": "permission_denied" }
            })
        );
    }

    #[test]
    fn maps_a_missing_file_to_not_found() {
        let error = CommandError::from(io::Error::from(io::ErrorKind::NotFound));

        assert_eq!(error.code, ErrorCode::NotFound);
        assert_eq!(error.message_key, "errors.not_found");
    }

    #[test]
    fn replaces_an_internal_error_with_a_generic_message() {
        let source = io::Error::other("/home/alice/secret.pdf");
        let error = CommandError::internal(&source);

        assert!(!to_json(&error).to_string().contains("alice"));
        assert_eq!(error.details, None);
    }
}
