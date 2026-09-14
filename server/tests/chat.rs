//! End-to-end tests for conversations and messages, against a real Postgres.

mod common;

use axum::http::StatusCode;
use common::deck_ages::*;
use common::*;
use serde_json::json;

struct Thread {
    a: String,
    a_id: uuid::Uuid,
    b: String,
    b_id: uuid::Uuid,
    match_id: String,
    id: String,
}

/// Deux comptes qui ont matché, et leur conversation ouverte.
async fn thread(app: &axum::Router, tag: &str, age: i32) -> Thread {
    let here = private_cluster();
    let (a, a_id) = candidate(app, &format!("{tag}-a"), age, "woman", Some(here)).await;
    let (b, b_id) = candidate(app, &format!("{tag}-b"), age, "man", Some(here)).await;

    for (token, target) in [(&a, b_id), (&b, a_id)] {
        call(
            app,
            request(
                "POST",
                "/api/v1/discovery/swipes",
                Some(token),
                Some(json!({ "target_profile_id": target, "decision": "like" })),
            ),
        )
        .await;
    }

    let (_, matches) = call(app, request("GET", "/api/v1/matches", Some(&a), None)).await;
    let match_id = matches["items"][0]["id"]
        .as_str()
        .expect("un match")
        .to_owned();

    let (status, body) = call(
        app,
        request(
            "POST",
            &format!("/api/v1/matches/{match_id}/conversation"),
            Some(&a),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "ouverture : {body}");

    Thread {
        a,
        a_id,
        b,
        b_id,
        match_id,
        id: body["id"].as_str().expect("conversation").to_owned(),
    }
}

async fn send(
    app: &axum::Router,
    token: &str,
    conversation: &str,
    body: &str,
) -> serde_json::Value {
    let (status, payload) = call(
        app,
        request(
            "POST",
            &format!("/api/v1/conversations/{conversation}/messages"),
            Some(token),
            Some(json!({ "client_id": uuid::Uuid::new_v4(), "body": body })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "envoi : {payload}");
    payload
}

/// La forme est le contrat : l'écran décode `Conversation` et `Message`.
#[tokio::test]
async fn a_conversation_comes_back_in_the_shape_the_client_decodes() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let t = thread(&app, "forme", MATCHING).await;

    let (status, body) = call(
        &app,
        request("GET", "/api/v1/conversations", Some(&t.a), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let found = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == t.id)
        .expect("la conversation doit apparaître");

    for key in [
        "id",
        "match_id",
        "participant",
        "last_message",
        "unread_count",
        "updated_at",
    ] {
        assert!(found.get(key).is_some(), "clé « {key} » absente de {found}");
    }
    assert_eq!(found["match_id"], t.match_id);
    assert_eq!(found["participant"]["id"], t.b_id.to_string());
    assert!(found["last_message"].is_null(), "rien n'a encore été dit");
    assert_eq!(found["unread_count"], 0);
}

/// Deux appareils qui ouvrent l'écran en même temps ne doivent pas créer deux
/// fils : la discussion serait coupée en deux moitiés invisibles l'une à
/// l'autre.
#[tokio::test]
async fn opening_a_conversation_twice_returns_the_same_one() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let t = thread(&app, "idempotent", REWIND).await;

    // Depuis l'autre côté du match, cette fois.
    let (status, again) = call(
        &app,
        request(
            "POST",
            &format!("/api/v1/matches/{}/conversation", t.match_id),
            Some(&t.b),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        again["id"], t.id,
        "une conversation par match, et une seule"
    );
    // Et chacun voit l'autre comme participant.
    assert_eq!(again["participant"]["id"], t.a_id.to_string());
}

#[tokio::test]
async fn a_message_reaches_the_other_side_and_counts_as_unread() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let t = thread(&app, "envoi", HIDDEN).await;

    let sent = send(&app, &t.a, &t.id, "  Bonjour, ça va ?  ").await;
    assert_eq!(sent["body"], "Bonjour, ça va ?", "les blancs sont coupés");
    assert_eq!(sent["sender_id"], t.a_id.to_string());
    assert!(sent["read_at"].is_null());

    // Chez le destinataire : non lu.
    let (_, theirs) = call(
        &app,
        request("GET", "/api/v1/conversations", Some(&t.b), None),
    )
    .await;
    let theirs = theirs["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == t.id)
        .unwrap();
    assert_eq!(theirs["unread_count"], 1);
    assert_eq!(theirs["last_message"]["body"], "Bonjour, ça va ?");

    // Chez l'expéditeur : ses propres messages ne sont jamais « non lus ».
    let (_, mine) = call(
        &app,
        request("GET", "/api/v1/conversations", Some(&t.a), None),
    )
    .await;
    let mine = mine["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == t.id)
        .unwrap();
    assert_eq!(mine["unread_count"], 0, "on ne se doit rien à soi-même");
}

/// Le cas pour lequel `client_id` existe : une connexion coupée entre l'envoi
/// et la réponse fait réessayer, et le message ne doit pas partir deux fois.
#[tokio::test]
async fn resending_the_same_client_id_does_not_duplicate_the_message() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let t = thread(&app, "rejeu", GENDER).await;

    let payload = json!({ "client_id": uuid::Uuid::new_v4(), "body": "Une seule fois." });
    let path = format!("/api/v1/conversations/{}/messages", t.id);

    let (first, one) = call(
        &app,
        request("POST", &path, Some(&t.a), Some(payload.clone())),
    )
    .await;
    let (second, two) = call(&app, request("POST", &path, Some(&t.a), Some(payload))).await;

    assert_eq!(first, StatusCode::OK);
    assert_eq!(second, StatusCode::OK, "un renvoi n'est pas une erreur");
    assert_eq!(one["id"], two["id"], "c'est le même message qui revient");

    let (_, thread_body) = call(&app, request("GET", &path, Some(&t.a), None)).await;
    assert_eq!(
        thread_body["items"].as_array().unwrap().len(),
        1,
        "le fil ne doit contenir qu'un message"
    );
}

#[tokio::test]
async fn an_empty_or_oversized_message_is_refused() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let t = thread(&app, "vide", AGE_RANGE).await;
    let path = format!("/api/v1/conversations/{}/messages", t.id);

    for body in ["", "   ", "\n\t "] {
        let (status, _) = call(
            &app,
            request(
                "POST",
                &path,
                Some(&t.a),
                Some(json!({ "client_id": uuid::Uuid::new_v4(), "body": body })),
            ),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "« {body} » n'est pas un message"
        );
    }

    let long = "a".repeat(2_001);
    let (status, _) = call(
        &app,
        request(
            "POST",
            &path,
            Some(&t.a),
            Some(json!({ "client_id": uuid::Uuid::new_v4(), "body": long })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn marking_read_clears_only_what_the_other_sent() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let t = thread(&app, "lecture", ORDERING).await;

    send(&app, &t.a, &t.id, "Un").await;
    send(&app, &t.a, &t.id, "Deux").await;
    send(&app, &t.b, &t.id, "Trois").await;

    let (status, _) = call(
        &app,
        request(
            "POST",
            &format!("/api/v1/conversations/{}/read", t.id),
            Some(&t.b),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // B a lu les deux messages de A.
    let (_, theirs) = call(
        &app,
        request("GET", "/api/v1/conversations", Some(&t.b), None),
    )
    .await;
    let theirs = theirs["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == t.id)
        .unwrap();
    assert_eq!(theirs["unread_count"], 0);

    // Mais le message de B reste non lu pour A : marquer ses propres messages
    // comme lus reviendrait à répondre à sa place.
    let (_, mine) = call(
        &app,
        request("GET", "/api/v1/conversations", Some(&t.a), None),
    )
    .await;
    let mine = mine["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == t.id)
        .unwrap();
    assert_eq!(
        mine["unread_count"], 1,
        "le message de l'autre reste à lire"
    );
}

/// Le fil remonte vers le passé, page par page, sans doublon ni trou.
#[tokio::test]
async fn paging_the_thread_walks_back_without_repeating_or_skipping() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let t = thread(&app, "pagination", CURSOR).await;

    let mut written = Vec::new();
    for index in 0..7 {
        written.push(
            send(&app, &t.a, &t.id, &format!("message {index}")).await["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }

    let mut seen = Vec::new();
    let mut query = "?limit=2".to_owned();
    loop {
        let (status, body) = call(
            &app,
            request(
                "GET",
                &format!("/api/v1/conversations/{}/messages{query}", t.id),
                Some(&t.a),
                None,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");

        for item in body["items"].as_array().unwrap() {
            seen.push(item["id"].as_str().unwrap().to_owned());
        }
        match body["next_cursor"].as_str() {
            Some(cursor) => query = format!("?limit=2&before={}", cursor.replace('|', "%7C")),
            None => break,
        }
        assert!(seen.len() <= 20, "pagination qui ne termine pas");
    }

    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(seen.len(), unique.len(), "un message est apparu deux fois");
    assert_eq!(unique.len(), written.len(), "un message a été sauté");
}

/// Une conversation à laquelle on n'appartient pas n'existe pas, de son point
/// de vue : en confirmer l'existence renseignerait déjà.
#[tokio::test]
async fn a_stranger_cannot_read_or_write_in_someone_elses_conversation() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let t = thread(&app, "intrus", NO_POSITION).await;
    send(&app, &t.a, &t.id, "Privé.").await;

    let (stranger, _) = candidate(&app, "etranger-chat", SELF_ACTIONS, "woman", None).await;

    for (method, path, body) in [
        (
            "GET",
            format!("/api/v1/conversations/{}/messages", t.id),
            None,
        ),
        (
            "POST",
            format!("/api/v1/conversations/{}/messages", t.id),
            Some(json!({ "client_id": uuid::Uuid::new_v4(), "body": "coucou" })),
        ),
        ("POST", format!("/api/v1/conversations/{}/read", t.id), None),
        (
            "POST",
            format!("/api/v1/matches/{}/conversation", t.match_id),
            None,
        ),
    ] {
        let (status, _) = call(&app, request(method, &path, Some(&stranger), body)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}");
    }

    // Et sa propre liste reste vide.
    let (_, mine) = call(
        &app,
        request("GET", "/api/v1/conversations", Some(&stranger), None),
    )
    .await;
    assert!(mine["items"].as_array().unwrap().is_empty());
}

/// Rien n'encadrait l'envoi : un compte pouvait remplir une conversation — et
/// la base — aussi vite que le réseau le permettait, pendant que le socket
/// poussait tout en direct chez l'autre. L'inondation est la forme de
/// harcèlement la moins chère à produire.
#[tokio::test]
async fn a_flood_of_messages_is_stopped_before_it_fills_the_thread() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let t = thread(&app, "inondation", FLOOD).await;

    // Le quota est de soixante par minute. On en envoie soixante-cinq : le
    // plafond doit tomber avant la fin, et rester tombé.
    let mut refused = 0;
    let mut accepted = 0;
    for n in 0..65 {
        let (status, body) = call(
            &app,
            request(
                "POST",
                &format!("/api/v1/conversations/{}/messages", t.id),
                Some(&t.a),
                Some(json!({ "client_id": uuid::Uuid::new_v4(), "body": format!("message {n}") })),
            ),
        )
        .await;
        match status {
            StatusCode::OK => accepted += 1,
            StatusCode::TOO_MANY_REQUESTS => refused += 1,
            other => panic!("réponse inattendue {other} : {body}"),
        }
    }

    assert_eq!(accepted, 60, "le quota doit laisser passer soixante messages");
    assert_eq!(refused, 5, "et refuser le reste");
}

/// Une conversation ordinaire ne doit jamais rencontrer ce plafond. Un quota
/// qui gêne celui qui écrit normalement est un quota mal réglé.
#[tokio::test]
async fn an_ordinary_exchange_never_meets_the_ceiling() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let t = thread(&app, "echange", ORDINARY_PACE).await;

    // Vingt allers-retours d'affilée : plus vif que tout ce qu'on écrit à la
    // main, et encore loin du plafond.
    for n in 0..20 {
        for token in [&t.a, &t.b] {
            let (status, body) = call(
                &app,
                request(
                    "POST",
                    &format!("/api/v1/conversations/{}/messages", t.id),
                    Some(token),
                    Some(json!({ "client_id": uuid::Uuid::new_v4(), "body": format!("oui {n}") })),
                ),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "tour {n} : {body}");
        }
    }
}

/// Le renvoi après une coupure ne doit rien coûter : ce n'est pas un nouveau
/// message, c'est le même qui arrive enfin. Sans ça, quelqu'un dans un tunnel
/// paierait pour la connexion qu'il n'a pas.
#[tokio::test]
async fn a_retry_after_a_dropped_connection_costs_nothing() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let t = thread(&app, "renvoi", RETRY_COST).await;

    let client_id = uuid::Uuid::new_v4();
    let payload = json!({ "client_id": client_id, "body": "tu es là ?" });

    // Le même message cent fois, comme un client qui réessaie sans relâche.
    for n in 0..100 {
        let (status, body) = call(
            &app,
            request(
                "POST",
                &format!("/api/v1/conversations/{}/messages", t.id),
                Some(&t.a),
                Some(payload.clone()),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "renvoi {n} : {body}");
    }

    // Et le quota est intact : cinquante-neuf nouveaux messages passent encore.
    for n in 0..59 {
        let (status, body) = call(
            &app,
            request(
                "POST",
                &format!("/api/v1/conversations/{}/messages", t.id),
                Some(&t.a),
                Some(json!({ "client_id": uuid::Uuid::new_v4(), "body": format!("suite {n}") })),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "message {n} après les renvois : {body}");
    }
}

#[tokio::test]
async fn the_chat_routes_refuse_an_anonymous_caller() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let someone = uuid::Uuid::new_v4();

    for (method, path, body) in [
        ("GET", "/api/v1/conversations".to_owned(), None),
        (
            "GET",
            format!("/api/v1/conversations/{someone}/messages"),
            None,
        ),
        (
            "POST",
            format!("/api/v1/conversations/{someone}/messages"),
            Some(json!({ "client_id": someone, "body": "x" })),
        ),
        (
            "POST",
            format!("/api/v1/conversations/{someone}/read"),
            None,
        ),
        (
            "POST",
            format!("/api/v1/matches/{someone}/conversation"),
            None,
        ),
    ] {
        let (status, _) = call(&app, request(method, &path, None, body)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
    }
}
