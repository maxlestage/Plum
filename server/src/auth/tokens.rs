use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// What rides inside the access token. Deliberately thin: an identifier and
/// an expiry. Anything else would be a copy of the database that goes stale
/// the moment it is issued.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Claims {
    pub sub: Uuid,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("jeton invalide")]
    Invalid,
    #[error("jeton expiré")]
    Expired,
}

#[derive(Clone)]
pub struct TokenIssuer {
    encoding: EncodingKey,
    decoding: DecodingKey,
    access_ttl: Duration,
}

/// Redacted on purpose: the keys must never reach a log line.
impl std::fmt::Debug for TokenIssuer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TokenIssuer")
            .field("access_ttl", &self.access_ttl)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessToken {
    pub value: String,
    pub expires_at: DateTime<Utc>,
}

impl TokenIssuer {
    pub fn new(secret: &str, access_ttl_minutes: i64) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret.as_bytes()),
            decoding: DecodingKey::from_secret(secret.as_bytes()),
            access_ttl: Duration::minutes(access_ttl_minutes),
        }
    }

    pub fn issue(&self, user_id: Uuid, now: DateTime<Utc>) -> Result<AccessToken, TokenError> {
        let expires_at = now + self.access_ttl;
        let claims = Claims {
            sub: user_id,
            exp: expires_at.timestamp(),
            iat: now.timestamp(),
        };
        let value = encode(&Header::new(Algorithm::HS256), &claims, &self.encoding)
            .map_err(|_| TokenError::Invalid)?;
        Ok(AccessToken { value, expires_at })
    }

    pub fn verify(&self, token: &str) -> Result<Claims, TokenError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.leeway = 0;
        decode::<Claims>(token, &self.decoding, &validation)
            .map(|data| data.claims)
            .map_err(|error| match error.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => TokenError::Expired,
                _ => TokenError::Invalid,
            })
    }
}

/// The refresh token itself: random, opaque, and never stored. Only its
/// digest reaches the database.
pub fn generate_refresh_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// SHA-256 is right here and Argon2 is not: the token is already 256 bits of
/// entropy, so there is nothing to slow an attacker down for.
pub fn digest(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "un-secret-de-test-suffisamment-long-pour-passer";

    #[test]
    fn a_freshly_issued_token_verifies() {
        let issuer = TokenIssuer::new(SECRET, 15);
        let user = Uuid::new_v4();
        let now = Utc::now();

        let token = issuer.issue(user, now).unwrap();
        let claims = issuer.verify(&token.value).unwrap();

        assert_eq!(claims.sub, user);
        assert_eq!(
            token.expires_at.timestamp(),
            (now + Duration::minutes(15)).timestamp()
        );
    }

    #[test]
    fn an_expired_token_is_reported_as_expired_not_merely_invalid() {
        let issuer = TokenIssuer::new(SECRET, 15);
        let long_ago = Utc::now() - Duration::hours(2);

        let token = issuer.issue(Uuid::new_v4(), long_ago).unwrap();

        assert!(matches!(
            issuer.verify(&token.value),
            Err(TokenError::Expired)
        ));
    }

    /// A token signed with another secret must not open anything, which is the
    /// whole point of signing it.
    #[test]
    fn a_token_from_another_secret_is_rejected() {
        let ours = TokenIssuer::new(SECRET, 15);
        let theirs = TokenIssuer::new("un-autre-secret-tout-aussi-long-mais-different", 15);

        let token = theirs.issue(Uuid::new_v4(), Utc::now()).unwrap();

        assert!(matches!(
            ours.verify(&token.value),
            Err(TokenError::Invalid)
        ));
    }

    #[test]
    fn nonsense_is_rejected() {
        let issuer = TokenIssuer::new(SECRET, 15);
        assert!(issuer.verify("pas.un.jeton").is_err());
        assert!(issuer.verify("").is_err());
    }

    #[test]
    fn refresh_tokens_are_unique_and_hex() {
        let first = generate_refresh_token();
        let second = generate_refresh_token();
        assert_ne!(first, second);
        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn the_digest_is_stable_and_hides_the_token() {
        let token = generate_refresh_token();
        assert_eq!(digest(&token), digest(&token));
        assert_ne!(digest(&token), token);
        assert_eq!(digest(&token).len(), 64);
    }
}
