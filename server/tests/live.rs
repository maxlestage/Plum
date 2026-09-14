//! Le socket, contre un vrai serveur écoutant sur un vrai port.
//!
//! Les autres suites passent par `tower::oneshot`, qui appelle le routeur sans
//! réseau. Une mise à niveau WebSocket ne survit pas à ça : il faut une
//! poignée de main, donc un port. Le prix est d'écouter vraiment ; le gain est
//! que ces tests échoueraient si l'authentification, le routage ou le format
//! des évènements se mettaient à diverger de ce que l'application attend.

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use axum::http::StatusCode;
use common::deck_ages::*;
use common::*;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// Combien de temps on accorde à un évènement avant de le déclarer absent.
///
/// Assez pour une machine de CI chargée, assez court pour qu'un test qui
/// attend en vain ne bloque pas la suite.
const PATIENCE: Duration = Duration::from_secs(5);

/// Le serveur sur un port que le système choisit, pour que deux tests
/// simultanés ne se disputent pas le même.
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

/// Le prochain évènement JSON, ou l'échec du test.
///
/// Les trames de service — le battement de cœur, la fermeture — sont sautées :
/// elles arrivent quand elles veulent et ne disent rien de ce qu'on vérifie.
async fn next_event(socket: &mut Socket) -> Value {
    let deadline = tokio::time::Instant::now() + PATIENCE;
    loop {
        let frame = tokio::time::timeout_at(deadline, socket.next())
            .await
            .expect("aucun évènement n'est arrivé à temps")
            .expect("le socket s'est fermé")
            .expect("trame lisible");
        match frame {
            Message::Text(text) => return serde_json::from_str(&text).expect("du JSON"),
            Message::Binary(bytes) => return serde_json::from_slice(&bytes).expect("du JSON"),
            _ => continue,
        }
    }
}

/// Vérifie qu'il *ne* se passe rien. Une absence se prouve par l'attente.
async fn stays_quiet(socket: &mut Socket) {
    let quiet = tokio::time::timeout(Duration::from_millis(600), socket.next()).await;
    if let Ok(Some(Ok(Message::Text(text)))) = quiet {
        panic!("un évènement est arrivé alors qu'il ne devait pas : {text}");
    }
}

struct Thread {
    a: String,
    b: String,
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
    let match_id = matches["items"][0]["id"].as_str().expect("un match");

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
        b,
        id: body["id"].as_str().expect("conversation").to_owned(),
    }
}

async fn send(app: &axum::Router, token: &str, conversation: &str, body: &str) -> Value {
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

/// Sans ça, n'importe qui écouterait la messagerie de tout le monde.
#[tokio::test]
async fn a_socket_without_a_token_is_refused() {
    let Some(db) = database().await else { return };
    let address = listening(plum_server::app(state(db))).await;

    let request = format!("ws://{address}/ws")
        .into_client_request()
        .expect("une requête");
    assert!(
        connect_async(request).await.is_err(),
        "la mise à niveau aurait dû être refusée"
    );
}

#[tokio::test]
async fn a_forged_token_is_refused_like_an_absent_one() {
    let Some(db) = database().await else { return };
    let address = listening(plum_server::app(state(db))).await;

    let mut request = format!("ws://{address}/ws")
        .into_client_request()
        .expect("une requête");
    request.headers_mut().insert(
        "authorization",
        "Bearer eyJhbGciOiJIUzI1NiJ9.bidon.bidon".parse().unwrap(),
    );
    assert!(connect_async(request).await.is_err());
}

/// Le cœur du sujet : un message envoyé par l'API arrive tout seul chez
/// l'autre, sans qu'il ait rien à recharger.
#[tokio::test]
async fn a_message_arrives_by_itself_on_the_other_persons_socket() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let address = listening(app.clone()).await;

    let thread = thread(&app, "direct", LIVE).await;
    let mut listener = open_socket(address, &thread.b).await;

    let written = send(&app, &thread.a, &thread.id, "tu fais quoi ce soir ?").await;
    let event = next_event(&mut listener).await;

    assert_eq!(event["type"], "message");
    assert_eq!(event["message"]["body"], "tu fais quoi ce soir ?");
    // Le même identifiant que la réponse de l'API : le client déduplique
    // là-dessus quand les deux arrivent.
    assert_eq!(event["message"]["id"], written["id"]);
}

