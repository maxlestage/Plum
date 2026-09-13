use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::auth::tokens::TokenIssuer;
use crate::config::Config;

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

pub struct Inner {
    pub db: DatabaseConnection,
    pub tokens: TokenIssuer,
    pub config: Config,
}

impl AppState {
    pub fn new(db: DatabaseConnection, config: Config) -> Self {
        let tokens = TokenIssuer::new(&config.jwt_secret, config.access_token_ttl_minutes);
        Self(Arc::new(Inner { db, tokens, config }))
    }
}

impl std::ops::Deref for AppState {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
