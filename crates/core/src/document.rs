//! Documents reçus par minim : où se trouve le fichier, d'où il vient et où
//! en est son traitement.
//!
//! Un [`Document`] relie un [`DocumentId`] au fichier qu'il désigne. Il reste
//! sur la machine : il est enregistré dans la base locale chiffrée, et
//! l'interface n'en reçoit qu'un [`DocumentSummary`], sans le chemin complet.

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::id::DocumentId;
use crate::ports::DocumentKind;

/// Empreinte SHA-256 du contenu d'un fichier, pour reconnaître un document
/// déjà reçu même s'il a été renommé ou déplacé.
///
/// Sérialisée en 64 caractères hexadécimaux minuscules.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(type = "string"))]
pub struct ContentHash([u8; 32]);

/// Erreur de lecture d'une empreinte.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("une empreinte SHA-256 s'écrit avec 64 caractères hexadécimaux")]
pub struct ContentHashError;

impl ContentHash {
    /// Construit une empreinte à partir de ses 32 octets.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Octets de l'empreinte.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

// L'empreinte ne révèle rien du contenu : elle peut apparaître dans les logs.
impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentHash({self})")
    }
}

impl FromStr for ContentHash {
    type Err = ContentHashError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let digits = s.as_bytes();
        if digits.len() != 64 {
            return Err(ContentHashError);
        }
        let mut bytes = [0_u8; 32];
        let (pairs, _) = digits.as_chunks::<2>();
        for (byte, &[high, low]) in bytes.iter_mut().zip(pairs) {
            let high = hex_value(high).ok_or(ContentHashError)?;
            let low = hex_value(low).ok_or(ContentHashError)?;
            *byte = (high << 4) | low;
        }
        Ok(Self(bytes))
    }
}

/// Valeur d'un chiffre hexadécimal minuscule.
const fn hex_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        _ => None,
    }
}

impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(D::Error::custom)
    }
}

/// Provenance d'un document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DocumentOrigin {
    /// Fichier choisi ou déposé par l'utilisateur.
    Manual,
    /// Impression reçue par l'imprimante virtuelle.
    Printer,
}

/// Avancement du traitement d'un document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus {
    /// Reçu, en attente de traitement.
    Pending,
    /// En cours de lecture, d'analyse ou d'exécution.
    Processing,
    /// Traité : les workflows qui le reconnaissent ont été exécutés.
    Processed,
    /// En zone d'attente : une décision de l'utilisateur est nécessaire.
    Waiting,
    /// Le traitement a échoué (lecture impossible, par exemple).
    Failed,
    /// Écarté par l'utilisateur ; minim ne le traite plus.
    Ignored,
}

impl DocumentStatus {
    /// Indique si un document peut passer de ce statut à `next` au fil du
    /// traitement :
    ///
    /// - `Pending` → `Processing` (traitement) ou `Ignored` ;
    /// - `Processing` → `Processed`, `Waiting` ou `Failed` (fin du
    ///   traitement), ou `Pending` (reprise après un arrêt brutal) ;
    /// - `Waiting` et `Failed` → `Processing` (nouvel essai, workflow choisi
    ///   par l'utilisateur) ou `Ignored`.
    ///
    /// `Processed` et `Ignored` sont des états finaux : seul un retraitement
    /// explicite ([`Document::reprocess`]) en fait sortir un document.
    #[must_use]
    pub const fn can_become(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Pending, Self::Processing | Self::Ignored)
                | (
                    Self::Processing,
                    Self::Processed | Self::Waiting | Self::Failed | Self::Pending
                )
                | (
                    Self::Waiting | Self::Failed,
                    Self::Processing | Self::Ignored
                )
        )
    }
}

/// Changement de statut refusé.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("un document ne peut pas passer de {from:?} à {to:?}")]
pub struct StatusTransitionError {
    /// Statut actuel.
    pub from: DocumentStatus,
    /// Statut demandé.
    pub to: DocumentStatus,
}

