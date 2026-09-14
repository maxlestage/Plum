use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{NaiveTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use uuid::Uuid;

use super::deck_query::{deck as query_deck, Candidate};
use super::types::*;
use crate::auth::routes::authenticate;
use crate::auth::types::Gender;
use crate::entities::{block, match_pair, profile, report, swipe};
use crate::error::{ApiError, ApiResult};
use crate::live::routes::announce_match;
use crate::profile::types::ProfileResponse;
use crate::state::AppState;

/// The client asks for 20. The cap is what stops someone asking for a
/// million and making the server sort the whole table.
const DEFAULT_LIMIT: u32 = 20;
const MAX_LIMIT: u32 = 50;
const MAX_REASON: usize = 500;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/discovery/deck", get(deck))
        .route("/discovery/swipes", post(swipe_route))
        .route("/discovery/rewind", post(rewind))
        .route("/profiles/{id}/report", post(report_profile))
        .route("/profiles/{id}/block", post(block_profile))
}

fn candidate_into_profile(candidate: Candidate) -> ProfileResponse {
    ProfileResponse {
        id: candidate.id,
        display_name: candidate.display_name,
        birth_date: candidate.birth_date.and_time(NaiveTime::MIN).and_utc(),
        gender: Gender::parse(&candidate.gender).unwrap_or(Gender::Other),
        bio: candidate.bio,
        city: candidate.city,
        photos: Vec::new(),
        interests: candidate.interests,
        distance_km: candidate.distance_km,
        last_active_at: candidate.last_active_at.map(Into::into),
    }
}

async fn deck(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DeckQuery>,
) -> ApiResult<Json<Page<ProfileResponse>>> {
    let claims = authenticate(&state, &headers)?;
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    // A cursor the server cannot read is a bad request, not an empty deck:
    // silently starting over would look like the deck looping.
    let cursor = match query.cursor.as_deref() {
        Some(raw) => Some(
            Cursor::decode(raw)
                .ok_or_else(|| ApiError::BadRequest("Curseur de pagination invalide.".into()))?,
        ),
        None => None,
    };

    // One more than asked for: if it comes back, there is another page, and
    // we know it without a second count query.
    let mut candidates = query_deck(&state.db, claims.sub, limit + 1, cursor).await?;

    let next_cursor = if candidates.len() as u32 > limit {
        candidates.truncate(limit as usize);
        candidates.last().map(|last| {
            Cursor {
                sort_km: last.sort_km,
                id: last.id,
            }
            .encode()
        })
    } else {
        None
    };

    let mut items: Vec<ProfileResponse> =
        candidates.into_iter().map(candidate_into_profile).collect();
    // Une requête pour les vingt cartes, pas vingt.
    crate::photos::routes::attach(&state, &mut items.iter_mut().collect::<Vec<_>>()).await?;

    Ok(Json(Page { items, next_cursor }))
}

