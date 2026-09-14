use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use uuid::Uuid;

use super::types::{MatchCursor, MatchesPage};
use crate::auth::routes::authenticate;
use crate::discovery::types::MatchResponse;
use crate::entities::{block, match_pair, profile};
use crate::error::{ApiError, ApiResult};
use crate::profile::types::ProfileResponse;
use crate::state::AppState;

const DEFAULT_LIMIT: u64 = 30;
const MAX_LIMIT: u64 = 100;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MatchesQuery {
    pub limit: Option<u64>,
    pub cursor: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/matches", get(list))
        .route("/matches/{id}", axum::routing::delete(unmatch))
}

/// Les matchs, du plus récent au plus ancien.
async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<MatchesQuery>,
) -> ApiResult<Json<MatchesPage>> {
    let claims = authenticate(&state, &headers)?;
    let viewer = claims.sub;
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    let cursor = match query.cursor.as_deref() {
        Some(raw) => Some(
            MatchCursor::decode(raw)
                .ok_or_else(|| ApiError::BadRequest("Curseur de pagination invalide.".into()))?,
        ),
        None => None,
    };

    // Un match est rangé par paire ordonnée, sans notion de sens : on est donc
    // d'un côté ou de l'autre.
    let mine = Condition::any()
        .add(match_pair::Column::LowerId.eq(viewer))
        .add(match_pair::Column::UpperId.eq(viewer));

    let mut select = match_pair::Entity::find().filter(mine);

    if let Some(cursor) = cursor {
        // Même raison que pour le deck : comparer la seule date sauterait ou
        // répéterait les matchs nés dans la même microseconde. Écrit en deux
        // branches plutôt qu'en n-uplet, parce que l'ordre est décroissant.
        select = select.filter(
            Condition::any()
                .add(match_pair::Column::MatchedAt.lt(cursor.matched_at))
                .add(
                    Condition::all()
                        .add(match_pair::Column::MatchedAt.eq(cursor.matched_at))
                        .add(match_pair::Column::Id.lt(cursor.id)),
                ),
        );
    }

    let rows = select
        .order_by_desc(match_pair::Column::MatchedAt)
        .order_by_desc(match_pair::Column::Id)
        // Un de plus que demandé : s'il revient, il y a une page suivante, et
        // on le sait sans seconde requête.
        .limit(limit + 1)
        .all(&state.db)
        .await?;

    let has_more = rows.len() as u64 > limit;
    let rows: Vec<_> = rows.into_iter().take(limit as usize).collect();

    let next_cursor = has_more.then(|| {
        rows.last().map(|last| {
            MatchCursor {
                matched_at: last.matched_at.into(),
                id: last.id,
            }
            .encode()
        })
    });

    // Les profils en une requête plutôt qu'une par match : trente allers-retours
    // pour une liste qu'on ouvre à chaque lancement, c'est le genre de détail
    // qui rend une application lente sans qu'on sache pourquoi.
    let others: Vec<Uuid> = rows.iter().map(|m| m.other(viewer)).collect();
    let profiles = profile::Entity::find()
        .filter(profile::Column::Id.is_in(others.clone()))
        .all(&state.db)
        .await?;

    // Bloquer supprime le match ; cette liste ne devrait donc rien avoir à
    // filtrer. Elle filtre quand même, en une requête bornée à la page, parce
    // que le blocage n'a pas toujours supprimé : jusqu'à ce changement il se
    // contentait d'écrire sa ligne. Tout blocage déjà enregistré a donc encore
    // son match — combien, on ne le sait pas d'ici, et c'est justement
    // pourquoi on ne parie pas dessus. Une liste de matchs qui affiche encore
    // quelqu'un qu'on a bloqué est un manquement visible, pas un détail de
    // cohérence.
    let walls = block::Entity::find()
        .filter(
            Condition::any()
                .add(
                    Condition::all()
                        .add(block::Column::BlockerId.eq(viewer))
                        .add(block::Column::BlockedId.is_in(others.clone())),
                )
                .add(
                    Condition::all()
                        .add(block::Column::BlockedId.eq(viewer))
                        .add(block::Column::BlockerId.is_in(others)),
                ),
        )
        .all(&state.db)
        .await?;

    let items = rows
        .iter()
        .filter_map(|m| {
            let other = m.other(viewer);
            if walls
                .iter()
                .any(|w| w.blocker_id == other || w.blocked_id == other)
            {
                return None;
            }
            // Le compte a pu partir depuis : le match existe encore le temps
            // que la cascade passe, mais il n'y a plus de carte à montrer.
            let found = profiles.iter().find(|p| p.id == other)?.clone();
            Some(MatchResponse {
                id: m.id,
                profile: ProfileResponse::own(found),
                matched_at: m.matched_at.into(),
                conversation_id: None,
            })
        })
        .collect::<Vec<MatchResponse>>();

    let mut items = items;
    crate::photos::routes::attach(
        &state,
        &mut items.iter_mut().map(|m| &mut m.profile).collect::<Vec<_>>(),
    )
    .await?;

    Ok(Json(MatchesPage {
        items,
        next_cursor: next_cursor.flatten(),
    }))
}

/// Défait un match.
///
/// Les verdicts restent en place, délibérément : le deck exclut tout profil
/// déjà jugé, donc les effacer ferait réapparaître la personne dont on vient
/// de se séparer. Défaire un match, c'est ne plus vouloir la voir, pas
/// recommencer avec elle.
async fn unmatch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<()> {
    let claims = authenticate(&state, &headers)?;

    let found = match_pair::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;

    // Un match auquel on n'appartient pas ne se défait pas, et son existence
    // ne se confirme pas non plus.
    if found.lower_id != claims.sub && found.upper_id != claims.sub {
        return Err(ApiError::NotFound);
    }

    match_pair::Entity::delete_by_id(id).exec(&state.db).await?;
    Ok(())
}
