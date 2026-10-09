//! Lecture des documents : ce qu'un lecteur extrait d'un fichier, avant
//! l'analyse en profil.

use std::fmt;
use std::io;
use std::path::Path;

use chrono::{DateTime, Utc};
use thiserror::Error;

/// Format d'un document, déduit de son extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentKind {
    /// Document PDF.
    Pdf,
    /// Image PNG.
    Png,
    /// Image JPEG.
    Jpeg,
    /// Image TIFF, éventuellement sur plusieurs pages.
    Tiff,
    /// Document Word.
    Docx,
    /// Classeur Excel.
    Xlsx,
}

impl DocumentKind {
    /// Déduit le format d'une extension, sans le point et sans tenir compte de
    /// la casse. Renvoie `None` pour un format non pris en charge.
    #[must_use]
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "pdf" => Some(Self::Pdf),
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "tif" | "tiff" => Some(Self::Tiff),
            "docx" => Some(Self::Docx),
            "xlsx" => Some(Self::Xlsx),
            _ => None,
        }
    }
}

/// Rectangle dans une page, en fractions de sa largeur et de sa hauteur
/// (entre 0 et 1), depuis le coin supérieur gauche. Ces coordonnées ne
/// dépendent ni de la résolution ni de l'unité du format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Bord gauche.
    pub x: f32,
    /// Bord supérieur.
    pub y: f32,
    /// Largeur.
    pub width: f32,
    /// Hauteur.
    pub height: f32,
}

/// Fragment de texte et sa position dans la page.
#[derive(Clone, PartialEq)]
pub struct TextSpan {
    /// Texte du fragment.
    pub text: String,
    /// Position du fragment.
    pub bounds: Rect,
    /// Confiance de la reconnaissance, entre 0 et 1 ; absente pour un texte
    /// extrait directement du fichier.
    pub confidence: Option<f32>,
}

/// Texte d'une page.
#[derive(Clone, Default, PartialEq)]
pub struct PageText {
    /// Texte complet de la page, dans l'ordre de lecture.
    pub text: String,
    /// Fragments positionnés ; vide si le lecteur ne fournit pas les positions.
    pub spans: Vec<TextSpan>,
}

impl PageText {
    /// Indique si la page n'a aucun texte (page scannée, par exemple) : elle
    /// devra passer par l'OCR.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

/// Page d'un document.
#[derive(Clone, PartialEq)]
pub struct Page {
    /// Numéro de la page, à partir de 1.
    pub number: u32,
    /// Texte de la page.
    pub text: PageText,
}

/// Métadonnées d'un document, quand le format les fournit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentMetadata {
    /// Date de création inscrite dans le document.
    pub created_at: Option<DateTime<Utc>>,
    /// Logiciel qui a produit le document (logiciel de numérisation, de
    /// bureautique…), utile pour reconnaître un document scanné.
    pub producer: Option<String>,
}

/// Contenu brut d'un document, tel que le lecteur l'a extrait.
#[derive(Debug, Clone, PartialEq)]
pub struct RawDocument {
    /// Format du document.
    pub kind: DocumentKind,
    /// Métadonnées.
    pub metadata: DocumentMetadata,
    /// Pages, dans l'ordre.
    pub pages: Vec<Page>,
}

// Le texte d'un document ne doit jamais apparaître dans un log : `Debug`
// n'affiche que sa longueur.

impl fmt::Debug for TextSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextSpan")
            .field("chars", &self.text.chars().count())
            .field("bounds", &self.bounds)
            .field("confidence", &self.confidence)
            .finish()
    }
}

impl fmt::Debug for PageText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PageText")
            .field("chars", &self.text.chars().count())
            .field("spans", &self.spans.len())
            .finish()
    }
}

impl fmt::Debug for Page {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Page")
            .field("number", &self.number)
            .field("text", &self.text)
            .finish()
    }
}

/// Échec de la lecture d'un document.
///
/// Le texte de ces erreurs peut être journalisé : il ne contient ni chemin ni
/// contenu du document.
#[derive(Debug, Error)]
pub enum ReadError {
    /// Ce lecteur ne prend pas en charge ce format.
    #[error("format {0:?} non pris en charge par ce lecteur")]
    Unsupported(DocumentKind),
    /// Le document est chiffré ou protégé par un mot de passe.
    #[error("document chiffré")]
    Encrypted,
    /// Le fichier est endommagé ou ne correspond pas à son format.
    #[error("document endommagé")]
    Corrupted,
    /// Le document dépasse la taille maximale acceptée.
    #[error("document trop volumineux (limite : {limit_bytes} octets)")]
    TooLarge {
        /// Taille maximale acceptée, en octets.
        limit_bytes: u64,
    },
    /// Le fichier ne peut pas être lu.
    #[error("lecture du fichier impossible")]
    Io(#[source] io::Error),
}

/// Lit un format de document et en extrait le texte, page par page.
pub trait DocumentReader: Send + Sync {
    /// Indique si ce lecteur prend en charge le format.
    fn supports(&self, kind: DocumentKind) -> bool;

    /// Lit le document situé à `path`, dont le format est `kind`.
    ///
    /// # Erreurs
    ///
    /// Renvoie [`ReadError::Unsupported`] si le format n'est pas pris en
    /// charge, et les autres variantes si le fichier ne peut pas être lu.
    fn read(&self, path: &Path, kind: DocumentKind) -> Result<RawDocument, ReadError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduces_the_kind_from_the_extension() {
        assert_eq!(DocumentKind::from_extension("PDF"), Some(DocumentKind::Pdf));
        assert_eq!(
            DocumentKind::from_extension("jpg"),
            Some(DocumentKind::Jpeg)
        );
        assert_eq!(
            DocumentKind::from_extension("tif"),
            Some(DocumentKind::Tiff)
        );
        assert_eq!(DocumentKind::from_extension("odt"), None);
        assert_eq!(DocumentKind::from_extension(".pdf"), None);
    }

    #[test]
    fn hides_the_text_in_debug_output() {
        let page = Page {
            number: 1,
            text: PageText {
                text: "Dossier médical de Mme Martin".to_owned(),
                spans: vec![TextSpan {
                    text: "Martin".to_owned(),
                    bounds: Rect {
                        x: 0.1,
                        y: 0.2,
                        width: 0.3,
                        height: 0.05,
                    },
                    confidence: None,
                }],
            },
        };

        let output = format!("{page:?}");
        assert!(!output.contains("Martin"), "{output}");
        assert!(output.contains("chars: 29"), "{output}");
    }

    #[test]
    fn treats_a_page_without_text_as_empty() {
        assert!(PageText::default().is_empty());
        assert!(
            PageText {
                text: " \n".to_owned(),
                spans: Vec::new()
            }
            .is_empty()
        );
    }
}
