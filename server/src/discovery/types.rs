use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::profile::types::ProfileResponse;

/// Mirrors the Swift enum. `superLike` is camelCase on the wire for the same
/// reason as `nonBinary`: Swift leaves raw values alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwipeDecision {
    #[serde(rename = "like")]
    Like,
    #[serde(rename = "pass")]
    Pass,
    #[serde(rename = "superLike")]
    SuperLike,
}

impl SwipeDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Like => "like",
            Self::Pass => "pass",
            Self::SuperLike => "superLike",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "like" => Some(Self::Like),
            "pass" => Some(Self::Pass),
            "superLike" => Some(Self::SuperLike),
            _ => None,
        }
    }

    /// A super like is a like that shouts. Both can make a match.
    pub fn is_affirmative(self) -> bool {
        matches!(self, Self::Like | Self::SuperLike)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SwipeRequest {
    pub target_profile_id: Uuid,
    pub decision: SwipeDecision,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct MatchResponse {
    pub id: Uuid,
    pub profile: ProfileResponse,
    pub matched_at: DateTime<Utc>,
    /// No conversations yet, so always absent. Present in the shape because
    /// the client declares it optional and will read it the day there are.
    pub conversation_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SwipeOutcome {
    pub matched: bool,
    pub r#match: Option<MatchResponse>,
    /// `null` means unlimited.
    ///
    /// Deliberately null: a daily like cap is what a dating app normally
    /// sells, and there is nothing to sell here yet. Inventing a quota would
    /// be guessing at a business model rather than implementing one.
    pub likes_remaining: Option<i32>,
}

/// The client's `Page<T>`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DeckQuery {
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ReportRequest {
    pub reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RewindResponse {
    pub profile: Option<ProfileResponse>,
}

/// Where the last page stopped: the sort key and the identifier that breaks
/// its ties.
///
/// Not an offset. A deck shifts while it is being read — other people swipe,
/// profiles appear and disappear — and an offset would silently repeat or
/// skip cards. The pair is compared as a tuple, which Postgres does natively.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cursor {
    pub sort_km: f64,
    pub id: Uuid,
}

impl Cursor {
    pub fn encode(&self) -> String {
        // `{}` on an f64 prints the shortest representation that parses back
        // to the same value, so the round trip is exact and two candidates at
        // the same distance stay distinguishable.
        format!("{}|{}", self.sort_km, self.id)
    }

    pub fn decode(raw: &str) -> Option<Self> {
        let (km, id) = raw.split_once('|')?;
        Some(Self {
            sort_km: km.parse().ok()?,
            id: id.parse().ok()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_decision_wire_values_match_the_swift_raw_values() {
        let pairs = [
            (SwipeDecision::Like, "\"like\""),
            (SwipeDecision::Pass, "\"pass\""),
            // The one that would be "super_like" if anyone tidied it.
            (SwipeDecision::SuperLike, "\"superLike\""),
        ];

        for (value, expected) in pairs {
            assert_eq!(serde_json::to_string(&value).unwrap(), expected);
            assert_eq!(SwipeDecision::parse(value.as_str()), Some(value));
        }
    }

    #[test]
    fn a_super_like_counts_as_a_like() {
        assert!(SwipeDecision::SuperLike.is_affirmative());
        assert!(SwipeDecision::Like.is_affirmative());
        assert!(!SwipeDecision::Pass.is_affirmative());
    }

    /// The cursor crosses the wire and comes back; a distance that does not
    /// survive the trip would put the reader back in the middle of a page it
    /// has already seen.
    #[test]
    fn a_cursor_survives_the_round_trip_exactly() {
        for km in [0.0, 1.0 / 3.0, 4.389_123_456_789, 1e9, f64::MAX] {
            let cursor = Cursor {
                sort_km: km,
                id: Uuid::from_u128(0x1234_5678_9abc_def0),
            };
            let back = Cursor::decode(&cursor.encode()).expect("décodable");
            assert_eq!(back.sort_km.to_bits(), km.to_bits(), "distance {km}");
            assert_eq!(back.id, cursor.id);
        }
    }

    /// A cursor arrives from the client, so it arrives from anywhere.
    #[test]
    fn nonsense_cursors_are_refused_rather_than_guessed() {
        for raw in ["", "|", "abc", "1.0|", "|abc", "1.0|pas-un-uuid", "x|y"] {
            assert!(Cursor::decode(raw).is_none(), "« {raw} » aurait dû échouer");
        }
    }
}
