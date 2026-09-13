//! End-to-end tests for the authentication slice, against a real Postgres.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn signing_up_returns_the_session_the_client_expects() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let email = unique_email("inscription");

    let (status, body) = call(&app, post("/api/v1/auth/sign-up", sign_up_body(&email))).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["user"]["email"], email);
    assert_eq!(body["user"]["profile_completed"], false);
    assert!(body["tokens"]["access_token"].as_str().is_some());
    assert!(body["tokens"]["refresh_token"].as_str().is_some());
    assert!(body["tokens"]["expires_at"].as_str().is_some());
    // camelCase anywhere here breaks every decode on the client.
    assert!(body["user"].get("profileCompleted").is_none());
}

#[tokio::test]
async fn the_same_address_cannot_sign_up_twice() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let email = unique_email("doublon");

    let (first, _) = call(&app, post("/api/v1/auth/sign-up", sign_up_body(&email))).await;
    assert_eq!(first, StatusCode::OK);

    let (second, body) = call(&app, post("/api/v1/auth/sign-up", sign_up_body(&email))).await;

    assert_eq!(second, StatusCode::CONFLICT);
    assert_eq!(body["code"], "email_taken");
    assert!(body["message"].as_str().unwrap().contains("déjà prise"));
}

/// The client checks the age too, but a client check is a courtesy.
#[tokio::test]
async fn a_minor_is_refused_even_if_the_client_let_them_through() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));

    let mut body = sign_up_body(&unique_email("mineur"));
    let recent = chrono::Utc::now() - chrono::Duration::days(365 * 15);
    body["birth_date"] = json!(recent.to_rfc3339());

    let (status, body) = call(&app, post("/api/v1/auth/sign-up", body)).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "too_young");
}

#[tokio::test]
async fn signing_in_works_and_a_wrong_password_does_not() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let email = unique_email("connexion");
    let _ = call(&app, post("/api/v1/auth/sign-up", sign_up_body(&email))).await;

    let (ok, _) = call(
        &app,
        post(
            "/api/v1/auth/sign-in",
            json!({ "email": &email, "password": "motdepasse" }),
        ),
    )
    .await;
    assert_eq!(ok, StatusCode::OK);

    let (refused, body) = call(
        &app,
        post(
            "/api/v1/auth/sign-in",
            json!({ "email": &email, "password": "pas-le-bon" }),
        ),
    )
    .await;
    assert_eq!(refused, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "invalid_credentials");
}

/// An unknown address and a wrong password must be indistinguishable, or the
/// endpoint becomes a way to ask which addresses have accounts.
#[tokio::test]
async fn an_unknown_address_answers_exactly_like_a_wrong_password() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let email = unique_email("enumeration");
    let _ = call(&app, post("/api/v1/auth/sign-up", sign_up_body(&email))).await;

    let (wrong_password, first) = call(
        &app,
        post(
            "/api/v1/auth/sign-in",
            json!({ "email": &email, "password": "pas-le-bon" }),
        ),
    )
    .await;
    let (unknown, second) = call(
        &app,
        post(
            "/api/v1/auth/sign-in",
            json!({ "email": unique_email("inconnu"), "password": "motdepasse" }),
        ),
    )
    .await;

    assert_eq!(wrong_password, unknown);
    assert_eq!(first, second);
}

#[tokio::test]
async fn the_access_token_opens_me_and_nothing_else_does() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let email = unique_email("moi");
    let (_, session) = call(&app, post("/api/v1/auth/sign-up", sign_up_body(&email))).await;
    let access = session["tokens"]["access_token"].as_str().unwrap();

    let authorised = Request::builder()
        .uri("/api/v1/me")
        .header("authorization", format!("Bearer {access}"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = call(&app, authorised).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["email"], email);

    for header in ["", "Bearer", "Bearer pas-un-jeton", "Basic abc"] {
        let mut builder = Request::builder().uri("/api/v1/me");
        if !header.is_empty() {
            builder = builder.header("authorization", header);
        }
        let (status, _) = call(&app, builder.body(Body::empty()).unwrap()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "en-tête : {header:?}");
    }
}

/// A refresh token that survives its own use is a refresh token someone can
/// replay.
#[tokio::test]
async fn a_refresh_token_is_spent_when_used() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let email = unique_email("rotation");
    let (_, session) = call(&app, post("/api/v1/auth/sign-up", sign_up_body(&email))).await;
    let refresh = session["tokens"]["refresh_token"]
        .as_str()
        .unwrap()
        .to_string();

    let (first, body) = call(
        &app,
        post("/api/v1/auth/refresh", json!({ "refresh_token": &refresh })),
    )
    .await;
    assert_eq!(first, StatusCode::OK, "{body}");
    assert!(body["access_token"].as_str().is_some());
    assert_ne!(body["refresh_token"].as_str().unwrap(), refresh);

    let (replay, _) = call(
        &app,
        post("/api/v1/auth/refresh", json!({ "refresh_token": &refresh })),
    )
    .await;
    assert_eq!(
        replay,
        StatusCode::UNAUTHORIZED,
        "un jeton déjà utilisé est mort"
    );
}

