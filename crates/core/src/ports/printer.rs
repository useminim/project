//! Imprimante virtuelle : imprimer vers elle envoie le document à minim.
//!
//! L'implémentation remet chaque impression à un [`JobSink`], appelé depuis le
//! fil de l'implémentation. `minim-core` n'impose ainsi aucun type de canal ni
//! aucun runtime : l'application choisit quoi faire de l'impression (canal,
//! événement Tauri…) dans son récepteur (D-28).

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::redacted::Redacted;

/// Impression reçue par l'imprimante virtuelle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedJob {
    /// Fichier PDF produit par l'impression, dans un dossier propre à minim.
    pub pdf_path: Redacted<PathBuf>,
    /// Titre de l'impression donné par l'application d'origine, souvent le
    /// nom du document.
    pub title: Option<Redacted<String>>,
    /// Date de réception.
    pub received_at: DateTime<Utc>,
}

/// Reçoit les impressions capturées.
pub trait JobSink: Send + Sync {
    /// Appelé pour chaque impression, depuis le fil de l'imprimante. Doit
    /// rendre la main rapidement.
    fn job_received(&self, job: CapturedJob);
}

/// Échec d'une opération sur l'imprimante virtuelle.
#[derive(Debug, Error)]
pub enum PrinterError {
    /// L'imprimante n'est pas installée.
    #[error("imprimante virtuelle non installée")]
    NotInstalled,
    /// L'opération demande des droits d'administrateur.
    #[error("droits d'administrateur nécessaires")]
    PermissionDenied,
    /// L'imprimante virtuelle n'est pas disponible sur ce système.
    #[error("imprimante virtuelle non disponible sur ce système")]
    Unsupported,
    /// Un fichier ou un dossier de l'imprimante est inaccessible.
    #[error("accès aux fichiers de l'imprimante impossible")]
    Io(#[source] io::Error),
    /// Le système d'impression a refusé l'opération.
    #[error("échec du système d'impression")]
    System(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Installation et écoute de l'imprimante virtuelle (une implémentation par
/// OS).
pub trait PrintCapture: Send + Sync {
    /// Installe l'imprimante.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si l'installation échoue.
    fn install(&self) -> Result<(), PrinterError>;

    /// Désinstalle l'imprimante.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si la désinstallation échoue.
    fn uninstall(&self) -> Result<(), PrinterError>;

    /// Indique si l'imprimante est installée.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si l'état ne peut pas être lu.
    fn is_installed(&self) -> Result<bool, PrinterError>;

    /// Commence à remettre les impressions à `sink`, jusqu'à l'appel de
    /// [`PrintCapture::stop`].
    ///
    /// # Erreurs
    ///
    /// Renvoie [`PrinterError::NotInstalled`] si l'imprimante n'est pas
    /// installée.
    fn start(&self, sink: Arc<dyn JobSink>) -> Result<(), PrinterError>;

    /// Arrête la remise des impressions.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si l'arrêt échoue.
    fn stop(&self) -> Result<(), PrinterError>;
}
