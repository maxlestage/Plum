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
    /// Optional. Without it the rate limiter falls back to this process, which
    /// counts per dyno instead of per account.
    pub redis_url: Option<String>,
    /// Where the built presentation site lives. Absent in a plain `cargo run`,
    /// present in the image.
    pub site_dir: Option<String>,
    /// Combien de profils le deck accepte de rendre par compte et par jour.
    ///
    /// Réglable par l'environnement pour deux raisons, dans cet ordre : sans
    /// cela le plafond serait impossible à éprouver autrement qu'en peuplant
    /// mille profils, donc il ne serait éprouvé par rien ; et un exploitant
    /// qui voit passer une récolte doit pouvoir resserrer sans réécrire le
    /// serveur. La valeur par défaut est celle qui compte : mille.
    pub deck_daily_budget: u32,
    /// Le jeton qui ouvre la file de modération, s'il y en a un.
    ///
    /// Absent par défaut, et c'est délibéré : tant qu'il n'est pas posé, la
    /// route n'existe pas — elle répond comme n'importe quelle adresse
    /// inconnue. Une porte d'administration qui s'annonce invite à chercher
    /// sa clé.
    pub admin_token: Option<String>,
    /// L'adresse publique de ce déploiement, sans barre finale.
    ///
    /// Elle sert à écrire les adresses des photos, qui doivent être absolues :
    /// côté SwiftUI, `AsyncImage` fait une requête nue et ne saurait pas
    /// résoudre un chemin relatif. Déduire l'adresse de l'en-tête `Host` de
    /// chaque requête serait plus souple et bien plus fragile — la moitié des
    /// réponses qui portent un profil sont construites loin de la requête.
    pub public_base_url: String,
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
            redis_url: env::var("REDIS_URL")
                .ok()
                .filter(|url| !url.is_empty())
                .map(|url| normalise_redis_url(&url)),
            site_dir: env::var("SITE_DIR").ok().filter(|path| !path.is_empty()),
            admin_token: admin_token(),
            deck_daily_budget: env::var("DECK_DAILY_BUDGET")
                .ok()
                .and_then(|raw| raw.parse().ok())
                .filter(|budget| *budget > 0)
                .unwrap_or(1_000),
            public_base_url: env::var("PUBLIC_BASE_URL")
                .ok()
                .filter(|url| !url.is_empty())
                .map(|url| url.trim_end_matches('/').to_owned())
                // Le repli vaut pour un `cargo run` sur une machine ; en
                // production la variable est posée, et le README dit pourquoi
                // s'en passer donnerait des photos introuvables.
                .unwrap_or_else(|| format!("http://127.0.0.1:{port}")),
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

/// Heroku hands out `rediss://` URLs whose certificate is self-signed, so
/// verifying it against the system roots fails and the connection never
/// opens. The limiter fails open, which means it would quietly stop limiting
/// anything — the worst kind of breakage, because nothing looks wrong.
///
/// The `#insecure` fragment tells the redis crate to skip verification. It is
/// what Heroku's own documentation prescribes for these add-ons, and the
/// traffic stays inside their private network. Applied here rather than to
/// the config var itself, because Heroku regenerates `REDIS_URL` on
/// maintenance and would drop a fragment added by hand.
pub fn normalise_redis_url(url: &str) -> String {
    if url.starts_with("rediss://") && !url.contains('#') {
        format!("{url}#insecure")
    } else {
        url.to_string()
    }
}

/// Le jeton d'administration, s'il est utilisable.
///
/// Trente-deux caractères au minimum. Un jeton court est plus dangereux que
/// pas de jeton du tout : il donne le sentiment d'avoir fermé une porte qu'on
/// peut enfoncer. Mais le refuser en silence l'est tout autant — quelqu'un
/// poserait six lettres, la file resterait close, et il croirait l'avoir
/// ouverte. D'où l'avertissement, qui nomme la raison.
fn admin_token() -> Option<String> {
    match env::var("ADMIN_TOKEN") {
        Err(_) => None,
        Ok(brut) if brut.trim().is_empty() => None,
        Ok(brut) if brut.trim().chars().count() < MIN_ADMIN_TOKEN => {
            tracing::warn!(
                "ADMIN_TOKEN fait moins de {MIN_ADMIN_TOKEN} caractères : il est ignoré, \
                 et la file de modération reste fermée. Un jeton court donne le sentiment \
                 d'avoir fermé une porte qu'on peut enfoncer."
            );
            None
        }
        Ok(brut) => Some(brut.trim().to_owned()),
    }
}

/// Assez pour qu'une recherche exhaustive soit hors de question, et court
/// assez pour être recopié depuis un téléphone.
const MIN_ADMIN_TOKEN: usize = 32;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heroku_redis_url_gets_the_fragment_that_lets_it_connect() {
        assert_eq!(
            normalise_redis_url("rediss://h:pw@host:1234"),
            "rediss://h:pw@host:1234#insecure"
        );
    }

    #[test]
    fn a_plain_redis_url_is_left_alone() {
        // No TLS, nothing to verify.
        assert_eq!(
            normalise_redis_url("redis://127.0.0.1:6379"),
            "redis://127.0.0.1:6379"
        );
    }

    /// Someone who wrote their own fragment meant it; appending a second one
    /// would produce a URL the crate cannot parse.
    #[test]
    fn an_existing_fragment_is_not_doubled() {
        assert_eq!(
            normalise_redis_url("rediss://host:1/#insecure"),
            "rediss://host:1/#insecure"
        );
    }

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
