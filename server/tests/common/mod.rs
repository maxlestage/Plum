//! The harness shared by every integration suite.
//!
//! It lives here rather than in one suite so the pool-per-test arrangement
//! below has exactly one copy: it is subtle enough that two would drift.
//!
//! The suites need `TEST_DATABASE_URL`. Without it they skip rather than fail:
//! a contributor running `cargo test` on a laptop should not have to stand up
//! a database to check the pure logic, and CI provides one.

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use plum_server::config::Config;
use plum_server::rate_limit::{InProcessLimiter, RateLimiter};
use plum_server::state::AppState;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use serde_json::{json, Value};
use tower::ServiceExt;

/// Identifies the migration lock. Any constant does; this one spells "PLUM".
pub const MIGRATION_LOCK: i64 = 0x504c_554d;

/// Routes the server's own logs into the test output. Without this an
/// internal failure reaches the assertion as a bare 500 with a polite message
/// and nothing to act on.
pub fn capture_server_logs() {
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .with_env_filter("plum_server=debug,sqlx=warn")
        .try_init();
}

/// The test database URL, or `None` when the suite should skip.
pub fn test_database_url() -> Option<String> {
    match std::env::var("TEST_DATABASE_URL") {
        Ok(url) => Some(plum_server::config::normalise_database_url(&url)),
        Err(_) => {
            // Ten green tests that executed nothing is worse than one red one.
            // CI sets REQUIRE_TEST_DATABASE so a Postgres service that failed
            // to start cannot pass for a clean run.
            assert!(
                std::env::var("REQUIRE_TEST_DATABASE").is_err(),
                "REQUIRE_TEST_DATABASE est posé mais TEST_DATABASE_URL manque : \
                 les tests d'intégration auraient été sautés en silence"
            );
            eprintln!("· sauté : TEST_DATABASE_URL non défini");
            None
        }
    }
}

/// A connection of this test's own, and the migrations applied once.
///
/// **Each test gets its own pool, deliberately.** Sharing one across the suite
/// is the obvious economy and it does not work: `#[tokio::test]` gives every
/// test its own current-thread runtime, and a sqlx connection registers its
/// socket with the IO driver of the runtime that opened it. Once the first
/// test finishes, that runtime is dropped and its driver with it — so a later
/// test handed one of those pooled connections waits on a socket nobody is
/// polling, and fails twenty seconds later with `ConnectionAcquire(Timeout)`.
/// The symptom is a handful of unrelated tests failing at random, including
/// one that does nothing but `ping`.
pub async fn database() -> Option<DatabaseConnection> {
    capture_server_logs();
    let url = test_database_url()?;

    migrate_once(&url).await;

    let mut options = ConnectOptions::new(url);
    options
        .max_connections(4)
        .min_connections(0)
        .acquire_timeout(std::time::Duration::from_secs(20))
        .sqlx_logging(false);

    Some(
        Database::connect(options)
            .await
            .expect("connexion à la base de test"),
    )
}

/// Applies the migrations under a Postgres advisory lock.
///
/// The migrator is idempotent, so the arrivals after the first do nothing —
/// but running them concurrently is not: several tests racing to create
/// `seaql_migrations` had Postgres reject all but one with a duplicate key on
/// `pg_type`. The lock is held by a session, so this pool has exactly one
/// connection and the three statements are guaranteed to share it.
///
/// A lock rather than a `OnceCell`: it serialises across threads, across test
/// binaries, and across a second `cargo test` someone starts in another
/// terminal, none of which a process-local cell can do.
pub async fn migrate_once(url: &str) {
    let mut options = ConnectOptions::new(url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .sqlx_logging(false);

    let db = Database::connect(options)
        .await
        .expect("connexion pour les migrations");

    db.execute_unprepared(&format!("SELECT pg_advisory_lock({MIGRATION_LOCK})"))
        .await
        .expect("prise du verrou de migration");

    let outcome = migration::Migrator::up(&db, None).await;

    // Released whatever happened: holding it through a panic would hang every
    // other test on the lock instead of failing them with the real reason.
    let unlock = db
        .execute_unprepared(&format!("SELECT pg_advisory_unlock({MIGRATION_LOCK})"))
        .await;

    outcome.expect("migrations");
    unlock.expect("libération du verrou de migration");
    db.close()
        .await
        .expect("fermeture de la connexion de migration");
}

pub fn state(db: DatabaseConnection) -> AppState {
    AppState::new(
        db,
        Config {
            database_url: String::new(),
            port: 0,
            jwt_secret: "un-secret-de-test-suffisamment-long-pour-passer".into(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 60,
            database_max_connections: 10,
            redis_url: None,
            site_dir: None,
        },
        // Each test builds its own app, so each gets a fresh limiter and one
        // test's attempts cannot exhaust another's quota.
        RateLimiter::InProcess(InProcessLimiter::new()),
    )
}

/// Every test invents its own address so they can share one database without
/// tripping over each other.
pub fn unique_email(tag: &str) -> String {
    format!("{tag}-{}@plum.app", uuid_like())
}

pub fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{nanos:x}")
}

pub async fn call(app: &axum::Router, request: Request<Body>) -> (StatusCode, Value) {
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

/// Like `call`, but keeps the bytes. `call` parses JSON and folds anything
/// else into `Null`, which cannot tell the site's HTML from an empty body.
pub async fn call_raw(app: &axum::Router, request: Request<Body>) -> (StatusCode, String) {
    let response = app.clone().oneshot(request).await.expect("réponse");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

pub fn post(path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

pub fn sign_up_body(email: &str) -> Value {
    json!({
        "email": email,
        "password": "motdepasse",
        "display_name": "Camille",
        // The client sends a full timestamp, not a bare date.
        "birth_date": "1996-04-12T00:00:00Z",
        "gender": "nonBinary"
    })
}

/// Builds a request with an optional bearer token and an optional JSON body.
/// The profile routes all need a token, and spelling the header out at each
/// call site buries what the test is actually about.
pub fn request(
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(path);

    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }

    match body {
        Some(body) => builder
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

/// A fresh account and its access token. Most profile tests need to be signed
/// in before they can say anything interesting.
pub async fn sign_up_and_token(app: &axum::Router, tag: &str) -> (String, Value) {
    let email = unique_email(tag);
    let (status, body) = call(app, post("/api/v1/auth/sign-up", sign_up_body(&email))).await;
    assert_eq!(status, StatusCode::OK, "inscription ratée : {body}");

    let token = body["tokens"]["access_token"]
        .as_str()
        .expect("jeton d'accès")
        .to_owned();

    (token, body["user"].clone())
}
