use chrono::{DateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::types::Gender;
use crate::entities::{preferences, profile};

/// As with the auth slice, the shapes here are dictated by the iOS client.
/// Keys cross as snake_case because its decoder converts from snake_case;
/// enum *values* cross verbatim, because Swift leaves raw values alone.

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PhotoResponse {
    pub id: Uuid,
    pub url: String,
    /// 0 is the cover.
    pub position: i32,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ProfileResponse {
    pub id: Uuid,
    pub display_name: String,
    /// A full timestamp, not a bare date: the client decodes every `Date` the
    /// same way and a bare `1998-04-12` fails its ISO 8601 parser.
    pub birth_date: DateTime<Utc>,
    pub gender: Gender,
    pub bio: String,
    pub city: String,
    /// Always present, empty until photos have somewhere to live: a dyno's
    /// filesystem is wiped on every restart, so this waits for object storage
    /// rather than shipping uploads that vanish.
    pub photos: Vec<PhotoResponse>,
    pub interests: Vec<String>,
    /// Absent from your own profile — there is no distance from yourself.
    pub distance_km: Option<f64>,
    pub last_active_at: Option<DateTime<Utc>>,
}

impl ProfileResponse {
    pub fn own(model: profile::Model) -> Self {
        let birth_date = model.birth_date.and_time(NaiveTime::MIN).and_utc();

        Self {
            id: model.id,
            // A row whose gender predates a rename should not take the whole
            // profile down; `Other` is the honest fallback.
            gender: Gender::parse(&model.gender).unwrap_or(Gender::Other),
            display_name: model.display_name,
            birth_date,
            bio: model.bio,
            city: model.city,
            photos: Vec::new(),
            interests: model.interests,
            distance_km: None,
            last_active_at: model.last_active_at.map(Into::into),
        }
    }
}

/// Every field optional: the client sends only what changed, and a missing key
/// must mean "leave it alone" rather than "clear it".
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProfileUpdate {
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub city: Option<String>,
    pub interests: Option<Vec<String>>,
}

/// Mirrors the Swift enum. Single lowercase words, so the raw values happen to
/// match snake_case — spelled out anyway, because that coincidence is exactly
/// what made `nonBinary` easy to get wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GenderPreference {
    #[serde(rename = "women")]
    Women,
    #[serde(rename = "men")]
    Men,
    #[serde(rename = "everyone")]
    Everyone,
}

impl GenderPreference {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Women => "women",
            Self::Men => "men",
            Self::Everyone => "everyone",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "women" => Some(Self::Women),
            "men" => Some(Self::Men),
            "everyone" => Some(Self::Everyone),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct PreferencesResponse {
    pub interested_in: GenderPreference,
    pub min_age: i32,
    pub max_age: i32,
    pub max_distance_km: i32,
    pub show_me_on_plum: bool,
}

/// What the client sends. Identical in shape to the response, and deliberately
/// a separate type: the response is what we promise, this is what we are
/// willing to be told, and they drift apart the moment a field becomes
/// server-owned.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PreferencesUpdate {
    pub interested_in: GenderPreference,
    pub min_age: i32,
    pub max_age: i32,
    pub max_distance_km: i32,
    pub show_me_on_plum: bool,
}

/// The bounds the client already applies in `DiscoveryPreferences.sanitized`,
/// applied again here. The client's version is a courtesy; this one is the
/// control, and the database carries the same rule as a CHECK.
pub fn clamp(update: PreferencesUpdate) -> PreferencesUpdate {
    let min_age = update.min_age.clamp(18, 99);
    PreferencesUpdate {
        interested_in: update.interested_in,
        min_age,
        max_age: update.max_age.clamp(min_age, 99),
        max_distance_km: update.max_distance_km.clamp(1, 300),
        show_me_on_plum: update.show_me_on_plum,
    }
}

impl PreferencesResponse {
    pub fn defaults() -> Self {
        Self {
            interested_in: GenderPreference::Everyone,
            min_age: 18,
            max_age: 45,
            max_distance_km: 50,
            show_me_on_plum: true,
        }
    }
}

impl From<preferences::Model> for PreferencesResponse {
    fn from(model: preferences::Model) -> Self {
        Self {
            interested_in: GenderPreference::parse(&model.interested_in)
                .unwrap_or(GenderPreference::Everyone),
            min_age: model.min_age,
            max_age: model.max_age,
            max_distance_km: model.max_distance_km,
            show_me_on_plum: model.show_me_on_plum,
        }
    }
}

/// `{latitude, longitude}`, straight from the client's `Coordinate`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LocationUpdate {
    pub latitude: f64,
    pub longitude: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update(min_age: i32, max_age: i32, max_distance_km: i32) -> PreferencesUpdate {
        PreferencesUpdate {
            interested_in: GenderPreference::Everyone,
            min_age,
            max_age,
            max_distance_km,
            show_me_on_plum: true,
        }
    }

    #[test]
    fn the_age_floor_is_the_legal_one() {
        let clamped = clamp(update(13, 30, 50));
        assert_eq!(clamped.min_age, 18);
    }

    /// An inverted range would ask the deck for nobody, and look like a bug in
    /// the deck rather than in the settings that caused it.
    #[test]
    fn an_inverted_range_closes_up_rather_than_staying_inverted() {
        let clamped = clamp(update(40, 20, 50));
        assert_eq!(clamped.min_age, 40);
        assert_eq!(clamped.max_age, 40);
    }

    #[test]
    fn the_distance_is_bounded_at_both_ends() {
        assert_eq!(clamp(update(18, 30, 0)).max_distance_km, 1);
        assert_eq!(clamp(update(18, 30, 99_999)).max_distance_km, 300);
    }

    #[test]
    fn a_reasonable_range_is_left_alone() {
        let clamped = clamp(update(25, 35, 20));
        assert_eq!((clamped.min_age, clamped.max_age), (25, 35));
        assert_eq!(clamped.max_distance_km, 20);
    }

    /// Swift does not transform enum raw values, so these must cross verbatim.
    /// Spelled out here as well as in the round trip, because this is the file
    /// someone would edit while "tidying" the names.
    #[test]
    fn the_gender_preference_wire_values_match_the_swift_raw_values() {
        let pairs = [
            (GenderPreference::Women, "\"women\""),
            (GenderPreference::Men, "\"men\""),
            (GenderPreference::Everyone, "\"everyone\""),
        ];

        for (value, expected) in pairs {
            assert_eq!(serde_json::to_string(&value).unwrap(), expected);
            assert_eq!(GenderPreference::parse(value.as_str()), Some(value));
        }
    }

    /// The defaults here and in the client's `DiscoveryPreferences.default`
    /// have to agree, or the first deck contradicts the settings screen that
    /// is showing it.
    #[test]
    fn the_defaults_match_the_clients() {
        let defaults = PreferencesResponse::defaults();
        assert_eq!(defaults.interested_in, GenderPreference::Everyone);
        assert_eq!(defaults.min_age, 18);
        assert_eq!(defaults.max_age, 45);
        assert_eq!(defaults.max_distance_km, 50);
        assert!(defaults.show_me_on_plum);
    }

    /// The response keys are the contract; the client decodes from snake_case.
    #[test]
    fn the_preferences_payload_uses_the_keys_the_client_expects() {
        let json = serde_json::to_value(PreferencesResponse::defaults()).unwrap();
        for key in [
            "interested_in",
            "min_age",
            "max_age",
            "max_distance_km",
            "show_me_on_plum",
        ] {
            assert!(json.get(key).is_some(), "clé « {key} » absente de {json}");
        }
    }
}
