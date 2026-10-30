//! Tests contre un vrai Supabase (local : `pnpm supabase start`), lancés en CI
//! par le job « Backend (Supabase) ». Sans les variables
//! `MINIM_SUPABASE_TEST_URL`, `MINIM_SUPABASE_TEST_ANON_KEY` et
//! `MINIM_SUPABASE_TEST_SERVICE_KEY`, ils ne font rien.
//!
//! Ils jouent aussi l'attaquant : avec la clé publique seule, puis avec le
//! jeton d'un autre utilisateur, ils tentent de lire et de modifier les
//! données et vérifient le refus (D-37). La clé « service » ne sert qu'à
//! créer des comptes déjà confirmés pour le test ; elle n'est jamais dans
//! l'application.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "fichier de tests : clippy.toml n'autorise unwrap que dans les fonctions #[test]"
)]

use minim_core::ports::{AuthError, AuthProvider as _, SignUpOutcome};
use minim_sync::{SupabaseAuth, SupabaseConfig};
use reqwest::StatusCode;
use serde_json::{Value, json};

struct Live {
    url: String,
    anon: String,
    service: String,
    http: reqwest::Client,
}

fn live() -> Option<Live> {
    let var = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
    let Some(url) = var("MINIM_SUPABASE_TEST_URL") else {
        eprintln!("MINIM_SUPABASE_TEST_URL absent : tests contre Supabase ignorés");
        return None;
    };
    Some(Live {
        url,
        anon: var("MINIM_SUPABASE_TEST_ANON_KEY").expect("MINIM_SUPABASE_TEST_ANON_KEY"),
        service: var("MINIM_SUPABASE_TEST_SERVICE_KEY").expect("MINIM_SUPABASE_TEST_SERVICE_KEY"),
        http: reqwest::Client::new(),
    })
}

/// Adresse unique pour chaque test.
fn email(name: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{name}-{nanos}@example.com")
}

const PASSWORD: &str = "motdepasse2026";

impl Live {
    fn auth(&self) -> SupabaseAuth {
        SupabaseAuth::new(SupabaseConfig {
            url: self.url.clone(),
            anon_key: self.anon.clone(),
        })
        .unwrap()
    }

    /// Compte déjà confirmé, créé avec la clé service (préparation du test).
    async fn confirmed_user(&self, email: &str, health_profession: bool) -> String {
        let response = self
            .http
            .post(format!("{}/auth/v1/admin/users", self.url))
            .header("apikey", &self.service)
            .bearer_auth(&self.service)
            .json(&json!({
                "email": email,
                "password": PASSWORD,
                "email_confirm": true,
                "user_metadata": { "health_profession": health_profession }
            }))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success(), "{}", response.status());
        response.json::<Value>().await.unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    /// Requête REST sur les profils, avec la clé publique et, si donné, un
    /// jeton d'utilisateur.
    async fn profiles(
        &self,
        method: reqwest::Method,
        query: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = self
            .http
            .request(method, format!("{}/rest/v1/profiles?{query}", self.url))
            .header("apikey", &self.anon)
            .header("Prefer", "return=representation");
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.unwrap();
        let status = response.status();
        (status, response.json().await.unwrap_or(Value::Null))
    }
}

#[tokio::test]
async fn public_sign_up_requires_email_confirmation() {
    let Some(live) = live() else { return };
    let auth = live.auth();
    let address = email("inscription");

    let outcome = auth.sign_up(&address, PASSWORD, false).await.unwrap();

    assert_eq!(outcome, SignUpOutcome::ConfirmationSent);
    assert!(matches!(
        auth.sign_in(&address, PASSWORD).await,
        Err(AuthError::EmailNotConfirmed)
    ));
    assert!(matches!(
        auth.sign_up(&email("faible"), "court", false).await,
        Err(AuthError::WeakPassword)
    ));
}

#[tokio::test]
async fn signs_in_refreshes_and_reads_only_its_own_profile() {
    let Some(live) = live() else { return };
    let auth = live.auth();
    let alice = email("alice");
    let bob = email("bob");
    let alice_id = live.confirmed_user(&alice, true).await;
    live.confirmed_user(&bob, false).await;

    assert!(matches!(
        auth.sign_in(&alice, "mauvais1").await,
        Err(AuthError::InvalidCredentials)
    ));
    let alice_session = auth.sign_in(&alice, PASSWORD).await.unwrap();
    let refreshed = auth
        .refresh(alice_session.refresh_token.expose())
        .await
        .unwrap();
    assert!(matches!(
        auth.refresh(alice_session.refresh_token.expose()).await,
        Err(AuthError::SessionExpired) | Ok(_)
    ));
    let bob_session = auth.sign_in(&bob, PASSWORD).await.unwrap();

    // Alice lit son profil, créé à l'inscription.
    let (status, rows) = live
        .profiles(
            reqwest::Method::GET,
            "select=*",
            Some(refreshed.access_token.expose()),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["health_profession"], true);

    // Attaquant anonyme : aucune lecture.
    let (status, _) = live
        .profiles(reqwest::Method::GET, "select=*", None, None)
        .await;
    assert!(
        status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN,
        "anon : {status}"
    );

    // Bob ne voit pas Alice et ne peut pas modifier son profil.
    let token = bob_session.access_token.expose();
    let (_, rows) = live
        .profiles(
            reqwest::Method::GET,
            &format!("id=eq.{alice_id}"),
            Some(token),
            None,
        )
        .await;
    assert_eq!(rows, json!([]), "Bob lit le profil d'Alice");
    let (_, rows) = live
        .profiles(
            reqwest::Method::PATCH,
            &format!("id=eq.{alice_id}"),
            Some(token),
            Some(json!({ "health_profession": false })),
        )
        .await;
    assert_eq!(rows, json!([]), "Bob modifie le profil d'Alice");
    let (status, _) = live
        .profiles(
            reqwest::Method::POST,
            "",
            Some(token),
            Some(json!({ "id": alice_id, "health_profession": false })),
        )
        .await;
    assert!(!status.is_success(), "Bob insère un profil : {status}");
}

#[tokio::test]
async fn deletes_the_account_and_its_profile() {
    let Some(live) = live() else { return };
    let auth = live.auth();
    let address = email("suppression");
    live.confirmed_user(&address, false).await;
    let session = auth.sign_in(&address, PASSWORD).await.unwrap();

    auth.delete_account(session.access_token.expose())
        .await
        .unwrap();

    assert!(matches!(
        auth.sign_in(&address, PASSWORD).await,
        Err(AuthError::InvalidCredentials)
    ));
    // Anonyme : la fonction de suppression est refusée.
    let response = live
        .http
        .post(format!("{}/rest/v1/rpc/delete_my_account", live.url))
        .header("apikey", &live.anon)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert!(
        !response.status().is_success(),
        "anon supprime : {}",
        response.status()
    );
}
