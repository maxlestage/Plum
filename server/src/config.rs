use std::env;

/// Everything the process needs from its environment.
///
/// Read once at boot and never again: a dyno that starts with a bad
/// configuration should fail loudly there, not on the first request of a
/// person trying to sign in.
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub jwt_secret: String,
    pub access_token_ttl_minutes: i64,
    pub refresh_token_ttl_days: i64,
    pub database_max_connections: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("variable d'environnement manquante : {0}")]
    Missing(&'static str),
    #[error("variable d'environnement illisible : {0}")]
    Invalid(&'static str),
    #[error("JWT_SECRET doit faire au moins 32 caractères")]
    WeakSecret,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url = env::var("DATABASE_URL")
            .map_err(|_| ConfigError::Missing("DATABASE_URL"))
            .map(|url| normalise_database_url(&url))?;

        // Heroku assigns the port; binding anything else makes the dyno fail
        // its boot check with no useful message.
        let port = env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .map_err(|_| ConfigError::Invalid("PORT"))?;

        let jwt_secret = env::var("JWT_SECRET").map_err(|_| ConfigError::Missing("JWT_SECRET"))?;
        if jwt_secret.len() < 32 {
            return Err(ConfigError::WeakSecret);
        }

        Ok(Self {
            database_url,
            port,
            jwt_secret,
            access_token_ttl_minutes: parse_or("ACCESS_TOKEN_TTL_MINUTES", 15)?,
            refresh_token_ttl_days: parse_or("REFRESH_TOKEN_TTL_DAYS", 60)?,
            // Heroku Postgres Essential-0 allows 20 connections for the whole
            // account, not per dyno. A default pool would happily try to open
            // more and fail in a way that names neither the plan nor the cap.
            database_max_connections: parse_or("DATABASE_MAX_CONNECTIONS", 10)? as u32,
        })
    }
}

fn parse_or(key: &'static str, fallback: i64) -> Result<i64, ConfigError> {
    match env::var(key) {
        Ok(value) => value.parse().map_err(|_| ConfigError::Invalid(key)),
        Err(_) => Ok(fallback),
    }
}

/// Heroku Postgres hands out `postgres://` URLs; SeaORM only recognises
/// `postgresql://`. One character apart, and the failure it produces names
/// neither.
pub fn normalise_database_url(url: &str) -> String {
    match url.strip_prefix("postgres://") {
        Some(rest) => format!("postgresql://{rest}"),
        None => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heroku_postgres_urls_are_rewritten() {
        assert_eq!(
            normalise_database_url("postgres://user:pw@host:5432/db"),
            "postgresql://user:pw@host:5432/db"
        );
    }

    #[test]
    fn other_urls_are_left_alone() {
        let url = "postgresql://user:pw@host:5432/db";
        assert_eq!(normalise_database_url(url), url);
    }

    #[test]
    fn a_url_that_merely_mentions_postgres_is_not_touched() {
        let url = "postgresql://user@postgres-host/db";
        assert_eq!(normalise_database_url(url), url);
    }
}
