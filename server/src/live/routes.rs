use std::time::Duration;

use axum::extract::ws::{Message as WsMessage, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::unbounded_channel;
use uuid::Uuid;

use crate::auth::routes::authenticate;
use crate::chat::types::MessageResponse;
use crate::discovery::types::MatchResponse;
use crate::entities::{conversation, match_pair};
use crate::error::ApiResult;
use crate::state::AppState;

/// Le routeur Heroku ferme une connexion restée muette 55 secondes. Un fil de
/// discussion peut très bien rester calme plus longtemps, donc le serveur
/// envoie un `ping` avant l'échéance ; sans lui, le socket tomberait toutes
/// les minutes et le client passerait son temps à se reconnecter.
const HEARTBEAT: Duration = Duration::from_secs(30);

/// Le rythme maximal auquel une connexion peut annoncer une frappe.
///
/// Chaque annonce coûte deux lectures en base pour vérifier l'appartenance.
/// Le client s'impose déjà un silence de trois secondes, mais c'est une
/// politesse qu'un client hostile n'a pas : la seule limite qui tienne est
/// celle du serveur.
const TYPING_INTERVAL: Duration = Duration::from_secs(1);

pub fn router() -> Router<AppState> {
    Router::new().route("/ws", get(open))
}

/// Ce que le serveur pousse.
///
/// Le direct est un confort, jamais la source de vérité : tout ce qui passe
/// ici est déjà en base et le client le relira au prochain chargement. C'est
/// ce qui autorise à ne rien garantir sur la livraison.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LiveEvent {
    Message {
        message: MessageResponse,
    },
    Read {
        message_id: Uuid,
        read_at: DateTime<Utc>,
    },
    Typing {
        conversation_id: Uuid,
        profile_id: Uuid,
    },
    #[serde(rename = "match")]
    Match {
        r#match: MatchResponse,
    },
}

/// Ce que le client envoie.
///
/// Seule la frappe passe par le socket : envoyer un message ou marquer un fil
/// comme lu reste une requête, parce que ces deux-là doivent pouvoir échouer
/// franchement et rendre la ressource écrite.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientCommand {
    Typing { conversation_id: Uuid },
}

