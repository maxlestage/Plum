//! La file de modération : ce qu'un signalement devient une fois écrit.
//!
//! Avant cette route, rien ne lisait jamais la table des signalements —
//! littéralement zéro occurrence dans le serveur. Quelqu'un signalait un
//! harcèlement, croyait avoir prévenu, et personne ne voyait rien. C'est le
//! même défaut que le blocage décoratif : une promesse tenue dans l'interface
//! et nulle part ailleurs.

mod common;

use axum::http::StatusCode;
use common::deck_ages::*;
use common::*;
use serde_json::json;

async fn signale(app: &axum::Router, token: &str, cible: uuid::Uuid, motif: &str) {
    let (status, body) = call(
        app,
        request(
            "POST",
            &format!("/api/v1/profiles/{cible}/report"),
            Some(token),
            Some(json!({ "reason": motif })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "signalement : {body}");
}

/// Sans jeton configuré, la porte n'existe pas — et elle répond `404`, pas
/// `401`.
///
/// La différence n'est pas cosmétique : un `401` annonce au monde qu'il y a
/// ici une administration et invite à chercher sa clé. C'est aussi l'état par
/// défaut de tout déploiement, donc celui qui compte le plus.
#[tokio::test]
async fn the_moderation_queue_does_not_exist_until_someone_opens_it() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));

    for jeton in [None, Some("n'importe quoi"), Some(ADMIN_TOKEN)] {
        let (status, _) = call(&app, request("GET", "/api/v1/admin/reports", jeton, None)).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "sans ADMIN_TOKEN, la route ne doit rien révéler d'elle-même"
        );
    }
}

/// Ouverte, elle refuse quand même tout ce qui n'est pas le jeton — et du
/// même `404`, pour ne pas confirmer qu'on a trouvé la bonne adresse.
#[tokio::test]
async fn an_open_queue_still_refuses_everything_but_the_token() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state_with_admin(db));

    for jeton in [None, Some("jeton-faux-mais-de-trente-deux-c"), Some("")] {
        let (status, _) = call(&app, request("GET", "/api/v1/admin/reports", jeton, None)).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "jeton accepté à tort : {jeton:?}"
        );
    }

    let (status, body) = call(
        &app,
        request("GET", "/api/v1/admin/reports", Some(ADMIN_TOKEN), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "le bon jeton doit ouvrir : {body}");
}

/// Un signalement écrit doit se retrouver dans la file, avec de quoi décider.
#[tokio::test]
async fn a_report_reaches_the_queue_with_what_it_takes_to_decide() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state_with_admin(db));
    let here = private_cluster();

    let (plaignant, plaignant_id) =
        candidate(&app, "mod-plaignant", MODERATION, "woman", Some(here)).await;
    let (_, vise_id) = candidate(&app, "mod-vise", MODERATION, "man", Some(here)).await;

    signale(
        &app,
        &plaignant,
        vise_id,
        "Messages insistants après un refus",
    )
    .await;

    let (status, body) = call(
        &app,
        request("GET", "/api/v1/admin/reports", Some(ADMIN_TOKEN), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let ligne = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["reported"]["id"] == vise_id.to_string())
        .expect("le signalement doit apparaître");

    assert_eq!(ligne["reason"], "Messages insistants après un refus");
    assert_eq!(ligne["reporter"]["id"], plaignant_id.to_string());
    assert_eq!(ligne["reporter"]["display_name"], "mod-plaignant");
    assert_eq!(ligne["reported"]["display_name"], "mod-vise");
    assert!(ligne.get("created_at").is_some());
}

