use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::Deserialize;
use std::collections::HashSet;
use uuid::Uuid;

use super::types::{
    AccountState, Named, ReportCursor, ReportRow, ReportsPage, ResolveRequest, SuspendRequest,
};
use crate::entities::{moderation_action, profile, refresh_token, report, user};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const DEFAULT_LIMIT: u64 = 50;
const MAX_LIMIT: u64 = 200;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ReportsQuery {
    pub limit: Option<u64>,
    pub cursor: Option<String>,
    /// `open` (par défaut) ou `all`.
    ///
    /// Par défaut la file ne montre que ce qui reste à faire. C'est ce qui la
    /// rend utilisable : une file qui remonte tout depuis le début ne se vide
    /// jamais, et au bout de deux semaines plus personne ne l'ouvre.
    pub state: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/reports", get(list_reports))
        .route("/admin/reports/{id}/resolution", post(resolve_report))
        .route(
            "/admin/profiles/{id}/suspension",
            post(suspend_account).delete(lift_suspension),
        )
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

    let only_open = match query.state.as_deref() {
        None | Some("open") => true,
        Some("all") => false,
        Some(autre) => {
            // Un filtre mal orthographié qui rend la file entière ferait
            // croire qu'il n'y a rien à faire, ou l'inverse. Les deux se
            // remarquent trop tard.
            return Err(ApiError::BadRequest(format!(
                "state doit valoir « open » ou « all », pas « {autre} »."
            )));
        }
    };

    let mut select = report::Entity::find();
    if only_open {
        select = select.filter(report::Column::ResolvedAt.is_null());
    }
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
        .filter(profile::Column::Id.is_in(nommes.clone()))
        .all(&state.db)
        .await?;

    // Qui, parmi les personnes nommées, est déjà suspendu. En une requête :
    // c'est l'information qui décide s'il reste quelque chose à faire sur une
    // ligne, donc elle doit être là pour les cinquante, pas au prix de
    // cinquante allers-retours.
    let suspendus: HashSet<Uuid> = user::Entity::find()
        .filter(user::Column::Id.is_in(nommes))
        .filter(user::Column::SuspendedAt.is_not_null())
        .select_only()
        .column(user::Column::Id)
        .into_tuple::<Uuid>()
        .all(&state.db)
        .await?
        .into_iter()
        .collect();

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
            reported_suspended: row
                .reported_id
                .is_some_and(|cible| suspendus.contains(&cible)),
            resolved_at: row.resolved_at.map(Into::into),
            resolution: row.resolution.clone(),
        });
    }

    Ok(Json(ReportsPage { items, next_cursor }))
}

/// Juger un signalement, et le sortir de la file.
///
/// Sans ce geste, la file ne se vide pas : le même signalement remonte à
/// chaque ouverture, personne ne sait ce qui a déjà été regardé, et deux
/// modérateurs traitent la même ligne sans le savoir. Une file qui ne se vide
/// pas cesse d'être ouverte au bout de deux semaines.
///
/// Juger n'est pas suspendre. « Classé sans suite » est une décision à part
/// entière, et elle est tracée comme l'autre : c'est elle qu'on relit quand
/// quelqu'un demande pourquoi rien n'a été fait.
async fn resolve_report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<ResolveRequest>,
) -> ApiResult<Json<ReportRow>> {
    admin(&state, &headers)?;

    let found = report::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;

    // Rejuger une ligne déjà jugée écraserait la première décision et sa date
    // sans laisser de trace de la première. Le refus est un conflit, pas une
    // erreur : la ligne existe, elle est simplement déjà traitée.
    if found.resolved_at.is_some() {
        return Err(ApiError::Conflict("Ce signalement a déjà été jugé.".into()));
    }

    let now = Utc::now();
    let cible = found.reported_id;
    let mut modifie: report::ActiveModel = found.into();
    modifie.resolved_at = Set(Some(now.into()));
    modifie.resolution = Set(Some(request.resolution.as_str().to_string()));
    let sauve = modifie.update(&state.db).await?;

    trace(
        &state,
        cible,
        Some(id),
        request.resolution.as_str(),
        request.note.as_deref().unwrap_or(""),
    )
    .await?;

    Ok(Json(ReportRow {
        id: sauve.id,
        reason: sauve.reason.clone(),
        created_at: sauve.created_at.into(),
        reporter: named(&state, sauve.reporter_id).await?,
        reported: named(&state, sauve.reported_id).await?,
        reported_total: 0,
        distinct_reporters: 0,
        reported_suspended: is_suspended(&state, sauve.reported_id).await?,
        resolved_at: sauve.resolved_at.map(Into::into),
        resolution: sauve.resolution.clone(),
    }))
}

