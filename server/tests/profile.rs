//! End-to-end tests for the profile slice, against a real Postgres.

mod common;

use axum::http::StatusCode;
use common::*;
use sea_orm::{ConnectionTrait, EntityTrait};
use serde_json::json;

/// The shape is the contract, and the client cannot renegotiate it: a missing
/// or renamed key is a decoding failure on a screen, not a compiler error.
#[tokio::test]
async fn the_profile_comes_back_in_the_shape_the_client_decodes() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "profil").await;

    let (status, body) = call(
        &app,
        request("GET", "/api/v1/me/profile", Some(&token), None),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");

    for key in [
        "id",
        "display_name",
        "birth_date",
        "gender",
        "bio",
        "city",
        "photos",
        "interests",
        "distance_km",
        "last_active_at",
    ] {
        assert!(body.get(key).is_some(), "clé « {key} » absente de {body}");
    }

    assert_eq!(body["display_name"], "Camille");
    // Swift leaves enum raw values alone, so this crosses as camelCase.
    assert_eq!(body["gender"], "nonBinary");
    // A full timestamp, never a bare date: the client's ISO 8601 parser
    // rejects `1996-04-12`.
    let birth_date = body["birth_date"].as_str().expect("birth_date");
    assert!(
        birth_date.contains('T') && birth_date.ends_with('Z'),
        "birth_date doit être un horodatage complet, reçu {birth_date}"
    );
    // Empty, not missing: the Swift model declares `photos` non-optional.
    assert_eq!(body["photos"], json!([]));
    // No distance from yourself.
    assert!(body["distance_km"].is_null());
}

