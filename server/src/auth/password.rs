use std::sync::LazyLock;

use argon2::Argon2;
use password_hash::rand_core::RngCore;
use password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use tokio::sync::Semaphore;

/// How many hashes may be computed at the same time.
///
/// Argon2id costs about 19 MiB of memory and ~100 ms of CPU per call, by
/// design — that cost is the whole defence. `spawn_blocking` keeps it off the
/// runtime's workers, but tokio's blocking pool grows to 512 threads, so a
/// burst of sign-ins would put hundreds of those 19 MiB allocations in flight
/// at once. On a 512 MB dyno that is an out-of-memory restart that anyone can
/// trigger by opening a few dozen connections.
///
/// The rate limiter does not cover this. It counts per address, and an
/// attacker uses a different address each time; nothing there bounds how many
/// hashes run concurrently. This does, and the excess waits its turn — a
/// slower sign-in under load, rather than a dyno that dies.
static HASHING_SLOTS: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(max_in_flight()));

/// Bounded above so a host that reports many cores cannot blow the memory
/// budget, and below so there is always at least one.
fn max_in_flight() -> usize {
    std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(2)
        .clamp(1, 8)
}

/// Argon2id is CPU-bound by design — that is the entire point — and it costs
/// roughly 100 ms. Running it on a runtime worker blocks that worker for the
/// duration; under real concurrency the executor starves and every other
/// request, including the database pool's own acquisitions, times out behind
/// it. So the work goes to a blocking thread, and only so many at a time.
pub async fn hash(password: String) -> Result<String, password_hash::Error> {
    // `ok()`: acquisition only fails on a closed semaphore, and this one is
    // never closed. Proceeding unbounded beats panicking inside a handler.
    let _slot = HASHING_SLOTS.acquire().await.ok();
    tokio::task::spawn_blocking(move || hash_blocking(&password))
        .await
        .unwrap_or(Err(password_hash::Error::Crypto))
}

/// Une empreinte Argon2id qu'aucun mot de passe ne vérifie.
///
/// Elle sert à faire travailler l'algorithme quand le compte n'existe pas.
/// Sans elle, une connexion sur une adresse inconnue revient sans avoir rien
/// calculé : mesuré sur ce code, 1,5 ms contre 455 ms — un facteur 250 qui
/// dit à quiconque chronomètre une seule requête si une adresse donnée a un
/// compte ici. Sur une application de rencontres, c'est la présence de
/// quelqu'un qu'on révèle.
///
/// Calculée une fois au premier besoin, à partir d'un secret tiré au sort à
/// chaque démarrage : personne ne peut la reconnaître, et aucun mot de passe
/// ne la satisfait.
static LEURRE: LazyLock<String> = LazyLock::new(|| {
    let mut aleatoire = [0u8; 32];
    OsRng.fill_bytes(&mut aleatoire);
    let secret: String = aleatoire.iter().map(|o| format!("{o:02x}")).collect();
    hash_blocking(&secret).unwrap_or_default()
});

/// Vérifie contre une empreinte de leurre, pour que le temps de réponse ne
/// dise pas si le compte existe.
///
/// Le résultat est ignoré et ne peut de toute façon pas être vrai : le leurre
/// est l'empreinte d'un secret tiré au sort au démarrage.
/// Calcule le leurre d'avance.
///
/// Sans cet appel au démarrage, la toute première connexion sur une adresse
/// inconnue paie la fabrication de l'empreinte — mesuré à 937 ms contre 440.
/// Le pic ne se produit qu'une fois, mais une fois suffit à celui qui
/// chronomètre, et un serveur qui redémarre le rend à nouveau disponible.
pub fn warm_decoy() {
    LazyLock::force(&LEURRE);
}

pub async fn burn_time_like_a_verification(password: String) -> bool {
    verify(password, LEURRE.clone()).await
}

pub async fn verify(password: String, hash: String) -> bool {
    let _slot = HASHING_SLOTS.acquire().await.ok();
    tokio::task::spawn_blocking(move || verify_blocking(&password, &hash))
        .await
        .unwrap_or(false)
}

/// Argon2id with the crate's defaults, which track the OWASP recommendation.
/// Nothing here is tunable on purpose: a knob on password hashing is a knob
/// someone eventually turns the wrong way.
pub fn hash_blocking(password: &str) -> Result<String, password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}

pub fn verify_blocking(password: &str, hash: &str) -> bool {
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
        let digest = hash_blocking("motdepasse-correct").unwrap();
        assert!(verify_blocking("motdepasse-correct", &digest));
    }

    #[test]
    fn a_different_password_does_not() {
        let digest = hash_blocking("motdepasse-correct").unwrap();
        assert!(!verify_blocking("motdepasse-incorrect", &digest));
    }

    /// The salt is what stops two identical passwords from sharing a digest,
    /// and a rainbow table from being worth building.
    #[test]
    fn the_same_password_hashes_differently_every_time() {
        let first = hash_blocking("identique").unwrap();
        let second = hash_blocking("identique").unwrap();
        assert_ne!(first, second);
        assert!(verify_blocking("identique", &first));
        assert!(verify_blocking("identique", &second));
    }

    #[test]
    fn a_corrupt_hash_fails_closed() {
        assert!(!verify_blocking("peu importe", "ceci n'est pas un hachage"));
        assert!(!verify_blocking("peu importe", ""));
    }

    #[test]
    fn the_number_in_flight_is_bounded_and_never_zero() {
        let slots = max_in_flight();
        assert!((1..=8).contains(&slots), "créneaux hors bornes : {slots}");
    }

    /// A semaphore in front of the hashing is a deadlock waiting to happen if
    /// a permit is ever dropped on the floor. More callers than there are
    /// permits, all of which must finish.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn more_concurrent_hashes_than_permits_all_complete() {
        let callers = max_in_flight() * 3 + 1;
        let mut running = Vec::with_capacity(callers);
        for index in 0..callers {
            running.push(tokio::spawn(hash(format!("motdepasse-{index}"))));
        }

        let mut digests = Vec::with_capacity(callers);
        for task in running {
            digests.push(task.await.expect("tâche").expect("hachage"));
        }

        assert_eq!(digests.len(), callers);
        // Every permit was returned, so the queue drained rather than wedged.
        assert_eq!(HASHING_SLOTS.available_permits(), max_in_flight());
    }
}
