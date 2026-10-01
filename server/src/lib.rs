pub mod admin;
pub mod auth;
pub mod chat;
pub mod config;
pub mod discovery;
pub mod entities;
pub mod error;
pub mod live;
pub mod matches;
pub mod photos;
pub mod profile;
pub mod rate_limit;
pub mod state;

use std::path::Path;

use axum::routing::get;
use axum::{middleware, Json, Router};
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

/// Kept separate from the site's catch-all so an API typo reads as one.
async fn unknown_api_route() -> error::ApiError {
    error::ApiError::NotFound
}

/// Une méthode qui n'existe pas sur une adresse répond comme une adresse qui
/// n'existe pas.
///
/// Sans ça, le `404` de la porte d'administration ne protège rien. Le garde
/// répond « introuvable » plutôt que « non autorisé » précisément pour que
/// l'adresse ne se révèle pas — mais le routeur, lui, répond `405` avant le
/// garde dès qu'on se trompe de verbe. Comparer les deux codes suffisait donc
/// à dessiner la surface entière :
///
/// ```text
/// GET  /api/v1/admin/reports                       → 404   (le garde a parlé)
/// POST /api/v1/admin/reports                       → 405   ← « ça existe »
/// GET  /api/v1/admin/profiles/{id}/suspension      → 405   ← « ça existe, et ça suspend »
/// GET  /api/v1/admin/nimportequoi                  → 404   (vraiment rien)
/// ```
///
/// Mesuré sur la production, pas supposé. Le prix est qu'un mauvais verbe
/// n'est plus distingué d'une mauvaise adresse pour qui explore l'API à la
/// main ; c'est peu cher payé, et l'application, elle, n'envoie jamais le
/// mauvais verbe.
async fn wrong_method() -> error::ApiError {
    error::ApiError::NotFound
}

pub fn app(state: AppState) -> Router {
    app_with_site(state, None)
}

/// Combien de temps un client peut garder ce qu'il vient de recevoir.
///
/// Le serveur n'en disait rien, et « rien » n'est pas « ne garde pas » : sans
/// `Cache-Control`, un navigateur applique une heuristique — il garde la page
/// un dixième de son âge — et peut donc continuer à servir une version qu'on
/// vient de remplacer. C'est exactement ce qui pouvait laisser quelqu'un
/// devant la page blanche d'avant un correctif.
///
/// Dans l'autre sens, le wasm fait 443 Kio et son nom change à chaque
/// construction : le re-télécharger à chaque visite était gratuit pour
/// personne, surtout en données mobiles.
///
/// Trois régimes, donc :
///
/// - **les fichiers empreintés** — `plum-site-4cf83b582da4d8dd.js`,
///   `theme-13a1e19600626da1.css` — gardent un an. Leur nom *est* leur
///   version : un contenu différent porte un nom différent, donc une copie
///   gardée ne peut pas être périmée.
/// - **les pages** se revalident à chaque fois (`no-cache` autorise la
///   garde mais impose de demander). Un déploiement arrive alors tout de
///   suite, ce qui est la propriété qui compte ici.
/// - **le reste** — icônes, manifeste — une heure. Ces noms-là ne portent pas
///   d'empreinte, donc un an les figerait.
fn duree_de_cache(chemin: &str) -> &'static str {
    if porte_une_empreinte(chemin) {
        "public, max-age=31536000, immutable"
    } else if est_une_page(chemin) {
        "no-cache"
    } else {
        "public, max-age=3600"
    }
}

/// Un nom de fichier dont Trunk a fait dépendre le contenu : seize chiffres
/// hexadécimaux après le dernier tiret, éventuellement suivis de `_bg`.
fn porte_une_empreinte(chemin: &str) -> bool {
    let fichier = chemin.rsplit('/').next().unwrap_or_default();
    let Some((_, fin)) = fichier.rsplit_once('-') else {
        return false;
    };
    let empreinte = fin.split(['.', '_']).next().unwrap_or_default();
    empreinte.len() == 16 && empreinte.bytes().all(|octet| octet.is_ascii_hexdigit())
}

/// Une adresse que le site rend comme une page, et non un fichier.
///
/// Tout ce qui ne ressemble pas à un fichier en est une : le repli du site
/// sert `index.html` pour n'importe quelle adresse inconnue, et cette
/// réponse-là doit se revalider comme les autres pages.
fn est_une_page(chemin: &str) -> bool {
    chemin.ends_with('/')
        || chemin.ends_with(".html")
        || !chemin.rsplit('/').next().unwrap_or_default().contains('.')
}

async fn poser_la_duree_de_cache(
    requete: axum::extract::Request,
    suite: middleware::Next,
) -> axum::response::Response {
    let duree = duree_de_cache(requete.uri().path());
    let mut reponse = suite.run(requete).await;
    reponse.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static(duree),
    );
    reponse
}

