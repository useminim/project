//! Comptes : authentification auprès du backend (D-37) et conservation de la
//! session sur la machine.
//!
//! Le port est asynchrone (voir [`BoxFuture`], D-27). Les jetons sont
//! enveloppés dans [`Redacted`] : ils n'apparaissent jamais dans un `Debug`
//! ni dans un log.

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::ports::sync::BoxFuture;
use crate::redacted::Redacted;

/// Utilisateur connecté.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthUser {
    /// Identifiant attribué par le backend.
    pub id: String,
    /// Adresse e-mail du compte.
    pub email: String,
}

/// Session ouverte auprès du backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// Utilisateur.
    pub user: AuthUser,
    /// Jeton d'accès, de courte durée.
    pub access_token: Redacted<String>,
    /// Jeton de rafraîchissement, à usage unique.
    pub refresh_token: Redacted<String>,
    /// Expiration du jeton d'accès.
    pub expires_at: DateTime<Utc>,
}

/// Résultat d'une inscription.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignUpOutcome {
    /// Le compte est créé et la session ouverte.
    SignedIn(Session),
    /// Le compte est créé ; un e-mail de confirmation a été envoyé.
    ConfirmationSent,
}

/// Échec d'une opération de compte. Ne contient ni e-mail ni jeton.
#[derive(Debug, Error)]
pub enum AuthError {
    /// E-mail ou mot de passe incorrect.
    #[error("identifiants incorrects")]
    InvalidCredentials,
    /// L'adresse e-mail n'a pas encore été confirmée.
    #[error("adresse e-mail non confirmée")]
    EmailNotConfirmed,
    /// Un compte existe déjà avec cette adresse.
    #[error("adresse e-mail déjà utilisée")]
    EmailTaken,
    /// Le mot de passe ne respecte pas les règles du backend.
    #[error("mot de passe trop faible")]
    WeakPassword,
    /// Trop de tentatives : il faut patienter.
    #[error("trop de tentatives")]
    RateLimited,
    /// La session a expiré ou a été révoquée : il faut se reconnecter.
    #[error("session expirée")]
    SessionExpired,
    /// Le backend est injoignable (pas de réseau, délai dépassé).
    #[error("backend injoignable")]
    Network,
    /// Réponse inattendue du backend.
    #[error("erreur du backend ({status})")]
    Server {
        /// Code HTTP reçu.
        status: u16,
    },
    /// Les comptes ne sont pas configurés dans cette version de l'application.
    #[error("comptes non configurés")]
    NotConfigured,
}

/// Authentification auprès du backend.
pub trait AuthProvider: Send + Sync {
    /// Crée un compte.
    fn sign_up<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
    ) -> BoxFuture<'a, Result<SignUpOutcome, AuthError>>;

    /// Ouvre une session.
    fn sign_in<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
    ) -> BoxFuture<'a, Result<Session, AuthError>>;

    /// Obtient de nouveaux jetons à partir du jeton de rafraîchissement.
    fn refresh<'a>(&'a self, refresh_token: &'a str) -> BoxFuture<'a, Result<Session, AuthError>>;

    /// Ferme la session côté backend.
    fn sign_out<'a>(&'a self, access_token: &'a str) -> BoxFuture<'a, Result<(), AuthError>>;

    /// Envoie un e-mail de réinitialisation du mot de passe (sans révéler si
    /// le compte existe).
    fn request_password_reset<'a>(&'a self, email: &'a str)
    -> BoxFuture<'a, Result<(), AuthError>>;
}

/// Échec du stockage de la session.
#[derive(Debug, Error)]
#[error("stockage de la session indisponible")]
pub struct SessionStoreError(#[source] pub Box<dyn std::error::Error + Send + Sync>);

/// Conservation de la session sur la machine (trousseau du système).
pub trait SessionStore: Send + Sync {
    /// Session enregistrée, s'il y en a une.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage est inaccessible.
    fn load(&self) -> Result<Option<Session>, SessionStoreError>;

    /// Enregistre la session.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage est inaccessible.
    fn save(&self, session: &Session) -> Result<(), SessionStoreError>;

    /// Efface la session.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage est inaccessible.
    fn clear(&self) -> Result<(), SessionStoreError>;
}