/// Le socket s'authentifie par le même en-tête que le reste de l'API.
///
/// Pas de jeton dans l'URL : une adresse se retrouve dans les journaux des
/// serveurs mandataires et dans l'historique du navigateur, un en-tête non.
async fn open(
    State(state): State<AppState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> ApiResult<Response> {
    let claims = authenticate(&state, &headers)?;
    let viewer = claims.sub;
    Ok(upgrade.on_upgrade(move |socket| serve(socket, state, viewer)))
}

async fn serve(socket: WebSocket, state: AppState, viewer: Uuid) {
    let (mut sink, mut stream) = socket.split();
    let (sender, mut outgoing) = unbounded_channel::<String>();
    let connection = state.hub.join(viewer, sender);

    // Une tâche pousse ce que le concentrateur dépose et entretient la
    // connexion, l'autre lit ce que le client envoie. La première qui s'arrête
    // fait tomber l'autre : un socket à moitié vivant ne sert personne.
    let mut pushing = tokio::spawn(async move {
        let mut beat = tokio::time::interval(HEARTBEAT);
        // Sans ça, un envoi bloqué cinq minutes serait suivi d'une rafale de
        // dix battements d'un coup.
        beat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // Le premier `tick` d'un `interval` part immédiatement ; sauté, sans
        // quoi chaque connexion s'ouvrirait sur un ping inutile.
        beat.tick().await;
        loop {
            let frame = tokio::select! {
                payload = outgoing.recv() => match payload {
                    Some(payload) => WsMessage::Text(Utf8Bytes::from(payload)),
                    None => break,
                },
                _ = beat.tick() => WsMessage::Ping(Vec::new().into()),
            };
            if sink.send(frame).await.is_err() {
                break;
            }
        }
    });

    let pulling_state = state.clone();
    let mut pulling = tokio::spawn(async move {
        let mut last_typing: Option<tokio::time::Instant> = None;
        while let Some(Ok(frame)) = stream.next().await {
            // Le client iOS émet des trames binaires (`URLSessionWebSocketTask`
            // encode du JSON en `Data`), un client web enverrait du texte. Les
            // deux portent la même chose.
            let raw = match &frame {
                WsMessage::Text(text) => text.as_bytes(),
                WsMessage::Binary(bytes) => bytes.as_ref(),
                _ => continue,
            };
            // Une trame illisible est ignorée plutôt que fatale : le socket
            // sert du confort, et le couper punirait la mauvaise personne.
            let Ok(command) = serde_json::from_slice::<ClientCommand>(raw) else {
                continue;
            };
            match command {
                ClientCommand::Typing { conversation_id } => {
                    let now = tokio::time::Instant::now();
                    if last_typing.is_some_and(|last| now - last < TYPING_INTERVAL) {
                        // Jetée avant la base, pas après : l'intérêt de la
                        // limite est justement d'épargner les deux requêtes.
                        continue;
                    }
                    last_typing = Some(now);
                    relay_typing(&pulling_state, viewer, conversation_id).await;
                }
            }
        }
    });

    tokio::select! {
        _ = &mut pushing => pulling.abort(),
        _ = &mut pulling => pushing.abort(),
    }

    state.hub.leave(viewer, connection);
}

/// Relaie « untel écrit » à l'autre participant, et à lui seul.
///
/// Vérifié en base plutôt que cru sur parole : sans ça, quiconque devinerait
/// un identifiant de conversation pourrait faire clignoter « en train
/// d'écrire » chez des inconnus.
async fn relay_typing(state: &AppState, viewer: Uuid, conversation_id: Uuid) {
    let Ok(Some(found)) = conversation::Entity::find_by_id(conversation_id)
        .one(&state.db)
        .await
    else {
        return;
    };

    let Ok(Some(pair)) = match_pair::Entity::find_by_id(found.match_id)
        .one(&state.db)
        .await
    else {
        return;
    };

    if pair.lower_id != viewer && pair.upper_id != viewer {
        return;
    }

    // Un compte fermé n'annonce plus qu'il écrit. Sans cette ligne, une
    // suspension pour harcèlement laisserait la personne suspendue faire
    // clignoter « en train d'écrire » chez celle qui l'a signalée, aussi
    // longtemps que le socket tient — ce qui est du harcèlement, avec moins
    // de mots.
    if crate::chat::routes::is_suspended(state, viewer)
        .await
        .unwrap_or(true)
    {
        return;
    }

    // Un blocage referme la conversation. Bloquer supprime le match, donc ce
    // relais s'arrête déjà plus haut — sauf pendant la poignée de
    // millisecondes où le blocage vient d'être posé et où la frappe est déjà
    // partie. C'est peu, mais c'est précisément le moment où la personne
    // bloquée est encore devant son écran.
    if crate::chat::routes::blocked_between(state, pair.lower_id, pair.upper_id)
        .await
        .unwrap_or(true)
    {
        return;
    }

    state.hub.send(
        pair.other(viewer),
        &LiveEvent::Typing {
            conversation_id,
            profile_id: viewer,
        },
    );
}

/// Pousse un message fraîchement écrit vers son destinataire.
pub fn announce_message(state: &AppState, to: Uuid, message: MessageResponse) {
    state.hub.send(to, &LiveEvent::Message { message });
}

/// Pousse les accusés de lecture vers celui qui avait écrit.
///
/// Un événement par message : le client indexe ses bulles par identifiant, et
/// un lot l'obligerait à un second format pour la même information.
pub fn announce_read(state: &AppState, to: Uuid, message_ids: &[Uuid], read_at: DateTime<Utc>) {
    for id in message_ids {
        state.hub.send(
            to,
            &LiveEvent::Read {
                message_id: *id,
                read_at,
            },
        );
    }
}

/// Pousse un match vers celui qui n'est pas en train de swiper.
///
/// Celui qui vient de balayer l'apprend par la réponse de sa requête ; l'autre
/// n'apprendrait rien avant d'avoir rouvert l'application.
pub fn announce_match(state: &AppState, to: Uuid, r#match: MatchResponse) {
    state.hub.send(to, &LiveEvent::Match { r#match });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn serialised(event: &LiveEvent) -> serde_json::Value {
        serde_json::to_value(event).expect("sérialisable")
    }

    /// Le client décode sur la clé `type` et rien d'autre ; un renommage
    /// silencieux ici casserait l'application sans casser un seul test.
    #[test]
    fn the_events_carry_the_names_the_client_reads() {
        let now = Utc::now();
        let read = serialised(&LiveEvent::Read {
            message_id: Uuid::nil(),
            read_at: now,
        });
        assert_eq!(read["type"], "read");
        assert_eq!(read["message_id"], Uuid::nil().to_string());

        let typing = serialised(&LiveEvent::Typing {
            conversation_id: Uuid::nil(),
            profile_id: Uuid::nil(),
        });
        assert_eq!(typing["type"], "typing");
        assert!(typing.get("conversation_id").is_some());
        assert!(typing.get("profile_id").is_some());
    }

    /// `match` est un mot réservé en Rust : le nom de la variante est
    /// forcément différent de ce qui part sur le fil, donc il se vérifie.
    #[test]
    fn the_match_event_is_not_named_after_its_rust_variant() {
        let event = LiveEvent::Match {
            r#match: MatchResponse {
                id: Uuid::nil(),
                profile: crate::profile::types::ProfileResponse {
                    id: Uuid::nil(),
                    display_name: "Camille".into(),
                    birth_date: Utc::now(),
                    gender: crate::auth::types::Gender::Other,
                    bio: String::new(),
                    city: String::new(),
                    photos: Vec::new(),
                    interests: Vec::new(),
                    distance_km: None,
                    last_active_at: None,
                },
                matched_at: Utc::now(),
                conversation_id: None,
            },
        };
        let value = serialised(&event);
        assert_eq!(value["type"], "match");
        assert!(value.get("match").is_some(), "{value}");
    }

    #[test]
    fn the_only_command_the_client_may_send_is_typing() {
        let id = Uuid::new_v4();
        let parsed: ClientCommand =
            serde_json::from_str(&format!(r#"{{"type":"typing","conversation_id":"{id}"}}"#))
                .expect("le client émet exactement ceci");
        let ClientCommand::Typing { conversation_id } = parsed;
        assert_eq!(conversation_id, id);

        // Envoyer un message par le socket doit rester impossible : il n'y a
        // pas de réponse à rendre, donc pas d'échec à montrer.
        assert!(
            serde_json::from_str::<ClientCommand>(r#"{"type":"message","body":"coucou"}"#).is_err()
        );
    }
}
