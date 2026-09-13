use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

/// The error envelope the iOS client decodes. Its `APIErrorBody` expects
/// exactly these two fields, and shows `message` to the person.
#[derive(Debug, Serialize)]
pub struct ApiErrorBody {
    pub message: String,
    pub code: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("Identifiants incorrects.")]
    InvalidCredentials,
    #[error("Votre session a expiré. Reconnectez-vous.")]
    Unauthorized,
    #[error("Introuvable.")]
    NotFound,
    #[error("Cette adresse est déjà prise.")]
    EmailTaken,
    #[error("Plum est réservé aux majeurs.")]
    TooYoung,
    #[error("Quelque chose s'est mal passé de notre côté.")]
    Internal(#[from] anyhow_lite::Error),
}

impl ApiError {
    fn parts(&self) -> (StatusCode, Option<&'static str>) {
        match self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, Some("bad_request")),
            Self::InvalidCredentials => (StatusCode::UNAUTHORIZED, Some("invalid_credentials")),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, Some("unauthorized")),
            Self::NotFound => (StatusCode::NOT_FOUND, Some("not_found")),
            Self::EmailTaken => (StatusCode::CONFLICT, Some("email_taken")),
            Self::TooYoung => (StatusCode::FORBIDDEN, Some("too_young")),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, None),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = self.parts();

        // Internal failures are logged in full and described vaguely: the
        // client has nothing useful to do with a database error, and it should
        // not learn our schema from one.
        if let Self::Internal(inner) = &self {
            tracing::error!(error = %inner, "échec interne");
        }

        let body = ApiErrorBody {
            message: self.to_string(),
            code: code.map(str::to_string),
        };
        (status, Json(body)).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

/// A minimal stand-in for `anyhow`, kept local so the dependency list stays
/// short: everything internal collapses into one opaque error that carries a
/// message for the logs.
pub mod anyhow_lite {
    #[derive(Debug, thiserror::Error)]
    #[error("{0}")]
    pub struct Error(String);

    impl Error {
        pub fn new(message: impl std::fmt::Display) -> Self {
            Self(message.to_string())
        }
    }

    impl From<sea_orm::DbErr> for Error {
        fn from(value: sea_orm::DbErr) -> Self {
            Self::new(value)
        }
    }
}

impl From<sea_orm::DbErr> for ApiError {
    fn from(value: sea_orm::DbErr) -> Self {
        Self::Internal(anyhow_lite::Error::new(value))
    }
}
