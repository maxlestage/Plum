use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, Set, Unchanged};
use uuid::Uuid;

use super::types::*;
use crate::auth::routes::authenticate;
use crate::auth::types::UserResponse;
use crate::entities::{preferences, profile, user};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// A display name has to fit the column; the rest are caps against abuse
/// rather than product rules.
const MAX_DISPLAY_NAME: usize = 80;
const MAX_BIO: usize = 500;
const MAX_CITY: usize = 120;
const MAX_INTERESTS: usize = 12;
const MAX_INTEREST: usize = 40;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/me/profile", get(my_profile).patch(update_profile))
        .route("/me/profile/complete", post(complete_profile))
        .route(
            "/me/preferences",
            get(my_preferences).patch(update_preferences),
        )
        .route("/me/location", patch(update_location))
}

async fn my_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<ProfileResponse>> {
    let claims = authenticate(&state, &headers)?;
    let found = load_profile(&state, claims.sub).await?;
    let mut response = ProfileResponse::own(found);
    crate::photos::routes::attach_one(&state, &mut response).await?;
    Ok(Json(response))
}

async fn update_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ProfileUpdate>,
) -> ApiResult<Json<ProfileResponse>> {
    let claims = authenticate(&state, &headers)?;
    let existing = load_profile(&state, claims.sub).await?;

    let mut update = profile::ActiveModel {
        id: Unchanged(existing.id),
        updated_at: Set(Utc::now().into()),
        ..Default::default()
    };

    if let Some(name) = request.display_name {
        let name = name.trim();
        if name.is_empty() {
            return Err(ApiError::BadRequest("Le prénom est obligatoire.".into()));
        }
        if name.chars().count() > MAX_DISPLAY_NAME {
            return Err(ApiError::BadRequest("Ce prénom est trop long.".into()));
        }
        update.display_name = Set(name.to_owned());
    }

    if let Some(bio) = request.bio {
        let bio = bio.trim();
        if bio.chars().count() > MAX_BIO {
            return Err(ApiError::BadRequest(
                "La description est trop longue.".into(),
            ));
        }
        update.bio = Set(bio.to_owned());
    }

    if let Some(city) = request.city {
        let city = city.trim();
        if city.chars().count() > MAX_CITY {
            return Err(ApiError::BadRequest("Cette ville est trop longue.".into()));
        }
        update.city = Set(city.to_owned());
    }

    if let Some(interests) = request.interests {
        update.interests = Set(tidy_interests(interests)?);
    }

    let saved = update.update(&state.db).await?;
    let mut response = ProfileResponse::own(saved);
    crate::photos::routes::attach_one(&state, &mut response).await?;
    Ok(Json(response))
}

/// Trims, drops the blanks, and removes duplicates while keeping the order the
/// person chose. Case-insensitive: "Cinéma" and "cinéma" are one interest, and
/// showing both on a card looks like a bug because it is one.
fn tidy_interests(raw: Vec<String>) -> ApiResult<Vec<String>> {
    let mut seen = Vec::new();
    let mut kept = Vec::new();

    for interest in raw {
        let interest = interest.trim();
        if interest.is_empty() {
            continue;
        }
        if interest.chars().count() > MAX_INTEREST {
            return Err(ApiError::BadRequest(
                "Un centre d'intérêt est trop long.".into(),
            ));
        }
        let key = interest.to_lowercase();
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        kept.push(interest.to_owned());
    }

    if kept.len() > MAX_INTERESTS {
        return Err(ApiError::BadRequest(format!(
            "Pas plus de {MAX_INTERESTS} centres d'intérêt."
        )));
    }

    Ok(kept)
}

/// Marks onboarding done so the client stops routing to it.
///
/// Idempotent: the app can retry this after a dropped connection without
/// needing to know whether the first attempt landed.
async fn complete_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<UserResponse>> {
    let claims = authenticate(&state, &headers)?;

    // The profile must exist before onboarding can be called finished —
    // otherwise the app leaves onboarding for a screen with nothing on it.
    load_profile(&state, claims.sub).await?;

    let account = user::Entity::find_by_id(claims.sub)
        .one(&state.db)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    if account.profile_completed {
        return Ok(Json(account.into()));
    }

    let saved = user::ActiveModel {
        id: Unchanged(account.id),
        profile_completed: Set(true),
        ..Default::default()
    }
    .update(&state.db)
    .await?;

    Ok(Json(saved.into()))
}

async fn my_preferences(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<PreferencesResponse>> {
    let claims = authenticate(&state, &headers)?;

    // Absent means never opened the settings, not an error: the defaults are
    // the same ones the client starts from.
    let found = preferences::Entity::find_by_id(claims.sub)
        .one(&state.db)
        .await?;

    Ok(Json(match found {
        Some(model) => model.into(),
        None => PreferencesResponse::defaults(),
    }))
}

async fn update_preferences(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PreferencesUpdate>,
) -> ApiResult<Json<PreferencesResponse>> {
    let claims = authenticate(&state, &headers)?;
    let wanted = clamp(request);
    let now = Utc::now();

    let row = preferences::ActiveModel {
        id: Set(claims.sub),
        interested_in: Set(wanted.interested_in.as_str().to_owned()),
        min_age: Set(wanted.min_age),
        max_age: Set(wanted.max_age),
        max_distance_km: Set(wanted.max_distance_km),
        show_me_on_plum: Set(wanted.show_me_on_plum),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };

    // One statement rather than read-then-write: two members of the same
    // account on two devices would otherwise race, and one would lose.
    let saved = preferences::Entity::insert(row)
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(preferences::Column::Id)
                .update_columns([
                    preferences::Column::InterestedIn,
                    preferences::Column::MinAge,
                    preferences::Column::MaxAge,
                    preferences::Column::MaxDistanceKm,
                    preferences::Column::ShowMeOnPlum,
                    preferences::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec_with_returning(&state.db)
        .await?;

    Ok(Json(saved.into()))
}

/// The deck sorts by distance, and without this call it has nothing to sort
/// by. Also doubles as a liveness signal, which is what drives the green dot.
async fn update_location(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LocationUpdate>,
) -> ApiResult<()> {
    let claims = authenticate(&state, &headers)?;

    // Out-of-range coordinates are not a rounding error, they are a bug or a
    // probe; either way they would poison every distance computed from them.
    if !(-90.0..=90.0).contains(&request.latitude) || !(-180.0..=180.0).contains(&request.longitude)
    {
        return Err(ApiError::BadRequest("Position invalide.".into()));
    }

    let existing = load_profile(&state, claims.sub).await?;
    let now = Utc::now();

    profile::ActiveModel {
        id: Unchanged(existing.id),
        latitude: Set(Some(request.latitude)),
        longitude: Set(Some(request.longitude)),
        last_active_at: Set(Some(now.into())),
        updated_at: Set(now.into()),
        ..Default::default()
    }
    .update(&state.db)
    .await?;

    Ok(())
}

async fn load_profile(state: &AppState, id: Uuid) -> ApiResult<profile::Model> {
    profile::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(ApiError::NotFound)
}