/// L'expéditeur ne doit pas recevoir l'écho de son propre message : le client
/// l'a déjà affiché, et le doublon ferait deux bulles.
#[tokio::test]
async fn the_sender_does_not_receive_their_own_message() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let address = listening(app.clone()).await;

    let thread = thread(&app, "echo", LIVE).await;
    let mut mine = open_socket(address, &thread.a).await;

    send(&app, &thread.a, &thread.id, "coucou").await;
    stays_quiet(&mut mine).await;
}

/// L'accusé de lecture remonte vers celui qui avait écrit, et lui seul.
#[tokio::test]
async fn a_read_receipt_reaches_the_person_who_wrote() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let address = listening(app.clone()).await;

    let thread = thread(&app, "lu", LIVE).await;
    let written = send(&app, &thread.a, &thread.id, "tu es là ?").await;

    let mut author = open_socket(address, &thread.a).await;
    let (status, _) = call(
        &app,
        request(
            "POST",
            &format!("/api/v1/conversations/{}/read", thread.id),
            Some(&thread.b),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let event = next_event(&mut author).await;
    assert_eq!(event["type"], "read");
    assert_eq!(event["message_id"], written["id"]);
    assert!(event["read_at"].is_string(), "{event}");
}

/// Relire un fil déjà lu ne doit rien annoncer : sinon chaque ouverture
/// d'écran renverrait l'historique complet des accusés.
#[tokio::test]
async fn reading_a_thread_twice_announces_nothing_the_second_time() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let address = listening(app.clone()).await;

    let thread = thread(&app, "relu", LIVE).await;
    send(&app, &thread.a, &thread.id, "tu es là ?").await;

    let read = || {
        call(
            &app,
            request(
                "POST",
                &format!("/api/v1/conversations/{}/read", thread.id),
                Some(&thread.b),
                None,
            ),
        )
    };

    read().await;
    let mut author = open_socket(address, &thread.a).await;
    read().await;
    stays_quiet(&mut author).await;
}

/// La frappe part du socket, pas de l'API : c'est la seule chose que le client
/// envoie par là.
#[tokio::test]
async fn typing_reaches_the_other_participant() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let address = listening(app.clone()).await;

    let thread = thread(&app, "frappe", LIVE).await;
    let mut writer = open_socket(address, &thread.a).await;
    let mut reader = open_socket(address, &thread.b).await;

    // En binaire, comme `URLSessionWebSocketTask` l'envoie.
    writer
        .send(Message::Binary(
            json!({ "type": "typing", "conversation_id": thread.id })
                .to_string()
                .into_bytes()
                .into(),
        ))
        .await
        .expect("trame envoyée");

    let event = next_event(&mut reader).await;
    assert_eq!(event["type"], "typing");
    assert_eq!(event["conversation_id"], thread.id);
}

/// Le client se tait trois secondes entre deux annonces, mais c'est une
/// politesse qu'un client hostile n'a pas — et chaque annonce coûte deux
/// lectures en base.
#[tokio::test]
async fn a_flood_of_typing_frames_is_relayed_once() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let address = listening(app.clone()).await;

    let thread = thread(&app, "rafale", LIVE).await;
    let mut writer = open_socket(address, &thread.a).await;
    let mut reader = open_socket(address, &thread.b).await;

    let frame = || {
        Message::Text(
            json!({ "type": "typing", "conversation_id": thread.id })
                .to_string()
                .into(),
        )
    };
    for _ in 0..20 {
        writer.send(frame()).await.expect("trame envoyée");
    }

    assert_eq!(next_event(&mut reader).await["type"], "typing");
    stays_quiet(&mut reader).await;
}

