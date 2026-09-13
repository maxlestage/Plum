//! End-to-end tests for the authentication slice, against a real Postgres.
//!
//! They need `TEST_DATABASE_URL`. Without it they skip rather than fail: a
//! contributor running `cargo test` on a laptop should not have to stand up a
//! database to check the pure logic, and CI provides one.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use plum_server::config::Config;
use plum_server::state::AppState;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use serde_json::{json, Value};
use tokio::sync::OnceCell;
use tower::ServiceExt;

/// The connection and the migrations, once for the whole binary.
///
/// Cargo runs these tests in parallel threads of one process. Each one calling
/// `Migrator::up` meant several racing to create `seaql_migrations`, and
/// Postgres rejecting all but the first — a failure that only ever appears
/// under real concurrency, which is to say only in CI.
static SHARED: OnceCell<Option<DatabaseConnection>> = OnceCell::const_new();

/// Routes the server's own logs into the test output. Without this an
/// internal failure reaches the assertion as a bare 500 with a polite message
/// and nothing to act on.
fn capture_server_logs() {
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .with_env_filter("plum_server=debug,sqlx=warn")
        .try_init();
}

async fn database() -> Option<DatabaseConnection> {
    capture_server_logs();
    SHARED
        .get_or_init(|| async {
            let url = match std::env::var("TEST_DATABASE_URL") {
                Ok(url) => url,
                Err(_) => {
                    // Ten green tests that executed nothing is worse than one
                    // red one. CI sets REQUIRE_TEST_DATABASE so a Postgres
                    // service that failed to start cannot pass for a clean run.
                    assert!(
                        std::env::var("REQUIRE_TEST_DATABASE").is_err(),
                        "REQUIRE_TEST_DATABASE est posé mais TEST_DATABASE_URL manque : \
                         les tests d'intégration auraient été sautés en silence"
                    );
                    eprintln!("· sauté : TEST_DATABASE_URL non défini");
                    return None;
                }
            };

            let mut options =
                ConnectOptions::new(plum_server::config::normalise_database_url(&url));
            options
                .max_connections(10)
                .acquire_timeout(std::time::Duration::from_secs(10))
                .sqlx_logging(false);

            let db = Database::connect(options)
                .await
                .expect("connexion à la base de test");
            migration::Migrator::up(&db, None)
                .await
                .expect("migrations");
            Some(db)
        })
        .await
        .clone()
}

fn state(db: DatabaseConnection) -> AppState {
    AppState::new(
        db,
        Config {
            database_url: String::new(),
            port: 0,
            jwt_secret: "un-secret-de-test-suffisamment-long-pour-passer".into(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 60,
            database_max_connections: 10,
        },
    )
}

/// Every test invents its own address so they can share one database without
/// tripping over each other.
fn unique_email(tag: &str) -> String {
    format!("{tag}-{}@plum.app", uuid_like())
}

fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{nanos:x}")
}

async fn call(app: &axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.expect("réponse");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, body)
}

fn post(path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn sign_up_body(email: &str) -> Value {
    json!({
        "email": email,
        "password": "motdepasse",
        "display_name": "Camille",
        // The client sends a full timestamp, not a bare date.
        "birth_date": "1996-04-12T00:00:00Z",
        "gender": "nonBinary"
    })
}

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
