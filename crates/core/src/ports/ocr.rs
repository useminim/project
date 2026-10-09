//! Reconnaissance de texte (OCR) sur l'image d'une page.

use std::fmt;

use thiserror::Error;

use crate::ports::reader::PageText;

/// Langue du texte à reconnaître.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// Français.
    French,
    /// Anglais.
    English,
}

impl Language {
    /// Code ISO 639-2 de la langue (`fra`, `eng`), utilisé par les moteurs
    /// d'OCR.
    #[must_use]
    pub const fn iso_639_2(self) -> &'static str {
        match self {
            Self::French => "fra",
            Self::English => "eng",
        }
    }
}

/// Image d'une page en niveaux de gris : un octet par pixel, ligne par ligne,
/// depuis le coin supérieur gauche.
#[derive(Clone, PartialEq, Eq)]
pub struct PageImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl PageImage {
    /// Crée une image. Renvoie `None` si le nombre de pixels ne correspond pas
    /// aux dimensions.
    #[must_use]
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        let expected = u64::from(width) * u64::from(height);
        (u64::try_from(pixels.len()).ok() == Some(expected)).then_some(Self {
            width,
            height,
            pixels,
        })
    }

    /// Largeur, en pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Hauteur, en pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Pixels, ligne par ligne.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

impl fmt::Debug for PageImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PageImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

/// Échec de la reconnaissance de texte.
///
/// Le texte de ces erreurs peut être journalisé : il ne contient aucun texte
/// du document.
#[derive(Debug, Error)]
pub enum OcrError {
    /// Le moteur ne dispose pas des données de cette langue.
    #[error("langue {0:?} non disponible")]
    UnsupportedLanguage(Language),
    /// L'image est vide ou trop petite pour être analysée.
    #[error("image inexploitable")]
    InvalidImage,
    /// Le moteur a échoué. Sa source ne doit contenir aucun texte du document.
    #[error("échec du moteur d'OCR")]
    Engine(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Moteur de reconnaissance de texte.
pub trait OcrEngine: Send + Sync {
    /// Reconnaît le texte de `image`, écrit dans l'une des `languages`.
    ///
    /// Les positions des fragments sont exprimées en fractions de l'image, et
    /// chaque fragment porte la confiance de sa reconnaissance.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si une langue n'est pas disponible, si l'image est
    /// inexploitable ou si le moteur échoue.
    fn recognize(&self, image: &PageImage, languages: &[Language]) -> Result<PageText, OcrError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_the_number_of_pixels() {
        assert!(PageImage::new(2, 3, vec![0; 6]).is_some());
        assert!(PageImage::new(2, 3, vec![0; 5]).is_none());
        assert!(PageImage::new(0, 0, Vec::new()).is_some());
    }

    #[test]
    fn gives_the_iso_code_of_each_language() {
        assert_eq!(Language::French.iso_639_2(), "fra");
        assert_eq!(Language::English.iso_639_2(), "eng");
    }
}
