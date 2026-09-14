use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, Unchanged,
};
use uuid::Uuid;

use super::types::*;
use crate::auth::routes::{authenticate, enforce};
use crate::entities::{block, conversation, match_pair, message, profile};
use crate::error::{ApiError, ApiResult};
use crate::live::routes::{announce_message, announce_read};
use crate::profile::types::ProfileResponse;
use crate::rate_limit::Quota;
use crate::state::AppState;

const DEFAULT_LIMIT: u64 = 30;
const MAX_LIMIT: u64 = 100;
/// Assez pour dire quelque chose, pas assez pour coller un roman dans une
/// bulle. Le client n'impose rien, donc c'est ici que ça se joue.
const MAX_BODY: usize = 2_000;
/// Soixante messages par minute et par expéditeur.
///
/// Rien n'encadrait l'envoi. Un compte pouvait remplir une conversation — et
/// la base — aussi vite que le réseau le permettait, et le socket poussait
/// tout en direct : le téléphone d'en face vibrait sans discontinuer. C'est
/// l'inondation, la forme de harcèlement la moins chère à produire.
///
/// Soixante parce qu'une conversation animée en compte dix ou vingt à la
/// minute : la marge est de trois fois, personne ne rencontrera ce plafond en
/// écrivant. Un script, lui, le touche à la seconde.
const MESSAGE_QUOTA: Quota = Quota::new(60, 60);

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/matches/{id}/conversation", post(open_conversation))
        .route("/conversations", get(list_conversations))
        .route(
            "/conversations/{id}/messages",
            get(list_messages).post(send_message),
        )
        .route("/conversations/{id}/read", post(mark_read))
}

/// Le match dont on fait partie, ou `NotFound`.
///
/// Introuvable plutôt qu'interdit : confirmer l'existence d'un match auquel on
/// n'appartient pas renseignerait déjà.
///
/// Un blocage, dans un sens ou dans l'autre, referme la conversation. Bloquer
/// supprime déjà le match, donc ce contrôle est une seconde barrière — mais
/// c'est celle qui compte : elle couvre la course entre un message en vol et
/// un blocage qui vient d'être posé, et elle tient même si un chemin futur
/// oubliait de supprimer le match.
async fn my_match(state: &AppState, viewer: Uuid, id: Uuid) -> ApiResult<match_pair::Model> {
    let found = match_pair::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;

    if found.lower_id != viewer && found.upper_id != viewer {
        return Err(ApiError::NotFound);
    }
    if blocked_between(state, found.lower_id, found.upper_id).await? {
        return Err(ApiError::NotFound);
    }
    Ok(found)
}

/// Un blocage existe-t-il entre ces deux-là, dans un sens ou dans l'autre ?
///
/// Dans les deux sens, délibérément : celui qui bloque ne veut plus rien
/// recevoir, et celui qui est bloqué ne doit pas pouvoir continuer d'écrire.
/// Une barrière à sens unique laisserait exactement le harcèlement qu'elle
/// prétend arrêter.
pub async fn blocked_between(state: &AppState, a: Uuid, b: Uuid) -> ApiResult<bool> {
    let found = block::Entity::find()
        .filter(
            Condition::any()
                .add(
                    Condition::all()
                        .add(block::Column::BlockerId.eq(a))
                        .add(block::Column::BlockedId.eq(b)),
                )
                .add(
                    Condition::all()
                        .add(block::Column::BlockerId.eq(b))
                        .add(block::Column::BlockedId.eq(a)),
                ),
        )
        .one(&state.db)
        .await?;
    Ok(found.is_some())
}

/// La conversation dont on fait partie, avec son match.
async fn my_conversation(
    state: &AppState,
    viewer: Uuid,
    id: Uuid,
) -> ApiResult<(conversation::Model, match_pair::Model)> {
    let found = conversation::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    let pair = my_match(state, viewer, found.match_id).await?;
    Ok((found, pair))
}

