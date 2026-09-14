use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait, Unchanged,
};
use serde::Deserialize;
use uuid::Uuid;

use super::encode::{self, PhotoError, MAX_UPLOAD};
use crate::auth::routes::{authenticate, enforce};
use crate::entities::photo;
use crate::error::{ApiError, ApiResult};
use crate::profile::types::{PhotoResponse, ProfileResponse};
use crate::rate_limit::Quota;
use crate::state::AppState;

/// Ce que le client affiche déjà comme maximum. Reposé ici : une limite qui
/// n'existe que dans l'application n'existe pas.
pub const MAX_PHOTOS: u64 = 6;

/// Vingt envois par heure et par compte.
///
/// Six photos et de quoi se raviser plusieurs fois : personne de réel n'ira
/// au bout. Ce que ça arrête est une boucle — chaque envoi décode jusqu'à
/// douze mégaoctets et réencode, ce qui occupe le seul dyno pendant ce
/// temps-là. Le plafond de six photos ne suffisait pas : on peut en retirer
/// une et en renvoyer une autre indéfiniment.
const UPLOAD_QUOTA: Quota = Quota::new(20, 60 * 60);

/// Le poids au-delà duquel on cesse d'accepter des photos.
///
/// Les quotas précédents comptent *qui* envoie, et se contournent donc en
/// changeant d'identité — une adresse email neuve, une adresse IP neuve. Cette
/// limite-ci compte la ressource elle-même : elle tient quel que soit le
/// nombre de comptes.
///
/// 700 Mio sur le gigaoctet du plan, ce qui laisse de quoi respirer aux
/// profils, aux messages et aux index. Passé ce seuil, l'envoi est refusé avec
/// un message qui dit ce qui se passe — et le reste de l'application continue
/// de fonctionner, ce qu'une base pleine ne permettrait plus.
const STORAGE_BUDGET: i64 = 700 * 1024 * 1024;

/// Les routes authentifiées, sous `/api/v1`.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/me/photos", post(upload))
        .route("/me/photos/order", patch(reorder))
        .route("/me/photos/{id}", delete(remove))
        // Axum n'accepte que deux mégaoctets par défaut, ce qui refuserait
        // toute photo de téléphone avec une erreur que rien n'explique.
        .layer(DefaultBodyLimit::max(MAX_UPLOAD + 64 * 1024))
}

/// La route qui sert les octets, à la racine et **sans authentification**.
///
/// Ce n'est pas un oubli : `AsyncImage`, côté SwiftUI, fait une requête nue
/// sans en-tête. L'adresse est donc la clé — un UUID tiré au sort, cent
/// vingt-deux bits, qui ne s'énumère pas.
///
/// La contrepartie doit être dite plutôt que découverte : qui détient le lien
/// garde l'image, y compris après un match défait. C'est ainsi que fonctionne
/// toute application dont les photos passent par un CDN ; ce qui change ici,
/// c'est que c'est écrit.
pub fn public_router() -> Router<AppState> {
    Router::new().route("/photos/{id}", get(serve))
}

impl From<PhotoError> for ApiError {
    fn from(error: PhotoError) -> Self {
        // Toutes des erreurs de la requête, et toutes lisibles telles quelles :
        // le message dit quoi faire, pas seulement que ça n'a pas marché.
        ApiError::BadRequest(error.to_string())
    }
}

