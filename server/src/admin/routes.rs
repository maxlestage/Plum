use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::Deserialize;
use std::collections::HashSet;
use uuid::Uuid;

use super::types::{Named, ReportCursor, ReportRow, ReportsPage};
use crate::entities::{profile, report};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const DEFAULT_LIMIT: u64 = 50;
const MAX_LIMIT: u64 = 200;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ReportsQuery {
    pub limit: Option<u64>,
    pub cursor: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/admin/reports", get(list_reports))
}

/// Le garde de la file de modération.
///
/// Sans `ADMIN_TOKEN` configuré, la route répond `NotFound` et non
/// `Unauthorized`. La différence compte : un `401` annonce au monde entier
/// qu'il existe ici une porte d'administration et invite à chercher sa clé,
/// alors qu'un `404` ne dit rien de plus que n'importe quelle adresse
/// inexistante. Le déploiement par défaut n'a pas de jeton, donc la porte
/// n'existe pas tant que quelqu'un ne l'ouvre pas.
///
/// La comparaison est à temps constant. Sur un réseau, la fuite d'un `==`
/// naïf est noyée dans la gigue, mais elle est réelle, et l'écrire
/// correctement coûte six lignes.
fn admin(state: &AppState, headers: &HeaderMap) -> ApiResult<()> {
    let Some(expected) = state.config.admin_token.as_deref() else {
        return Err(ApiError::NotFound);
    };

    let given = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or("");

    if constant_time_eq(given.as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        Err(ApiError::NotFound)
    }
}

/// Égalité qui ne s'arrête pas au premier octet qui diffère.
///
/// La longueur, elle, fuit forcément — elle est dans le temps de boucle. Ce
/// n'est pas grave : connaître la longueur d'un jeton tiré au sort n'aide pas
/// à le deviner.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Les signalements, du plus récent au plus ancien.
///
/// Écrire un signalement sans jamais le lire, c'est le même défaut que le
/// blocage décoratif : quelqu'un signale un harcèlement, croit avoir prévenu,
/// et personne ne voit rien. Cette route est la plus petite chose qui rende
/// la promesse vraie.
async fn list_reports(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ReportsQuery>,
) -> ApiResult<Json<ReportsPage>> {
    admin(&state, &headers)?;
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    let cursor = match query.cursor.as_deref() {
        Some(raw) => Some(
            ReportCursor::decode(raw)
                .ok_or_else(|| ApiError::BadRequest("Curseur de pagination invalide.".into()))?,
        ),
        None => None,
    };

    let mut select = report::Entity::find();
    if let Some(cursor) = cursor {
        select = select.filter(
            Condition::any()
                .add(report::Column::CreatedAt.lt(cursor.created_at))
                .add(
                    Condition::all()
                        .add(report::Column::CreatedAt.eq(cursor.created_at))
                        .add(report::Column::Id.lt(cursor.id)),
                ),
        );
    }

    let rows = select
        .order_by_desc(report::Column::CreatedAt)
        .order_by_desc(report::Column::Id)
        .limit(limit + 1)
        .all(&state.db)
        .await?;

    let has_more = rows.len() as u64 > limit;
    let rows: Vec<_> = rows.into_iter().take(limit as usize).collect();

    let next_cursor = has_more
        .then(|| {
            rows.last().map(|last| {
                ReportCursor {
                    created_at: last.created_at.into(),
                    id: last.id,
                }
                .encode()
            })
        })
        .flatten();

    // Les noms en une requête plutôt qu'une par ligne : une file de
    // modération se lit par cinquante, et cent allers-retours pour l'afficher
    // la rendraient inutilisable au moment où elle sert.
    let nommes: Vec<Uuid> = rows
        .iter()
        .flat_map(|r| [r.reporter_id, r.reported_id])
        .flatten()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let profils = profile::Entity::find()
        .filter(profile::Column::Id.is_in(nommes))
        .all(&state.db)
        .await?;

    let nomme = |id: Option<Uuid>| -> Option<Named> {
        let id = id?;
        Some(Named {
            id,
            display_name: profils
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.display_name.clone()),
        })
    };

    let mut items = Vec::with_capacity(rows.len());
    for row in &rows {
        // Les deux compteurs, par personne signalée. Une requête par ligne
        // serait de trop, mais la page en contient au plus deux cents et les
        // signalements sont rares : c'est le bon compromis tant que la table
        // tient dans un index.
        let (reported_total, distinct_reporters) = match row.reported_id {
            Some(cible) => {
                let total = report::Entity::find()
                    .filter(report::Column::ReportedId.eq(cible))
                    .count(&state.db)
                    .await?;
                let distincts = report::Entity::find()
                    .filter(report::Column::ReportedId.eq(cible))
                    .select_only()
                    .column(report::Column::ReporterId)
                    .distinct()
                    .into_tuple::<Option<Uuid>>()
                    .all(&state.db)
                    .await?
                    .len() as u64;
                (total, distincts)
            }
            None => (0, 0),
        };

        items.push(ReportRow {
            id: row.id,
            reason: row.reason.clone(),
            created_at: row.created_at.into(),
            reporter: nomme(row.reporter_id),
            reported: nomme(row.reported_id),
            reported_total,
            distinct_reporters,
        });
    }

    Ok(Json(ReportsPage { items, next_cursor }))
}