/// Ouvre la conversation d'un match, ou rend celle qui existe déjà.
///
/// Idempotent par la clé unique sur `match_id` : deux appareils qui ouvrent
/// l'écran en même temps ne peuvent pas créer deux fils, ce qui couperait la
/// discussion en deux moitiés invisibles l'une à l'autre.
async fn open_conversation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ConversationResponse>> {
    let claims = authenticate(&state, &headers)?;
    let pair = my_match(&state, claims.sub, id).await?;
    let now = Utc::now();

    let row = conversation::ActiveModel {
        id: Set(Uuid::new_v4()),
        match_id: Set(pair.id),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };

    // `DO NOTHING` puis relecture plutôt qu'un `DO UPDATE` : il n'y a rien à
    // mettre à jour, et toucher `updated_at` remonterait la conversation en
    // haut de la liste pour un simple écran ouvert.
    conversation::Entity::insert(row)
        .on_conflict(
            OnConflict::column(conversation::Column::MatchId)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(&state.db)
        .await?;

    let found = conversation::Entity::find()
        .filter(conversation::Column::MatchId.eq(pair.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Conversation introuvable après création.".into()))?;

    Ok(Json(hydrate(&state, claims.sub, found, &pair).await?))
}

/// Complète une conversation avec ce que le client affiche dans sa liste.
async fn hydrate(
    state: &AppState,
    viewer: Uuid,
    row: conversation::Model,
    pair: &match_pair::Model,
) -> ApiResult<ConversationResponse> {
    let other = pair.other(viewer);
    let participant = profile::Entity::find_by_id(other)
        .one(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;

    let last_message = message::Entity::find()
        .filter(message::Column::ConversationId.eq(row.id))
        .order_by_desc(message::Column::SentAt)
        .order_by_desc(message::Column::Id)
        .one(&state.db)
        .await?;

    // Ce que *celui qui demande* n'a pas lu : jamais ses propres messages.
    let unread = message::Entity::find()
        .filter(message::Column::ConversationId.eq(row.id))
        .filter(message::Column::SenderId.ne(viewer))
        .filter(message::Column::ReadAt.is_null())
        .count(&state.db)
        .await?;

    let mut participant = ProfileResponse::own(participant);
    crate::photos::routes::attach_one(state, &mut participant).await?;

    Ok(ConversationResponse {
        id: row.id,
        match_id: row.match_id,
        participant,
        last_message: last_message.map(Into::into),
        unread_count: unread.try_into().unwrap_or(i32::MAX),
        updated_at: row.updated_at.into(),
    })
}

async fn list_conversations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ConversationsQuery>,
) -> ApiResult<Json<Page<ConversationResponse>>> {
    let claims = authenticate(&state, &headers)?;
    let viewer = claims.sub;
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    let cursor = match query.cursor.as_deref() {
        Some(raw) => Some(
            ThreadCursor::decode(raw)
                .ok_or_else(|| ApiError::BadRequest("Curseur de pagination invalide.".into()))?,
        ),
        None => None,
    };

    let mine = Condition::any()
        .add(match_pair::Column::LowerId.eq(viewer))
        .add(match_pair::Column::UpperId.eq(viewer));
    let pairs = match_pair::Entity::find()
        .filter(mine)
        .all(&state.db)
        .await?;

    // La liste ne passe pas par `my_match`, donc elle doit écarter les
    // bloqués elle-même — sans quoi une conversation fermée resterait
    // affichée, et son dernier message avec.
    let mut visible = Vec::with_capacity(pairs.len());
    for pair in pairs {
        if !blocked_between(&state, pair.lower_id, pair.upper_id).await? {
            visible.push(pair);
        }
    }
    let pairs = visible;
    let pair_ids: Vec<Uuid> = pairs.iter().map(|p| p.id).collect();

    let mut select =
        conversation::Entity::find().filter(conversation::Column::MatchId.is_in(pair_ids));

    if let Some(cursor) = cursor {
        select = select.filter(
            Condition::any()
                .add(conversation::Column::UpdatedAt.lt(cursor.at))
                .add(
                    Condition::all()
                        .add(conversation::Column::UpdatedAt.eq(cursor.at))
                        .add(conversation::Column::Id.lt(cursor.id)),
                ),
        );
    }

    let rows = select
        .order_by_desc(conversation::Column::UpdatedAt)
        .order_by_desc(conversation::Column::Id)
        .limit(limit + 1)
        .all(&state.db)
        .await?;

    let has_more = rows.len() as u64 > limit;
    let rows: Vec<_> = rows.into_iter().take(limit as usize).collect();

    let next_cursor = has_more
        .then(|| {
            rows.last().map(|last| {
                ThreadCursor {
                    at: last.updated_at.into(),
                    id: last.id,
                }
                .encode()
            })
        })
        .flatten();

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(pair) = pairs.iter().find(|p| p.id == row.match_id) else {
            continue;
        };
        items.push(hydrate(&state, viewer, row, pair).await?);
    }

    Ok(Json(Page { items, next_cursor }))
}

/// Le fil, du plus récent au plus ancien.
async fn list_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<MessagesQuery>,
) -> ApiResult<Json<Page<MessageResponse>>> {
    let claims = authenticate(&state, &headers)?;
    let (row, _) = my_conversation(&state, claims.sub, id).await?;
    let limit = query.limit.unwrap_or(50).clamp(1, MAX_LIMIT);

    let before = match query.before.as_deref() {
        Some(raw) => Some(
            ThreadCursor::decode(raw)
                .ok_or_else(|| ApiError::BadRequest("Curseur de pagination invalide.".into()))?,
        ),
        None => None,
    };

    let mut select = message::Entity::find().filter(message::Column::ConversationId.eq(row.id));

    if let Some(before) = before {
        select = select.filter(
            Condition::any()
                .add(message::Column::SentAt.lt(before.at))
                .add(
                    Condition::all()
                        .add(message::Column::SentAt.eq(before.at))
                        .add(message::Column::Id.lt(before.id)),
                ),
        );
    }

    let rows = select
        .order_by_desc(message::Column::SentAt)
        .order_by_desc(message::Column::Id)
        .limit(limit + 1)
        .all(&state.db)
        .await?;

    let has_more = rows.len() as u64 > limit;
    let rows: Vec<_> = rows.into_iter().take(limit as usize).collect();

    let next_cursor = has_more
        .then(|| {
            rows.last().map(|last| {
                ThreadCursor {
                    at: last.sent_at.into(),
                    id: last.id,
                }
                .encode()
            })
        })
        .flatten();

    Ok(Json(Page {
        items: rows.into_iter().map(Into::into).collect(),
        next_cursor,
    }))
}

async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<SendMessageRequest>,
) -> ApiResult<Json<MessageResponse>> {
    let claims = authenticate(&state, &headers)?;
    let (row, pair) = my_conversation(&state, claims.sub, id).await?;

    let body = request.body.trim();
    if body.is_empty() {
        return Err(ApiError::BadRequest(
            "Un message vide n'en est pas un.".into(),
        ));
    }
    if body.chars().count() > MAX_BODY {
        return Err(ApiError::BadRequest("Ce message est trop long.".into()));
    }

    // Rejeu : le client a renvoyé après une connexion coupée. Le message
    // existe déjà, on le rend tel quel plutôt que d'en écrire un second.
    if let Some(already) = message::Entity::find()
        .filter(message::Column::ConversationId.eq(row.id))
        .filter(message::Column::ClientId.eq(request.client_id))
        .one(&state.db)
        .await?
    {
        return Ok(Json(already.into()));
    }

    // Le quota se compte ici, après le rejeu : quelqu'un dans un tunnel qui
    // renvoie le même message ne doit pas payer pour la connexion qu'il n'a
    // pas. Ce n'est pas un nouveau message, c'est le même qui arrive enfin.
    enforce(&state, "messages", &claims.sub.to_string(), MESSAGE_QUOTA).await?;

    let now = Utc::now();
    let written = message::ActiveModel {
        id: Set(Uuid::new_v4()),
        conversation_id: Set(row.id),
        sender_id: Set(claims.sub),
        client_id: Set(request.client_id),
        body: Set(body.to_owned()),
        sent_at: Set(now.into()),
        read_at: Set(None),
    }
    .insert(&state.db)
    .await?;

    // La liste des conversations trie là-dessus : sans cette mise à jour, une
    // conversation active resterait au fond.
    conversation::ActiveModel {
        id: Unchanged(row.id),
        updated_at: Set(now.into()),
        ..Default::default()
    }
    .update(&state.db)
    .await?;

    let response = MessageResponse::from(written);
    // Après l'écriture, jamais avant : pousser d'abord ferait apparaître chez
    // l'autre une bulle qu'un échec de la base aurait fait disparaître au
    // rechargement.
    announce_message(&state, pair.other(claims.sub), response.clone());

    Ok(Json(response))
}

