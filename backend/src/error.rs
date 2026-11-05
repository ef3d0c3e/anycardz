//! API errors. The body is `{"error": "<code>"}`: the frontend turns codes into
//! localized messages, so codes are stable and messages are not sent.

use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("not found: {0}")]
    NotFound(&'static str),
    /// Input refused, the code names the rule
    #[error("invalid: {0}")]
    Invalid(&'static str),
    /// The request is fine but the game state refuses it (no coins, already claimed)
    #[error("conflict: {0}")]
    Conflict(&'static str),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct Body {
    error: &'static str,
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Invalid(_) => StatusCode::UNPROCESSABLE_ENTITY,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let error = match self {
            ApiError::Unauthorized => "unauthorized",
            ApiError::NotFound(code) | ApiError::Invalid(code) | ApiError::Conflict(code) => code,
            ApiError::Internal(err) => {
                tracing::error!("internal error: {err:#}");
                "internal"
            }
        };
        HttpResponse::build(self.status_code()).json(Body { error })
    }
}

impl From<diesel::result::Error> for ApiError {
    fn from(err: diesel::result::Error) -> Self {
        ApiError::Internal(err.into())
    }
}

impl From<diesel_async::pooled_connection::deadpool::PoolError> for ApiError {
    fn from(err: diesel_async::pooled_connection::deadpool::PoolError) -> Self {
        ApiError::Internal(err.into())
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
