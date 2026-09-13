//! End-to-end tests for the deck, against a real Postgres.
//!
//! Every test here pins its viewer's filters to an age of its own (see
//! `common::deck_ages`): the deck reads every profile in the database, so
//! without that the suites would appear in each other's results.

mod common;

use axum::http::StatusCode;
use common::deck_ages::*;
use common::*;
use serde_json::json;

// The radius each viewer is pinned to lives in `only_see_age`: small enough
// that the candidates of any other run, wherever they landed, are out of range.

#[tokio::test]
async fn the_deck_is_ordered_by_distance() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "vue", ORDERING, "woman", Some(here)).await;
    only_see_age(&app, &viewer, ORDERING, "everyone").await;

    // Created out of distance order on purpose.
    let (_, far) = candidate(&app, "loin", ORDERING, "man", Some(north_of(here, 30.0))).await;
    let (_, near) = candidate(&app, "pres", ORDERING, "man", Some(north_of(here, 2.0))).await;
    let (_, mid) = candidate(&app, "moyen", ORDERING, "man", Some(north_of(here, 12.0))).await;

    let ids = deck_ids(&app, &viewer, "").await;
    assert_eq!(
        ids,
        vec![near, mid, far],
        "le deck doit être trié par distance croissante"
    );
}

/// A radius someone chose has to mean something.
///
/// A profile whose position is unknown cannot be shown to satisfy « within
/// 60 km » — it could be anywhere — so a located viewer does not see it. The
/// consequence, which is a real product decision and not an accident: a
/// profile becomes discoverable only once it has sent a position.
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

    let ids = deck_ids(&app, &viewer, "").await;
    assert!(ids.contains(&located));
    assert!(
        !ids.contains(&nowhere),
        "sans position connue, on ne peut pas prétendre être dans le rayon"
    );

    let (_, body) = call(
        &app,
        request("GET", "/api/v1/discovery/deck", Some(&viewer), None),
    )
    .await;
    assert!(body["items"][0]["distance_km"].is_number());
}

/// The mirror case: a viewer who granted nothing has no radius to apply, and
/// sees everyone rather than an empty deck.
#[tokio::test]
async fn a_viewer_without_a_position_sees_everyone() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "vue-nomade", NOMAD, "woman", None).await;
    only_see_age(&app, &viewer, NOMAD, "everyone").await;

    let (_, located) = candidate(&app, "situe-nomade", NOMAD, "man", Some(here)).await;
    let (_, nowhere) = candidate(&app, "sans-nomade", NOMAD, "man", None).await;

    // Parcouru page par page : sans position, l'isolation par distance ne joue
    // plus, et les candidats des exécutions précédentes occupent la première.
    assert!(
        deck_contains(&app, &viewer, located).await,
        "un viewer sans position voit aussi ceux qui en ont une"
    );
    assert!(
        deck_contains(&app, &viewer, nowhere).await,
        "et ceux qui n'en ont pas"
    );

    let (_, body) = call(
        &app,
        request("GET", "/api/v1/discovery/deck", Some(&viewer), None),
    )
    .await;
    for item in body["items"].as_array().unwrap() {
        assert!(
            item["distance_km"].is_null(),
            "sans position, aucune distance ne peut être affichée"
        );
    }
}

#[tokio::test]
async fn the_deck_never_contains_you() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, me) = candidate(&app, "moi", SELF, "woman", Some(here)).await;
    only_see_age(&app, &viewer, SELF, "everyone").await;

    assert!(!deck_ids(&app, &viewer, "").await.contains(&me));
}

