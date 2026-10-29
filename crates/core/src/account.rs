//! Compte de l'utilisateur dans l'application : session, rafraîchissement
//! automatique, hors ligne. Aucun compte n'est obligatoire (D-37) : sans
//! session, l'application fonctionne en local.

use std::sync::{Arc, Mutex, PoisonError};

use chrono::Duration;
use serde::Serialize;

use crate::ports::{AuthError, AuthProvider, Clock, Session, SessionStore, SignUpOutcome};

/// Marge avant l'expiration du jeton d'accès à partir de laquelle il est
/// rafraîchi.
pub const REFRESH_MARGIN: Duration = Duration::seconds(60);

/// État du compte, transmis à l'interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AccountStatus {
    /// Aucun compte connecté (l'application fonctionne en local).
    SignedOut,
    /// Connecté.
    SignedIn {
        /// Adresse e-mail du compte.
        email: String,
    },
    /// Connecté, mais le backend est injoignable : la session reste ouverte
    /// et sera rafraîchie au retour du réseau.
    Offline {
        /// Adresse e-mail du compte.
        email: String,
    },
    /// Les comptes ne sont pas configurés dans cette version.
    Unavailable,
}

/// Résultat d'une inscription, transmis à l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum SignUpResult {
    /// Compte créé et connecté.
    SignedIn,
    /// Compte créé ; l'utilisateur doit confirmer son adresse e-mail.
    ConfirmationSent,
}

/// Gestion du compte et de la session.
#[derive(Clone)]
pub struct AccountService {
    provider: Option<Arc<dyn AuthProvider>>,
    store: Arc<dyn SessionStore>,
    clock: Arc<dyn Clock>,
    /// Session en mémoire : reste disponible si le trousseau est inaccessible.
    session: Arc<Mutex<Option<Session>>>,
    loaded: Arc<Mutex<bool>>,
}

