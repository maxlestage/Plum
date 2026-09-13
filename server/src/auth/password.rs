use argon2::Argon2;
use password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};

/// Argon2id with the crate's defaults, which track the OWASP recommendation.
/// Nothing here is tunable on purpose: a knob on password hashing is a knob
/// someone eventually turns the wrong way.
pub fn hash(password: &str) -> Result<String, password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}

pub fn verify(password: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        // A malformed hash in the database is a failed verification, not a
        // crash, and certainly not a successful sign-in.
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_verifies_against_its_own_hash() {
        let digest = hash("motdepasse-correct").unwrap();
        assert!(verify("motdepasse-correct", &digest));
    }

    #[test]
    fn a_different_password_does_not() {
        let digest = hash("motdepasse-correct").unwrap();
        assert!(!verify("motdepasse-incorrect", &digest));
    }

    /// The salt is what stops two identical passwords from sharing a digest,
    /// and a rainbow table from being worth building.
    #[test]
    fn the_same_password_hashes_differently_every_time() {
        let first = hash("identique").unwrap();
        let second = hash("identique").unwrap();
        assert_ne!(first, second);
        assert!(verify("identique", &first));
        assert!(verify("identique", &second));
    }

    #[test]
    fn a_corrupt_hash_fails_closed() {
        assert!(!verify("peu importe", "ceci n'est pas un hachage"));
        assert!(!verify("peu importe", ""));
    }
}