async fn upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut form: Multipart,
) -> ApiResult<Json<PhotoResponse>> {
    let claims = authenticate(&state, &headers)?;
    let viewer = claims.sub;
    // Avant de lire le corps, pas après : l'intérêt est d'épargner le
    // décodage, et refuser une fois les douze mégaoctets ingérés ne coûterait
    // pas moins cher qu'accepter.
    enforce(&state, "photo-upload", &viewer.to_string(), UPLOAD_QUOTA).await?;
    if storage_is_full(&state, STORAGE_BUDGET).await? {
        return Err(ApiError::BadRequest(
            "Les photos ne rentrent plus. Réessayez plus tard.".into(),
        ));
    }

    // Le premier champ, quel que soit son nom : le client envoie `file`, et
    // rien ne gagne à refuser un envoi par ailleurs valide pour une étiquette.
    let raw: Bytes = form
        .next_field()
        .await
        .map_err(|error| ApiError::BadRequest(format!("Envoi illisible : {error}")))?
        .ok_or_else(|| ApiError::BadRequest("Aucun fichier reçu.".into()))?
        .bytes()
        .await
        .map_err(|_| ApiError::BadRequest("Envoi interrompu.".into()))?;
    let prepared = encode::prepare(&raw)?;

    // Compté et inséré dans la même transaction : deux envois simultanés
    // passeraient sinon tous les deux le contrôle et feraient une septième
    // photo, que rien ensuite ne viendrait retirer.
    let stored = state
        .db
        .transaction::<_, photo::Model, ApiError>(move |txn| {
            Box::pin(async move {
                let existing = photo::Entity::find()
                    .filter(photo::Column::ProfileId.eq(viewer))
                    .count(txn)
                    .await?;

                if existing >= MAX_PHOTOS {
                    return Err(ApiError::BadRequest(
                        "Six photos suffisent. Retirez-en une avant d'en ajouter une autre.".into(),
                    ));
                }

                let created = photo::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    profile_id: Set(viewer),
                    // À la suite : la couverture se choisit ensuite, et la
                    // première photo envoyée est couverture par défaut, ce qui
                    // est ce qu'on attend.
                    position: Set(existing as i32),
                    content_type: Set(prepared.content_type.to_owned()),
                    width: Set(prepared.width as i32),
                    height: Set(prepared.height as i32),
                    bytes: Set(prepared.bytes),
                    created_at: Set(chrono::Utc::now().into()),
                }
                .insert(txn)
                .await?;

                Ok(created)
            })
        })
        .await
        .map_err(flatten)?;

    Ok(Json(PhotoResponse::new(
        &stored,
        &state.config.public_base_url,
    )))
}

#[derive(Debug, Deserialize)]
pub struct ReorderRequest {
    pub photo_ids: Vec<Uuid>,
}

/// Remet les photos dans l'ordre donné, la première étant la couverture.
///
/// La liste doit être exactement celle qu'on possède : plus courte, elle
/// laisserait des photos orphelines à une position arbitraire ; plus longue ou
/// mélangée avec celle d'autrui, elle déplacerait les photos d'un inconnu.
async fn reorder(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ReorderRequest>,
) -> ApiResult<Json<Vec<PhotoResponse>>> {
    let claims = authenticate(&state, &headers)?;
    let viewer = claims.sub;

    let mut held: Vec<Uuid> = photo::Entity::find()
        .select_only()
        .column(photo::Column::Id)
        .filter(photo::Column::ProfileId.eq(viewer))
        .into_tuple()
        .all(&state.db)
        .await?;

    let mut wanted = request.photo_ids.clone();
    wanted.sort_unstable();
    wanted.dedup();
    held.sort_unstable();

    if wanted != held {
        return Err(ApiError::BadRequest(
            "Cette liste ne correspond pas à vos photos.".into(),
        ));
    }

    state
        .db
        .transaction::<_, (), sea_orm::DbErr>(move |txn| {
            let order = request.photo_ids;
            Box::pin(async move {
                for (position, id) in order.into_iter().enumerate() {
                    photo::ActiveModel {
                        id: Unchanged(id),
                        position: Set(position as i32),
                        ..Default::default()
                    }
                    .update(txn)
                    .await?;
                }
                Ok(())
            })
        })
        .await
        .map_err(|error| match error {
            sea_orm::TransactionError::Transaction(inner) => ApiError::from(inner),
            sea_orm::TransactionError::Connection(inner) => ApiError::from(inner),
        })?;

    let reordered: Vec<(Uuid, i32)> = photo::Entity::find()
        .select_only()
        .column(photo::Column::Id)
        .column(photo::Column::Position)
        .filter(photo::Column::ProfileId.eq(viewer))
        .order_by_asc(photo::Column::Position)
        .into_tuple()
        .all(&state.db)
        .await?;

    let base = &state.config.public_base_url;
    Ok(Json(
        reordered
            .into_iter()
            .map(|(id, position)| PhotoResponse {
                id,
                url: format!("{base}/photos/{id}"),
                position,
            })
            .collect(),
    ))
}

