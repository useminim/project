//! Doubles de test des ports et tests de contrat, disponibles avec la feature
//! `test-support`.
//!
//! - [`memory`] : dépôts en mémoire ;
//! - [`FixedClock`], [`SequentialIds`], [`StaticReader`] : horloge,
//!   identifiants et lecteur déterministes ;
//! - [`contracts`] : comportement attendu de toute implémentation des dépôts,
//!   à appeler depuis les tests de chaque implémentation ;
//! - [`samples`] : workflows et exécutions valides pour les tests.

pub mod contracts;
pub mod memory;
pub mod samples;

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use chrono::{DateTime, Duration, Utc};
use ulid::Ulid;

use crate::ports::{
    Clock, DocumentKind, DocumentMetadata, DocumentReader, IdGenerator, Page, PageText,
    RawDocument, ReadError,
};

/// Horloge arrêtée, avancée à la main.
#[derive(Debug)]
pub struct FixedClock {
    now: Mutex<DateTime<Utc>>,
}

impl FixedClock {
    /// Crée une horloge arrêtée sur `now`.
    #[must_use]
    pub const fn new(now: DateTime<Utc>) -> Self {
        Self {
            now: Mutex::new(now),
        }
    }

    /// Avance l'horloge.
    pub fn advance(&self, duration: Duration) {
        let mut now = self.now.lock().unwrap_or_else(PoisonError::into_inner);
        *now += duration;
    }
}

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Générateur d'identifiants séquentiels : 1, 2, 3… Les identifiants produits
/// sont triés dans l'ordre de leur création.
#[derive(Debug, Default)]
pub struct SequentialIds {
    last: AtomicU64,
}

impl IdGenerator for SequentialIds {
    fn next_ulid(&self) -> Ulid {
        let next = self.last.fetch_add(1, Ordering::Relaxed) + 1;
        Ulid::from(u128::from(next))
    }
}

/// Lecteur qui renvoie toujours le même résultat, pour un seul format.
#[derive(Debug)]
pub struct StaticReader {
    kind: DocumentKind,
    outcome: Outcome,
}

#[derive(Debug)]
enum Outcome {
    Document(RawDocument),
    Error(fn() -> ReadError),
}

impl StaticReader {
    /// Lecteur qui renvoie `document` pour son format.
    #[must_use]
    pub const fn new(document: RawDocument) -> Self {
        Self {
            kind: document.kind,
            outcome: Outcome::Document(document),
        }
    }

    /// Lecteur qui renvoie un document dont chaque page a le texte donné.
    #[must_use]
    pub fn with_pages(kind: DocumentKind, pages: &[&str]) -> Self {
        Self::new(RawDocument {
            kind,
            metadata: DocumentMetadata::default(),
            pages: (1..)
                .zip(pages)
                .map(|(number, text)| Page {
                    number,
                    text: PageText {
                        text: (*text).to_owned(),
                        spans: Vec::new(),
                    },
                })
                .collect(),
        })
    }

    /// Lecteur qui échoue avec l'erreur produite par `error`.
    #[must_use]
    pub const fn failing(kind: DocumentKind, error: fn() -> ReadError) -> Self {
        Self {
            kind,
            outcome: Outcome::Error(error),
        }
    }
}

impl DocumentReader for StaticReader {
    fn supports(&self, kind: DocumentKind) -> bool {
        kind == self.kind
    }

    fn read(&self, _path: &Path, kind: DocumentKind) -> Result<RawDocument, ReadError> {
        if kind != self.kind {
            return Err(ReadError::Unsupported(kind));
        }
        match &self.outcome {
            Outcome::Document(document) => Ok(document.clone()),
            Outcome::Error(error) => Err(error()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::WorkflowId;

    #[test]
    fn advances_the_fixed_clock() {
        let start = samples::date(0);
        let clock = FixedClock::new(start);

        clock.advance(Duration::minutes(5));

        assert_eq!(clock.now(), start + Duration::minutes(5));
    }

    #[test]
    fn generates_sequential_identifiers() {
        let ids = SequentialIds::default();

        let first = ids.workflow_id();
        let second = ids.workflow_id();

        assert_eq!(first, WorkflowId::from_ulid(Ulid::from(1_u128)));
        assert!(first < second);
    }

    #[test]
    fn returns_the_prepared_document() {
        let reader = StaticReader::with_pages(DocumentKind::Pdf, &["page 1", ""]);

        let document = reader.read(Path::new("a.pdf"), DocumentKind::Pdf).unwrap();

        assert_eq!(document.pages.len(), 2);
        assert_eq!(document.pages[1].number, 2);
        assert!(document.pages[1].text.is_empty());
        assert!(!reader.supports(DocumentKind::Png));
        assert!(matches!(
            reader.read(Path::new("a.png"), DocumentKind::Png),
            Err(ReadError::Unsupported(DocumentKind::Png))
        ));
    }

    #[test]
    fn returns_the_prepared_error() {
        let reader = StaticReader::failing(DocumentKind::Pdf, || ReadError::Encrypted);

        assert!(matches!(
            reader.read(Path::new("a.pdf"), DocumentKind::Pdf),
            Err(ReadError::Encrypted)
        ));
    }
}
