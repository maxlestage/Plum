use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::message;
use crate::profile::types::ProfileResponse;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct MessageResponse {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub sender_id: Uuid,
    pub body: String,
    pub sent_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
}

impl From<message::Model> for MessageResponse {
    fn from(model: message::Model) -> Self {
        Self {
            id: model.id,
            conversation_id: model.conversation_id,
            sender_id: model.sender_id,
            body: model.body,
            sent_at: model.sent_at.into(),
            read_at: model.read_at.map(Into::into),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ConversationResponse {
    pub id: Uuid,
    pub match_id: Uuid,
    pub participant: ProfileResponse,
    pub last_message: Option<MessageResponse>,
    /// Ce qui n'a pas été lu **par celui qui demande**, donc jamais ses
    /// propres messages.
    pub unread_count: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

/// `client_id` est le garde-fou contre le double envoi : le client le tire au
/// sort avant d'émettre, et un renvoi après une connexion coupée retrouve le
/// message déjà écrit au lieu d'en créer un second.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SendMessageRequest {
    pub client_id: Uuid,
    pub body: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MessagesQuery {
    pub limit: Option<u64>,
    /// Le fil remonte vers le passé : `before` est le curseur du plus ancien
    /// message déjà affiché.
    pub before: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConversationsQuery {
    pub limit: Option<u64>,
    pub cursor: Option<String>,
}

/// Un point du fil : la date et l'identifiant, pour la même raison que
/// partout ailleurs — deux messages peuvent naître dans la même microseconde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadCursor {
    pub at: DateTime<Utc>,
    pub id: Uuid,
}

impl ThreadCursor {
    pub fn encode(&self) -> String {
        format!("{}|{}", self.at.timestamp_micros(), self.id)
    }

    pub fn decode(raw: &str) -> Option<Self> {
        let (micros, id) = raw.split_once('|')?;
        Some(Self {
            at: DateTime::from_timestamp_micros(micros.parse().ok()?)?,
            id: id.parse().ok()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cursor_survives_the_round_trip_to_the_microsecond() {
        let cursor = ThreadCursor {
            at: DateTime::from_timestamp_micros(1_757_800_000_123_456).unwrap(),
            id: Uuid::from_u128(0x00c0_ffee),
        };
        assert_eq!(ThreadCursor::decode(&cursor.encode()), Some(cursor));
    }

    #[test]
    fn nonsense_cursors_are_refused_rather_than_guessed() {
        for raw in ["", "|", "abc", "12|", "|x", "12|pas-un-uuid"] {
            assert!(ThreadCursor::decode(raw).is_none(), "« {raw} »");
        }
    }

    /// La forme est le contrat : l'écran de conversation décode ces clés.
    #[test]
    fn the_message_payload_uses_the_keys_the_client_expects() {
        let json = serde_json::to_value(MessageResponse {
            id: Uuid::nil(),
            conversation_id: Uuid::nil(),
            sender_id: Uuid::nil(),
            body: "bonjour".into(),
            sent_at: DateTime::from_timestamp(0, 0).unwrap(),
            read_at: None,
        })
        .unwrap();

        for key in [
            "id",
            "conversation_id",
            "sender_id",
            "body",
            "sent_at",
            "read_at",
        ] {
            assert!(json.get(key).is_some(), "clé « {key} » absente de {json}");
        }
    }
}
