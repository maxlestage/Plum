//! End-to-end tests for the matches list, against a real Postgres.

mod common;

use axum::http::StatusCode;
use common::deck_ages::*;
use common::*;

/// Fait matcher deux comptes et rend leurs jetons et identifiants.
async fn matched_pair(
    app: &axum::Router,
    tag: &str,
    age: i32,
    here: (f64, f64),
) -> (String, uuid::Uuid, String, uuid::Uuid) {
    let (a, a_id) = candidate(app, &format!("{tag}-a"), age, "woman", Some(here)).await;
    let (b, b_id) = candidate(app, &format!("{tag}-b"), age, "man", Some(here)).await;

    // Un seul message ouvre le fil : il n'y a plus de double oui à orchestrer.
    let (status, body) = write_to(app, &a, b_id, "Premier message").await;
    assert_eq!(status, StatusCode::OK, "premier message : {body}");

    (a, a_id, b, b_id)
}

/// La forme est le contrat : l'écran Matchs décode `Page<Match>`.
#[tokio::test]
async fn a_match_appears_for_both_people_in_the_shape_the_client_decodes() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (a, a_id, b, b_id) = matched_pair(&app, "liste", MATCHING, here).await;

    for (token, expected_other) in [(&a, b_id), (&b, a_id)] {
        let (status, body) = call(&app, request("GET", "/api/v1/matches", Some(token), None)).await;
        assert_eq!(status, StatusCode::OK, "{body}");

        let items = body["items"].as_array().expect("items");
        let found = items
            .iter()
            .find(|m| m["profile"]["id"] == expected_other.to_string())
            .expect("le match doit apparaître des deux côtés");

        for key in ["id", "profile", "matched_at", "conversation_id"] {
            assert!(found.get(key).is_some(), "clé « {key} » absente de {found}");
        }
        assert!(found["matched_at"]
            .as_str()
            .is_some_and(|s| s.contains('T')));
        // Pas encore de messagerie : présent parce que le modèle Swift le
        // déclare optionnel, nul parce qu'il n'y a rien à désigner.
        assert!(found["conversation_id"].is_null());
        assert!(found["profile"]["display_name"].as_str().is_some());
    }
}

/// Laisser passer ne crée rien.
///
/// Ce test remplace « un j'aime sans réponse n'est pas un match », qui n'a
/// plus de situation à décrire : il n'y a plus de j'aime qui attend, et écrire
/// ouvre le fil tout de suite. Ce qui reste à vérifier est l'autre moitié —
/// que l'issue négative, elle, n'ouvre rien.
#[tokio::test]
async fn passing_on_someone_creates_nothing() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (a, _) = candidate(&app, "sans-retour-a", MATCHING, "woman", Some(here)).await;
    let (_, b_id) = candidate(&app, "sans-retour-b", MATCHING, "man", Some(here)).await;
    assert!(selection_contains(&app, &a, b_id).await);

    assert_eq!(pass(&app, &a, b_id).await, StatusCode::OK);

    let (_, body) = call(&app, request("GET", "/api/v1/matches", Some(&a), None)).await;
    let trouve = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["profile"]["id"] == b_id.to_string());
    assert!(!trouve, "laisser passer ne doit ouvrir aucun fil");
}