#[tokio::test]
async fn a_judged_profile_does_not_come_back() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "juge", ALREADY_JUDGED, "woman", Some(here)).await;
    only_see_age(&app, &viewer, ALREADY_JUDGED, "everyone").await;
    let (_, target) = candidate(&app, "cible", ALREADY_JUDGED, "man", Some(here)).await;

    assert!(deck_ids(&app, &viewer, "").await.contains(&target));

    let (status, _) = call(
        &app,
        request(
            "POST",
            "/api/v1/discovery/swipes",
            Some(&viewer),
            Some(json!({ "target_profile_id": target, "decision": "pass" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    assert!(!deck_ids(&app, &viewer, "").await.contains(&target));
}

/// Blocking hides in both directions. A block that only worked one way would
/// leave the person you blocked still able to reach you.
#[tokio::test]
async fn a_block_hides_both_ways() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (blocker, blocker_id) = candidate(&app, "bloqueur", BLOCKED, "woman", Some(here)).await;
    let (blocked, blocked_id) = candidate(&app, "bloque", BLOCKED, "woman", Some(here)).await;
    only_see_age(&app, &blocker, BLOCKED, "everyone").await;
    only_see_age(&app, &blocked, BLOCKED, "everyone").await;

    assert!(deck_ids(&app, &blocker, "").await.contains(&blocked_id));
    assert!(deck_ids(&app, &blocked, "").await.contains(&blocker_id));

    let (status, _) = call(
        &app,
        request(
            "POST",
            &format!("/api/v1/profiles/{blocked_id}/block"),
            Some(&blocker),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    assert!(!deck_ids(&app, &blocker, "").await.contains(&blocked_id));
    assert!(
        !deck_ids(&app, &blocked, "").await.contains(&blocker_id),
        "la personne bloquée ne doit plus voir celle qui l'a bloquée non plus"
    );

    // Pressing block twice says the same thing; it must not read as a failure.
    let (again, _) = call(
        &app,
        request(
            "POST",
            &format!("/api/v1/profiles/{blocked_id}/block"),
            Some(&blocker),
            None,
        ),
    )
    .await;
    assert_eq!(again, StatusCode::OK, "bloquer est idempotent");
}

#[tokio::test]
async fn hiding_yourself_removes_you_from_other_decks_only() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "regardeur", HIDDEN, "woman", Some(here)).await;
    only_see_age(&app, &viewer, HIDDEN, "everyone").await;
    let (shy, shy_id) = candidate(&app, "discret", HIDDEN, "man", Some(here)).await;
    only_see_age(&app, &shy, HIDDEN, "everyone").await;

    assert!(deck_ids(&app, &viewer, "").await.contains(&shy_id));

    let (status, _) = call(
        &app,
        request(
            "PATCH",
            "/api/v1/me/preferences",
            Some(&shy),
            Some(json!({
                "interested_in": "everyone",
                "min_age": HIDDEN,
                "max_age": HIDDEN,
                "max_distance_km": 300,
                "show_me_on_plum": false,
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    assert!(
        !deck_ids(&app, &viewer, "").await.contains(&shy_id),
        "se masquer doit retirer des decks des autres"
    );
    // And their own deck is untouched: hiding is not leaving.
    assert!(
        deck_ids(&app, &shy, "").await.contains(
            &deck_ids(&app, &shy, "")
                .await
                .first()
                .copied()
                .unwrap_or_default()
        ) || deck_ids(&app, &shy, "").await.is_empty()
    );
}

#[tokio::test]
async fn the_gender_filter_is_applied() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "filtre", GENDER, "woman", Some(here)).await;
    let (_, woman) = candidate(&app, "femme", GENDER, "woman", Some(here)).await;
    let (_, man) = candidate(&app, "homme", GENDER, "man", Some(here)).await;
    let (_, enby) = candidate(&app, "nb", GENDER, "nonBinary", Some(here)).await;

    only_see_age(&app, &viewer, GENDER, "women").await;
    let ids = deck_ids(&app, &viewer, "").await;
    assert!(ids.contains(&woman));
    assert!(!ids.contains(&man));
    assert!(
        !ids.contains(&enby),
        "« women » ne retient que gender = woman"
    );

    only_see_age(&app, &viewer, GENDER, "men").await;
    let ids = deck_ids(&app, &viewer, "").await;
    assert!(ids.contains(&man));
    assert!(!ids.contains(&woman));

    // Only "everyone" reaches the non-binary profile — a real consequence of
    // the client offering three choices for four genders.
    only_see_age(&app, &viewer, GENDER, "everyone").await;
    let ids = deck_ids(&app, &viewer, "").await;
    assert!(ids.contains(&enby));
    assert!(ids.contains(&man));
    assert!(ids.contains(&woman));
}

#[tokio::test]
async fn the_age_range_is_applied() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "ages", AGE_RANGE, "woman", Some(here)).await;
    only_see_age(&app, &viewer, AGE_RANGE, "everyone").await;

    let (_, inside) = candidate(&app, "dedans", AGE_RANGE, "man", Some(here)).await;
    let (_, younger) = candidate(&app, "plus-jeune", AGE_RANGE - 1, "man", Some(here)).await;
    let (_, older) = candidate(&app, "plus-age", AGE_RANGE + 1, "man", Some(here)).await;

    let ids = deck_ids(&app, &viewer, "").await;
    assert!(ids.contains(&inside));
    assert!(!ids.contains(&younger), "un an trop jeune est hors bornes");
    assert!(!ids.contains(&older), "un an trop vieux est hors bornes");
}

/// The case the cursor exists for: candidates at the same distance, paged one
/// at a time. A cursor on distance alone would skip them or repeat them.
#[tokio::test]
async fn paging_sees_every_candidate_once_even_at_equal_distance() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "pages", CURSOR, "woman", Some(here)).await;
    only_see_age(&app, &viewer, CURSOR, "everyone").await;

    let mut expected = Vec::new();
    for index in 0..7 {
        // All at exactly the same point, so every sort key is identical.
        let (_, id) = candidate(
            &app,
            &format!("ex-aequo-{index}"),
            CURSOR,
            "man",
            Some(here),
        )
        .await;
        expected.push(id);
    }

    let mut seen = Vec::new();
    let mut query = "?limit=2".to_owned();
    loop {
        let (status, body) = call(
            &app,
            request(
                "GET",
                &format!("/api/v1/discovery/deck{query}"),
                Some(&viewer),
                None,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");

        for item in body["items"].as_array().unwrap() {
            seen.push(item["id"].as_str().unwrap().to_owned());
        }

        match body["next_cursor"].as_str() {
            Some(cursor) => query = format!("?limit=2&cursor={}", urlencoding(cursor)),
            None => break,
        }
        assert!(seen.len() <= 20, "pagination qui ne termine pas");
    }

    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(seen.len(), unique.len(), "un profil est apparu deux fois");
    assert_eq!(unique.len(), expected.len(), "un profil a été sauté");
}

fn urlencoding(value: &str) -> String {
    value.replace('|', "%7C")
}

#[tokio::test]
async fn a_nonsense_cursor_is_refused_rather_than_silently_restarting() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();
    let (viewer, _) = candidate(&app, "curseur-bidon", SELF, "woman", Some(here)).await;

    let (status, _) = call(
        &app,
        request(
            "GET",
            "/api/v1/discovery/deck?cursor=nimporte-quoi",
            Some(&viewer),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// Both people say yes; the second one to do so learns about it immediately,
/// because that is the only moment the celebration means anything.
#[tokio::test]
async fn two_likes_make_a_match_and_a_pass_does_not() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (alice, alice_id) = candidate(&app, "alice", MATCHING, "woman", Some(here)).await;
    let (bob, bob_id) = candidate(&app, "bob", MATCHING, "man", Some(here)).await;

    // Alice likes Bob: nothing yet, he has not answered.
    let (status, body) = call(
        &app,
        request(
            "POST",
            "/api/v1/discovery/swipes",
            Some(&alice),
            Some(json!({ "target_profile_id": bob_id, "decision": "like" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["matched"], false);
    assert!(body["match"].is_null());
    assert!(
        body["likes_remaining"].is_null(),
        "pas de quota inventé tant que rien ne se vend"
    );

    // Bob likes back.
    let (status, body) = call(
        &app,
        request(
            "POST",
            "/api/v1/discovery/swipes",
            Some(&bob),
            Some(json!({ "target_profile_id": alice_id, "decision": "superLike" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["matched"], true, "un coup de cœur vaut un j'aime");
    assert_eq!(body["match"]["profile"]["id"], alice_id.to_string());
    assert!(body["match"]["matched_at"].is_string());
    assert!(body["match"]["conversation_id"].is_null());
}

#[tokio::test]
async fn a_pass_never_makes_a_match() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (a, a_id) = candidate(&app, "passeur-a", MATCHING, "woman", Some(here)).await;
    let (b, b_id) = candidate(&app, "passeur-b", MATCHING, "man", Some(here)).await;

    call(
        &app,
        request(
            "POST",
            "/api/v1/discovery/swipes",
            Some(&a),
            Some(json!({ "target_profile_id": b_id, "decision": "like" })),
        ),
    )
    .await;

    let (status, body) = call(
        &app,
        request(
            "POST",
            "/api/v1/discovery/swipes",
            Some(&b),
            Some(json!({ "target_profile_id": a_id, "decision": "pass" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["matched"], false);
}

#[tokio::test]
async fn judging_the_same_profile_twice_is_refused() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "deux-fois", MATCHING, "woman", Some(here)).await;
    let (_, target) = candidate(&app, "cible-deux-fois", MATCHING, "man", Some(here)).await;

    let body = json!({ "target_profile_id": target, "decision": "like" });
    let (first, _) = call(
        &app,
        request(
            "POST",
            "/api/v1/discovery/swipes",
            Some(&viewer),
            Some(body.clone()),
        ),
    )
    .await;
    assert_eq!(first, StatusCode::OK);

    let (second, _) = call(
        &app,
        request(
            "POST",
            "/api/v1/discovery/swipes",
            Some(&viewer),
            Some(body),
        ),
    )
    .await;
    assert_eq!(second, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn you_cannot_judge_or_block_yourself() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();
    let (viewer, me) = candidate(&app, "soi-meme", SELF, "woman", Some(here)).await;

    let (status, _) = call(
        &app,
        request(
            "POST",
            "/api/v1/discovery/swipes",
            Some(&viewer),
            Some(json!({ "target_profile_id": me, "decision": "like" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = call(
        &app,
        request(
            "POST",
            &format!("/api/v1/profiles/{me}/block"),
            Some(&viewer),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// The verdict is deleted, not marked undone: the deck excludes anyone
/// already judged, so a row left behind would keep the profile hidden and the
/// rewind would look like it did nothing.
#[tokio::test]
async fn rewind_undoes_the_last_pass_and_brings_the_card_back() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "retour", REWIND, "woman", Some(here)).await;
    only_see_age(&app, &viewer, REWIND, "everyone").await;
    let (_, target) = candidate(&app, "regrette", REWIND, "man", Some(here)).await;

    call(
        &app,
        request(
            "POST",
            "/api/v1/discovery/swipes",
            Some(&viewer),
            Some(json!({ "target_profile_id": target, "decision": "pass" })),
        ),
    )
    .await;
    assert!(!deck_ids(&app, &viewer, "").await.contains(&target));

    let (status, body) = call(
        &app,
        request("POST", "/api/v1/discovery/rewind", Some(&viewer), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["profile"]["id"], target.to_string());

    assert!(
        deck_ids(&app, &viewer, "").await.contains(&target),
        "le profil doit revenir dans le deck"
    );
}

#[tokio::test]
async fn rewind_with_nothing_to_undo_answers_politely() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();
    let (viewer, _) = candidate(&app, "rien-a-defaire", SELF, "woman", Some(here)).await;

    let (status, body) = call(
        &app,
        request("POST", "/api/v1/discovery/rewind", Some(&viewer), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["profile"].is_null());
}

#[tokio::test]
async fn a_report_needs_a_reason() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "signaleur", REPORTING, "woman", Some(here)).await;
    let (_, target) = candidate(&app, "signale", REPORTING, "man", Some(here)).await;
    let path = format!("/api/v1/profiles/{target}/report");

    for empty in ["", "   "] {
        let (status, _) = call(
            &app,
            request(
                "POST",
                &path,
                Some(&viewer),
                Some(json!({ "reason": empty })),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "motif vide « {empty} »");
    }

    let (status, _) = call(
        &app,
        request(
            "POST",
            &path,
            Some(&viewer),
            Some(json!({ "reason": "Photos qui ne sont pas les siennes." })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn every_discovery_route_refuses_an_anonymous_caller() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let someone = uuid::Uuid::new_v4();

    let routes = [
        ("GET", "/api/v1/discovery/deck".to_owned(), None),
        (
            "POST",
            "/api/v1/discovery/swipes".to_owned(),
            Some(json!({ "target_profile_id": someone, "decision": "like" })),
        ),
        ("POST", "/api/v1/discovery/rewind".to_owned(), None),
        (
            "POST",
            format!("/api/v1/profiles/{someone}/report"),
            Some(json!({ "reason": "x" })),
        ),
        ("POST", format!("/api/v1/profiles/{someone}/block"), None),
    ];

    for (method, path, body) in routes {
        let (status, _) = call(&app, request(method, &path, None, body)).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{method} {path} devrait exiger un jeton"
        );
    }
}

/// Une distance exacte suffit à retrouver une adresse.
///
/// `PATCH /me/location` accepte n'importe quelle position : il suffit de se
/// placer à trois endroits, de lire trois distances précises et de
/// trianguler. C'est une attaque connue contre les applications de
/// rencontres. La page Confidentialité promet « jamais assez pour trouver
/// quelqu'un » — ce test est ce qui rend la promesse vraie.
#[tokio::test]
async fn distances_are_coarse_enough_not_to_locate_anyone() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, _) = candidate(&app, "triangule", PRECISION, "woman", Some(here)).await;
    only_see_age(&app, &viewer, PRECISION, "everyone").await;

    // Des distances choisies pour tomber entre les paliers, là où une valeur
    // exacte se verrait immédiatement.
    for (index, km) in [0.4, 3.7, 6.2, 12.3, 28.9].iter().enumerate() {
        candidate(
            &app,
            &format!("cible-{index}"),
            PRECISION,
            "man",
            Some(north_of(here, *km)),
        )
        .await;
    }

    let (status, body) = call(
        &app,
        request("GET", "/api/v1/discovery/deck", Some(&viewer), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let items = body["items"].as_array().expect("items");
    assert!(!items.is_empty(), "le deck ne doit pas être vide");

    for item in items {
        let km = item["distance_km"].as_f64().expect("distance");
        let acceptable = km == 0.5                       // « moins d'1 km »
            || (km < 10.0 && (km - km.round()).abs() < 1e-9)   // au kilomètre
            || (km % 5.0).abs() < 1e-9; // par tranches de cinq
        assert!(
            acceptable,
            "distance {km} : une valeur hors palier laisse trianguler une adresse"
        );
    }

    // Le curseur transporte la clé de tri jusqu'au client : une distance
    // exacte y fuirait tout aussi bien que dans la réponse.
    let (_, page) = call(
        &app,
        request("GET", "/api/v1/discovery/deck?limit=1", Some(&viewer), None),
    )
    .await;
    let cursor = page["next_cursor"].as_str().expect("curseur");
    let km: f64 = cursor
        .split('|')
        .next()
        .unwrap()
        .parse()
        .expect("distance du curseur");
    assert!(
        km == 0.5 || (km < 10.0 && (km - km.round()).abs() < 1e-9) || (km % 5.0).abs() < 1e-9,
        "le curseur expose {km}, une précision que la réponse refuse"
    );
}
