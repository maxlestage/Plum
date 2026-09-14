use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use uuid::Uuid;

use super::types::*;
use super::{password, tokens};
use crate::entities::{profile, refresh_token, user};
use crate::error::{ApiError, ApiResult};
use crate::rate_limit::Quota;
use crate::state::AppState;

/// Ten tries a quarter of an hour, per address. Enough for someone who has
/// genuinely forgotten which password they used; not enough to walk a
/// dictionary.
const SIGN_IN_QUOTA: Quota = Quota::new(10, 15 * 60);

/// Cinq par heure et par adresse, ce qui empêche de marteler la même mais
/// rien de plus : un script qui change d'adresse à chaque essai repart dans un
/// seau neuf. C'est le second quota qui tient la porte.
const SIGN_UP_QUOTA: Quota = Quota::new(5, 60 * 60);

/// Dix par heure et par adresse IP, quelle que soit l'adresse email.
///
/// Sans lui, créer des comptes ne coûte rien — et chaque compte peut déposer
/// six photos. Le gigaoctet du plan Postgres se remplirait en un jour, et le
/// décodage de chaque image occuperait le seul dyno pendant ce temps-là.
///
/// Dix laisse passer un foyer, un bureau ou un café derrière une même adresse ;
/// ça arrête une boucle.
const SIGN_UP_PER_IP: Quota = Quota::new(10, 60 * 60);

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/sign-up", post(sign_up))
        .route("/auth/sign-in", post(sign_in))
        .route("/auth/refresh", post(refresh))
        .route("/auth/sign-out", post(sign_out))
        .route("/me", get(me).delete(delete_me))
}

