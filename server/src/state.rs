use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::auth::tokens::TokenIssuer;
use crate::config::Config;
use crate::rate_limit::RateLimiter;

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

pub struct Inner {
    pub db: DatabaseConnection,
    pub tokens: TokenIssuer,
    pub limiter: RateLimiter,
    pub config: Config,
}

impl AppState {
    pub fn new(db: DatabaseConnection, config: Config, limiter: RateLimiter) -> Self {
        let tokens = TokenIssuer::new(&config.jwt_secret, config.access_token_ttl_minutes);
        Self(Arc::new(Inner {
            db,
            tokens,
            limiter,
            config,
        }))
    }
}

impl std::ops::Deref for AppState {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
