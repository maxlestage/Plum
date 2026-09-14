//! Ce que bloquer quelqu'un doit faire — et ce que ça ne doit pas laisser passer.
//!
//! Bloquer est l'outil qu'on utilise quand on est harcelé. Le retirer du deck
//! ne suffit pas : tant que la conversation reste ouverte, la personne bloquée
//! continue d'écrire et ses messages continuent d'arriver. Cette suite tient
//! la promesse par les deux bouts — le fil se ferme, et il se ferme dans les
//! deux sens.

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use axum::http::StatusCode;
use common::deck_ages::*;
use common::*;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

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

async fn block(app: &axum::Router, token: &str, target: uuid::Uuid) {
    let (status, body) = call(
        app,
        request(
            "POST",
            &format!("/api/v1/profiles/{target}/block"),
            Some(token),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "blocage : {body}");
}

async fn send(app: &axum::Router, token: &str, conversation: &str) -> (StatusCode, Value) {
    call(
        app,
        request(
            "POST",
            &format!("/api/v1/conversations/{conversation}/messages"),
            Some(token),
            Some(json!({ "client_id": uuid::Uuid::new_v4(), "body": "encore moi" })),
        ),
    )
    .await
}

/// Le cœur de la promesse : après un blocage, le fil n'existe plus pour
/// personne — ni pour celui qui a bloqué, ni pour celui qui est bloqué.
#[tokio::test]
async fn blocking_closes_the_conversation_for_both_sides() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let t = thread(&app, "bloc-coupe", BLOCK_CUTS).await;

    // Avant : la conversation vit, des deux côtés.
    for token in [&t.a, &t.b] {
        let (status, _) = send(&app, token, &t.id).await;
        assert_eq!(status, StatusCode::OK, "le fil doit vivre avant le blocage");
    }

    block(&app, &t.a, t.b_id).await;

    // Après : plus de fil. `NotFound` et pas `Forbidden` — confirmer qu'une
    // conversation existe encore renseignerait déjà celui qu'on a bloqué.
    for (who, token) in [("le bloqué", &t.b), ("le bloqueur", &t.a)] {
        let (status, body) = send(&app, token, &t.id).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{who} écrit encore : {body}");

        let (status, body) = call(
            &app,
            request(
                "GET",
                &format!("/api/v1/conversations/{}/messages", t.id),
                Some(token),
                None,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{who} relit encore : {body}");

        let (status, body) = call(
            &app,
            request(
                "POST",
                &format!("/api/v1/matches/{}/conversation", t.match_id),
                Some(token),
                None,
            ),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{who} rouvre le fil : {body}"
        );
    }
}

/// Le fil n'est pas masqué, il est supprimé.
///
/// Les trois autres barrières — le contrôle à l'ouverture, la liste des
/// conversations, la liste des matchs — cachent la même chose et se couvrent
/// l'une l'autre : elles restent vertes même si la suppression disparaît. Ce
/// test est le seul qui regarde la base. Il compte, parce qu'on ne garde pas
/// les messages de quelqu'un dont on vient de dire qu'on ne veut plus rien
/// recevoir de lui.
#[tokio::test]
async fn blocking_deletes_the_match_and_its_messages_rather_than_hiding_them() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db.clone()));
    let t = thread(&app, "bloc-efface", BLOCK_ERASES).await;

    let (status, _) = send(&app, &t.a, &t.id).await;
    assert_eq!(status, StatusCode::OK, "il faut un message à effacer");

    let match_id: uuid::Uuid = t.match_id.parse().unwrap();
    let conversation_id: uuid::Uuid = t.id.parse().unwrap();

    block(&app, &t.a, t.b_id).await;

    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

    let pairs = plum_server::entities::match_pair::Entity::find_by_id(match_id)
        .count(&db)
        .await
        .unwrap();
    assert_eq!(pairs, 0, "le match est resté en base");

    let threads = plum_server::entities::conversation::Entity::find_by_id(conversation_id)
        .count(&db)
        .await
        .unwrap();
    assert_eq!(threads, 0, "la conversation est restée en base");

    let kept = plum_server::entities::message::Entity::find()
        .filter(plum_server::entities::message::Column::ConversationId.eq(conversation_id))
        .count(&db)
        .await
        .unwrap();
    assert_eq!(kept, 0, "les messages sont restés en base");
}