/// Document reçu par minim.
///
/// Le chemin et le nom du fichier peuvent révéler des données personnelles :
/// `Debug` ne les affiche pas. Le document se sérialise pour la base locale
/// chiffrée, jamais pour l'interface ni le cloud (voir [`DocumentSummary`]).
/// Un chemin qui n'est pas en UTF-8 ne peut pas être sérialisé.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    /// Identifiant du document.
    pub id: DocumentId,
    /// Emplacement actuel du fichier, mis à jour quand une action le déplace
    /// ou le renomme.
    pub path: PathBuf,
    /// Nom du fichier à sa réception, extension comprise.
    pub original_name: String,
    /// Empreinte du contenu à la réception.
    pub sha256: ContentHash,
    /// Taille en octets à la réception.
    pub size: u64,
    /// Format du document.
    pub kind: DocumentKind,
    /// Provenance.
    pub origin: DocumentOrigin,
    /// Date de réception.
    pub received_at: DateTime<Utc>,
    /// Avancement du traitement.
    pub status: DocumentStatus,
}

impl fmt::Debug for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Document")
            .field("id", &self.id)
            .field("sha256", &self.sha256)
            .field("size", &self.size)
            .field("kind", &self.kind)
            .field("origin", &self.origin)
            .field("received_at", &self.received_at)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl Document {
    /// Fait passer le document au statut `next`.
    ///
    /// # Erreurs
    ///
    /// Renvoie [`StatusTransitionError`] si la transition n'est pas permise
    /// (voir [`DocumentStatus::can_become`]) ; le statut est alors inchangé.
    pub fn transition(&mut self, next: DocumentStatus) -> Result<(), StatusTransitionError> {
        if !self.status.can_become(next) {
            return Err(StatusTransitionError {
                from: self.status,
                to: next,
            });
        }
        self.status = next;
        Ok(())
    }

    /// Remet en attente de traitement un document traité ou écarté, à la
    /// demande de l'utilisateur.
    ///
    /// # Erreurs
    ///
    /// Renvoie [`StatusTransitionError`] si le document n'est ni `Processed`
    /// ni `Ignored` : les autres statuts ont leurs propres transitions.
    pub fn reprocess(&mut self) -> Result<(), StatusTransitionError> {
        if !matches!(
            self.status,
            DocumentStatus::Processed | DocumentStatus::Ignored
        ) {
            return Err(StatusTransitionError {
                from: self.status,
                to: DocumentStatus::Pending,
            });
        }
        self.status = DocumentStatus::Pending;
        Ok(())
    }

    /// Nom actuel du fichier, ou le nom d'origine si le chemin n'en a pas.
    #[must_use]
    pub fn file_name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.original_name.clone(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    /// Résumé destiné à l'interface.
    #[must_use]
    pub fn summary(&self) -> DocumentSummary {
        DocumentSummary {
            id: self.id,
            name: self.file_name(),
            original_name: self.original_name.clone(),
            size: self.size,
            kind: self.kind,
            origin: self.origin,
            received_at: self.received_at,
            status: self.status,
        }
    }
}

/// Document tel que l'interface l'affiche : son nom, mais pas son chemin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct DocumentSummary {
    /// Identifiant du document.
    pub id: DocumentId,
    /// Nom actuel du fichier.
    pub name: String,
    /// Nom du fichier à sa réception.
    pub original_name: String,
    /// Taille en octets à la réception.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub size: u64,
    /// Format du document.
    pub kind: DocumentKind,
    /// Provenance.
    pub origin: DocumentOrigin,
    /// Date de réception.
    pub received_at: DateTime<Utc>,
    /// Avancement du traitement.
    pub status: DocumentStatus,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::testing::samples;

    const ALL: [DocumentStatus; 6] = [
        DocumentStatus::Pending,
        DocumentStatus::Processing,
        DocumentStatus::Processed,
        DocumentStatus::Waiting,
        DocumentStatus::Failed,
        DocumentStatus::Ignored,
    ];

    #[test]
    fn allows_only_the_documented_transitions() {
        use DocumentStatus::{Failed, Ignored, Pending, Processed, Processing, Waiting};

        let allowed = [
            (Pending, Processing),
            (Pending, Ignored),
            (Processing, Processed),
            (Processing, Waiting),
            (Processing, Failed),
            (Processing, Pending),
            (Waiting, Processing),
            (Waiting, Ignored),
            (Failed, Processing),
            (Failed, Ignored),
        ];
        for from in ALL {
            for to in ALL {
                assert_eq!(
                    from.can_become(to),
                    allowed.contains(&(from, to)),
                    "{from:?} → {to:?}"
                );
            }
        }
    }

    #[test]
    fn changes_the_status_only_when_allowed() {
        let mut document = samples::document(samples::document_id(1), 1);

        document.transition(DocumentStatus::Processing).unwrap();
        document.transition(DocumentStatus::Processed).unwrap();
        assert_eq!(
            document.transition(DocumentStatus::Pending),
            Err(StatusTransitionError {
                from: DocumentStatus::Processed,
                to: DocumentStatus::Pending,
            })
        );
        assert_eq!(document.status, DocumentStatus::Processed);
    }

    #[test]
    fn reprocesses_only_a_finished_document() {
        let mut document = samples::document(samples::document_id(1), 1);
        assert!(document.reprocess().is_err(), "déjà en attente");

        document.status = DocumentStatus::Processed;
        document.reprocess().unwrap();
        assert_eq!(document.status, DocumentStatus::Pending);

        document.status = DocumentStatus::Ignored;
        document.reprocess().unwrap();
        assert_eq!(document.status, DocumentStatus::Pending);

        document.status = DocumentStatus::Processing;
        assert!(document.reprocess().is_err());
    }

    #[test]
    fn writes_the_hash_in_lowercase_hexadecimal() {
        let hash = samples::content_hash(0xab);
        let text = hash.to_string();

        assert_eq!(text, "ab".repeat(32));
        assert_eq!(text.parse::<ContentHash>(), Ok(hash));
        assert_eq!(serde_json::to_value(hash).unwrap(), json!("ab".repeat(32)));
    }

    #[test]
    fn rejects_an_invalid_hash() {
        let invalid = [
            String::new(),
            "a".repeat(63),
            "a".repeat(65),
            "AB".repeat(32),
            "zz".repeat(32),
        ];
        for text in invalid {
            assert_eq!(text.parse::<ContentHash>(), Err(ContentHashError), "{text}");
        }
        assert!(serde_json::from_value::<ContentHash>(json!("abc")).is_err());
    }

    #[test]
    fn hides_the_path_and_the_name_in_debug_output() {
        let document = samples::document(samples::document_id(1), 1);

        let output = format!("{document:?}");

        assert!(!output.contains("facture"), "{output}");
        assert!(output.contains("Pending"), "{output}");
    }

    #[test]
    fn summarizes_the_document_without_its_path() {
        let mut document = samples::document(samples::document_id(1), 1);
        document.path = PathBuf::from("/home/alice/factures/2026-10-01.pdf");

        let summary = serde_json::to_value(document.summary()).unwrap();

        assert_eq!(summary["name"], "2026-10-01.pdf");
        assert_eq!(summary["original_name"], "facture-1.pdf");
        assert!(!summary.to_string().contains("alice"), "{summary}");
    }

    #[test]
    fn serializes_the_origin_with_a_type_tag() {
        let origin = DocumentOrigin::Manual;

        let value = serde_json::to_value(origin).unwrap();

        assert_eq!(value, json!({ "type": "manual" }));
        assert_eq!(
            serde_json::from_value::<DocumentOrigin>(value).unwrap(),
            origin
        );
        assert_eq!(
            serde_json::to_value(DocumentOrigin::Printer).unwrap(),
            json!({ "type": "printer" })
        );
    }
}