/// Le cas pour lequel le curseur existe : plusieurs matchs, paginés un par un.
#[tokio::test]
async fn paging_the_matches_sees_each_one_once() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (viewer, viewer_id) = candidate(&app, "pagine", REWIND, "woman", Some(here)).await;

    let mut expected = Vec::new();
    for index in 0..5 {
        let (other, other_id) =
            candidate(&app, &format!("match-{index}"), REWIND, "man", Some(here)).await;
        // Chacun ne cherche que des femmes : sa sélection du jour ne contient
        // donc que l'observatrice, et pas les quatre hommes créés à côté. Sans
        // ça, trois places pour cinq candidats et le tirage laisse dehors la
        // seule personne à qui ce test veut faire écrire.
        only_see_age(&app, &other, REWIND, "women").await;
        // C'est l'autre qui écrit : l'observateur ne peut écrire qu'aux trois
        // personnes de sa sélection, et il en faut cinq ici. Chacun a la
        // sienne, donc chacun peut écrire une fois.
        let (status, body) = write_to(&app, &other, viewer_id, "Bonjour").await;
        assert_eq!(status, StatusCode::OK, "{body}");
        expected.push(other_id.to_string());
    }

    let mut seen = Vec::new();
    let mut query = "?limit=1".to_owned();
    loop {
        let (status, body) = call(
            &app,
            request(
                "GET",
                &format!("/api/v1/matches{query}"),
                Some(&viewer),
                None,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        for item in body["items"].as_array().unwrap() {
            seen.push(item["profile"]["id"].as_str().unwrap().to_owned());
        }
        match body["next_cursor"].as_str() {
            Some(cursor) => query = format!("?limit=1&cursor={}", cursor.replace('|', "%7C")),
            None => break,
        }
        assert!(seen.len() <= 15, "pagination qui ne termine pas");
    }

    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(seen.len(), unique.len(), "un match est apparu deux fois");
    assert_eq!(unique.len(), expected.len(), "un match a été sauté");
}

#[tokio::test]
async fn a_nonsense_cursor_is_refused() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (viewer, _) = candidate(&app, "curseur-match", BAD_CURSOR, "woman", None).await;

    let (status, _) = call(
        &app,
        request(
            "GET",
            "/api/v1/matches?cursor=nimporte-quoi",
            Some(&viewer),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// Défaire un match ne doit pas faire revenir la personne dans le deck : on ne
/// veut plus la voir, on ne recommence pas avec elle.
#[tokio::test]
async fn unmatching_removes_it_for_both_without_reviving_the_card() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (a, _, b, b_id) = matched_pair(&app, "defaire", HIDDEN, here).await;
    only_see_age(&app, &a, HIDDEN, "everyone").await;

    let (_, body) = call(&app, request("GET", "/api/v1/matches", Some(&a), None)).await;
    let match_id = body["items"][0]["id"]
        .as_str()
        .expect("un match")
        .to_owned();

    let (status, _) = call(
        &app,
        request(
            "DELETE",
            &format!("/api/v1/matches/{match_id}"),
            Some(&a),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    for (token, label) in [(&a, "celui qui défait"), (&b, "l'autre")] {
        let (_, body) = call(&app, request("GET", "/api/v1/matches", Some(token), None)).await;
        assert!(
            body["items"].as_array().unwrap().is_empty(),
            "le match doit disparaître pour {label}"
        );
    }

    assert!(
        !selection_ids(&app, &a).await.contains(&b_id),
        "la carte ne doit pas revenir : le verdict tient toujours"
    );
}

#[tokio::test]
async fn you_cannot_unmatch_someone_elses_match() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let here = private_cluster();

    let (a, _, _, _) = matched_pair(&app, "intrus", GENDER, here).await;
    let (_, body) = call(&app, request("GET", "/api/v1/matches", Some(&a), None)).await;
    let match_id = body["items"][0]["id"]
        .as_str()
        .expect("un match")
        .to_owned();

    let (stranger, _) = candidate(&app, "etranger", SELF_ACTIONS, "woman", None).await;
    let (status, _) = call(
        &app,
        request(
            "DELETE",
            &format!("/api/v1/matches/{match_id}"),
            Some(&stranger),
            None,
        ),
    )
    .await;
    // Introuvable plutôt qu'interdit : confirmer l'existence du match
    // renseignerait déjà quelqu'un qui n'a rien à y voir.
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, still) = call(&app, request("GET", "/api/v1/matches", Some(&a), None)).await;
    assert_eq!(
        still["items"].as_array().unwrap().len(),
        1,
        "le match tient"
    );
}

#[tokio::test]
async fn the_matches_routes_refuse_an_anonymous_caller() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let someone = uuid::Uuid::new_v4();

    for (method, path) in [
        ("GET", "/api/v1/matches".to_owned()),
        ("DELETE", format!("/api/v1/matches/{someone}")),
    ] {
        let (status, _) = call(&app, request(method, &path, None, None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
    }
}
