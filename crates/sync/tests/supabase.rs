//! Client Supabase Auth contre un faux serveur (`wiremock`).

#![allow(
    clippy::unwrap_used,
    reason = "fichier de tests : clippy.toml n'autorise unwrap que dans les fonctions #[test]"
)]

use std::time::Duration;

use minim_core::ports::{AuthError, AuthProvider as _, SignUpOutcome};
use minim_sync::{SupabaseAuth, SupabaseConfig};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ANON: &str = "anon-key";

fn auth(server: &MockServer) -> SupabaseAuth {
    SupabaseAuth::new(SupabaseConfig {
        url: format!("{}/", server.uri()),
        anon_key: ANON.to_owned(),
    })
    .unwrap()
    .with_backoff(Duration::from_millis(1))
}

fn session_body(n: u32) -> serde_json::Value {
    json!({
        "access_token": format!("access-{n}"),
        "token_type": "bearer",
        "expires_in": 3600,
        "expires_at": 1_900_000_000,
        "refresh_token": format!("refresh-{n}"),
        "user": { "id": "8d0f0b5e-0000-0000-0000-000000000001", "email": "alice@example.com" }
    })
}

fn error(status: u16, code: &str) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(
        json!({ "code": status, "error_code": code, "msg": "alice@example.com : détail" }),
    )
}

#[tokio::test]
async fn signs_in_with_the_project_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "password"))
        .and(header("apikey", ANON))
        .and(body_json(
            json!({ "email": "alice@example.com", "password": "correct horse" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(session_body(1)))
        .expect(1)
        .mount(&server)
        .await;

    let session = auth(&server)
        .sign_in("alice@example.com", "correct horse")
        .await
        .unwrap();

    assert_eq!(session.user.email, "alice@example.com");
    assert_eq!(session.access_token.expose(), "access-1");
    let remaining = (session.expires_at - chrono::Utc::now()).num_seconds();
    assert!((3590..=3600).contains(&remaining), "{remaining}");
}

#[tokio::test]
async fn maps_errors_without_keeping_the_server_message() {
    let cases = [
        (error(400, "invalid_credentials"), "invalid_credentials"),
        (error(400, "email_not_confirmed"), "email_not_confirmed"),
        (error(429, "over_request_rate_limit"), "rate_limited"),
        (ResponseTemplate::new(500), "server"),
    ];
    for (response, expected) in cases {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(response)
            .expect(1)
            .mount(&server)
            .await;

        let error = auth(&server)
            .sign_in("alice@example.com", "x")
            .await
            .unwrap_err();

        let found = match error {
            AuthError::InvalidCredentials => "invalid_credentials",
            AuthError::EmailNotConfirmed => "email_not_confirmed",
            AuthError::RateLimited => "rate_limited",
            AuthError::Server { status: 500 } => "server",
            other => panic!("erreur inattendue : {other:?}"),
        };
        assert_eq!(found, expected);
        assert!(!format!("{error:?}").contains("alice"));
    }
}

#[tokio::test]
async fn signs_up_with_or_without_email_confirmation() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/v1/signup"))
        .and(body_json(json!({ "email": "confirm@example.com", "password": "pw" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "u2", "email": "confirm@example.com", "confirmation_sent_at": "2026-10-29T10:00:00Z" })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/auth/v1/signup"))
        .and(body_json(
            json!({ "email": "alice@example.com", "password": "pw" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(session_body(1)))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/auth/v1/signup"))
        .and(body_json(
            json!({ "email": "taken@example.com", "password": "pw" }),
        ))
        .respond_with(error(422, "user_already_exists"))
        .mount(&server)
        .await;
    let auth = auth(&server);

    assert_eq!(
        auth.sign_up("confirm@example.com", "pw").await.unwrap(),
        SignUpOutcome::ConfirmationSent
    );
    assert!(matches!(
        auth.sign_up("alice@example.com", "pw").await.unwrap(),
        SignUpOutcome::SignedIn(_)
    ));
    assert!(matches!(
        auth.sign_up("taken@example.com", "pw").await,
        Err(AuthError::EmailTaken)
    ));
}

#[tokio::test]
async fn refreshes_and_reports_an_expired_session() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .and(body_json(json!({ "refresh_token": "refresh-1" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(session_body(2)))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(body_json(json!({ "refresh_token": "refresh-0" })))
        .respond_with(error(400, "refresh_token_already_used"))
        .mount(&server)
        .await;
    let auth = auth(&server);

    assert_eq!(
        auth.refresh("refresh-1")
            .await
            .unwrap()
            .refresh_token
            .expose(),
        "refresh-2"
    );
    assert!(matches!(
        auth.refresh("refresh-0").await,
        Err(AuthError::SessionExpired)
    ));
}

#[tokio::test]
async fn signs_out_and_requests_a_password_reset() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/v1/logout"))
        .and(header("authorization", "Bearer access-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/auth/v1/recover"))
        .and(body_json(json!({ "email": "alice@example.com" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(1)
        .mount(&server)
        .await;
    let auth = auth(&server);

    auth.sign_out("access-1").await.unwrap();
    auth.request_password_reset("alice@example.com")
        .await
        .unwrap();
}

#[tokio::test]
async fn retries_network_errors_only() {
    // Aucun serveur sur ce port : connexion refusée, trois tentatives.
    let unreachable = SupabaseAuth::new(SupabaseConfig {
        url: "http://127.0.0.1:9".to_owned(),
        anon_key: ANON.to_owned(),
    })
    .unwrap()
    .with_backoff(Duration::from_millis(1));
    assert!(matches!(
        unreachable.sign_in("a@b.c", "x").await,
        Err(AuthError::Network)
    ));

    // Une erreur HTTP n'est pas réessayée.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;
    assert!(matches!(
        auth(&server).sign_in("a@b.c", "x").await,
        Err(AuthError::Server { status: 503 })
    ));
}
