//! Actions : les opérations appliquées à un document reconnu.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::field::FieldName;
use crate::template::Template;

/// Opération appliquée au document par un workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Renomme le fichier à partir d'un modèle.
    Rename {
        /// Nouveau nom, avec des variables `{champ}`.
        template: Template,
    },
    /// Déplace le fichier dans un dossier.
    Move {
        /// Dossier de destination.
        destination: PathBuf,
        /// Crée le dossier de destination s'il n'existe pas.
        #[serde(default)]
        create_folders: bool,
    },
    /// Copie le fichier dans un dossier.
    Copy {
        /// Dossier de destination.
        destination: PathBuf,
    },
    /// Convertit le document dans un autre format.
    Convert {
        /// Format cible.
        to: ConvertFormat,
    },
    /// Extrait des champs du document dans un fichier de données.
    ExtractFields {
        /// Champs à extraire.
        fields: Vec<FieldName>,
        /// Format du fichier produit.
        output: ExportFormat,
    },
}

/// Type d'une action, sans ses paramètres. Utilisé dans le journal d'exécution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// Voir [`Action::Rename`].
    Rename,
    /// Voir [`Action::Move`].
    Move,
    /// Voir [`Action::Copy`].
    Copy,
    /// Voir [`Action::Convert`].
    Convert,
    /// Voir [`Action::ExtractFields`].
    ExtractFields,
}

/// Format cible d'une conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ConvertFormat {
    /// Document PDF.
    Pdf,
    /// Image PNG, une par page.
    Png,
}

/// Format du fichier produit par une extraction de champs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    /// Fichier CSV.
    Csv,
    /// Fichier JSON.
    Json,
}

impl Action {
    /// Renvoie le type de l'action.
    #[must_use]
    pub const fn kind(&self) -> ActionKind {
        match self {
            Self::Rename { .. } => ActionKind::Rename,
            Self::Move { .. } => ActionKind::Move,
            Self::Copy { .. } => ActionKind::Copy,
            Self::Convert { .. } => ActionKind::Convert,
            Self::ExtractFields { .. } => ActionKind::ExtractFields,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_the_documented_example() {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/action.json")).unwrap();
        let action: Action = serde_json::from_value(value.clone()).unwrap();

        assert_eq!(action.kind(), ActionKind::Rename);
        assert_eq!(serde_json::to_value(&action).unwrap(), value);
    }

    #[test]
    fn reads_every_action_type() {
        let actions = json!([
            { "type": "rename", "template": "{date}.pdf" },
            { "type": "move", "destination": "/factures/2026", "create_folders": true },
            { "type": "copy", "destination": "/archives" },
            { "type": "convert", "to": "png" },
            { "type": "extract_fields", "fields": ["date", "invoice_total"], "output": "csv" }
        ]);
        let parsed: Vec<Action> = serde_json::from_value(actions.clone()).unwrap();

        assert_eq!(serde_json::to_value(&parsed).unwrap(), actions);
        assert_eq!(
            parsed.iter().map(Action::kind).collect::<Vec<_>>(),
            [
                ActionKind::Rename,
                ActionKind::Move,
                ActionKind::Copy,
                ActionKind::Convert,
                ActionKind::ExtractFields,
            ]
        );
    }

    #[test]
    fn rejects_an_invalid_template() {
        let result = serde_json::from_value::<Action>(json!({ "type": "rename", "template": "{" }));
        assert!(result.is_err());
    }
}
