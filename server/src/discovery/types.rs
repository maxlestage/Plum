use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::profile::types::ProfileResponse;

/// Ce qu'on peut décider d'une personne proposée. Deux issues, pas trois.
///
/// Il y avait « j'aime », « passer » et « coup de cœur », et un match naissait
/// d'un double oui. Ça demandait un geste par carte sur un paquet sans fond —
/// c'est-à-dire le balayage, dont on ne veut plus.
///
/// Il reste : on écrit, ou on laisse passer. Écrire n'est pas un vote qu'on
/// espère voir confirmé, c'est une conversation qui commence ; ne rien
/// répondre est une réponse, et elle n'a pas besoin d'écran.
///
/// Les anciennes valeurs (`like`, `superLike`) restent lisibles en base : ce
/// sont des verdicts déjà rendus, et ce qui compte d'eux — « cette personne a
/// déjà été vue » — n'a pas changé. Rien à migrer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Written,
    Passed,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Written => "written",
            Self::Passed => "pass",
        }
    }
}

/// La sélection du jour.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SelectionResponse {
    pub items: Vec<ProfileResponse>,
    /// Quand la prochaine sélection sera tirée.
    ///
    /// Minuit UTC, et il faut le dire plutôt que le laisser deviner : sans
    /// fuseau par compte, « demain » est le demain du serveur. Pour la France
    /// ça décale le renouvellement à une ou deux heures du matin, ce qui est
    /// acceptable ; pour quelqu'un à l'autre bout du monde, non. Le jour où
    /// des comptes vivent ailleurs, c'est ici que ça se règle.
    pub refreshes_at: DateTime<Utc>,
    /// Combien la sélection compte quand elle est pleine, pour que l'écran
    /// puisse dire « il en reste deux » sans le déduire.
    pub size: u32,
}

/// Quelqu'un qu'on a bloqué.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct BlockedPerson {
    pub id: Uuid,
    /// Absent quand le compte est parti depuis. La ligne de blocage, elle,
    /// reste — et l'écran doit dire « compte supprimé » plutôt qu'une ligne
    /// vide qui ressemble à un bogue.
    pub display_name: Option<String>,
    pub blocked_at: DateTime<Utc>,
}

/// Le premier message, celui qui ouvre tout.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WriteRequest {
    pub body: String,
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

/// The client's `Page<T>`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ReportRequest {
    pub reason: String,
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

    /// Ce que le client lit sur le fil. Deux valeurs, et « pass » garde son
    /// orthographe d'avant : la colonne contient déjà des lignes qui la
    /// portent, et les relire autrement rendrait au tirage des gens qui ont
    /// été écartés.
    #[test]
    fn the_verdicts_keep_the_spelling_the_database_already_holds() {
        assert_eq!(Verdict::Written.as_str(), "written");
        assert_eq!(Verdict::Passed.as_str(), "pass");
        assert_eq!(
            serde_json::to_string(&Verdict::Written).unwrap(),
            "\"written\""
        );
        assert_eq!(
            serde_json::to_string(&Verdict::Passed).unwrap(),
            "\"passed\""
        );
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