/// Fermer un compte.
///
/// Ce que ça fait vraiment, et c'est le seul endroit où c'est écrit en entier :
///
/// - le profil sort des decks — plus personne ne le voit ;
/// - les jetons de rafraîchissement sont révoqués, donc la session en cours
///   meurt au plus tard à l'expiration du jeton d'accès ;
/// - la connexion est refusée, avec un message qui dit pourquoi.
///
/// **La fenêtre est de quinze minutes**, la durée d'un jeton d'accès. Une
/// vérification en base à chaque requête la fermerait, au prix d'un aller en
/// base sur *toutes* les routes authentifiées — le jeton est autoportant
/// précisément pour éviter ça. Le compromis retenu : la révocation est
/// immédiate là où le mal se fait, l'envoi d'un message, et bornée à un quart
/// d'heure ailleurs. C'est écrit ici plutôt que sous-entendu, parce qu'un
/// modérateur qui suspend un harceleur a le droit de savoir ce qu'il vient
/// exactement d'obtenir.
///
/// Le compte n'est **pas** supprimé : une suspension se lève, et les messages
/// qu'elle sanctionne sont la preuve de la décision.
async fn suspend_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<SuspendRequest>,
) -> ApiResult<Json<AccountState>> {
    admin(&state, &headers)?;

    let motif = request.reason.trim();
    if motif.is_empty() {
        return Err(ApiError::BadRequest(
            "Une suspension doit dire pourquoi.".into(),
        ));
    }

    let compte = user::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;

    // Suspendre un compte déjà suspendu décalerait sa date de fermeture, ce
    // qui réécrit le passé. La réponse dit l'état, qui est celui voulu.
    if compte.suspended_at.is_some() {
        return Ok(Json(account_state(&state, compte).await?));
    }

    let now = Utc::now();
    let mut modifie: user::ActiveModel = compte.into();
    modifie.suspended_at = Set(Some(now.into()));
    modifie.updated_at = Set(now.into());
    let ferme = modifie.update(&state.db).await?;

    revoke_sessions(&state, id).await?;
    trace(&state, Some(id), None, "suspended", motif).await?;

    Ok(Json(account_state(&state, ferme).await?))
}

/// Rouvrir un compte.
///
/// La trace de la suspension reste : lever une décision écrit une ligne de
/// plus, elle n'efface pas celle d'avant. Un historique qui se réécrit ne
/// vaut rien devant une contestation.
///
/// Les sessions ne reviennent pas — elles ont été révoquées et un jeton
/// révoqué le reste. La personne se reconnecte, ce qui est de toute façon ce
/// qu'elle fera en trouvant l'application fermée.
async fn lift_suspension(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<AccountState>> {
    admin(&state, &headers)?;

    let compte = user::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;

    if compte.suspended_at.is_none() {
        return Ok(Json(account_state(&state, compte).await?));
    }

    let mut modifie: user::ActiveModel = compte.into();
    modifie.suspended_at = Set(None);
    modifie.updated_at = Set(Utc::now().into());
    let ouvert = modifie.update(&state.db).await?;

    trace(&state, Some(id), None, "lifted", "").await?;

    Ok(Json(account_state(&state, ouvert).await?))
}

/// Écrit la décision, quelle qu'elle soit.
///
/// Appelée pour les quatre issues — suspension, levée, signalement retenu,
/// signalement classé — et jamais mise à jour ensuite. C'est une trace, pas un
/// état.
async fn trace(
    state: &AppState,
    subject: Option<Uuid>,
    report_id: Option<Uuid>,
    action: &str,
    reason: &str,
) -> ApiResult<()> {
    moderation_action::ActiveModel {
        id: Set(Uuid::new_v4()),
        subject_id: Set(subject),
        report_id: Set(report_id),
        action: Set(action.to_string()),
        reason: Set(reason.to_string()),
        created_at: Set(Utc::now().into()),
    }
    .insert(&state.db)
    .await?;
    Ok(())
}

/// Coupe toutes les sessions d'un compte.
///
/// Les jetons de rafraîchissement, pas seulement celui de l'appareil courant :
/// quelqu'un qu'on suspend a souvent plus d'un appareil, et en laisser un
/// ouvert revient à ne rien avoir fait.
async fn revoke_sessions(state: &AppState, owner: Uuid) -> ApiResult<()> {
    refresh_token::Entity::update_many()
        .col_expr(
            refresh_token::Column::RevokedAt,
            sea_orm::sea_query::Expr::value(Some(Utc::now().fixed_offset())),
        )
        .filter(refresh_token::Column::UserId.eq(owner))
        .filter(refresh_token::Column::RevokedAt.is_null())
        .exec(&state.db)
        .await?;
    Ok(())
}

async fn account_state(state: &AppState, compte: user::Model) -> ApiResult<AccountState> {
    let decisions = moderation_action::Entity::find()
        .filter(moderation_action::Column::SubjectId.eq(compte.id))
        .count(&state.db)
        .await?;
    Ok(AccountState {
        id: compte.id,
        suspended: compte.suspended_at.is_some(),
        suspended_at: compte.suspended_at.map(Into::into),
        decisions,
    })
}

async fn named(state: &AppState, id: Option<Uuid>) -> ApiResult<Option<Named>> {
    let Some(id) = id else { return Ok(None) };
    let profil = profile::Entity::find_by_id(id).one(&state.db).await?;
    Ok(Some(Named {
        id,
        display_name: profil.map(|p| p.display_name),
    }))
}

async fn is_suspended(state: &AppState, id: Option<Uuid>) -> ApiResult<bool> {
    let Some(id) = id else { return Ok(false) };
    Ok(user::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .is_some_and(|u| u.suspended_at.is_some()))
}
