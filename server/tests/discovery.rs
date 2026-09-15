//! La sélection du jour, contre un vrai Postgres.
//!
//! Elle remplace le deck. Ce qui a disparu avec lui : la pagination, le
//! curseur, le retour en arrière, le double oui, et le budget qui bornait ce
//! qu'on pouvait emporter d'un paquet sans fond. Trois profils par jour bornent
//! la récolte bien plus serré que mille ne le faisaient.
//!
//! Ce qui reste, et qui est éprouvé ici : le *tirage* est le même — mêmes
//! écarts, même tri — et c'est de lui que dépendent la distance, les filtres,
//! le blocage et le retrait.
//!
//! Chaque test épingle les critères de son observateur sur un âge qui n'est
//! qu'à lui (voir `common::deck_ages`) : le tirage lit toute la base, et sans
//! ça les suites apparaîtraient dans les résultats les unes des autres.

mod common;

use axum::http::StatusCode;
use common::deck_ages::*;
use common::*;
use serde_json::json;

#[tokio::test]
async fn the_selection_is_ordered_by_distance() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "vue", ORDERING, "woman", Some(here)).await;
    only_see_age(&app, &viewer, ORDERING, "everyone").await;

    // Créés dans le désordre exprès.
    let (_, far) = candidate(&app, "loin", ORDERING, "man", Some(north_of(here, 30.0))).await;
    let (_, near) = candidate(&app, "pres", ORDERING, "man", Some(north_of(here, 2.0))).await;
    let (_, mid) = candidate(&app, "moyen", ORDERING, "man", Some(north_of(here, 12.0))).await;

    assert_eq!(
        selection_ids(&app, &viewer).await,
        vec![near, mid, far],
        "la sélection doit être triée par distance croissante"
    );
}

/// Un rayon choisi doit vouloir dire quelque chose.
///
/// Un profil dont la position est inconnue ne peut pas être montré comme
/// satisfaisant « à moins de 60 km » — il pourrait être n'importe où. La
/// conséquence, qui est une décision de produit et non un accident : on n'est
/// proposé qu'une fois qu'on a envoyé une position.
#[tokio::test]
async fn an_unlocated_profile_is_hidden_from_a_located_viewer() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "vue-sans", NO_POSITION, "woman", Some(here)).await;
    only_see_age(&app, &viewer, NO_POSITION, "everyone").await;

    let (_, located) =
        candidate(&app, "situe", NO_POSITION, "man", Some(north_of(here, 3.0))).await;
    let (_, nowhere) = candidate(&app, "nulle-part", NO_POSITION, "man", None).await;

    let ids = selection_ids(&app, &viewer).await;
    assert!(ids.contains(&located), "le profil situé doit être proposé");
    assert!(
        !ids.contains(&nowhere),
        "un profil sans position ne peut pas satisfaire un rayon"
    );
}

#[tokio::test]
async fn the_selection_never_contains_you() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, me) = candidate(&app, "moi", SELF, "woman", Some(here)).await;
    only_see_age(&app, &viewer, SELF, "everyone").await;
    candidate(&app, "autre", SELF, "man", Some(north_of(here, 1.0))).await;

    assert!(!selection_ids(&app, &viewer).await.contains(&me));
}

/// Ce qui est tranché ne revient pas — et disparaît de la sélection en cours.
///
/// La sélection est stable, pas figée : elle ne se retire pas parce qu'on
/// rouvre l'application, mais elle ne garde pas non plus quelqu'un dont on
/// vient de décider.
#[tokio::test]
async fn a_judged_profile_leaves_the_selection_and_does_not_come_back() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "juge", ALREADY_JUDGED, "woman", Some(here)).await;
    only_see_age(&app, &viewer, ALREADY_JUDGED, "everyone").await;
    let (_, target) = candidate(
        &app,
        "cible",
        ALREADY_JUDGED,
        "man",
        Some(north_of(here, 1.0)),
    )
    .await;

    assert!(selection_contains(&app, &viewer, target).await);
    assert_eq!(pass(&app, &viewer, target).await, StatusCode::OK);
    assert!(
        !selection_contains(&app, &viewer, target).await,
        "un profil qu'on vient de laisser passer reste affiché"
    );
}

