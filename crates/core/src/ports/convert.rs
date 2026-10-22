//! Conversion de documents : rendu des pages d'un PDF en images, et
//! assemblage d'images en PDF.
//!
//! Les conversions travaillent sur des octets en mémoire ; c'est l'action
//! `convert` qui lit et écrit les fichiers (`FileSystem`), sans jamais
//! écraser.

use std::io;

use thiserror::Error;

use crate::ports::reader::{DocumentKind, ReadError};

/// Format des images produites à partir d'un PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    /// PNG, sans perte.
    Png,
    /// JPEG, avec la qualité donnée (1 à 100).
    Jpeg {
        /// Qualité de compression.
        quality: u8,
    },
}

/// Rendu d'une page en image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    /// Format des images.
    pub format: ImageFormat,
    /// Résolution, en points par pouce.
    pub dpi: u16,
}

/// Échec d'une conversion. Ne contient ni chemin ni contenu de document.
#[derive(Debug, Error)]
pub enum ConvertError {
    /// Le document source n'a pas pu être lu (chiffré, endommagé, trop gros,
    /// trop long, format non pris en charge…).
    #[error(transparent)]
    Read(#[from] ReadError),
    /// Une page rendue dépasse la taille d'image autorisée à cette
    /// résolution.
    #[error("page trop grande pour être rendue à cette résolution")]
    PageTooLarge,
    /// L'image produite n'a pas pu être écrite.
    #[error("écriture d'une image produite impossible")]
    Output(#[source] io::Error),
}

/// Convertisseur de documents.
pub trait DocumentConverter: Send + Sync {
    /// Taille maximale, en octets, d'un document à convertir : l'appelant ne
    /// lit pas un fichier plus gros.
    fn max_input_bytes(&self) -> u64;

    /// Rend chaque page du PDF `pdf` en image et la passe à `page`, dans
    /// l'ordre, avec son numéro (à partir de 1). Les pages sont produites une
    /// par une : un document de 500 pages ne tient jamais entier en mémoire.
    /// Renvoie le nombre de pages.
    ///
    /// # Erreurs
    ///
    /// [`ConvertError::Read`] si le PDF ne peut pas être lu,
    /// [`ConvertError::PageTooLarge`] si une page est trop grande,
    /// [`ConvertError::Output`] si `page` échoue ; la conversion s'arrête
    /// alors.
    fn pdf_to_images(
        &self,
        pdf: &[u8],
        options: RenderOptions,
        page: &mut dyn FnMut(u32, Vec<u8>) -> io::Result<()>,
    ) -> Result<u32, ConvertError>;

    /// Assemble des images (PNG ou JPEG) en un PDF, une page par image, à la
    /// taille de l'image et dans l'orientation indiquée par ses données EXIF.
    ///
    /// # Erreurs
    ///
    /// [`ConvertError::Read`] si une image ne peut pas être lue
    /// ([`ReadError::Unsupported`] pour un format autre que PNG ou JPEG).
    fn images_to_pdf(&self, images: &[(DocumentKind, &[u8])]) -> Result<Vec<u8>, ConvertError>;
}