/// Le chiffre qui change une décision : un signalement isolé peut être un
/// dépit, cinq personnes différentes sont un motif.
///
/// Les deux compteurs sont séparés délibérément — cinq signalements d'une
/// même personne ne disent pas la même chose que cinq personnes qui
/// signalent. Ce test tomberait si on confondait les deux.
#[tokio::test]
async fn the_queue_separates_a_grudge_from_a_pattern() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state_with_admin(db));
    let here = private_cluster();

    let (_, harceleur) = candidate(&app, "mod-motif", MODERATION_PATTERN, "man", Some(here)).await;
    let (rancunier, _) = candidate(
        &app,
        "mod-rancunier",
        MODERATION_PATTERN,
        "woman",
        Some(here),
    )
    .await;
    let (_, cible_isolee) =
        candidate(&app, "mod-isole", MODERATION_PATTERN, "man", Some(here)).await;

    // Trois personnes différentes signalent le premier.
    for n in 0..3 {
        let (temoin, _) = candidate(
            &app,
            &format!("mod-temoin-{n}"),
            MODERATION_PATTERN,
            "woman",
            Some(here),
        )
        .await;
        signale(&app, &temoin, harceleur, "Harcèlement").await;
    }

    // Une seule personne signale le second, trois fois.
    for n in 0..3 {
        signale(&app, &rancunier, cible_isolee, &format!("Encore {n}")).await;
    }

    let (_, body) = call(
        &app,
        request(
            "GET",
            "/api/v1/admin/reports?limit=200",
            Some(ADMIN_TOKEN),
            None,
        ),
    )
    .await;
    let items = body["items"].as_array().unwrap();

    let motif = items
        .iter()
        .find(|r| r["reported"]["id"] == harceleur.to_string())
        .expect("le signalé par trois personnes");
    let depit = items
        .iter()
        .find(|r| r["reported"]["id"] == cible_isolee.to_string())
        .expect("le signalé trois fois par une seule");

    assert_eq!(motif["reported_total"], 3);
    assert_eq!(motif["distinct_reporters"], 3, "trois personnes distinctes");

    assert_eq!(depit["reported_total"], 3);
    assert_eq!(
        depit["distinct_reporters"], 1,
        "trois signalements d'une même personne n'en font qu'une"
    );
}

/// Un compte supprimé emporte son nom, pas la trace.
///
/// La contrainte en base est `ON DELETE SET NULL` des deux côtés : la ligne
/// survit avec son motif et sa date, mais `reporter_id` et `reported_id`
/// deviennent nuls. C'est délibéré et c'est le bon arbitrage RGPD — garder
/// l'identifiant de quelqu'un qui a demandé son effacement reviendrait à ne
/// pas l'effacer.
///
/// **La contrepartie est réelle et mérite d'être dite** : quelqu'un qui
/// supprime son compte efface son historique de signalements, et peut se
/// réinscrire vierge. La parade n'est pas de garder l'identifiant — ce serait
/// reprendre d'une main ce qu'on donne de l'autre — mais de décider avant la
/// suppression, ce qui suppose que quelqu'un lise la file. C'est précisément
/// ce que cette route rend possible.
///
/// J'avais écrit ce test à l'envers, en supposant que le signalement gardait
/// sa cible. Il vérifie maintenant ce que le code fait vraiment, et pourquoi.
#[tokio::test]
async fn a_deleted_account_takes_its_name_but_not_the_record() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state_with_admin(db));
    let here = private_cluster();

    let (plaignant, _) = candidate(&app, "mod-part", MODERATION_SHUT, "woman", Some(here)).await;
    let (parti, parti_id) = candidate(&app, "mod-parti", MODERATION_SHUT, "man", Some(here)).await;

    let motif = format!("Propos déplacés {}", uuid_like());
    signale(&app, &plaignant, parti_id, &motif).await;

    // Avant la suppression, la file le nomme.
    let (_, avant) = call(
        &app,
        request(
            "GET",
            "/api/v1/admin/reports?limit=200",
            Some(ADMIN_TOKEN),
            None,
        ),
    )
    .await;
    assert!(
        avant["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["reported"]["id"] == parti_id.to_string()),
        "le signalement doit nommer sa cible tant qu'elle existe"
    );

    let (status, _) = call(&app, request("DELETE", "/api/v1/me", Some(&parti), None)).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "la suppression de compte répond 200"
    );

    let (_, apres) = call(
        &app,
        request(
            "GET",
            "/api/v1/admin/reports?limit=200",
            Some(ADMIN_TOKEN),
            None,
        ),
    )
    .await;
    let items = apres["items"].as_array().unwrap();

    let ligne = items
        .iter()
        .find(|r| r["reason"] == motif)
        .expect("la ligne doit survivre au compte");

    assert!(
        ligne["reported"].is_null(),
        "la cible doit être effacée, pas conservée : {ligne}"
    );
    assert!(
        !items
            .iter()
            .any(|r| r["reported"]["id"] == parti_id.to_string()),
        "plus aucune ligne ne doit porter l'identifiant du compte supprimé"
    );
}
