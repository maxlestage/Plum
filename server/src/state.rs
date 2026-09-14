use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::auth::tokens::TokenIssuer;
use crate::config::Config;
use crate::live::hub::Hub;
use crate::rate_limit::RateLimiter;

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

pub struct Inner {
    pub db: DatabaseConnection,
    pub tokens: TokenIssuer,
    pub limiter: RateLimiter,
    pub config: Config,
    /// Les sockets ouverts sur ce dyno. Vide et inerte tant que personne
    /// n'est connecté, donc rien à configurer pour l'utiliser.
    pub hub: Hub,
}

impl AppState {
    pub fn new(db: DatabaseConnection, config: Config, limiter: RateLimiter) -> Self {
        let tokens = TokenIssuer::new(&config.jwt_secret, config.access_token_ttl_minutes);
        Self(Arc::new(Inner {
            db,
            tokens,
            limiter,
            config,
            hub: Hub::new(),
        }))
    }
}

impl std::ops::Deref for AppState {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
