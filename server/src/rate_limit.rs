//! Rate limiting, backed by Redis when there is one and by this process when
//! there is not.
//!
//! Optional on purpose. Requiring a paid add-on to boot would mean a Heroku
//! app that cannot start until someone adds one, and an add-on outage that
//! takes the whole API down with it. Degrading to a per-process limiter is a
//! worse defence — it counts per dyno — but it is a defence, and the service
//! stays up.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How many of something, over how long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quota {
    pub limit: u32,
    pub window: Duration,
}

/// L'adresse du client telle que le routeur d'hébergement l'a vue.
///
/// Heroku **ajoute** l'adresse d'origine à droite de `X-Forwarded-For` : si le
/// client en envoyait déjà un, le sien est poussé devant et celui du routeur
/// se retrouve en dernier. C'est donc la dernière valeur qu'on lit, et elle
/// seule — tout ce qui précède vient du client et se falsifie à volonté.
///
/// Absente, on rend `None` plutôt qu'une clé commune : sans en-tête on n'est
/// pas derrière le routeur, donc en local ou dans les tests, et mettre tout le
/// monde dans le même seau y bloquerait la deuxième inscription venue.
pub fn client_ip(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.rsplit(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// Le bloc d'adresses auquel appartient un client, plutôt que son adresse.
///
/// Une limite par adresse exacte se contourne en changeant d'adresse, et c'est
/// le comportement *normal* de tout hébergeur, de tout VPN, de tout mandataire :
/// mesuré depuis une machine ordinaire, sept adresses d'un même bloc ont suffi
/// à multiplier le quota par sept.
///
/// Grouper par /24 en IPv4 et par /64 en IPv6 — la taille qu'on attribue
/// couramment — ramène un pool entier à un seul seau. Le prix est qu'un
/// opérateur mobile derrière un NAT partagé range beaucoup de monde dans le
/// même bloc : le quota associé doit donc être bien plus large que celui par
/// adresse, pas identique.
pub fn client_block(ip: &str) -> String {
    if let Some((prefix, _)) = ip.rsplit_once('.') {
        // IPv4 : on garde les trois premiers octets.
        if prefix.split('.').count() == 3 {
            return format!("{prefix}.0/24");
        }
    }
    match ip.split(':').collect::<Vec<_>>() {
        // IPv6 : les quatre premiers groupes.
        groups if groups.len() > 4 => format!("{}::/64", groups[..4].join(":")),
        _ => ip.to_owned(),
    }
}

impl Quota {
    pub const fn new(limit: u32, window_seconds: u64) -> Self {
        Self {
            limit,
            window: Duration::from_secs(window_seconds),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub allowed: bool,
    /// What is left of the quota once this call is counted.
    pub remaining: u32,
    /// How long until the window rolls over. Sent verbatim as `Retry-After`,
    /// which the iOS client already reads and honours.
    pub retry_after: Duration,
}

pub enum RateLimiter {
    // Boxed: the manager is 320 bytes and the fallback a handful, so every
    // value of this enum would otherwise carry the larger of the two.
    Redis(Box<redis::aio::ConnectionManager>),
    /// Per-process fallback. Counts per dyno rather than per account, which is
    /// weaker — and stated as such rather than papered over.
    InProcess(InProcessLimiter),
}

impl RateLimiter {
    pub async fn check(&self, key: &str, quota: Quota) -> Decision {
        match self {
            Self::Redis(manager) => match redis_check((**manager).clone(), key, quota).await {
                Ok(decision) => decision,
                Err(error) => {
                    // A limiter that fails closed turns a Redis blip into an
                    // outage. Let the request through and say so.
                    tracing::warn!(%error, "limiteur indisponible, requête laissée passer");
                    Decision {
                        allowed: true,
                        remaining: quota.limit,
                        retry_after: Duration::ZERO,
                    }
                }
            },
            Self::InProcess(limiter) => limiter.check(key, quota),
        }
    }

    pub fn describe(&self) -> &'static str {
        match self {
            Self::Redis(_) => "redis",
            Self::InProcess(_) => "en mémoire (par dyno)",
        }
    }
}

/// Fixed window: `INCR`, and set the expiry only on the first hit so the
/// window starts when the first request does and is not pushed back by every
/// subsequent one.
async fn redis_check(
    mut manager: redis::aio::ConnectionManager,
    key: &str,
    quota: Quota,
) -> redis::RedisResult<Decision> {
    let namespaced = format!("plum:rl:{key}");

    // `SET … NX` pose le compteur *et* son échéance, et seulement s'il
    // n'existe pas encore. L'`EXPIRE` inconditionnel d'avant repoussait
    // l'échéance à chaque requête : le compteur ne retombait donc qu'après une
    // fenêtre entière de silence, et quelqu'un qui atteignait la limite puis
    // continuait d'essayer se bloquait lui-même sans fin — pendant que le
    // message lui disait de réessayer dans un instant.
    //
    // `SET NX` plutôt qu'`EXPIRE NX`, qui demanderait Redis 7 : celui-ci
    // fonctionne depuis toujours.
    let (_, count, ttl): (bool, u32, i64) = redis::pipe()
        .atomic()
        .cmd("SET")
        .arg(&namespaced)
        .arg(0)
        .arg("EX")
        .arg(quota.window.as_secs())
        .arg("NX")
        .cmd("INCR")
        .arg(&namespaced)
        .cmd("TTL")
        .arg(&namespaced)
        .query_async(&mut manager)
        .await?;

    Ok(decide(count, quota, Duration::from_secs(ttl.max(0) as u64)))
}

fn decide(count: u32, quota: Quota, retry_after: Duration) -> Decision {
    Decision {
        allowed: count <= quota.limit,
        remaining: quota.limit.saturating_sub(count),
        retry_after,
    }
}

#[derive(Default)]
pub struct InProcessLimiter {
    windows: Mutex<HashMap<String, (u32, Instant)>>,
}

impl InProcessLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    fn check(&self, key: &str, quota: Quota) -> Decision {
        let now = Instant::now();
        let mut windows = self.windows.lock().unwrap_or_else(|poisoned| {
            // A poisoned lock means another thread panicked while holding it.
            // The counters are not worth taking the process down for.
            poisoned.into_inner()
        });

        // Opportunistic pruning: without it the map grows once per address
        // that ever knocked, forever.
        if windows.len() > 10_000 {
            windows.retain(|_, (_, started)| now.duration_since(*started) < quota.window);
        }

        let entry = windows.entry(key.to_string()).or_insert((0, now));
        if now.duration_since(entry.1) >= quota.window {
            *entry = (0, now);
        }
        entry.0 += 1;

        let elapsed = now.duration_since(entry.1);
        decide(entry.0, quota, quota.window.saturating_sub(elapsed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUOTA: Quota = Quota::new(3, 60);

    #[test]
    fn the_first_requests_of_a_window_are_allowed() {
        let limiter = InProcessLimiter::new();

        for expected_remaining in [2, 1, 0] {
            let decision = limiter.check("moi@plum.app", QUOTA);
            assert!(decision.allowed);
            assert_eq!(decision.remaining, expected_remaining);
        }
    }

    #[test]
    fn the_one_past_the_quota_is_refused() {
        let limiter = InProcessLimiter::new();
        for _ in 0..3 {
            limiter.check("moi@plum.app", QUOTA);
        }

        let decision = limiter.check("moi@plum.app", QUOTA);

        assert!(!decision.allowed);
        assert_eq!(decision.remaining, 0);
        assert!(decision.retry_after > Duration::ZERO);
    }

    /// One account being hammered must not lock out another.
    #[test]
    fn keys_are_counted_separately() {
        let limiter = InProcessLimiter::new();
        for _ in 0..4 {
            limiter.check("cible@plum.app", QUOTA);
        }

        let other = limiter.check("quelqun-dautre@plum.app", QUOTA);

        assert!(other.allowed);
        assert_eq!(other.remaining, 2);
    }

    #[test]
    fn the_window_rolls_over() {
        let limiter = InProcessLimiter::new();
        let instant = Quota::new(2, 0);

        for _ in 0..5 {
            assert!(limiter.check("moi@plum.app", instant).allowed);
        }
    }

    #[test]
    fn the_decision_counts_the_current_call() {
        // The caller has already been counted when it reads `remaining`, so a
        // quota of three leaves two after the first call, not three.
        assert_eq!(decide(1, QUOTA, Duration::from_secs(60)).remaining, 2);
        assert!(decide(3, QUOTA, Duration::from_secs(60)).allowed);
        assert!(!decide(4, QUOTA, Duration::from_secs(60)).allowed);
        assert_eq!(decide(9, QUOTA, Duration::from_secs(60)).remaining, 0);
    }
}

#[cfg(test)]
mod blocs {
    use super::client_block;

    /// Sept adresses d'un même bloc, mesurées depuis une machine ordinaire,
    /// suffisaient à multiplier le quota par sept. Elles doivent maintenant
    /// tomber dans le même seau.
    #[test]
    fn a_rotating_pool_lands_in_one_bucket() {
        let pool = [
            "160.79.106.128",
            "160.79.106.129",
            "160.79.106.131",
            "160.79.106.137",
        ];
        for address in pool {
            assert_eq!(client_block(address), "160.79.106.0/24", "{address}");
        }
    }

    #[test]
    fn two_different_blocks_stay_apart() {
        assert_ne!(
            client_block("160.79.106.1"),
            client_block("160.79.107.1"),
            "deux blocs voisins ne doivent pas se confondre"
        );
    }

    #[test]
    fn ipv6_is_grouped_by_the_usual_allocation() {
        assert_eq!(
            client_block("2001:db8:1234:5678:9abc:def0:1234:5678"),
            "2001:db8:1234:5678::/64"
        );
    }

    /// Une valeur qu'on ne sait pas lire sert de clé telle quelle plutôt que
    /// d'ouvrir une brèche : mieux vaut un seau trop fin qu'aucun seau.
    #[test]
    fn something_unreadable_is_still_a_key() {
        assert_eq!(client_block("pas-une-adresse"), "pas-une-adresse");
    }
}

/// Les tests du limiteur Redis, sautés sans `TEST_REDIS_URL`.
///
/// Ils existent parce que les deux implémentations avaient divergé, et que
/// c'est celle de production qui avait tort : l'`EXPIRE` posé à chaque requête
/// repoussait l'échéance, donc le compteur ne retombait qu'après une fenêtre
/// entière de silence. La version en mémoire, elle, gardait bien l'instant de
/// départ. Une seule des deux était testée.
#[cfg(test)]
mod redis_tests {
    use super::*;

    fn url() -> Option<String> {
        match std::env::var("TEST_REDIS_URL") {
            Ok(url) if !url.is_empty() => Some(url),
            _ => {
                // Deux tests verts qui n'ont rien exécuté valent moins qu'un
                // rouge. C'est précisément ce silence qui a laissé les deux
                // implémentations du limiteur diverger : la CI pose
                // `REQUIRE_TEST_REDIS` pour qu'un service qui n'a pas démarré
                // ne passe pas pour une exécution propre.
                assert!(
                    std::env::var("REQUIRE_TEST_REDIS").is_err(),
                    "REQUIRE_TEST_REDIS est posé mais TEST_REDIS_URL manque : \
                     les tests du limiteur Redis auraient été sautés en silence"
                );
                eprintln!("· sauté : TEST_REDIS_URL non défini");
                None
            }
        }
    }

    async fn limiter(url: &str) -> RateLimiter {
        let client = redis::Client::open(url).expect("client redis");
        let manager = redis::aio::ConnectionManager::new(client)
            .await
            .expect("connexion redis");
        RateLimiter::Redis(Box::new(manager))
    }

    #[tokio::test]
    async fn the_window_does_not_move_when_someone_keeps_knocking() {
        let Some(url) = url() else { return };
        let limiter = limiter(&url).await;
        let quota = Quota::new(3, 2);
        let key = format!("essai-fenetre-{}", uuid::Uuid::new_v4());

        for _ in 0..4 {
            limiter.check(&key, quota).await;
        }
        assert!(!limiter.check(&key, quota).await.allowed, "seau plein");

        // On frappe *pendant* toute la fenêtre, et c'est tout l'objet du test :
        // dormir en silence ne prouverait rien, puisque l'échéance finirait par
        // tomber d'elle-même. C'est l'obstination qui révélait le défaut —
        // chaque tentative repoussait l'échéance d'une fenêtre entière.
        let jusqu_a = std::time::Instant::now() + std::time::Duration::from_millis(2_600);
        while std::time::Instant::now() < jusqu_a {
            limiter.check(&key, quota).await;
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }

        assert!(
            limiter.check(&key, quota).await.allowed,
            "la fenêtre ne s'est jamais refermée : s'obstiner suffisait à rester \
             bloqué sans fin, pendant que le message disait de réessayer dans un \
             instant"
        );
    }

    #[tokio::test]
    async fn the_limit_is_the_limit() {
        let Some(url) = url() else { return };
        let limiter = limiter(&url).await;
        let quota = Quota::new(3, 60);
        let key = format!("essai-plafond-{}", uuid::Uuid::new_v4());

        let verdicts: Vec<bool> = {
            let mut v = Vec::new();
            for _ in 0..5 {
                v.push(limiter.check(&key, quota).await.allowed);
            }
            v
        };
        assert_eq!(verdicts, vec![true, true, true, false, false]);
    }
}
