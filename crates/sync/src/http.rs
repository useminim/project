//! Client HTTP commun : délais, nouvelles tentatives sur les erreurs réseau.

use std::sync::Once;
use std::time::Duration;

use reqwest::{Client, RequestBuilder, Response};

/// Délai maximal d'une requête.
pub(crate) const TIMEOUT: Duration = Duration::from_secs(15);
/// Délai maximal d'établissement de la connexion.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Nombre total de tentatives pour une erreur réseau.
pub(crate) const ATTEMPTS: u32 = 3;
/// Première attente avant une nouvelle tentative, doublée à chaque fois.
const FIRST_BACKOFF: Duration = Duration::from_millis(250);

static CRYPTO: Once = Once::new();

/// Client HTTPS (ou HTTP vers `localhost` en développement).
pub(crate) fn client() -> reqwest::Result<Client> {
    CRYPTO.call_once(|| {
        // Un autre composant a pu installer un fournisseur : sans effet alors.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    Client::builder()
        .timeout(TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .user_agent(concat!("minim/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// Envoie la requête construite par `request`, en réessayant avec une
/// attente exponentielle **seulement** sur une erreur réseau (connexion
/// impossible, délai dépassé) : une réponse HTTP, même en erreur, est rendue
/// telle quelle.
pub(crate) async fn send_with_retry(
    request: impl Fn() -> RequestBuilder,
    backoff: Duration,
) -> reqwest::Result<Response> {
    let mut wait = backoff;
    let mut attempt = 1;
    loop {
        match request().send().await {
            Err(error) if attempt < ATTEMPTS && (error.is_connect() || error.is_timeout()) => {
                tokio::time::sleep(wait).await;
                wait *= 2;
                attempt += 1;
            }
            result => return result,
        }
    }
}

/// Attente par défaut avant la première nouvelle tentative.
pub(crate) const fn default_backoff() -> Duration {
    FIRST_BACKOFF
}
