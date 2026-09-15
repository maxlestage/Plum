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

// ---------------------------------------------------------------------------
// Ce qu'on peut *faire* d'un signalement
//
// Lire la file sans pouvoir agir, c'était la même panne d'un cran plus haut :
// quelqu'un signale un harcèlement, un modérateur le lit, et n'a aucun levier.
// Les tests qui suivent tiennent la promesse par ses quatre bouts — le deck,
// la connexion, le renouvellement, l'envoi d'un message — parce que fermer un
// compte à trois endroits sur quatre, c'est ne pas le fermer.
// ---------------------------------------------------------------------------

/// Inscrit quelqu'un et rend aussi son adresse, dont les tests de connexion
/// ont besoin.
async fn compte(
    app: &axum::Router,
    tag: &str,
    genre: &str,
    age: i32,
    ou: (f64, f64),
) -> (String, uuid::Uuid, String) {
    let email = unique_email(tag);
    let (status, body) = call(
        app,
        post(
            "/api/v1/auth/sign-up",
            json!({
                "email": email,
                "password": "motdepasse",
                "display_name": tag,
                "birth_date": birth_date_for(age),
                "gender": genre,
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "inscription de {tag} : {body}");

    let token = body["tokens"]["access_token"].as_str().unwrap().to_owned();
    let id: uuid::Uuid = body["user"]["id"].as_str().unwrap().parse().unwrap();
    let (latitude, longitude) = ou;
    let (status, _) = call(
        app,
        request(
            "PATCH",
            "/api/v1/me/location",
            Some(&token),
            Some(json!({ "latitude": latitude, "longitude": longitude })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "position de {tag}");
    (token, id, email)
}

async fn suspend(app: &axum::Router, cible: uuid::Uuid, motif: &str) -> serde_json::Value {
    let (status, body) = call(
        app,
        request(
            "POST",
            &format!("/api/v1/admin/profiles/{cible}/suspension"),
            Some(ADMIN_TOKEN),
            Some(json!({ "reason": motif })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "suspension : {body}");
    body
}

/// Le levier existe, et il ferme vraiment la porte d'entrée.
///
/// Refusée **après** le mot de passe, pas avant : sinon l'erreur « compte
/// fermé » répondrait à n'importe qui tapant une adresse et dirait au monde
/// qui a été suspendu. Les deux moitiés sont vérifiées ici, et la seconde est
/// celle qu'on oublie.
#[tokio::test]
async fn a_suspended_account_cannot_sign_in_and_is_told_why() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state_with_admin(db));

    let (_, id, email) = compte(
        &app,
        "susp-entree",
        "man",
        SUSPENSION_DOOR,
        private_cluster(),
    )
    .await;
    suspend(&app, id, "Harcèlement signalé par trois personnes").await;

    let (status, body) = call(
        &app,
        post(
            "/api/v1/auth/sign-in",
            json!({ "email": email, "password": "motdepasse" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(
        body["code"], "account_suspended",
        "la personne doit apprendre que son compte est fermé, pas croire \
         qu'elle tape mal son mot de passe : {body}"
    );

    // Et le mauvais mot de passe reste un mauvais mot de passe : la
    // suspension ne doit pas devenir un oracle qui dit qui existe.
    let (status, body) = call(
        &app,
        post(
            "/api/v1/auth/sign-in",
            json!({ "email": email, "password": "pas-le-bon" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "invalid_credentials");
}

/// La porte de derrière : le renouvellement, et ses **deux** verrous.
///
/// Suspendre révoque les jetons de rafraîchissement *et* le renouvellement
/// vérifie la suspension. Ces deux-là se masquent l'un l'autre : écrit d'un
/// seul tenant, ce test restait vert en retirant la vérification, parce que la
/// révocation suffisait à faire échouer l'appel. Il ne prouvait donc rien de
/// ce qu'il annonçait — mesuré en retirant la vérification, pas supposé.
///
/// D'où le second temps, qui remet en base un jeton vivant sur un compte
/// fermé. Ce n'est pas un état artificiel : c'est exactement ce que laisse une
/// course entre un renouvellement en vol, qui écrit son jeton neuf, et une
/// suspension dont le `UPDATE ... WHERE revoked_at IS NULL` est déjà passé.
/// Sans la vérification, ce jeton-là renouvelle indéfiniment.
#[tokio::test]
async fn a_suspended_account_cannot_renew_its_session() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state_with_admin(db.clone()));

    let email = unique_email("susp-renouvellement");
    let (status, body) = call(
        &app,
        post(
            "/api/v1/auth/sign-up",
            json!({
                "email": email,
                "password": "motdepasse",
                "display_name": "susp-renouvellement",
                "birth_date": birth_date_for(SUSPENSION_RENEW),
                "gender": "man",
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let refresh = body["tokens"]["refresh_token"].as_str().unwrap().to_owned();
    let id: uuid::Uuid = body["user"]["id"].as_str().unwrap().parse().unwrap();

    // Le témoin : le même jeton marche avant la décision. Sans lui, le test
    // passerait aussi si le renouvellement était cassé pour tout le monde.
    let (status, body) = call(
        &app,
        post("/api/v1/auth/refresh", json!({ "refresh_token": refresh })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "avant la suspension : {body}");
    let refresh = body["refresh_token"].as_str().unwrap().to_owned();

    suspend(&app, id, "Faux profil").await;

    // Premier verrou : la suspension a révoqué les sessions ouvertes.
    let (status, body) = call(
        &app,
        post("/api/v1/auth/refresh", json!({ "refresh_token": refresh })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "la suspension doit révoquer les sessions ouvertes : {body}"
    );

    // Second verrou, celui que le premier masquait. On rend la vie aux jetons
    // du compte, ce qui reproduit l'état laissé par la course décrite plus
    // haut, et le renouvellement doit refuser quand même.
    ressusciter_les_jetons(&db, id).await;
    let (status, body) = call(
        &app,
        post("/api/v1/auth/refresh", json!({ "refresh_token": refresh })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "un jeton vivant sur un compte fermé doit être refusé pour ce qu'il \
         est, pas seulement parce qu'il a été révoqué : {body}"
    );
    assert_eq!(body["code"], "account_suspended");
}

/// Remet en base un jeton de rafraîchissement vivant sur un compte donné.
///
/// Le raccourci vaut celui de `legacy_block` dans la suite des blocages : il
/// écrit directement l'état qu'une course produit, parce qu'aucune séquence
/// d'appels ne le produit de façon fiable.
async fn ressusciter_les_jetons(db: &sea_orm::DatabaseConnection, owner: uuid::Uuid) {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    plum_server::entities::refresh_token::Entity::update_many()
        .col_expr(
            plum_server::entities::refresh_token::Column::RevokedAt,
            sea_orm::sea_query::Expr::value(None::<chrono::DateTime<chrono::FixedOffset>>),
        )
        .filter(plum_server::entities::refresh_token::Column::UserId.eq(owner))
        .exec(db)
        .await
        .expect("les jetons se relèvent");
}

/// Le quart d'heure qui reste, et l'endroit où il ne doit pas s'appliquer.
///
/// Le jeton d'accès vit quinze minutes et n'est pas vérifié en base à chaque
/// route — c'est le compromis assumé. L'envoi d'un message, lui, paie sa
/// requête : un quart d'heure de messages est exactement ce qu'une suspension
/// pour harcèlement doit empêcher.
#[tokio::test]
async fn a_suspended_account_cannot_keep_writing_with_a_live_token() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state_with_admin(db));

    let ici = private_cluster();
    let (a, a_id, _) = compte(&app, "susp-fil-a", "woman", SUSPENSION_THREAD, ici).await;
    let (b, b_id, _) = compte(&app, "susp-fil-b", "man", SUSPENSION_THREAD, ici).await;

    for (token, cible) in [(&a, b_id), (&b, a_id)] {
        let (status, body) = call(
            &app,
            request(
                "POST",
                "/api/v1/discovery/swipes",
                Some(token),
                Some(json!({ "target_profile_id": cible, "decision": "like" })),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "swipe : {body}");
    }

    let (status, body) = call(&app, request("GET", "/api/v1/matches", Some(&a), None)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let match_id = body["items"][0]["id"].as_str().unwrap().to_owned();

    let (status, body) = call(
        &app,
        request(
            "POST",
            &format!("/api/v1/matches/{match_id}/conversation"),
            Some(&a),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let conversation = body["id"].as_str().unwrap().to_owned();

    let envoyer = |token: String, texte: &'static str| {
        let app = app.clone();
        let conversation = conversation.clone();
        async move {
            call(
                &app,
                request(
                    "POST",
                    &format!("/api/v1/conversations/{conversation}/messages"),
                    Some(&token),
                    Some(json!({ "body": texte, "client_id": uuid::Uuid::new_v4() })),
                ),
            )
            .await
        }
    };

    // Le témoin : b écrit sans difficulté avant la décision.
    let (status, body) = envoyer(b.clone(), "Salut").await;
    assert_eq!(status, StatusCode::OK, "avant la suspension : {body}");

    suspend(&app, b_id, "Messages insistants après un refus").await;

    // Le jeton d'accès de b est toujours valide — c'est tout l'intérêt du test.
    let (status, body) = envoyer(b.clone(), "Réponds-moi").await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "un compte fermé ne doit plus écrire, même avec un jeton encore \
         valide : {body}"
    );
    assert_eq!(body["code"], "account_suspended");

    // Et l'autre personne n'est pas punie pour avoir signalé.
    let (status, body) = envoyer(a.clone(), "Toujours là ?").await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

/// Le profil disparaît des decks — de tous les decks, pas du sien.
#[tokio::test]
async fn a_suspended_profile_leaves_every_deck() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state_with_admin(db));

    let ici = private_cluster();
    let (spectateur, _, _) = compte(&app, "susp-deck-vu", "woman", SUSPENSION_DECK, ici).await;
    let (_, vise_id, _) = compte(&app, "susp-deck-cible", "man", SUSPENSION_DECK, ici).await;
    only_see_age(&app, &spectateur, SUSPENSION_DECK, "men").await;

    // Page après page : la première ne prouve rien, les candidats des autres
    // exécutions la remplissent.
    let visible = |token: String| {
        let app = app.clone();
        async move { deck_contains(&app, &token, vise_id).await }
    };

    assert!(
        visible(spectateur.clone()).await,
        "le témoin : la cible doit d'abord être visible, sinon ce test \
         passerait pour de mauvaises raisons"
    );

    suspend(&app, vise_id, "Photos qui ne sont pas les siennes").await;

    assert!(
        !visible(spectateur.clone()).await,
        "un compte fermé ne doit plus apparaître dans un deck"
    );
}

/// Une suspension se lève, et la trace reste.
///
/// Un historique qui se réécrit ne vaut rien devant une contestation : lever
/// une décision écrit une ligne de plus, elle n'efface pas celle d'avant.
#[tokio::test]
async fn lifting_a_suspension_reopens_the_account_without_erasing_the_record() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state_with_admin(db));

    let (_, id, email) = compte(
        &app,
        "susp-levee",
        "man",
        SUSPENSION_LIFT,
        private_cluster(),
    )
    .await;
    let apres_fermeture = suspend(&app, id, "Erreur de modération à venir").await;
    assert_eq!(apres_fermeture["suspended"], true);
    assert_eq!(apres_fermeture["decisions"], 1);

    let (status, body) = call(
        &app,
        request(
            "DELETE",
            &format!("/api/v1/admin/profiles/{id}/suspension"),
            Some(ADMIN_TOKEN),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["suspended"], false);
    assert!(body["suspended_at"].is_null());
    assert_eq!(
        body["decisions"], 2,
        "la levée s'ajoute à la suspension, elle ne la remplace pas : {body}"
    );

    let (status, body) = call(
        &app,
        post(
            "/api/v1/auth/sign-in",
            json!({ "email": email, "password": "motdepasse" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "le compte doit rouvrir : {body}");
}

/// Une file qui ne se vide pas cesse d'être ouverte au bout de deux semaines.
#[tokio::test]
async fn judging_a_report_takes_it_out_of_the_queue_but_not_out_of_the_record() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state_with_admin(db));

    let ici = private_cluster();
    let (plaignant, _, _) = compte(&app, "juge-plaignant", "woman", JUDGE_QUEUE, ici).await;
    let (_, vise_id, _) = compte(&app, "juge-vise", "man", JUDGE_QUEUE, ici).await;
    signale(&app, &plaignant, vise_id, "Propos déplacés").await;

    let file = |etat: &'static str| {
        let app = app.clone();
        async move {
            let chemin = format!("/api/v1/admin/reports?state={etat}");
            let (status, body) = call(&app, request("GET", &chemin, Some(ADMIN_TOKEN), None)).await;
            assert_eq!(status, StatusCode::OK, "{body}");
            body
        }
    };

    let ouverts = file("open").await;
    let ligne = ouverts["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["reported"]["id"] == vise_id.to_string())
        .expect("le signalement doit d'abord être dans la file")
        .clone();
    let report_id = ligne["id"].as_str().unwrap().to_owned();
    assert_eq!(
        ligne["reported_suspended"], false,
        "la ligne doit dire si le compte est déjà fermé"
    );

    let (status, body) = call(
        &app,
        request(
            "POST",
            &format!("/api/v1/admin/reports/{report_id}/resolution"),
            Some(ADMIN_TOKEN),
            Some(json!({ "resolution": "dismissed", "note": "Malentendu" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["resolution"], "dismissed");

    let encore_ouvert = file("open")
        .await
        .get("items")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["id"] == report_id);
    assert!(
        !encore_ouvert,
        "un signalement jugé doit sortir de la file ouverte"
    );

    let dans_tout = file("all")
        .await
        .get("items")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["id"] == report_id);
    assert!(
        dans_tout,
        "sortir de la file n'est pas disparaître : `state=all` doit le \
         retrouver, avec ce qui a été décidé"
    );
}

/// Rejuger une ligne écraserait la première décision et sa date sans laisser
/// de trace de la première.
#[tokio::test]
async fn a_report_is_judged_once() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state_with_admin(db));

    let ici = private_cluster();
    let (plaignant, _, _) = compte(&app, "deuxfois-plaignant", "woman", JUDGE_ONCE, ici).await;
    let (_, vise_id, _) = compte(&app, "deuxfois-vise", "man", JUDGE_ONCE, ici).await;
    signale(&app, &plaignant, vise_id, "Propos déplacés").await;

    let (_, body) = call(
        &app,
        request("GET", "/api/v1/admin/reports", Some(ADMIN_TOKEN), None),
    )
    .await;
    let report_id = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["reported"]["id"] == vise_id.to_string())
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let juger = |issue: &'static str| {
        let app = app.clone();
        let report_id = report_id.clone();
        async move {
            call(
                &app,
                request(
                    "POST",
                    &format!("/api/v1/admin/reports/{report_id}/resolution"),
                    Some(ADMIN_TOKEN),
                    Some(json!({ "resolution": issue })),
                ),
            )
            .await
        }
    };

    let (status, _) = juger("upheld").await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = juger("dismissed").await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}

/// Une suspension sans motif est la décision qu'on ne peut pas relire six mois
/// plus tard — et c'est exactement celle qui se conteste.
#[tokio::test]
async fn a_suspension_has_to_say_why() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state_with_admin(db));

    let (_, id, _) = compte(
        &app,
        "susp-sans-motif",
        "man",
        SUSPENSION_REASON,
        private_cluster(),
    )
    .await;

    for motif in ["", "   "] {
        let (status, body) = call(
            &app,
            request(
                "POST",
                &format!("/api/v1/admin/profiles/{id}/suspension"),
                Some(ADMIN_TOKEN),
                Some(json!({ "reason": motif })),
            ),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "« {motif} » accepté : {body}"
        );
    }
}

/// Les trois gestes sont derrière le même garde que la lecture. Un levier
/// ouvert à tous serait pire que pas de levier du tout.
#[tokio::test]
async fn the_levers_are_behind_the_same_door_as_the_queue() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state_with_admin(db));
    let (_, id, _) = compte(
        &app,
        "porte-fermee",
        "man",
        SUSPENSION_GUARD,
        private_cluster(),
    )
    .await;

    for (methode, chemin, corps) in [
        (
            "POST",
            format!("/api/v1/admin/profiles/{id}/suspension"),
            Some(json!({ "reason": "peu importe" })),
        ),
        (
            "DELETE",
            format!("/api/v1/admin/profiles/{id}/suspension"),
            None,
        ),
        (
            "POST",
            format!("/api/v1/admin/reports/{}/resolution", uuid::Uuid::new_v4()),
            Some(json!({ "resolution": "dismissed" })),
        ),
    ] {
        for jeton in [None, Some("jeton-faux-mais-de-trente-deux-c")] {
            let (status, body) = call(&app, request(methode, &chemin, jeton, corps.clone())).await;
            assert_eq!(
                status,
                StatusCode::NOT_FOUND,
                "{methode} {chemin} sans le jeton : {body}"
            );
        }
    }
}