/// A PATCH carries only what changed. Treating a missing key as "clear it"
/// would wipe someone's bio the first time they edited their city.
#[tokio::test]
async fn updating_one_field_leaves_the_others_alone() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "partiel").await;

    let (status, _) = call(
        &app,
        request(
            "PATCH",
            "/api/v1/me/profile",
            Some(&token),
            Some(json!({ "bio": "J'aime les plums.", "city": "Lyon" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = call(
        &app,
        request(
            "PATCH",
            "/api/v1/me/profile",
            Some(&token),
            Some(json!({ "city": "Marseille" })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["city"], "Marseille");
    assert_eq!(
        body["bio"], "J'aime les plums.",
        "une clé absente doit laisser le champ tranquille"
    );
    assert_eq!(body["display_name"], "Camille");
}

/// Blanks, duplicates and casing all reach us from a free-text field. Showing
/// "Cinéma" twice on a card looks like a bug because it is one.
#[tokio::test]
async fn interests_are_trimmed_deduplicated_and_kept_in_order() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "interets").await;

    let (status, body) = call(
        &app,
        request(
            "PATCH",
            "/api/v1/me/profile",
            Some(&token),
            Some(json!({
                "interests": ["  Cinéma ", "cinéma", "", "   ", "Randonnée", "Cinema"]
            })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["interests"],
        json!(["Cinéma", "Randonnée", "Cinema"]),
        "attendu : coupé, dédupliqué sans tenir compte de la casse, ordre conservé"
    );
}

#[tokio::test]
async fn an_absurd_number_of_interests_is_refused() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "trop-interets").await;

    let many: Vec<String> = (0..40).map(|index| format!("centre {index}")).collect();
    let (status, _) = call(
        &app,
        request(
            "PATCH",
            "/api/v1/me/profile",
            Some(&token),
            Some(json!({ "interests": many })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn an_empty_display_name_is_refused() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "sans-prenom").await;

    let (status, _) = call(
        &app,
        request(
            "PATCH",
            "/api/v1/me/profile",
            Some(&token),
            Some(json!({ "display_name": "   " })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// Never opening the settings is not an error, and the defaults must be the
/// ones the client already starts from — otherwise the first deck contradicts
/// the settings screen showing it.
#[tokio::test]
async fn preferences_default_before_they_are_ever_set() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "prefs-defaut").await;

    let (status, body) = call(
        &app,
        request("GET", "/api/v1/me/preferences", Some(&token), None),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "interested_in": "everyone",
            "min_age": 18,
            "max_age": 45,
            "max_distance_km": 50,
            "show_me_on_plum": true
        })
    );
}

/// The client clamps these too. That is a courtesy; this is the control.
#[tokio::test]
async fn preferences_are_clamped_on_the_server() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "prefs-bornes").await;

    let (status, body) = call(
        &app,
        request(
            "PATCH",
            "/api/v1/me/preferences",
            Some(&token),
            Some(json!({
                "interested_in": "women",
                // Below the legal minimum, inverted, and absurd.
                "min_age": 13,
                "max_age": 4,
                "max_distance_km": 99_999,
                "show_me_on_plum": false
            })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["min_age"], 18,
        "Plum est 18+, quoi qu'envoie le client"
    );
    assert_eq!(body["max_age"], 18, "un intervalle inversé est refermé");
    assert_eq!(body["max_distance_km"], 300);
    assert_eq!(body["interested_in"], "women");
    assert_eq!(body["show_me_on_plum"], false);
}

/// Writing twice must update rather than collide: the row is keyed by account.
#[tokio::test]
async fn preferences_survive_being_written_twice() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "prefs-deux-fois").await;

    let send = |value: &str| {
        json!({
            "interested_in": value,
            "min_age": 25,
            "max_age": 35,
            "max_distance_km": 20,
            "show_me_on_plum": true
        })
    };

    let (first, _) = call(
        &app,
        request(
            "PATCH",
            "/api/v1/me/preferences",
            Some(&token),
            Some(send("men")),
        ),
    )
    .await;
    assert_eq!(first, StatusCode::OK);

    let (second, body) = call(
        &app,
        request(
            "PATCH",
            "/api/v1/me/preferences",
            Some(&token),
            Some(send("everyone")),
        ),
    )
    .await;
    assert_eq!(second, StatusCode::OK, "{body}");
    assert_eq!(body["interested_in"], "everyone");

    // And it is what a later read gives back.
    let (_, reread) = call(
        &app,
        request("GET", "/api/v1/me/preferences", Some(&token), None),
    )
    .await;
    assert_eq!(reread["interested_in"], "everyone");
    assert_eq!(reread["min_age"], 25);
}

/// The same trap as `Gender`: Swift does not transform raw values, so these
/// cross the wire exactly as spelled in the enum.
#[tokio::test]
async fn the_gender_preference_wire_values_match_the_swift_raw_values() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "prefs-genre").await;

    for wanted in ["women", "men", "everyone"] {
        let (status, body) = call(
            &app,
            request(
                "PATCH",
                "/api/v1/me/preferences",
                Some(&token),
                Some(json!({
                    "interested_in": wanted,
                    "min_age": 18,
                    "max_age": 99,
                    "max_distance_km": 50,
                    "show_me_on_plum": true
                })),
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK, "« {wanted} » refusé : {body}");
        assert_eq!(body["interested_in"], wanted);
    }
}

/// The app may retry this after a dropped connection without knowing whether
/// the first attempt landed.
#[tokio::test]
async fn completing_the_profile_is_idempotent() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, user) = sign_up_and_token(&app, "onboarding").await;

    assert_eq!(
        user["profile_completed"], false,
        "un compte neuf n'a pas fini l'accueil"
    );

    for attempt in 0..2 {
        let (status, body) = call(
            &app,
            request("POST", "/api/v1/me/profile/complete", Some(&token), None),
        )
        .await;

        assert_eq!(status, StatusCode::OK, "tentative {attempt} : {body}");
        assert_eq!(body["profile_completed"], true);
        // The account shape, not the profile: the client routes on this.
        assert!(body.get("email").is_some(), "réponse inattendue : {body}");
    }
}

#[tokio::test]
async fn a_position_out_of_range_is_refused() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));
    let (token, _) = sign_up_and_token(&app, "position-absurde").await;

    for (latitude, longitude) in [(91.0, 2.35), (48.85, 181.0), (-90.5, 0.0)] {
        let (status, _) = call(
            &app,
            request(
                "PATCH",
                "/api/v1/me/location",
                Some(&token),
                Some(json!({ "latitude": latitude, "longitude": longitude })),
            ),
        )
        .await;

        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "({latitude}, {longitude}) aurait dû être refusé"
        );
    }
}