#[tokio::test]
async fn a_block_hides_both_ways() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (blocker, blocker_id) = candidate(&app, "bloqueur", BLOCKED, "woman", Some(here)).await;
    let (blocked, blocked_id) =
        candidate(&app, "bloque", BLOCKED, "man", Some(north_of(here, 1.0))).await;
    only_see_age(&app, &blocker, BLOCKED, "everyone").await;
    only_see_age(&app, &blocked, BLOCKED, "everyone").await;

    assert!(selection_contains(&app, &blocker, blocked_id).await);
    assert!(selection_contains(&app, &blocked, blocker_id).await);

    let (status, body) = call(
        &app,
        request(
            "POST",
            &format!("/api/v1/profiles/{blocked_id}/block"),
            Some(&blocker),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    assert!(
        !selection_contains(&app, &blocker, blocked_id).await,
        "la personne bloquée reste visible chez qui l'a bloquée"
    );
    assert!(
        !selection_contains(&app, &blocked, blocker_id).await,
        "bloquer ne coupe que dans un sens"
    );
}

#[tokio::test]
async fn hiding_yourself_removes_you_from_other_selections_only() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "regarde", HIDDEN, "woman", Some(here)).await;
    let (hidden, hidden_id) =
        candidate(&app, "cache", HIDDEN, "man", Some(north_of(here, 1.0))).await;
    only_see_age(&app, &viewer, HIDDEN, "everyone").await;
    only_see_age(&app, &hidden, HIDDEN, "everyone").await;

    assert!(selection_contains(&app, &viewer, hidden_id).await);

    let (status, body) = call(
        &app,
        request(
            "PATCH",
            "/api/v1/me/preferences",
            Some(&hidden),
            Some(json!({
                "interested_in": "everyone",
                "min_age": HIDDEN,
                "max_age": HIDDEN,
                "max_distance_km": 60,
                "show_me_on_plum": false,
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // Le lendemain, donc : la sélection d'aujourd'hui est déjà tirée.
    assert!(
        !selection_contains(&app, &viewer, hidden_id).await,
        "se retirer doit retirer des sélections des autres"
    );
    assert!(
        !selection_ids(&app, &hidden).await.is_empty(),
        "se retirer ne doit rien changer à sa propre sélection"
    );
}

#[tokio::test]
async fn the_gender_filter_is_applied() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "filtre", GENDER, "woman", Some(here)).await;
    only_see_age(&app, &viewer, GENDER, "women").await;

    let (_, femme) = candidate(&app, "femme", GENDER, "woman", Some(north_of(here, 1.0))).await;
    let (_, homme) = candidate(&app, "homme", GENDER, "man", Some(north_of(here, 2.0))).await;

    let ids = selection_ids(&app, &viewer).await;
    assert!(ids.contains(&femme));
    assert!(!ids.contains(&homme));
}

#[tokio::test]
async fn the_age_range_is_applied() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "age", AGE_RANGE, "woman", Some(here)).await;
    only_see_age(&app, &viewer, AGE_RANGE, "everyone").await;

    let (_, dedans) = candidate(&app, "dedans", AGE_RANGE, "man", Some(north_of(here, 1.0))).await;
    let (_, dehors) = candidate(
        &app,
        "dehors",
        AGE_RANGE + 5,
        "man",
        Some(north_of(here, 2.0)),
    )
    .await;

    let ids = selection_ids(&app, &viewer).await;
    assert!(ids.contains(&dedans));
    assert!(!ids.contains(&dehors));
}

// ---------------------------------------------------------------------------
// La sélection elle-même
// ---------------------------------------------------------------------------

/// Trois, et pas un de plus — même quand il y a de quoi en servir dix.
///
/// Ce n'est pas un quota posé à côté du tirage : c'est le tirage. Il n'y a
/// rien à compter, on tire une fois.
#[tokio::test]
async fn the_selection_is_capped_whatever_the_pool_holds() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "plafond", SELECTION_CAP, "woman", Some(here)).await;
    only_see_age(&app, &viewer, SELECTION_CAP, "everyone").await;
    for index in 0..8 {
        candidate(
            &app,
            &format!("foule-{index}"),
            SELECTION_CAP,
            "man",
            Some(north_of(here, 1.0 + f64::from(index))),
        )
        .await;
    }

    let body = selection(&app, &viewer).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 3);
    assert_eq!(body["size"], 3);
}