async fn sign_up(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SignUpRequest>,
) -> ApiResult<Json<SessionResponse>> {
    let email = normalise_email(&request.email);
    enforce(&state, "sign-up", &email, SIGN_UP_QUOTA).await?;
    if let Some(ip) = crate::rate_limit::client_ip(&headers) {
        enforce(&state, "sign-up-ip", &ip, SIGN_UP_PER_IP).await?;
    }

    if !looks_like_an_address(&email) {
        return Err(ApiError::BadRequest("Adresse email invalide.".into()));
    }
    if request.password.chars().count() < 8 {
        return Err(ApiError::BadRequest(
            "Le mot de passe doit faire au moins 8 caractères.".into(),
        ));
    }

    let birth_date = request.birth_date.date_naive();
    if !is_old_enough(birth_date, Utc::now().date_naive()) {
        return Err(ApiError::TooYoung);
    }

    let display_name = request.display_name.trim();
    if display_name.is_empty() {
        return Err(ApiError::BadRequest("Le prénom est obligatoire.".into()));
    }

    let existing = user::Entity::find()
        .filter(user::Column::Email.eq(email.clone()))
        .one(&state.db)
        .await?;
    if existing.is_some() {
        return Err(ApiError::EmailTaken);
    }

    let password_hash = password::hash(request.password.clone())
        .await
        .map_err(|error| ApiError::Internal(crate::error::anyhow_lite::Error::new(error)))?;

    let user_id = Uuid::new_v4();
    let now = Utc::now();

    // Account and profile are one thing to the person signing up; they must be
    // one thing to the database too, or a failure halfway leaves an account
    // that no screen can render.
    let created = state
        .db
        .transaction::<_, user::Model, sea_orm::DbErr>(|txn| {
            let email = email.clone();
            let display_name = display_name.to_string();
            let gender = request.gender.as_str().to_string();
            Box::pin(async move {
                let created = user::ActiveModel {
                    id: Set(user_id),
                    email: Set(email),
                    password_hash: Set(password_hash),
                    profile_completed: Set(false),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await?;

                profile::ActiveModel {
                    id: Set(user_id),
                    display_name: Set(display_name),
                    birth_date: Set(birth_date),
                    gender: Set(gender),
                    bio: Set(String::new()),
                    city: Set(String::new()),
                    interests: Set(Vec::new()),
                    latitude: Set(None),
                    longitude: Set(None),
                    last_active_at: Set(Some(now.into())),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await?;

                Ok(created)
            })
        })
        .await
        .map_err(|error| match error {
            sea_orm::TransactionError::Connection(inner) => ApiError::from(inner),
            sea_orm::TransactionError::Transaction(inner) => ApiError::from(inner),
        })?;

    let session = issue_session(&state, created).await?;
    Ok(Json(session))
}

async fn sign_in(
    State(state): State<AppState>,
    Json(request): Json<SignInRequest>,
) -> ApiResult<Json<SessionResponse>> {
    let email = normalise_email(&request.email);
    // Counted before the password is checked, and whatever the outcome: a
    // limiter that only counts failures tells an attacker which guesses were
    // close.
    enforce(&state, "sign-in", &email, SIGN_IN_QUOTA).await?;

    let found = user::Entity::find()
        .filter(user::Column::Email.eq(email))
        .one(&state.db)
        .await?;

    // One message and one code path whether the address is unknown or the
    // password is wrong: anything else tells a stranger which addresses exist.
    let Some(found) = found else {
        return Err(ApiError::InvalidCredentials);
    };
    if !password::verify(request.password.clone(), found.password_hash.clone()).await {
        return Err(ApiError::InvalidCredentials);
    }

    let session = issue_session(&state, found).await?;
    Ok(Json(session))
}

async fn refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> ApiResult<Json<TokensResponse>> {
    let digest = tokens::digest(&request.refresh_token);
    let now = Utc::now();

    let stored = refresh_token::Entity::find()
        .filter(refresh_token::Column::TokenDigest.eq(digest))
        .one(&state.db)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    if stored.revoked_at.is_some() || stored.expires_at < now {
        return Err(ApiError::Unauthorized);
    }

    let owner = user::Entity::find_by_id(stored.user_id)
        .one(&state.db)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    // Rotate: the token just used is spent. A refresh token that survives its
    // own use is a refresh token someone can replay.
    let mut spent: refresh_token::ActiveModel = stored.into();
    spent.revoked_at = Set(Some(now.into()));
    spent.update(&state.db).await?;

    let session = issue_session(&state, owner).await?;
    Ok(Json(session.tokens))
}

async fn sign_out(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Json<()>> {
    let claims = authenticate(&state, &headers)?;

    // Every session on this account, not just this device's: signing out is
    // what people reach for when they think something is wrong.
    refresh_token::Entity::update_many()
        .col_expr(
            refresh_token::Column::RevokedAt,
            sea_orm::sea_query::Expr::value(Some(chrono::Utc::now().fixed_offset())),
        )
        .filter(refresh_token::Column::UserId.eq(claims.sub))
        .filter(refresh_token::Column::RevokedAt.is_null())
        .exec(&state.db)
        .await?;

    Ok(Json(()))
}

async fn me(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Json<UserResponse>> {
    let claims = authenticate(&state, &headers)?;
    let found = user::Entity::find_by_id(claims.sub)
        .one(&state.db)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    Ok(Json(found.into()))
}

// MARK: - Helpers

async fn issue_session(state: &AppState, owner: user::Model) -> ApiResult<SessionResponse> {
    let now = Utc::now();
    let access = state
        .tokens
        .issue(owner.id, now)
        .map_err(|error| ApiError::Internal(crate::error::anyhow_lite::Error::new(error)))?;

    let refresh_value = tokens::generate_refresh_token();
    let expires_at = now + Duration::days(state.config.refresh_token_ttl_days);

    refresh_token::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(owner.id),
        token_digest: Set(tokens::digest(&refresh_value)),
        expires_at: Set(expires_at.into()),
        revoked_at: Set(None),
        created_at: Set(now.into()),
    }
    .insert(&state.db)
    .await?;

    Ok(SessionResponse {
        user: owner.into(),
        tokens: TokensResponse {
            access_token: access.value,
            refresh_token: refresh_value,
            expires_at: access.expires_at,
        },
    })
}

/// Supprime le compte, et tout ce qui en dépend.
///
/// Le client iOS appelle cette route depuis ses réglages, et la page
/// Confidentialité promet la suppression « à tout moment, sans demande à
/// formuler ». C'est aussi le droit d'effacement de l'article 17 du RGPD.
/// La route n'existait pas : le bouton renvoyait 404, et la promesse était
/// publiée en trois langues.
///
/// Une seule instruction suffit parce que le schéma a été construit pour :
/// profil, préférences, jetons de rafraîchissement, verdicts, matchs et
/// blocages partent en cascade. Les signalements survivent avec une
/// référence vidée — ils sont la trace de décisions prises, et les effacer
/// avec le compte reviendrait à laisser quelqu'un supprimer les preuves le
/// concernant.
async fn delete_me(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<()> {
    let claims = authenticate(&state, &headers)?;

    let outcome = user::Entity::delete_by_id(claims.sub)
        .exec(&state.db)
        .await?;

    // Un jeton valide pour un compte déjà parti : rien à supprimer, et rien
    // à signaler comme une erreur de plus.
    if outcome.rows_affected == 0 {
        return Err(ApiError::Unauthorized);
    }

    Ok(())
}

pub fn authenticate(state: &AppState, headers: &HeaderMap) -> ApiResult<super::tokens::Claims> {
    let raw = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(ApiError::Unauthorized)?;

    state.tokens.verify(raw).map_err(|_| ApiError::Unauthorized)
}

pub(crate) async fn enforce(
    state: &AppState,
    bucket: &str,
    key: &str,
    quota: Quota,
) -> ApiResult<()> {
    let decision = state.limiter.check(&format!("{bucket}:{key}"), quota).await;

    if decision.allowed {
        return Ok(());
    }

    Err(ApiError::RateLimited {
        retry_after_seconds: decision.retry_after.as_secs().max(1),
    })
}

fn normalise_email(email: &str) -> String {
    email.trim().to_lowercase()
}

/// Deliberately loose. Address validation by pattern is a losing game; this
/// only catches the typo that would otherwise cost a round trip.
fn looks_like_an_address(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !email.contains(' ')
        && email.len() <= 320
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_lowercased_and_trimmed() {
        assert_eq!(normalise_email("  Moi@Plum.App "), "moi@plum.app");
    }

    #[test]
    fn obvious_typos_are_caught() {
        assert!(looks_like_an_address("moi@plum.app"));
        assert!(!looks_like_an_address("moi@plum"));
        assert!(!looks_like_an_address("@plum.app"));
        assert!(!looks_like_an_address("moi plum@plum.app"));
        assert!(!looks_like_an_address("moi@plum.app."));
        assert!(!looks_like_an_address(""));
    }
}
