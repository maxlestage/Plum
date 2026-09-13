use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::user;

/// The wire shapes are dictated by the iOS client, which already exists: its
/// `JSONDecoder` converts from snake_case and parses every date as ISO 8601.
/// Field names here are not a preference, they are the contract.

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SignUpRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
    /// The client encodes every `Date` as a full ISO 8601 timestamp, including
    /// this one — so it arrives as `1998-04-12T00:00:00Z`, never as a bare
    /// date, and the day is what we keep.
    pub birth_date: DateTime<Utc>,
    pub gender: Gender,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SignInRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// Mirrors the Swift enum. Its raw values are *not* snake_cased — Swift leaves
/// raw values alone — so `nonBinary` crosses the wire exactly like that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gender {
    #[serde(rename = "woman")]
    Woman,
    #[serde(rename = "man")]
    Man,
    #[serde(rename = "nonBinary")]
    NonBinary,
    #[serde(rename = "other")]
    Other,
}

impl Gender {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Woman => "woman",
            Self::Man => "man",
            Self::NonBinary => "nonBinary",
            Self::Other => "other",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "woman" => Some(Self::Woman),
            "man" => Some(Self::Man),
            "nonBinary" => Some(Self::NonBinary),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub profile_completed: bool,
}

impl From<user::Model> for UserResponse {
    fn from(model: user::Model) -> Self {
        Self {
            id: model.id,
            email: model.email,
            created_at: model.created_at.into(),
            profile_completed: model.profile_completed,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct TokensResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct SessionResponse {
    pub user: UserResponse,
    pub tokens: TokensResponse,
}

/// Plum is 18+. The client checks it too, but a client check is a courtesy,
/// not a control.
pub fn is_old_enough(birth_date: NaiveDate, today: NaiveDate) -> bool {
    match birth_date.checked_add_months(chrono::Months::new(18 * 12)) {
        Some(eighteenth) => eighteenth <= today,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    #[test]
    fn the_age_gate_admits_exactly_eighteen() {
        let today = date(2026, 9, 13);
        assert!(is_old_enough(date(2008, 9, 13), today));
    }

    #[test]
    fn it_refuses_the_day_before_the_eighteenth_birthday() {
        let today = date(2026, 9, 13);
        assert!(!is_old_enough(date(2008, 9, 14), today));
    }

    /// A 29 February birthday has no anniversary most years; the gate must
    /// still open, not trap someone forever.
    #[test]
    fn a_leap_day_birthday_still_comes_of_age() {
        assert!(is_old_enough(date(2008, 2, 29), date(2026, 3, 1)));
        assert!(is_old_enough(date(2008, 2, 29), date(2026, 2, 28)));
        assert!(!is_old_enough(date(2008, 2, 29), date(2025, 12, 31)));
    }

    #[test]
    fn the_gender_wire_values_match_the_swift_raw_values() {
        // `nonBinary` is the one that would break if anyone "tidied" it into
        // snake_case: Swift does not transform raw values.
        assert_eq!(
            serde_json::to_string(&Gender::NonBinary).unwrap(),
            "\"nonBinary\""
        );
        assert_eq!(
            serde_json::from_str::<Gender>("\"nonBinary\"").unwrap(),
            Gender::NonBinary
        );
        assert_eq!(Gender::parse("nonBinary"), Some(Gender::NonBinary));
        assert_eq!(Gender::parse("non_binary"), None);
    }

    #[test]
    fn the_session_payload_uses_the_keys_the_client_expects() {
        let session = SessionResponse {
            user: UserResponse {
                id: Uuid::nil(),
                email: "moi@plum.app".into(),
                created_at: DateTime::from_timestamp(0, 0).unwrap(),
                profile_completed: false,
            },
            tokens: TokensResponse {
                access_token: "a".into(),
                refresh_token: "r".into(),
                expires_at: DateTime::from_timestamp(0, 0).unwrap(),
            },
        };

        let json = serde_json::to_value(&session).unwrap();
        assert!(json["user"]["profile_completed"].is_boolean());
        assert!(json["user"]["created_at"].is_string());
        assert!(json["tokens"]["access_token"].is_string());
        assert!(json["tokens"]["expires_at"].is_string());
        // camelCase here would silently break every client decode.
        assert!(json["user"].get("profileCompleted").is_none());
    }

    #[test]
    fn a_sign_up_payload_from_the_client_deserialises() {
        let body = serde_json::json!({
            "email": "moi@plum.app",
            "password": "motdepasse",
            "display_name": "Camille",
            "birth_date": "1998-04-12T00:00:00Z",
            "gender": "nonBinary"
        });

        let request: SignUpRequest = serde_json::from_value(body).unwrap();

        assert_eq!(request.display_name, "Camille");
        assert_eq!(request.gender, Gender::NonBinary);
        assert_eq!(request.birth_date.date_naive(), date(1998, 4, 12));
    }
}
