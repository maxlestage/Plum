pub mod auth;
pub mod config;
pub mod entities;
pub mod error;
pub mod state;

use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use tower_http::trace::TraceLayer;

use state::AppState;

#[derive(Serialize)]
struct Health {
    status: &'static str,
    version: &'static str,
}

/// Heroku's health check, and the first thing worth making work: a dyno that
/// answers here proves the image, the port binding and the boot sequence.
async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .nest("/api/v1", auth::routes::router())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