/// Without this call the deck has nothing to sort by, and the green dot has
/// nothing to go on.
///
/// The coordinates are checked in the row rather than through the API,
/// because nothing exposes them yet: a profile never reports its own
/// position, only the distance others are from it. Asserting only on what the
/// endpoint returns would let it accept a position and drop it on the floor.
#[tokio::test]
async fn a_position_is_stored_and_refreshes_the_activity_stamp() {
    let Some(db) = database().await else { return };
    let reader = db.clone();
    let app = plum_server::app(state(db));
    let (token, user) = sign_up_and_token(&app, "position").await;
    let id: uuid::Uuid = user["id"].as_str().expect("id").parse().expect("uuid");

    let (_, before) = call(
        &app,
        request("GET", "/api/v1/me/profile", Some(&token), None),
    )
    .await;
    // Signing up already marks the account active, so this is never null —
    // what matters is that the position moves it forward.
    let before_active = before["last_active_at"].as_str().expect("last_active_at");

    let (status, _) = call(
        &app,
        request(
            "PATCH",
            "/api/v1/me/location",
            Some(&token),
            Some(json!({ "latitude": 48.8566, "longitude": 2.3522 })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let row = plum_server::entities::profile::Entity::find_by_id(id)
        .one(&reader)
        .await
        .expect("lecture du profil")
        .expect("profil");

    assert_eq!(row.latitude, Some(48.8566));
    assert_eq!(row.longitude, Some(2.3522));

    let (_, after) = call(
        &app,
        request("GET", "/api/v1/me/profile", Some(&token), None),
    )
    .await;
    let after_active = after["last_active_at"].as_str().expect("last_active_at");
    assert_ne!(
        after_active, before_active,
        "la position doit rafraîchir l'horodatage d'activité"
    );
}

/// Every one of these reads or writes someone's own data. A missing token has
/// to be a 401 on all of them, not on most of them.
#[tokio::test]
async fn every_profile_route_refuses_an_anonymous_caller() {
    let Some(db) = database().await else { return };
    let app = plum_server::app(state(db));

    let routes = [
        ("GET", "/api/v1/me/profile", None),
        ("PATCH", "/api/v1/me/profile", Some(json!({ "bio": "x" }))),
        ("POST", "/api/v1/me/profile/complete", None),
        ("GET", "/api/v1/me/preferences", None),
        (
            "PATCH",
            "/api/v1/me/preferences",
            Some(json!({
                "interested_in": "men",
                "min_age": 18,
                "max_age": 30,
                "max_distance_km": 10,
                "show_me_on_plum": true
            })),
        ),
        (
            "PATCH",
            "/api/v1/me/location",
            Some(json!({ "latitude": 0.0, "longitude": 0.0 })),
        ),
    ];

    for (method, path, body) in routes {
        let (status, _) = call(&app, request(method, path, None, body)).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{method} {path} devrait exiger un jeton"
        );
    }
}

/// The handler clamps, and so does the database. This checks the second one is
/// really there: a constraint that was written but never exercised is a
/// comment, and the next endpoint to touch this table would find out the hard
/// way.
#[tokio::test]
async fn the_database_refuses_an_illegal_age_range_of_its_own_accord() {
    let Some(db) = database().await else { return };
    let reader = db.clone();
    let app = plum_server::app(state(db));
    let (_, user) = sign_up_and_token(&app, "contrainte").await;
    let id = user["id"].as_str().expect("id");

    let cases = [
        (17, 40, 50, "preferences_age_range_is_legal"),
        (40, 20, 50, "preferences_age_range_is_legal"),
        (18, 40, 0, "preferences_distance_is_positive"),
    ];

    for (min_age, max_age, distance, constraint) in cases {
        let outcome = reader
            .execute_unprepared(&format!(
                "INSERT INTO preferences (id, interested_in, min_age, max_age, \
                 max_distance_km, show_me_on_plum) \
                 VALUES ('{id}', 'everyone', {min_age}, {max_age}, {distance}, true)"
            ))
            .await;

        // Naming the constraint matters: a plain `is_err` would also pass if
        // the insert had failed on the foreign key or a duplicate, which would
        // prove nothing about the rule under test.
        let error = outcome.expect_err(&format!(
            "({min_age}, {max_age}, {distance}) aurait dû être refusé par la base"
        ));
        let message = error.to_string();
        assert!(
            message.contains(constraint),
            "refusé, mais pas par « {constraint} » : {message}"
        );
    }
}