async fn swipe_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SwipeRequest>,
) -> ApiResult<Json<SwipeOutcome>> {
    let claims = authenticate(&state, &headers)?;
    let viewer = claims.sub;
    let target = request.target_profile_id;

    if viewer == target {
        return Err(ApiError::BadRequest(
            "On ne peut pas se juger soi-même.".into(),
        ));
    }

    // The target has to exist, or a typo would be recorded as a verdict on
    // nobody and quietly remove a card that was never there.
    let target_profile = profile::Entity::find_by_id(target)
        .one(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;

    let decision = request.decision;
    let now = Utc::now();

    // The verdict and the match it may create go in together. A like written
    // without its match would leave two people who both said yes and never
    // hear about it, and no later request would put it right.
    let created = state
        .db
        .transaction::<_, Option<match_pair::Model>, sea_orm::DbErr>(|txn| {
            Box::pin(async move {
                swipe::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    viewer_id: Set(viewer),
                    target_id: Set(target),
                    decision: Set(decision.as_str().to_owned()),
                    created_at: Set(now.into()),
                }
                .insert(txn)
                .await?;

                if !decision.is_affirmative() {
                    return Ok(None);
                }

                // Did they already say yes to us?
                let reciprocal = swipe::Entity::find()
                    .filter(swipe::Column::ViewerId.eq(target))
                    .filter(swipe::Column::TargetId.eq(viewer))
                    .one(txn)
                    .await?;

                let mutual = reciprocal
                    .and_then(|other| SwipeDecision::parse(&other.decision))
                    .is_some_and(SwipeDecision::is_affirmative);

                if !mutual {
                    return Ok(None);
                }

                let (lower, upper) = match_pair::ordered(viewer, target);
                let created = match_pair::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    lower_id: Set(lower),
                    upper_id: Set(upper),
                    matched_at: Set(now.into()),
                }
                .insert(txn)
                .await?;

                Ok(Some(created))
            })
        })
        .await
        .map_err(|error| match error {
            // The unique key on (viewer, target) speaking: the card was
            // already judged. Answering 409 lets the client drop a stale card
            // rather than show a failure for something already done.
            sea_orm::TransactionError::Transaction(sea_orm::DbErr::Query(_))
            | sea_orm::TransactionError::Transaction(sea_orm::DbErr::Exec(_)) => {
                ApiError::BadRequest("Ce profil a déjà été jugé.".into())
            }
            sea_orm::TransactionError::Transaction(other) => ApiError::from(other),
            sea_orm::TransactionError::Connection(other) => ApiError::from(other),
        })?;

    // L'autre l'apprend par le socket s'il est là, sinon en rouvrant
    // l'application : un match est en base avant d'être annoncé, donc rien ne
    // se perd quand personne n'écoute.
    if let Some(model) = &created {
        if let Some(mine) = profile::Entity::find_by_id(viewer).one(&state.db).await? {
            // Le profil de celui qui vient de balayer : c'est lui que l'autre
            // découvre, pas le sien — photos comprises, sans quoi la carte
            // « c'est un match » s'ouvrirait sur un dégradé.
            let mut profile = ProfileResponse::own(mine);
            crate::photos::routes::attach_one(&state, &mut profile).await?;
            announce_match(
                &state,
                target,
                MatchResponse {
                    id: model.id,
                    matched_at: model.matched_at.into(),
                    profile,
                    conversation_id: None,
                },
            );
        }
    }

    let matched = created.is_some();
    let mut r#match = created.map(|model| MatchResponse {
        id: model.id,
        matched_at: model.matched_at.into(),
        profile: ProfileResponse::own(target_profile),
        conversation_id: None,
    });
    if let Some(found) = r#match.as_mut() {
        crate::photos::routes::attach_one(&state, &mut found.profile).await?;
    }

    let response = SwipeOutcome {
        matched,
        r#match,
        likes_remaining: None,
    };

    Ok(Json(response))
}

/// Undoes the last pass.
///
/// The row is deleted rather than marked undone: the deck excludes anyone
/// already judged, so a row left in place would keep the profile hidden and
/// the rewind would appear to do nothing.
async fn rewind(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<RewindResponse>> {
    let claims = authenticate(&state, &headers)?;

    let last_pass = swipe::Entity::find()
        .filter(swipe::Column::ViewerId.eq(claims.sub))
        .filter(swipe::Column::Decision.eq(SwipeDecision::Pass.as_str()))
        .order_by_desc(swipe::Column::CreatedAt)
        .one(&state.db)
        .await?;

    let Some(last_pass) = last_pass else {
        return Ok(Json(RewindResponse { profile: None }));
    };

    let restored = profile::Entity::find_by_id(last_pass.target_id)
        .one(&state.db)
        .await?;

    swipe::Entity::delete_by_id(last_pass.id)
        .exec(&state.db)
        .await?;

    // The account may have gone since the pass. The verdict is still undone —
    // there is simply no card to hand back.
    Ok(Json(RewindResponse {
        profile: restored.map(ProfileResponse::own),
    }))
}

async fn report_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<ReportRequest>,
) -> ApiResult<()> {
    let claims = authenticate(&state, &headers)?;

    let reason = request.reason.trim();
    if reason.is_empty() {
        return Err(ApiError::BadRequest("Le motif est obligatoire.".into()));
    }
    if reason.chars().count() > MAX_REASON {
        return Err(ApiError::BadRequest("Le motif est trop long.".into()));
    }

    report::ActiveModel {
        id: Set(Uuid::new_v4()),
        reporter_id: Set(Some(claims.sub)),
        reported_id: Set(Some(id)),
        reason: Set(reason.to_owned()),
        created_at: Set(Utc::now().into()),
    }
    .insert(&state.db)
    .await?;

    Ok(())
}

/// Blocking is idempotent: someone pressing it twice is telling us the same
/// thing, and an error would read as though it had not worked.
async fn block_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<()> {
    let claims = authenticate(&state, &headers)?;

    if claims.sub == id {
        return Err(ApiError::BadRequest(
            "On ne peut pas se bloquer soi-même.".into(),
        ));
    }

    let existing = block::Entity::find()
        .filter(block::Column::BlockerId.eq(claims.sub))
        .filter(block::Column::BlockedId.eq(id))
        .one(&state.db)
        .await?;

    if existing.is_some() {
        return Ok(());
    }

    block::ActiveModel {
        id: Set(Uuid::new_v4()),
        blocker_id: Set(claims.sub),
        blocked_id: Set(id),
        created_at: Set(Utc::now().into()),
    }
    .insert(&state.db)
    .await?;

    Ok(())
}