#[tokio::test]
async fn signing_out_revokes_every_session_of_the_account() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let email = unique_email("deconnexion");
    let (_, first) = call(&app, post("/api/v1/auth/sign-up", sign_up_body(&email))).await;
    let (_, second) = call(
        &app,
        post(
            "/api/v1/auth/sign-in",
            json!({ "email": &email, "password": "motdepasse" }),
        ),
    )
    .await;

    let access = first["tokens"]["access_token"].as_str().unwrap();
    let sign_out = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/sign-out")
        .header("authorization", format!("Bearer {access}"))
        .body(Body::empty())
        .unwrap();
    let (status, _) = call(&app, sign_out).await;
    assert_eq!(status, StatusCode::OK);

    // Both devices' refresh tokens are dead, not just the one that asked.
    for session in [&first, &second] {
        let token = session["tokens"]["refresh_token"].as_str().unwrap();
        let (status, _) = call(
            &app,
            post("/api/v1/auth/refresh", json!({ "refresh_token": token })),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn health_answers_without_a_database() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));

    let (status, body) = call(
        &app,
        Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn the_database_is_reachable_at_all() {
    let Some(db) = database().await else { return };
    db.ping().await.expect("la base doit répondre");
}

/// The client already understands a 429 with `Retry-After` — it decodes it as
/// `APIError.rateLimited` and its retry policy waits exactly that long. The
/// server had no way of producing one.
#[tokio::test]
async fn repeated_sign_in_attempts_are_throttled() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let email = unique_email("bruteforce");
    let _ = call(&app, post("/api/v1/auth/sign-up", sign_up_body(&email))).await;

    let attempt = || {
        post(
            "/api/v1/auth/sign-in",
            json!({ "email": &email, "password": "pas-le-bon" }),
        )
    };

    // The quota is ten per quarter hour, and the sign-up already spent none of
    // this bucket: the first ten are refused on the password, the eleventh on
    // the quota.
    for index in 0..10 {
        let (status, _) = call(&app, attempt()).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "tentative {index} devrait être refusée sur le mot de passe"
        );
    }

    let response = app.clone().oneshot(attempt()).await.unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    let retry_after = response
        .headers()
        .get("retry-after")
        .expect("le client lit cet en-tête pour savoir combien attendre")
        .to_str()
        .unwrap()
        .parse::<u64>()
        .expect("des secondes, pas une date");
    assert!(retry_after > 0);
}

/// Throttling one account must not lock out everybody else.
#[tokio::test]
async fn throttling_is_per_account() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let targeted = unique_email("cible");
    let bystander = unique_email("passant");
    let _ = call(&app, post("/api/v1/auth/sign-up", sign_up_body(&targeted))).await;
    let _ = call(&app, post("/api/v1/auth/sign-up", sign_up_body(&bystander))).await;

    for _ in 0..12 {
        let _ = call(
            &app,
            post(
                "/api/v1/auth/sign-in",
                json!({ "email": &targeted, "password": "pas-le-bon" }),
            ),
        )
        .await;
    }

    let (status, _) = call(
        &app,
        post(
            "/api/v1/auth/sign-in",
            json!({ "email": &bystander, "password": "motdepasse" }),
        ),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "le voisin doit pouvoir se connecter"
    );
}

/// One dyno serves the site and the API. The point of the arrangement is that
/// neither shadows the other: an unknown path must reach the single-page site
/// rather than 404, and `/health` must keep answering JSON rather than being
/// swallowed by the static handler.
#[tokio::test]
async fn the_site_and_the_api_share_one_host_without_shadowing_each_other() {
    let Some(db) = database().await else { return };

    let directory = std::env::temp_dir().join(format!("plum-site-{}", uuid_like()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("index.html"),
        "<!doctype html><title>Plum</title>",
    )
    .unwrap();

    let app = plum_server::app_with_site(state(db), Some(&directory));

    let (root, _) = call(
        &app,
        Request::builder().uri("/").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(root, StatusCode::OK, "la racine doit servir le site");

    // A route of the single-page site, typed straight into the address bar.
    // The status matters as much as the body: `not_found_service` would serve
    // this exact page under a 404, which search engines take at their word.
    let (deep, deep_body) = call_raw(
        &app,
        Request::builder()
            .uri("/confidentialite")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(deep, StatusCode::OK, "une route du site ne doit pas 404");
    assert!(
        deep_body.contains("<title>Plum</title>"),
        "la route profonde doit servir index.html, reçu : {deep_body}"
    );

    let (health, body) = call(
        &app,
        Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(health, StatusCode::OK);
    assert_eq!(body["status"], "ok", "l'API ne doit pas être masquée");

    // And the API's own 401 must survive too, rather than becoming the site.
    let (unauthorised, _) = call(
        &app,
        Request::builder()
            .uri("/api/v1/me")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(unauthorised, StatusCode::UNAUTHORIZED);

    // A typo under /api must read as a typo. Falling through to the site would
    // hand the client 200 and a page of HTML, and the iOS app would report a
    // decoding failure instead of a missing route.
    let (missing, missing_body) = call_raw(
        &app,
        Request::builder()
            .uri("/api/v1/nexiste-pas")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(missing, StatusCode::NOT_FOUND);
    assert!(
        !missing_body.contains("<title>Plum</title>"),
        "une route d'API inconnue ne doit pas servir le site : {missing_body}"
    );

    std::fs::remove_dir_all(&directory).ok();
}
