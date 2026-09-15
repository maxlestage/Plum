use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Qui est nommé dans un signalement, quand le compte existe encore.
///
/// Les deux côtés sont facultatifs : un signalement survit aux comptes qu'il
/// nomme, parce qu'il est la trace de *pourquoi* une décision a été prise.
/// Un modérateur qui ouvre la liste doit donc voir « compte supprimé » et non
/// une ligne vide qui ressemble à un bogue.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Named {
    pub id: Uuid,
    pub display_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ReportRow {
    pub id: Uuid,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub reporter: Option<Named>,
    pub reported: Option<Named>,
    /// Combien de fois cette personne a été signalée, en tout.
    ///
    /// C'est le seul chiffre qui change une décision. Un signalement isolé
    /// peut être un dépit ; cinq signalements par cinq personnes différentes
    /// sont un motif. Sans ce compte, un modérateur devrait faire le tri à la
    /// main, et ne le ferait pas.
    pub reported_total: u64,
    /// Combien de personnes *distinctes* l'ont signalée.
    ///
    /// Séparé du total, parce que cinq signalements d'une même personne ne
    /// disent pas la même chose que cinq personnes qui signalent.
    pub distinct_reporters: u64,
    /// Le compte visé est-il déjà fermé ?
    ///
    /// Sans ça, un modérateur qui reprend la file après quelqu'un d'autre
    /// suspend une deuxième fois un compte déjà suspendu, ou n'ose pas
    /// toucher à un compte qu'il croit ouvert. C'est l'information qui décide
    /// s'il reste quelque chose à faire sur cette ligne.
    pub reported_suspended: bool,
    /// Quand cette ligne a été jugée, et ce qui a été décidé. `None` des deux
    /// côtés tant qu'elle est dans la file.
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolution: Option<String>,
}

/// Ce qu'un modérateur décide d'un signalement.
///
/// Deux issues seulement, et pas de texte libre à la place : « classé » et
/// « retenu » ne se comptent que s'ils s'écrivent pareil à chaque fois. La
/// nuance va dans `note`, qui n'est lue que par un humain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// Le signalement était fondé.
    Upheld,
    /// Sans suite. Ce n'est pas « rien ne s'est passé » : c'est une décision,
    /// et elle est tracée comme l'autre.
    Dismissed,
}

impl Resolution {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Upheld => "upheld",
            Self::Dismissed => "dismissed",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ResolveRequest {
    pub resolution: Resolution,
    /// Facultative, et c'est volontaire : exiger une justification écrite sur
    /// chaque ligne d'une file de cinquante donne cinquante fois « ok ».
    pub note: Option<String>,
}

/// La raison d'une suspension, elle, est obligatoire.
///
/// Fermer le compte de quelqu'un sans écrire pourquoi est la décision qu'on
/// ne peut pas relire six mois plus tard, et c'est exactement celle qui se
/// conteste.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SuspendRequest {
    pub reason: String,
}

/// L'état du compte après une décision, pour que l'appelant n'ait pas à le
/// redemander — et pour qu'un outil puisse vérifier que l'ordre a pris.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AccountState {
    pub id: Uuid,
    pub suspended: bool,
    pub suspended_at: Option<DateTime<Utc>>,
    /// Combien de décisions ont déjà été prises sur ce compte, celle-ci
    /// comprise.
    pub decisions: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ReportsPage {
    pub items: Vec<ReportRow>,
    pub next_cursor: Option<String>,
}

/// Le curseur de la file de modération : même forme que celui des matchs.
#[derive(Debug, Clone, Copy)]
pub struct ReportCursor {
    pub created_at: DateTime<Utc>,
    pub id: Uuid,
}

impl ReportCursor {
    pub fn encode(&self) -> String {
        // En microsecondes, pour la même raison qu'ailleurs : la colonne est
        // un `timestamptz`, et arrondir ferait sauter ou répéter les lignes
        // nées dans le même millième de seconde.
        format!("{}|{}", self.created_at.timestamp_micros(), self.id)
    }

    pub fn decode(raw: &str) -> Option<Self> {
        let (micros, id) = raw.split_once('|')?;
        Some(Self {
            created_at: DateTime::from_timestamp_micros(micros.parse().ok()?)?,
            id: id.parse().ok()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cursor_survives_its_round_trip() {
        let cursor = ReportCursor {
            created_at: DateTime::from_timestamp_micros(1_757_800_000_123_456).unwrap(),
            id: Uuid::from_u128(0xfeed_face),
        };
        let back = ReportCursor::decode(&cursor.encode()).expect("relisible");
        assert_eq!(back.created_at, cursor.created_at);
        assert_eq!(back.id, cursor.id);
    }

    #[test]
    fn a_cursor_that_is_not_one_is_refused_rather_than_guessed() {
        for bancal in ["", "|", "abc|def", "123", "123|pas-un-uuid"] {
            assert!(
                ReportCursor::decode(bancal).is_none(),
                "« {bancal} » accepté"
            );
        }
    }
}