/// La même sélection le matin et le soir.
///
/// Sans trace en base, chaque ouverture retirerait, et « la sélection du
/// jour » ne voudrait rien dire — on retomberait sur un paquet sans fond
/// servi trois par trois.
#[tokio::test]
async fn the_selection_is_the_same_all_day() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "stable", SELECTION_STABLE, "woman", Some(here)).await;
    only_see_age(&app, &viewer, SELECTION_STABLE, "everyone").await;
    for index in 0..6 {
        candidate(
            &app,
            &format!("stable-{index}"),
            SELECTION_STABLE,
            "man",
            Some(north_of(here, 1.0 + f64::from(index))),
        )
        .await;
    }

    let premier = selection_ids(&app, &viewer).await;
    assert_eq!(premier.len(), 3);
    for _ in 0..3 {
        assert_eq!(
            selection_ids(&app, &viewer).await,
            premier,
            "rouvrir l'application ne doit pas retirer"
        );
    }
}

/// Écrire ouvre un fil, tout de suite, sans attendre de réciproque.
///
/// C'est le cœur du changement. Il n'y a plus de « j'aime » qui espère son
/// double, donc plus d'écran « c'est un match » : une conversation existe
/// parce que quelqu'un a écrit quelque chose.
#[tokio::test]
async fn writing_opens_a_thread_without_waiting_for_anyone() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (moi, _) = candidate(&app, "ecrit", WRITING, "woman", Some(here)).await;
    let (lui, lui_id) = candidate(&app, "recoit", WRITING, "man", Some(north_of(here, 1.0))).await;
    only_see_age(&app, &moi, WRITING, "everyone").await;

    let (status, message) =
        write_to(&app, &moi, lui_id, "Votre deuxième phrase m'a fait rire.").await;
    assert_eq!(status, StatusCode::OK, "{message}");
    assert_eq!(message["body"], "Votre deuxième phrase m'a fait rire.");

    // L'autre le trouve dans ses conversations, avec le message dedans.
    let (status, fils) = call(
        &app,
        request("GET", "/api/v1/conversations", Some(&lui), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{fils}");
    let fil = fils["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == message["conversation_id"])
        .expect("le fil doit apparaître chez la personne à qui on a écrit");
    assert_eq!(
        fil["last_message"]["body"],
        "Votre deuxième phrase m'a fait rire."
    );
}

/// On n'écrit qu'aux personnes de sa sélection.
///
/// C'est *la* limite, et elle n'est pas un quota à côté : sans elle, l'adresse
/// accepterait n'importe quel identifiant et l'envoi en masse serait trivial.
#[tokio::test]
async fn you_can_only_write_to_someone_in_your_selection() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (moi, _) = candidate(&app, "hors-sel", OUT_OF_SELECTION, "woman", Some(here)).await;
    only_see_age(&app, &moi, OUT_OF_SELECTION, "everyone").await;
    for index in 0..4 {
        candidate(
            &app,
            &format!("hors-{index}"),
            OUT_OF_SELECTION,
            "man",
            Some(north_of(here, 1.0 + f64::from(index))),
        )
        .await;
    }

    let proposes = selection_ids(&app, &moi).await;
    assert_eq!(proposes.len(), 3);

    // Le témoin : on écrit sans difficulté à quelqu'un de la sélection.
    let (status, body) = write_to(&app, &moi, proposes[0], "Bonjour").await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // Le quatrième existe, est à portée, correspond aux critères — et n'a pas
    // été proposé aujourd'hui.
    let (viewer_bis, _) = candidate(&app, "temoin", OUT_OF_SELECTION, "woman", Some(here)).await;
    only_see_age(&app, &viewer_bis, OUT_OF_SELECTION, "everyone").await;
    let tous = selection_ids(&app, &viewer_bis).await;
    let hors = tous
        .iter()
        .find(|id| !proposes.contains(id))
        .copied()
        .expect("il reste quelqu'un hors de la sélection");

    let (status, body) = write_to(&app, &moi, hors, "Et vous ?").await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "écrire à quelqu'un qu'on ne nous a pas proposé doit être refusé : {body}"
    );
}

#[tokio::test]
async fn an_empty_first_message_is_refused() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (moi, _) = candidate(&app, "vide", EMPTY_WRITE, "woman", Some(here)).await;
    let (_, lui) = candidate(
        &app,
        "vide-cible",
        EMPTY_WRITE,
        "man",
        Some(north_of(here, 1.0)),
    )
    .await;
    only_see_age(&app, &moi, EMPTY_WRITE, "everyone").await;
    assert!(selection_contains(&app, &moi, lui).await);

    for corps in ["", "   ", "\n\t "] {
        let (status, body) = write_to(&app, &moi, lui, corps).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "« {corps} » accepté : {body}"
        );
    }
}