/// Un blocage posé en base, sans passer par la route.
///
/// C'est l'état que la production contient déjà : les blocages enregistrés
/// avant que `POST /profiles/{id}/block` supprime le match ont laissé ce
/// match derrière eux. C'est aussi, à la milliseconde près, l'état d'une
/// course entre un message en vol et un blocage qui vient d'être posé.
///
/// Sans ce raccourci, les trois barrières secondaires ne seraient vérifiées
/// par rien : la suppression du match les masque toutes, et les retirer une à
/// une laissait la suite entièrement verte.
async fn legacy_block(db: &sea_orm::DatabaseConnection, blocker: uuid::Uuid, blocked: uuid::Uuid) {
    use sea_orm::{ActiveModelTrait, Set};
    plum_server::entities::block::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        blocker_id: Set(blocker),
        blocked_id: Set(blocked),
        created_at: Set(chrono::Utc::now().into()),
    }
    .insert(db)
    .await
    .expect("le blocage s'écrit");
}

/// La seconde barrière : le fil se referme même si le match, lui, est resté.
#[tokio::test]
async fn a_block_without_a_deletion_still_closes_the_thread() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db.clone()));
    let t = thread(&app, "bloc-restant", BLOCK_LEFTOVER).await;

    legacy_block(&db, t.a_id, t.b_id).await;

    for (who, token) in [("le bloqué", &t.b), ("le bloqueur", &t.a)] {
        let (status, body) = send(&app, token, &t.id).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{who} écrit encore : {body}");

        let (status, body) = call(
            &app,
            request(
                "POST",
                &format!("/api/v1/matches/{}/conversation", t.match_id),
                Some(token),
                None,
            ),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{who} rouvre le fil : {body}"
        );
    }

    // Et la liste, qui passe par un autre chemin que l'ouverture.
    for (who, token) in [("le bloqué", &t.b), ("le bloqueur", &t.a)] {
        let (_, list) = call(
            &app,
            request("GET", "/api/v1/conversations", Some(token), None),
        )
        .await;
        assert!(
            !list["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["id"] == t.id),
            "{who} voit encore le fil : {list}"
        );
    }
}

/// Et la liste des matchs, qui ne passe par aucun des deux.
#[tokio::test]
async fn a_block_without_a_deletion_still_hides_the_match() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db.clone()));
    let t = thread(&app, "bloc-restant-liste", BLOCK_LEFTOVER_LIST).await;

    legacy_block(&db, t.b_id, t.a_id).await;

    for (who, token, other) in [("le bloqueur", &t.b, t.a_id), ("le bloqué", &t.a, t.b_id)] {
        let (_, matches) = call(&app, request("GET", "/api/v1/matches", Some(token), None)).await;
        assert!(
            !matches["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["profile"]["id"] == other.to_string()),
            "{who} voit encore l'autre dans ses matchs : {matches}"
        );
    }
}

/// La liste des conversations ne doit plus rien montrer non plus : un fil
/// qu'on ne peut pas ouvrir mais qui reste affiché, c'est un blocage qu'on
/// croit raté.
#[tokio::test]
async fn a_blocked_thread_leaves_the_conversation_list() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let t = thread(&app, "bloc-liste", BLOCK_SILENCE).await;

    for token in [&t.a, &t.b] {
        let (_, list) = call(
            &app,
            request("GET", "/api/v1/conversations", Some(token), None),
        )
        .await;
        assert!(
            list["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["id"] == t.id),
            "le fil doit être là avant le blocage"
        );
    }

    block(&app, &t.b, t.a_id).await;

    for (who, token) in [("le bloqueur", &t.b), ("le bloqué", &t.a)] {
        let (_, list) = call(
            &app,
            request("GET", "/api/v1/conversations", Some(token), None),
        )
        .await;
        assert!(
            !list["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["id"] == t.id),
            "{who} voit encore le fil : {list}"
        );
    }
}

/// Et la liste des matchs : c'est le premier écran où l'on irait vérifier que
/// le blocage a bien pris.
#[tokio::test]
async fn a_blocked_person_leaves_the_match_list() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let t = thread(&app, "bloc-matchs", BLOCK_LIST).await;

    block(&app, &t.a, t.b_id).await;

    for (who, token, other) in [("le bloqueur", &t.a, t.b_id), ("le bloqué", &t.b, t.a_id)] {
        let (_, matches) = call(&app, request("GET", "/api/v1/matches", Some(token), None)).await;
        assert!(
            !matches["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["profile"]["id"] == other.to_string()),
            "{who} voit encore l'autre dans ses matchs : {matches}"
        );
    }
}

/// Bloqué dans un sens, refermé dans les deux. Une barrière à sens unique
/// laisserait exactement le harcèlement qu'elle prétend arrêter : il suffirait
/// d'être celui qui bloque en premier pour garder la parole.
#[tokio::test]
async fn the_wall_stands_whichever_side_raised_it() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let t = thread(&app, "bloc-sens", BLOCK_EITHER_WAY).await;

    // C'est `b` qui bloque ; c'est donc `b` qu'on vérifie en premier.
    block(&app, &t.b, t.a_id).await;

    let (status, body) = send(&app, &t.b, &t.id).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "celui qui bloque garde la parole : {body}"
    );
}