/// The API, and optionally the presentation site in front of it.
///
/// One dyno serves both. A second one for a few static files would cost more
/// per month than the first, and splitting them would put the site, the API
/// and the legal pages the App Store asks for on different hosts.
pub fn app_with_site(state: AppState, site: Option<&Path>) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        // Hors de `/api/v1` : une mise à niveau WebSocket n'est pas une
        // requête versionnée, et le client la cherche à la racine.
        .merge(live::routes::router())
        // Hors de `/api/v1` et sans jeton : `AsyncImage` fait une requête nue.
        // L'adresse est la clé — un UUID tiré au sort ne s'énumère pas.
        .merge(photos::routes::public_router())
        .nest(
            "/api/v1",
            // An unknown path under /api must not fall through to the site:
            // a client asking for JSON would get 200 and a page of HTML, and
            // would report the decoding failure rather than the typo.
            auth::routes::router()
                .merge(admin::routes::router())
                .merge(profile::routes::router())
                .merge(discovery::routes::router())
                .merge(matches::routes::router())
                .merge(chat::routes::router())
                .merge(photos::routes::router())
                .fallback(unknown_api_route)
                .method_not_allowed_fallback(wrong_method),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    match site.filter(|directory| directory.is_dir()) {
        Some(directory) => {
            // Anything the API does not claim is a route of the single-page
            // site, so an unknown path serves index.html rather than a 404 —
            // otherwise /conditions only works when typed from inside the app.
            //
            // `fallback`, not `not_found_service`: the latter is documented
            // for exactly this case, and it wraps the fallback in a
            // `SetStatus` that forces 404. The page would render and the
            // status would still say the page does not exist — which search
            // engines believe, and which some clients treat as an error.
            let index = directory.join("index.html");
            let files = ServeDir::new(directory).fallback(ServeFile::new(index));
            // La couche ne porte que sur le site : montée sur `api`, elle
            // poserait aussi des durées de cache sur des réponses d'API, dont
            // aucune n'est rejouable.
            let site = Router::new()
                .fallback_service(files)
                .layer(middleware::from_fn(poser_la_duree_de_cache));
            api.fallback_service(site)
        }
        None => api,
    }
}

#[cfg(test)]
mod tests_du_cache {
    use super::{duree_de_cache, est_une_page, porte_une_empreinte};

    #[test]
    fn un_fichier_empreinte_se_garde_un_an() {
        for chemin in [
            "/plum-site-4cf83b582da4d8dd.js",
            "/plum-site-4cf83b582da4d8dd_bg.wasm",
            "/theme-13a1e19600626da1.css",
        ] {
            assert!(porte_une_empreinte(chemin), "{chemin} porte une empreinte");
            assert_eq!(
                duree_de_cache(chemin),
                "public, max-age=31536000, immutable"
            );
        }
    }

    /// Le piège : ces noms-là ont un tiret, et aucun n'est une empreinte. Les
    /// figer un an rendrait une icône impossible à remplacer.
    #[test]
    fn un_nom_a_tiret_nest_pas_une_empreinte() {
        for chemin in [
            "/apple-touch-icon.png",
            "/favicon-16.png",
            "/favicon-512.png",
            "/site.webmanifest",
            "/partage.png",
        ] {
            assert!(!porte_une_empreinte(chemin), "{chemin} n'en porte pas");
            assert_eq!(duree_de_cache(chemin), "public, max-age=3600");
        }
    }

    /// Seize chiffres hexadécimaux, pas quinze, pas dix-sept, et pas des
    /// lettres au-delà de `f`.
    #[test]
    fn lempreinte_a_une_forme_exacte() {
        assert!(porte_une_empreinte("/x-0123456789abcdef.js"));
        assert!(!porte_une_empreinte("/x-0123456789abcde.js"), "quinze");
        assert!(!porte_une_empreinte("/x-0123456789abcdef0.js"), "dix-sept");
        assert!(
            !porte_une_empreinte("/x-0123456789abcdeg.js"),
            "g n'est pas hexadécimal"
        );
        assert!(!porte_une_empreinte("/sans-tiret-du-tout"));
    }

    #[test]
    fn une_page_se_revalide_toujours() {
        for chemin in [
            "/",
            "/fr/",
            "/es/privacidad/",
            "/fr/conditions/index.html",
            // Une adresse inconnue : le repli du site rend `index.html`, donc
            // la réponse est une page et doit se revalider comme telle.
            "/adresse/inventee",
        ] {
            assert!(est_une_page(chemin), "{chemin} est une page");
            assert_eq!(duree_de_cache(chemin), "no-cache");
        }
    }

    /// Ce qui compte vraiment : un déploiement doit atteindre quelqu'un qui a
    /// déjà la page. `no-cache` n'interdit pas de garder — il impose de
    /// demander avant de réutiliser — et c'est bien ce qu'on veut.
    #[test]
    fn aucune_page_nest_gardee_sans_demander() {
        for chemin in ["/", "/fr/", "/en/help/"] {
            let duree = duree_de_cache(chemin);
            assert!(duree.contains("no-cache"), "{chemin} : {duree}");
            assert!(!duree.contains("max-age=31536000"), "{chemin} : {duree}");
        }
    }
}
