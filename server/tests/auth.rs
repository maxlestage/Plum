//! End-to-end tests for the authentication slice, against a real Postgres.
//!
//! They need `TEST_DATABASE_URL`. Without it they skip rather than fail: a
//! contributor running `cargo test` on a laptop should not have to stand up a
//! database to check the pure logic, and CI provides one.

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
const MIGRATION_LOCK: i64 = 0x504c_554d;

/// Routes the server's own logs into the test output. Without this an
/// internal failure reaches the assertion as a bare 500 with a polite message
/// and nothing to act on.
fn capture_server_logs() {
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .with_env_filter("plum_server=debug,sqlx=warn")
        .try_init();
}

/// The test database URL, or `None` when the suite should skip.
fn test_database_url() -> Option<String> {
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
async fn database() -> Option<DatabaseConnection> {
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
async fn migrate_once(url: &str) {
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

/// Like `call`, but keeps the bytes. `call` parses JSON and folds anything
/// else into `Null`, which cannot tell the site's HTML from an empty body.
async fn call_raw(app: &axum::Router, request: Request<Body>) -> (StatusCode, String) {
    let response = app.clone().oneshot(request).await.expect("réponse");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
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
