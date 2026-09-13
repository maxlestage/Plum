pub mod auth;
pub mod config;
pub mod entities;
pub mod error;
pub mod rate_limit;
pub mod state;

use std::path::Path;

use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use tower_http::services::{ServeDir, ServeFile};
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
    app_with_site(state, None)
}

/// The API, and optionally the presentation site in front of it.
///
/// One dyno serves both. A second one for a few static files would cost more
/// per month than the first, and splitting them would put the site, the API
/// and the legal pages the App Store asks for on different hosts.
pub fn app_with_site(state: AppState, site: Option<&Path>) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .nest("/api/v1", auth::routes::router())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    match site.filter(|directory| directory.is_dir()) {
        Some(directory) => {
            // Anything the API does not claim is a route of the single-page
            // site, so an unknown path serves index.html rather than a 404 —
            // otherwise /conditions only works when typed from inside the app.
            let index = directory.join("index.html");
            let files = ServeDir::new(directory).not_found_service(ServeFile::new(index));
            api.fallback_service(files)
        }
        None => api,
    }
}