/// Retire une photo, et resserre les positions derrière elle.
///
/// Sans le resserrage, retirer la couverture laisserait la position 0 vide et
/// la photo suivante ne deviendrait jamais couverture.
async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let claims = authenticate(&state, &headers)?;
    let viewer = claims.sub;

    let owner: Option<Uuid> = photo::Entity::find_by_id(id)
        .select_only()
        .column(photo::Column::ProfileId)
        .into_tuple()
        .one(&state.db)
        .await?;

    // Introuvable plutôt qu'interdit : confirmer l'existence de la photo de
    // quelqu'un d'autre renseignerait déjà.
    if owner != Some(viewer) {
        return Err(ApiError::NotFound);
    }

    state
        .db
        .transaction::<_, (), sea_orm::DbErr>(move |txn| {
            Box::pin(async move {
                photo::Entity::delete_by_id(id).exec(txn).await?;

                let remaining: Vec<(Uuid, i32)> = photo::Entity::find()
                    .select_only()
                    .column(photo::Column::Id)
                    .column(photo::Column::Position)
                    .filter(photo::Column::ProfileId.eq(viewer))
                    .order_by_asc(photo::Column::Position)
                    .into_tuple()
                    .all(txn)
                    .await?;

                for (position, (row_id, row_position)) in remaining.into_iter().enumerate() {
                    if row_position == position as i32 {
                        continue;
                    }
                    photo::ActiveModel {
                        id: Unchanged(row_id),
                        position: Set(position as i32),
                        ..Default::default()
                    }
                    .update(txn)
                    .await?;
                }
                Ok(())
            })
        })
        .await
        .map_err(|error| match error {
            sea_orm::TransactionError::Transaction(inner) => ApiError::from(inner),
            sea_orm::TransactionError::Connection(inner) => ApiError::from(inner),
        })?;

    Ok(StatusCode::NO_CONTENT)
}

/// Sert les octets.
async fn serve(State(state): State<AppState>, Path(id): Path<Uuid>) -> Response {
    let Ok(Some(found)) = photo::Entity::find_by_id(id).one(&state.db).await else {
        return (StatusCode::NOT_FOUND, "").into_response();
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&found.content_type)
            .unwrap_or(HeaderValue::from_static("image/jpeg")),
    );
    // Le contenu d'une adresse ne change jamais : une photo modifiée est une
    // photo différente, donc une autre adresse. Sans ça, chaque ouverture du
    // deck redemanderait vingt images à un dyno qui n'en peut déjà pas trop.
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );

    (headers, found.bytes).into_response()
}

/// La table des photos a-t-elle dépassé son budget ?
///
/// `pg_total_relation_size` plutôt qu'une somme sur les octets : c'est une
/// lecture du catalogue, pas un parcours de table, et elle compte le stockage
/// externe où Postgres range réellement les `bytea` — qu'une somme sur la
/// colonne ne verrait pas de la même façon.
pub async fn storage_is_full(state: &AppState, budget: i64) -> ApiResult<bool> {
    use sea_orm::{ConnectionTrait, Statement};

    let row = state
        .db
        .query_one(Statement::from_string(
            state.db.get_database_backend(),
            "SELECT pg_total_relation_size('photos') AS taille",
        ))
        .await?;

    let taille: i64 = match row {
        Some(row) => row.try_get("", "taille").unwrap_or(0),
        // Table absente : rien de stocké, donc rien à refuser.
        None => 0,
    };
    Ok(taille >= budget)
}

/// Accroche leurs photos à une page de profils.
///
/// Une seule requête pour toute la page, pas une par profil : le deck en rend
/// vingt d'un coup, et vingt allers-retours de plus sont exactement ce qui
/// rend une application lente sans qu'on sache pourquoi.
pub async fn attach(state: &AppState, profiles: &mut [&mut ProfileResponse]) -> ApiResult<()> {
    if profiles.is_empty() {
        return Ok(());
    }

    let ids: Vec<Uuid> = profiles.iter().map(|p| p.id).collect();
    // Trois colonnes, jamais `bytes`. Construire une adresse ne demande que
    // l'identifiant ; charger la colonne d'octets ferait traverser au deck une
    // vingtaine de mégaoctets par ouverture — pour n'en écrire aucun.
    let rows: Vec<(Uuid, Uuid, i32)> = photo::Entity::find()
        .select_only()
        .column(photo::Column::Id)
        .column(photo::Column::ProfileId)
        .column(photo::Column::Position)
        .filter(photo::Column::ProfileId.is_in(ids))
        .order_by_asc(photo::Column::Position)
        .into_tuple()
        .all(&state.db)
        .await?;

    let base = &state.config.public_base_url;
    for profile in profiles.iter_mut() {
        profile.photos = rows
            .iter()
            .filter(|(_, owner, _)| *owner == profile.id)
            .map(|(id, _, position)| PhotoResponse {
                id: *id,
                url: format!("{base}/photos/{id}"),
                position: *position,
            })
            .collect();
    }
    Ok(())
}

/// Le cas d'un seul profil, qui est le plus courant.
pub async fn attach_one(state: &AppState, profile: &mut ProfileResponse) -> ApiResult<()> {
    attach(state, &mut [profile]).await
}

/// Ramène une erreur de transaction à une erreur d'API.
fn flatten(error: sea_orm::TransactionError<ApiError>) -> ApiError {
    match error {
        sea_orm::TransactionError::Transaction(inner) => inner,
        sea_orm::TransactionError::Connection(inner) => ApiError::from(inner),
    }
}
