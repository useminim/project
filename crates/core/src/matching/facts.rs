//! Propriétés d'un fichier, sur lesquelles portent les déclencheurs externes.

use std::fs;
use std::io;
use std::path::Path;

use chrono::{DateTime, Utc};

use crate::redacted::Redacted;

/// Propriétés d'un fichier reçu. Le nom est enveloppé dans [`Redacted`] :
/// `Debug` ne l'affiche jamais.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFacts {
    name: Redacted<String>,
    size: u64,
    modified_at: Option<DateTime<Utc>>,
    created_at: Option<DateTime<Utc>>,
}

impl FileFacts {
    /// Fichier de nom `name` (sans dossier) et de taille `size` en octets.
    pub fn new(name: impl Into<String>, size: u64) -> Self {
        Self {
            name: Redacted::new(name.into()),
            size,
            modified_at: None,
            created_at: None,
        }
    }

    /// Ajoute les dates de modification et de création, quand le système les
    /// fournit.
    #[must_use]
    pub const fn with_dates(
        mut self,
        modified_at: Option<DateTime<Utc>>,
        created_at: Option<DateTime<Utc>>,
    ) -> Self {
        self.modified_at = modified_at;
        self.created_at = created_at;
        self
    }

    /// Lit les propriétés du fichier situé à `path`. Un nom qui n'est pas de
    /// l'Unicode valide est converti en remplaçant les octets invalides.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si les métadonnées du fichier ne peuvent pas être
    /// lues.
    pub fn from_path(path: &Path) -> io::Result<Self> {
        let metadata = fs::metadata(path)?;
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let modified_at = metadata.modified().ok().map(DateTime::<Utc>::from);
        let created_at = metadata.created().ok().map(DateTime::<Utc>::from);
        Ok(Self::new(name, metadata.len()).with_dates(modified_at, created_at))
    }

    /// Nom complet (`Facture_2026.pdf`).
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.expose()
    }

    /// Nom sans la dernière extension (`Facture_2026`, `archive.tar`).
    #[must_use]
    pub fn stem(&self) -> &str {
        split_extension(self.name()).0
    }

    /// Dernière extension, sans le point et telle qu'écrite (`pdf`, `gz`).
    /// Absente pour `LISEZMOI`, `.bashrc` ou `nom.`.
    #[must_use]
    pub fn extension(&self) -> Option<&str> {
        split_extension(self.name()).1
    }

    /// Taille, en octets.
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// Date de dernière modification.
    #[must_use]
    pub const fn modified_at(&self) -> Option<DateTime<Utc>> {
        self.modified_at
    }

    /// Date de création.
    #[must_use]
    pub const fn created_at(&self) -> Option<DateTime<Utc>> {
        self.created_at
    }
}

/// Sépare le nom en radical et dernière extension, comme `Path::extension`,
/// mais sans dépendre des séparateurs de l'OS : un point en tête (fichier
/// caché) ou en fin de nom n'introduit pas d'extension.
fn split_extension(name: &str) -> (&str, Option<&str>) {
    match name.rfind('.') {
        Some(dot) if dot > 0 && dot + 1 < name.len() => (&name[..dot], Some(&name[dot + 1..])),
        _ => (name, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_the_name_like_the_file_system() {
        let cases = [
            ("Facture_2026.pdf", "Facture_2026", Some("pdf")),
            ("archive.tar.gz", "archive.tar", Some("gz")),
            ("LISEZMOI", "LISEZMOI", None),
            (".bashrc", ".bashrc", None),
            (".config.json", ".config", Some("json")),
            ("nom.", "nom.", None),
            ("SCAN.PDF", "SCAN", Some("PDF")),
        ];
        for (name, stem, extension) in cases {
            let facts = FileFacts::new(name, 1);
            assert_eq!(facts.stem(), stem, "{name}");
            assert_eq!(facts.extension(), extension, "{name}");
        }
    }

    #[test]
    fn never_prints_the_name() {
        let facts = FileFacts::new("Dossier_Martin.pdf", 10);

        assert!(!format!("{facts:?}").contains("Martin"));
    }

    #[test]
    fn reads_the_facts_of_a_file() {
        let dir = tempfile_dir();
        let path = dir.join("Relevé 2026.pdf");
        fs::write(&path, b"12345").unwrap();

        let facts = FileFacts::from_path(&path).unwrap();

        assert_eq!(facts.name(), "Relevé 2026.pdf");
        assert_eq!(facts.size(), 5);
        assert!(facts.modified_at().is_some());
        fs::remove_dir_all(dir).unwrap();
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("minim-facts-{}", ulid::Ulid::generate()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