/// Marque comme lus les messages reçus, jamais les siens.
async fn mark_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<()> {
    let claims = authenticate(&state, &headers)?;
    let (row, pair) = my_conversation(&state, claims.sub, id).await?;

    // Relevés avant la mise à jour : après, ils ne se distinguent plus des
    // messages lus il y a une heure, et l'horodatage ne suffit pas à les
    // retrouver — deux lectures dans la même microseconde se confondraient.
    let freshly_read: Vec<Uuid> = message::Entity::find()
        .select_only()
        .column(message::Column::Id)
        .filter(message::Column::ConversationId.eq(row.id))
        .filter(message::Column::SenderId.ne(claims.sub))
        .filter(message::Column::ReadAt.is_null())
        .into_tuple()
        .all(&state.db)
        .await?;

    if freshly_read.is_empty() {
        return Ok(());
    }

    let now = Utc::now();
    message::Entity::update_many()
        .col_expr(
            message::Column::ReadAt,
            sea_orm::sea_query::Expr::value(chrono::DateTime::<chrono::FixedOffset>::from(now)),
        )
        .filter(message::Column::Id.is_in(freshly_read.clone()))
        .exec(&state.db)
        .await?;

    // À celui qui avait écrit : c'est lui que l'accusé de lecture concerne.
    announce_read(&state, pair.other(claims.sub), &freshly_read, now);

    Ok(())
}
