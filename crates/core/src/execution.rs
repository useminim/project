//! Journal d'exécution : la trace locale du passage d'un document dans un
//! workflow.

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::action::ActionKind;
use crate::id::{DocumentId, RunId, WorkflowId};

/// Entrée du journal pour une exécution de workflow sur un document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ExecutionLog {
    /// Identifiant de l'exécution.
    pub id: RunId,
    /// Document traité.
    pub document_id: DocumentId,
    /// Workflow exécuté.
    pub workflow_id: WorkflowId,
    /// Version du workflow au moment de l'exécution.
    pub workflow_version: u32,
    /// Résultat global.
    pub status: ExecutionStatus,
    /// Actions exécutées, dans l'ordre.
    pub actions: Vec<ActionRecord>,
    /// Message d'erreur si l'exécution a échoué.
    pub error: Option<String>,
    /// Début de l'exécution.
    pub started_at: DateTime<Utc>,
}

/// Résultat global d'une exécution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// Toutes les actions ont réussi.
    Success,
    /// Une action a échoué.
    Failed,
    /// Le document a été placé en zone d'attente.
    SentToWaitingZone,
}

/// Trace d'une action au sein d'une exécution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ActionRecord {
    /// Type de l'action.
    #[serde(rename = "type")]
    pub kind: ActionKind,
    /// Résultat de l'action.
    pub status: ActionStatus,
    /// Durée de l'action en millisecondes.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub duration_ms: u64,
    /// Opération faite sur le disque, de quoi l'annuler (prompt 24). Absente
    /// si l'action n'a rien changé (renommage vers le même nom, par exemple).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub operation: Option<FileOperation>,
}

/// Nature d'une opération sur un fichier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    /// Le fichier a changé de nom dans son dossier.
    Rename,
    /// Le fichier a changé de dossier.
    Move,
    /// Une copie du fichier a été créée ; l'original n'a pas bougé.
    Copy,
}

/// Opération faite sur un fichier, enregistrée dans le journal chiffré pour
/// pouvoir l'annuler. `Debug` n'affiche pas les chemins.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FileOperation {
    /// Nature de l'opération.
    pub kind: OperationKind,
    /// Emplacement d'origine (pour une copie : le fichier copié).
    pub from: PathBuf,
    /// Emplacement produit.
    pub to: PathBuf,
    /// Empreinte du fichier produit : l'annulation vérifie qu'il n'a pas
    /// changé depuis.
    pub fingerprint: Fingerprint,
}

impl fmt::Debug for FileOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileOperation")
            .field("kind", &self.kind)
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

/// Empreinte blake3 du contenu d'un fichier, écrite en 64 caractères
/// hexadécimaux minuscules. Elle ne révèle rien du contenu.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(type = "string"))]
pub struct Fingerprint([u8; 32]);

/// Empreinte mal écrite.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("une empreinte s'écrit avec 64 caractères hexadécimaux minuscules")]
pub struct FingerprintError;

impl Fingerprint {
    /// Empreinte à partir de ses 32 octets.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint({self})")
    }
}

impl FromStr for Fingerprint {
    type Err = FingerprintError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let digits = s.as_bytes();
        if digits.len() != 64 {
            return Err(FingerprintError);
        }
        let mut bytes = [0_u8; 32];
        let (pairs, _) = digits.as_chunks::<2>();
        for (byte, &[high, low]) in bytes.iter_mut().zip(pairs) {
            let high = hex_value(high).ok_or(FingerprintError)?;
            let low = hex_value(low).ok_or(FingerprintError)?;
            *byte = (high << 4) | low;
        }
        Ok(Self(bytes))
    }
}

const fn hex_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        _ => None,
    }
}

impl Serialize for Fingerprint {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Fingerprint {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// Résultat d'une action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    /// L'action a réussi.
    Success,
    /// L'action a échoué.
    Failed,
    /// L'action avait réussi, puis a été annulée parce qu'une action suivante
    /// du workflow a échoué (D-10).
    RolledBack,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_the_documented_example() {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/execution_log.json")).unwrap();
        let log: ExecutionLog = serde_json::from_value(value.clone()).unwrap();

        assert_eq!(log.status, ExecutionStatus::Success);
        assert_eq!(log.actions[0].kind, ActionKind::Rename);
        assert_eq!(serde_json::to_value(&log).unwrap(), value);
    }

    #[test]
    fn records_the_file_operation_of_an_action() {
        let fingerprint = "ab".repeat(32);
        let value = json!({
            "type": "move",
            "status": "rolled_back",
            "duration_ms": 12,
            "operation": {
                "kind": "move",
                "from": "/home/alice/Factures/a.pdf",
                "to": "/home/alice/Archives/a.pdf",
                "fingerprint": fingerprint
            }
        });
        let record: ActionRecord = serde_json::from_value(value.clone()).unwrap();

        assert_eq!(record.status, ActionStatus::RolledBack);
        assert_eq!(serde_json::to_value(&record).unwrap(), value);
        assert!(
            !format!("{record:?}").contains("alice"),
            "Debug n'affiche pas les chemins"
        );
    }

    #[test]
    fn rejects_a_malformed_fingerprint() {
        for text in [
            "",
            "AB".repeat(32).as_str(),
            "zz".repeat(32).as_str(),
            "ab".repeat(31).as_str(),
        ] {
            assert!(text.parse::<Fingerprint>().is_err(), "{text}");
        }
    }

    #[test]
    fn reads_the_waiting_zone_status() {
        let status: ExecutionStatus =
            serde_json::from_value(json!("sent_to_waiting_zone")).unwrap();
        assert_eq!(status, ExecutionStatus::SentToWaitingZone);
    }
}