impl AccountService {
    /// Service au-dessus de `provider` (absent si les comptes ne sont pas
    /// configurés) ; la session est gardée dans `store`.
    pub fn new(
        provider: Option<Arc<dyn AuthProvider>>,
        store: Arc<dyn SessionStore>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            provider,
            store,
            clock,
            session: Arc::new(Mutex::new(None)),
            loaded: Arc::new(Mutex::new(false)),
        }
    }

    fn provider(&self) -> Result<&dyn AuthProvider, AuthError> {
        self.provider.as_deref().ok_or(AuthError::NotConfigured)
    }

    /// Session en mémoire, chargée depuis le stockage au premier appel.
    fn current(&self) -> Option<Session> {
        let mut loaded = self.loaded.lock().unwrap_or_else(PoisonError::into_inner);
        let mut session = self.session.lock().unwrap_or_else(PoisonError::into_inner);
        if !*loaded {
            *loaded = true;
            match self.store.load() {
                Ok(stored) => *session = stored,
                Err(error) => tracing::warn!(%error, "session enregistrée illisible"),
            }
        }
        session.clone()
    }

    fn remember(&self, session: Option<Session>) {
        let result = match &session {
            Some(session) => self.store.save(session),
            None => self.store.clear(),
        };
        if let Err(error) = result {
            // La session reste en mémoire pour ce lancement.
            tracing::warn!(%error, "session non enregistrée dans le trousseau");
        }
        *self.session.lock().unwrap_or_else(PoisonError::into_inner) = session;
    }

    /// État du compte. Rafraîchit la session si son jeton d'accès expire
    /// bientôt ; une session révoquée est effacée.
    pub async fn status(&self) -> AccountStatus {
        if self.provider.is_none() {
            return AccountStatus::Unavailable;
        }
        match self.fresh_session().await {
            Ok(Some(session)) => AccountStatus::SignedIn {
                email: session.user.email,
            },
            Ok(None) => AccountStatus::SignedOut,
            Err((AuthError::Network, Some(session))) => AccountStatus::Offline {
                email: session.user.email,
            },
            Err((error, session)) => {
                tracing::warn!(%error, "rafraîchissement de la session impossible");
                session.map_or(AccountStatus::SignedOut, |session| AccountStatus::Offline {
                    email: session.user.email,
                })
            }
        }
    }

    /// Session valide, rafraîchie si besoin. En cas d'échec réseau, renvoie
    /// l'erreur avec la session gardée.
    async fn fresh_session(&self) -> Result<Option<Session>, (AuthError, Option<Session>)> {
        let Some(session) = self.current() else {
            return Ok(None);
        };
        if session.expires_at - REFRESH_MARGIN > self.clock.now() {
            return Ok(Some(session));
        }
        let provider = self
            .provider()
            .map_err(|error| (error, Some(session.clone())))?;
        match provider.refresh(session.refresh_token.expose()).await {
            Ok(fresh) => {
                self.remember(Some(fresh.clone()));
                Ok(Some(fresh))
            }
            Err(AuthError::SessionExpired | AuthError::InvalidCredentials) => {
                self.remember(None);
                Ok(None)
            }
            Err(error) => Err((error, Some(session))),
        }
    }

    /// Jeton d'accès valide, pour les appels au backend (synchronisation).
    ///
    /// # Erreurs
    ///
    /// [`AuthError::SessionExpired`] sans session ; [`AuthError::Network`] si
    /// le rafraîchissement n'a pas pu se faire.
    pub async fn access_token(&self) -> Result<String, AuthError> {
        match self.fresh_session().await {
            Ok(Some(session)) => Ok(session.access_token.into_inner()),
            Ok(None) => Err(AuthError::SessionExpired),
            Err((error, _)) => Err(error),
        }
    }

    /// Ouvre une session.
    ///
    /// # Erreurs
    ///
    /// Renvoie l'erreur du backend (identifiants incorrects, e-mail non
    /// confirmé, réseau…).
    pub async fn sign_in(&self, email: &str, password: &str) -> Result<AccountStatus, AuthError> {
        let session = self.provider()?.sign_in(email.trim(), password).await?;
        let status = AccountStatus::SignedIn {
            email: session.user.email.clone(),
        };
        self.remember(Some(session));
        Ok(status)
    }

    /// Crée un compte.
    ///
    /// # Erreurs
    ///
    /// Renvoie l'erreur du backend (adresse déjà utilisée, mot de passe trop
    /// faible, réseau…).
    pub async fn sign_up(&self, email: &str, password: &str) -> Result<SignUpResult, AuthError> {
        match self.provider()?.sign_up(email.trim(), password).await? {
            SignUpOutcome::SignedIn(session) => {
                self.remember(Some(session));
                Ok(SignUpResult::SignedIn)
            }
            SignUpOutcome::ConfirmationSent => Ok(SignUpResult::ConfirmationSent),
        }
    }

    /// Ferme la session : côté backend si possible, et toujours sur la
    /// machine.
    pub async fn sign_out(&self) -> AccountStatus {
        if let (Some(session), Ok(provider)) = (self.current(), self.provider())
            && let Err(error) = provider.sign_out(session.access_token.expose()).await
        {
            // La session est effacée localement de toute façon.
            tracing::warn!(%error, "déconnexion côté backend impossible");
        }
        self.remember(None);
        if self.provider.is_some() {
            AccountStatus::SignedOut
        } else {
            AccountStatus::Unavailable
        }
    }

    /// Demande un e-mail de réinitialisation du mot de passe.
    ///
    /// # Erreurs
    ///
    /// Renvoie l'erreur du backend (réseau, trop de tentatives…).
    pub async fn request_password_reset(&self, email: &str) -> Result<(), AuthError> {
        self.provider()?.request_password_reset(email.trim()).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use chrono::{DateTime, Utc};

    use super::*;
    use crate::ports::{AuthUser, BoxFuture, SessionStoreError};
    use crate::redacted::Redacted;
    use crate::testing::{FixedClock, samples};

    /// Backend simulé : un seul compte, des jetons numérotés.
    #[derive(Default)]
    struct FakeAuth {
        refreshes: AtomicUsize,
        offline: Mutex<bool>,
        revoked: Mutex<bool>,
        signed_out: AtomicUsize,
        now: Mutex<Option<DateTime<Utc>>>,
    }

    impl FakeAuth {
        fn session(&self, number: usize) -> Session {
            let now = self.now.lock().unwrap().unwrap_or_else(|| samples::date(0));
            Session {
                user: AuthUser {
                    id: "user-1".to_owned(),
                    email: "alice@example.com".to_owned(),
                },
                access_token: Redacted::new(format!("access-{number}")),
                refresh_token: Redacted::new(format!("refresh-{number}")),
                expires_at: now + Duration::hours(1),
            }
        }
    }

    impl AuthProvider for FakeAuth {
        fn sign_up<'a>(
            &'a self,
            email: &'a str,
            _: &'a str,
        ) -> BoxFuture<'a, Result<SignUpOutcome, AuthError>> {
            Box::pin(async move {
                match email {
                    "taken@example.com" => Err(AuthError::EmailTaken),
                    "confirm@example.com" => Ok(SignUpOutcome::ConfirmationSent),
                    _ => Ok(SignUpOutcome::SignedIn(self.session(0))),
                }
            })
        }

        fn sign_in<'a>(
            &'a self,
            email: &'a str,
            password: &'a str,
        ) -> BoxFuture<'a, Result<Session, AuthError>> {
            Box::pin(async move {
                if *self.offline.lock().unwrap() {
                    return Err(AuthError::Network);
                }
                if email == "alice@example.com" && password == "correct horse" {
                    Ok(self.session(0))
                } else {
                    Err(AuthError::InvalidCredentials)
                }
            })
        }

        fn refresh<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<Session, AuthError>> {
            Box::pin(async move {
                if *self.offline.lock().unwrap() {
                    return Err(AuthError::Network);
                }
                if *self.revoked.lock().unwrap() {
                    return Err(AuthError::SessionExpired);
                }
                let number = self.refreshes.fetch_add(1, Ordering::SeqCst) + 1;
                Ok(self.session(number))
            })
        }

        fn sign_out<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<(), AuthError>> {
            Box::pin(async move {
                self.signed_out.fetch_add(1, Ordering::SeqCst);
                if *self.offline.lock().unwrap() {
                    Err(AuthError::Network)
                } else {
                    Ok(())
                }
            })
        }

        fn request_password_reset<'a>(
            &'a self,
            _: &'a str,
        ) -> BoxFuture<'a, Result<(), AuthError>> {
            Box::pin(async { Ok(()) })
        }
    }

    /// Trousseau simulé, éventuellement en panne.
    #[derive(Default)]
    struct MemoryStore {
        session: Mutex<Option<Session>>,
        broken: bool,
    }

    impl SessionStore for MemoryStore {
        fn load(&self) -> Result<Option<Session>, SessionStoreError> {
            if self.broken {
                return Err(SessionStoreError("trousseau".into()));
            }
            Ok(self.session.lock().unwrap().clone())
        }
        fn save(&self, session: &Session) -> Result<(), SessionStoreError> {
            if self.broken {
                return Err(SessionStoreError("trousseau".into()));
            }
            *self.session.lock().unwrap() = Some(session.clone());
            Ok(())
        }
        fn clear(&self) -> Result<(), SessionStoreError> {
            *self.session.lock().unwrap() = None;
            Ok(())
        }
    }

    struct Fixture {
        service: AccountService,
        auth: Arc<FakeAuth>,
        store: Arc<MemoryStore>,
        clock: Arc<FixedClock>,
    }

    fn fixture(store: MemoryStore) -> Fixture {
        let auth = Arc::new(FakeAuth::default());
        let store = Arc::new(store);
        let clock = Arc::new(FixedClock::new(samples::date(0)));
        let service = AccountService::new(Some(auth.clone()), store.clone(), clock.clone());
        Fixture {
            service,
            auth,
            store,
            clock,
        }
    }

    /// Exécuteur minimal pour les tests : les futures simulées ne
    /// s'interrompent jamais.
    fn run<T>(future: impl std::future::Future<Output = T>) -> T {
        use std::task::{Context, Poll, Waker};
        let mut future = std::pin::pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("future simulée en attente"),
        }
    }

    fn signed_in() -> AccountStatus {
        AccountStatus::SignedIn {
            email: "alice@example.com".to_owned(),
        }
    }

    #[test]
    fn signs_in_keeps_the_session_and_signs_out() {
        let Fixture {
            service,
            auth,
            store,
            ..
        } = fixture(MemoryStore::default());

        assert_eq!(run(service.status()), AccountStatus::SignedOut);
        assert!(matches!(
            run(service.sign_in("alice@example.com", "faux")),
            Err(AuthError::InvalidCredentials)
        ));
        assert_eq!(
            run(service.sign_in(" alice@example.com ", "correct horse")).unwrap(),
            signed_in()
        );
        assert_eq!(run(service.status()), signed_in());
        assert!(store.session.lock().unwrap().is_some());

        assert_eq!(run(service.sign_out()), AccountStatus::SignedOut);
        assert_eq!(auth.signed_out.load(Ordering::SeqCst), 1);
        assert!(store.session.lock().unwrap().is_none());
        assert_eq!(run(service.status()), AccountStatus::SignedOut);
    }

    #[test]
    fn restores_the_session_and_refreshes_it_before_it_expires() {
        let Fixture {
            service,
            auth,
            store,
            clock,
        } = fixture(MemoryStore::default());
        run(service.sign_in("alice@example.com", "correct horse")).unwrap();
        let restarted = AccountService::new(Some(auth.clone()), store.clone(), clock.clone());

        assert_eq!(run(restarted.status()), signed_in());
        assert_eq!(auth.refreshes.load(Ordering::SeqCst), 0);

        clock.advance(Duration::minutes(59) + Duration::seconds(30));
        *auth.now.lock().unwrap() = Some(clock.now());
        assert_eq!(run(restarted.access_token()).unwrap(), "access-1");
        assert_eq!(auth.refreshes.load(Ordering::SeqCst), 1);
        assert_eq!(
            store
                .session
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .refresh_token
                .expose(),
            "refresh-1"
        );
    }

    #[test]
    fn stays_signed_in_offline_and_forgets_a_revoked_session() {
        let Fixture {
            service,
            auth,
            store,
            clock,
        } = fixture(MemoryStore::default());
        run(service.sign_in("alice@example.com", "correct horse")).unwrap();
        clock.advance(Duration::hours(2));

        *auth.offline.lock().unwrap() = true;
        assert_eq!(
            run(service.status()),
            AccountStatus::Offline {
                email: "alice@example.com".to_owned()
            }
        );
        assert!(matches!(
            run(service.access_token()),
            Err(AuthError::Network)
        ));
        assert!(store.session.lock().unwrap().is_some(), "la session reste");

        *auth.offline.lock().unwrap() = false;
        *auth.revoked.lock().unwrap() = true;
        assert_eq!(run(service.status()), AccountStatus::SignedOut);
        assert!(store.session.lock().unwrap().is_none());
    }

    #[test]
    fn signs_out_locally_even_when_offline() {
        let Fixture {
            service,
            auth,
            store,
            ..
        } = fixture(MemoryStore::default());
        run(service.sign_in("alice@example.com", "correct horse")).unwrap();
        *auth.offline.lock().unwrap() = true;

        assert_eq!(run(service.sign_out()), AccountStatus::SignedOut);
        assert!(store.session.lock().unwrap().is_none());
    }

    #[test]
    fn signs_up_with_or_without_email_confirmation() {
        let Fixture { service, .. } = fixture(MemoryStore::default());

        assert_eq!(
            run(service.sign_up("confirm@example.com", "pw")).unwrap(),
            SignUpResult::ConfirmationSent
        );
        assert_eq!(run(service.status()), AccountStatus::SignedOut);
        assert!(matches!(
            run(service.sign_up("taken@example.com", "pw")),
            Err(AuthError::EmailTaken)
        ));
        assert_eq!(
            run(service.sign_up("bob@example.com", "pw")).unwrap(),
            SignUpResult::SignedIn
        );
        assert_eq!(run(service.status()), signed_in());
    }

    #[test]
    fn keeps_the_session_in_memory_without_a_keychain() {
        let Fixture { service, .. } = fixture(MemoryStore {
            broken: true,
            ..MemoryStore::default()
        });

        assert_eq!(run(service.status()), AccountStatus::SignedOut);
        assert_eq!(
            run(service.sign_in("alice@example.com", "correct horse")).unwrap(),
            signed_in()
        );
        assert_eq!(run(service.status()), signed_in());
    }

    #[test]
    fn reports_unconfigured_accounts() {
        let service = AccountService::new(
            None,
            Arc::new(MemoryStore::default()),
            Arc::new(FixedClock::new(samples::date(0))),
        );

        assert_eq!(run(service.status()), AccountStatus::Unavailable);
        assert!(matches!(
            run(service.sign_in("a@b.c", "x")),
            Err(AuthError::NotConfigured)
        ));
        assert_eq!(run(service.sign_out()), AccountStatus::Unavailable);
    }

    #[test]
    fn never_prints_tokens() {
        let auth = FakeAuth::default();
        let output = format!("{:?}", auth.session(7));
        assert!(
            !output.contains("access-7") && !output.contains("refresh-7"),
            "{output}"
        );
    }
}