/// Bloquer supprime le match, mais pas les verdicts : le deck exclut les
/// profils déjà jugés, et les effacer ferait réapparaître la personne qu'on
/// vient de bloquer.
#[tokio::test]
async fn blocking_does_not_hand_the_deck_back_the_blocked_profile() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let t = thread(&app, "bloc-deck", BLOCK_VERDICTS).await;

    block(&app, &t.a, t.b_id).await;

    for (who, token, other) in [("le bloqueur", &t.a, t.b_id), ("le bloqué", &t.b, t.a_id)] {
        let (_, deck) = call(
            &app,
            request(
                "GET",
                &format!("/api/v1/discovery/deck?limit={}", 50),
                Some(token),
                None,
            ),
        )
        .await;
        assert!(
            !deck["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["id"] == other.to_string()),
            "{who} revoit l'autre dans son deck : {deck}"
        );
    }
}

// --- le direct ---------------------------------------------------------
//
// Le socket demande un vrai port : une mise à niveau WebSocket ne survit pas
// à `tower::oneshot`.

async fn listening(app: axum::Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("un port libre");
    let address = listener.local_addr().expect("l'adresse du serveur");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    address
}

async fn open_socket(address: SocketAddr, token: &str) -> Socket {
    let mut request = format!("ws://{address}/ws")
        .into_client_request()
        .expect("une requête de mise à niveau");
    request.headers_mut().insert(
        "authorization",
        format!("Bearer {token}")
            .parse()
            .expect("un en-tête valide"),
    );
    let (socket, _) = connect_async(request).await.expect("la poignée de main");
    socket
}

/// Vérifie qu'il *ne* se passe rien. Une absence se prouve par l'attente.
async fn stays_quiet(socket: &mut Socket, who: &str) {
    let quiet = tokio::time::timeout(Duration::from_millis(800), socket.next()).await;
    match quiet {
        Ok(Some(Ok(Message::Text(text)))) => panic!("{who} a reçu quelque chose : {text}"),
        Ok(Some(Ok(Message::Binary(bytes)))) => {
            panic!(
                "{who} a reçu quelque chose : {:?}",
                String::from_utf8_lossy(&bytes)
            )
        }
        _ => {}
    }
}

/// Le socket est le chemin qui reste ouvert quand tout le reste est fermé :
/// le relais de frappe ne passe pas par l'API, et il ne lisait rien des
/// blocages. Sans ce garde-fou, la personne bloquée pouvait encore faire
/// clignoter « en train d'écrire » chez celle qui l'avait bloquée.
#[tokio::test]
async fn a_blocked_person_cannot_make_the_other_screen_blink() {
    let Some(db) = database().await else { return };
    let db_for_socket = db.clone();
    let app = plum_server::app(state(db));
    let t = thread(&app, "bloc-frappe", BLOCK_TYPING).await;
    let address = listening(app.clone()).await;

    // Le blocage est posé en base sans supprimer le match : sinon le relais
    // s'arrêterait plus haut, sur la conversation disparue, et ce test serait
    // vert même sans le garde-fou qu'il prétend vérifier.
    legacy_block(&db_for_socket, t.a_id, t.b_id).await;

    let mut blocker = open_socket(address, &t.a).await;
    let mut blocked = open_socket(address, &t.b).await;

    use futures_util::SinkExt;
    blocked
        .send(Message::Text(
            json!({ "type": "typing", "conversation_id": t.id })
                .to_string()
                .into(),
        ))
        .await
        .expect("la trame part");

    stays_quiet(&mut blocker, "celui qui a bloqué").await;

    // Et dans l'autre sens : celui qui a bloqué ne doit pas non plus signaler
    // sa présence à celui qu'il fuit.
    blocker
        .send(Message::Text(
            json!({ "type": "typing", "conversation_id": t.id })
                .to_string()
                .into(),
        ))
        .await
        .expect("la trame part");

    stays_quiet(&mut blocked, "celui qui est bloqué").await;
}
