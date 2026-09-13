use std::net::SocketAddr;

use migration::MigratorTrait;
use plum_server::config::Config;
use plum_server::rate_limit::RateLimiter;
use plum_server::state::AppState;
use sea_orm::{ConnectOptions, Database};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    // Read the whole configuration before touching anything else: a dyno with
    // a bad environment should die at boot with a clear message, not on some
    // unlucky person's first request.
    // Printed rather than returned: `?` would surface the Debug form, and the
    // person reading a crashed dyno's logs deserves the sentence, not the
    // enum variant.
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Configuration invalide : {error}");
            std::process::exit(1);
        }
    };
    let port = config.port;

    let mut options = ConnectOptions::new(config.database_url.clone());
    options
        .max_connections(config.database_max_connections)
        // Ten seconds, not the default thirty: a request that cannot get a
        // connection should fail while someone is still looking at the screen.
        .acquire_timeout(std::time::Duration::from_secs(10))
        .sqlx_logging(false);

    let db = Database::connect(options).await?;

    // Migrating at boot keeps the deployment to a single image. It holds
    // because there is one dyno: with several, two would race here and this
    // belongs in a Heroku release phase instead.
    migration::Migrator::up(&db, None).await?;
    tracing::info!("schéma à jour");

    let limiter = build_limiter(config.redis_url.as_deref()).await;
    tracing::info!("limitation de débit : {}", limiter.describe());

    let state = AppState::new(db, config, limiter);
    let app = plum_server::app(state);

    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!("Plum écoute sur {address}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Redis is optional. Without it, or when it refuses to answer at boot, the
/// limiter counts in this process: weaker, because it counts per dyno, but the
/// service starts and defends itself rather than refusing to start at all.
async fn build_limiter(url: Option<&str>) -> RateLimiter {
    let Some(url) = url else {
        return RateLimiter::InProcess(plum_server::rate_limit::InProcessLimiter::new());
    };

    if url.starts_with("rediss://") && !url.contains("#insecure") {
        // Heroku's Key-Value Store presents a self-signed certificate, and
        // their own documentation tells you to skip verification. Saying so
        // beats a connection error that names neither.
        tracing::warn!(
            "URL rediss:// sans #insecure — Heroku utilise un certificat \
             auto-signé et la connexion échouera probablement"
        );
    }

    match redis::Client::open(url) {
        Ok(client) => match redis::aio::ConnectionManager::new(client).await {
            Ok(manager) => RateLimiter::Redis(Box::new(manager)),
            Err(error) => {
                tracing::warn!(%error, "Redis injoignable, repli en mémoire");
                RateLimiter::InProcess(plum_server::rate_limit::InProcessLimiter::new())
            }
        },
        Err(error) => {
            tracing::warn!(%error, "REDIS_URL illisible, repli en mémoire");
            RateLimiter::InProcess(plum_server::rate_limit::InProcessLimiter::new())
        }
    }
}

/// Heroku sends SIGTERM and waits 30 seconds before pulling the plug. Draining
/// in-flight requests in that window is the difference between a deploy nobody
/// notices and one that drops connections.
async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut terminate = match signal(SignalKind::terminate()) {
        Ok(signal) => signal,
        Err(_) => return,
    };

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }

    tracing::info!("arrêt demandé, fermeture en cours");
}