/// Sans la vérification en base, deviner un identifiant de conversation
/// suffirait à faire clignoter « en train d'écrire » chez des inconnus.
///
/// Les **deux** participants écoutent, et c'est ce qui fait le test : la table
/// range le match en `(lower_id, upper_id)`, donc un relais non vérifié part
/// vers l'un des deux sans qu'on sache lequel. N'en surveiller qu'un laissait
/// passer une fois sur deux — ce qui, mesuré, passait toujours.
#[tokio::test]
async fn a_stranger_cannot_make_someone_elses_conversation_blink() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let address = listening(app.clone()).await;

    let thread = thread(&app, "intrus", LIVE).await;
    let (stranger, _) = candidate(&app, "intrus-c", LIVE, "woman", None).await;

    let mut intruder = open_socket(address, &stranger).await;
    let mut first = open_socket(address, &thread.a).await;
    let mut second = open_socket(address, &thread.b).await;

    intruder
        .send(Message::Text(
            json!({ "type": "typing", "conversation_id": thread.id })
                .to_string()
                .into(),
        ))
        .await
        .expect("trame envoyée");

    stays_quiet(&mut first).await;
    stays_quiet(&mut second).await;
}

/// Le même garde-fou, vu de l'autre côté : une conversation qui n'existe pas
/// ne doit rien déclencher non plus.
#[tokio::test]
async fn typing_on_an_invented_conversation_goes_nowhere() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let address = listening(app.clone()).await;

    let thread = thread(&app, "fantome", LIVE).await;
    let mut writer = open_socket(address, &thread.a).await;
    let mut reader = open_socket(address, &thread.b).await;

    writer
        .send(Message::Text(
            json!({ "type": "typing", "conversation_id": uuid::Uuid::new_v4() })
                .to_string()
                .into(),
        ))
        .await
        .expect("trame envoyée");

    stays_quiet(&mut reader).await;
}

/// Une trame que le serveur ne comprend pas ne doit pas couper le socket :
/// une version plus récente du client en enverra, et la punir ferait tomber
/// la messagerie entière pour une commande inconnue.
#[tokio::test]
async fn an_unreadable_frame_does_not_close_the_socket() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let address = listening(app.clone()).await;

    let thread = thread(&app, "charabia", LIVE).await;
    let mut listener = open_socket(address, &thread.b).await;

    listener
        .send(Message::Text("ceci n'est pas du JSON".into()))
        .await
        .expect("trame envoyée");

    // Le socket doit toujours servir après.
    send(&app, &thread.a, &thread.id, "toujours là ?").await;
    let event = next_event(&mut listener).await;
    assert_eq!(event["type"], "message");
}

/// Celui qui balaye l'apprend par sa réponse ; l'autre n'apprendrait rien
/// avant d'avoir rouvert l'application.
#[tokio::test]
async fn a_match_is_pushed_to_the_person_who_was_not_swiping() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let address = listening(app.clone()).await;

    let here = private_cluster();
    let (a, a_id) = candidate(&app, "match-direct-a", LIVE_MATCH, "woman", Some(here)).await;
    let (b, b_id) = candidate(&app, "match-direct-b", LIVE_MATCH, "man", Some(here)).await;

    // A aime en premier : rien encore, personne n'a répondu.
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

    let mut waiting = open_socket(address, &a).await;

    call(
        &app,
        request(
            "POST",
            "/api/v1/discovery/swipes",
            Some(&b),
            Some(json!({ "target_profile_id": a_id, "decision": "like" })),
        ),
    )
    .await;

    let event = next_event(&mut waiting).await;
    assert_eq!(event["type"], "match");
    // Le profil poussé est celui de l'*autre*, sinon l'écran afficherait son
    // propre visage sous « c'est un match ».
    assert_eq!(event["match"]["profile"]["id"], b_id.to_string());
}

/// Deux fenêtres de la même personne — le téléphone et l'iPad — doivent voir
/// la même chose.
#[tokio::test]
async fn both_windows_of_the_same_person_receive_the_event() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let address = listening(app.clone()).await;

    let thread = thread(&app, "deux-fenetres", LIVE).await;
    let mut phone = open_socket(address, &thread.b).await;
    let mut tablet = open_socket(address, &thread.b).await;

    send(&app, &thread.a, &thread.id, "sur les deux").await;

    for socket in [&mut phone, &mut tablet] {
        let event = next_event(socket).await;
        assert_eq!(event["message"]["body"], "sur les deux");
    }
}