#[tokio::test]
async fn deciding_twice_is_refused() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (moi, _) = candidate(&app, "deux-fois", DECIDE_TWICE, "woman", Some(here)).await;
    let (_, lui) = candidate(
        &app,
        "deux-fois-cible",
        DECIDE_TWICE,
        "man",
        Some(north_of(here, 1.0)),
    )
    .await;
    only_see_age(&app, &moi, DECIDE_TWICE, "everyone").await;
    assert!(selection_contains(&app, &moi, lui).await);

    assert_eq!(pass(&app, &moi, lui).await, StatusCode::OK);
    assert_eq!(
        pass(&app, &moi, lui).await,
        StatusCode::CONFLICT,
        "laisser passer deux fois la même personne doit être refusé"
    );
}

#[tokio::test]
async fn you_cannot_decide_on_or_block_yourself() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (moi, mon_id) = candidate(&app, "soi", SELF_ACTIONS, "woman", Some(here)).await;

    assert_eq!(pass(&app, &moi, mon_id).await, StatusCode::BAD_REQUEST);
    let (status, _) = write_to(&app, &moi, mon_id, "Coucou").await;
    assert_ne!(status, StatusCode::OK, "s'écrire à soi-même");

    let (status, _) = call(
        &app,
        request(
            "POST",
            &format!("/api/v1/profiles/{mon_id}/block"),
            Some(&moi),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_report_needs_a_reason() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (moi, _) = candidate(&app, "signale", REPORTING, "woman", Some(here)).await;
    let (_, cible) = candidate(
        &app,
        "signale-cible",
        REPORTING,
        "man",
        Some(north_of(here, 1.0)),
    )
    .await;

    for motif in ["", "   "] {
        let (status, body) = call(
            &app,
            request(
                "POST",
                &format!("/api/v1/profiles/{cible}/report"),
                Some(&moi),
                Some(json!({ "reason": motif })),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }

    let (status, body) = call(
        &app,
        request(
            "POST",
            &format!("/api/v1/profiles/{cible}/report"),
            Some(&moi),
            Some(json!({ "reason": "Photos qui ne sont pas les siennes" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn every_discovery_route_refuses_an_anonymous_caller() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let quelconque = uuid::Uuid::new_v4();

    for (methode, chemin, corps) in [
        ("GET", "/api/v1/discovery/selection".to_owned(), None),
        (
            "POST",
            format!("/api/v1/profiles/{quelconque}/write"),
            Some(json!({ "body": "Bonjour" })),
        ),
        ("POST", format!("/api/v1/profiles/{quelconque}/pass"), None),
        (
            "POST",
            format!("/api/v1/profiles/{quelconque}/report"),
            Some(json!({ "reason": "peu importe" })),
        ),
        ("POST", format!("/api/v1/profiles/{quelconque}/block"), None),
    ] {
        let (status, body) = call(&app, request(methode, &chemin, None, corps)).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{methode} {chemin} sans jeton : {body}"
        );
    }
}

/// Une distance exacte suffit à retrouver une adresse : il suffit de se placer
/// à trois endroits, de lire trois distances précises et de trianguler. C'est
/// une attaque connue contre les applications de rencontres. La page
/// Confidentialité promet « jamais assez pour trouver quelqu'un » — ce test
/// est ce qui rend la promesse vraie.
///
/// Le curseur qui transportait la même fuite a disparu avec la pagination.
#[tokio::test]
async fn distances_are_coarse_enough_not_to_locate_anyone() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "triangule", PRECISION, "woman", Some(here)).await;
    only_see_age(&app, &viewer, PRECISION, "everyone").await;

    // Des distances choisies pour tomber entre les paliers, là où une valeur
    // exacte se verrait immédiatement.
    for (index, km) in [0.4, 3.7, 6.2].iter().enumerate() {
        candidate(
            &app,
            &format!("cible-{index}"),
            PRECISION,
            "man",
            Some(north_of(here, *km)),
        )
        .await;
    }

    let body = selection(&app, &viewer).await;
    let items = body["items"].as_array().expect("items");
    assert!(!items.is_empty(), "la sélection ne doit pas être vide");

    for item in items {
        let km = item["distance_km"].as_f64().expect("distance");
        let acceptable = km == 0.5                             // « moins d'1 km »
            || (km < 10.0 && (km - km.round()).abs() < 1e-9)   // au kilomètre
            || (km % 5.0).abs() < 1e-9; // par tranches de cinq
        assert!(
            acceptable,
            "distance {km} : une valeur hors palier laisse trianguler une adresse"
        );
    }
}
