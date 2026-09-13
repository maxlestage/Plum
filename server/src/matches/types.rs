use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// Où la page précédente s'est arrêtée.
///
/// Les matchs sont rendus du plus récent au plus ancien, et deux peuvent
/// naître dans la même milliseconde — deux personnes qui aiment en retour au
/// même instant. L'identifiant départage, comme pour le deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchCursor {
    pub matched_at: DateTime<Utc>,
    pub id: Uuid,
}

impl MatchCursor {
    pub fn encode(&self) -> String {
        // En microsecondes : la colonne est un `timestamptz`, dont Postgres
        // garde la microseconde. Arrondir à la milliseconde ferait réapparaître
        // ou disparaître les matchs nés dans le même millième de seconde.
        format!("{}|{}", self.matched_at.timestamp_micros(), self.id)
    }

    pub fn decode(raw: &str) -> Option<Self> {
        let (micros, id) = raw.split_once('|')?;
        Some(Self {
            matched_at: DateTime::from_timestamp_micros(micros.parse().ok()?)?,
            id: id.parse().ok()?,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct MatchesPage {
    pub items: Vec<super::super::discovery::types::MatchResponse>,
    pub next_cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cursor_survives_the_round_trip_to_the_microsecond() {
        let cursor = MatchCursor {
            matched_at: DateTime::from_timestamp_micros(1_757_800_000_123_456).unwrap(),
            id: Uuid::from_u128(0xfeed_face),
        };
        let back = MatchCursor::decode(&cursor.encode()).expect("décodable");
        assert_eq!(back, cursor);
    }

    #[test]
    fn nonsense_cursors_are_refused_rather_than_guessed() {
        for raw in ["", "|", "abc", "123|", "|abc", "123|pas-un-uuid"] {
            assert!(
                MatchCursor::decode(raw).is_none(),
                "« {raw} » aurait dû échouer"
            );
        }
    }
}
