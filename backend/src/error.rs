use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Invalid request: {0:?}")]
    ValidationError(#[from] validator::ValidationErrors),
    #[error("Authentication failed: {0:?}")]
    AuthenticationError(password_hash::Error),
    #[error("JWT error: {0:?}")]
    JwtError(#[from] jsonwebtoken::errors::Error),
    #[error("Forbidden request")]
    ForbiddenError(serde_json::Value),
    #[error("SQL failed: {0:?}")]
    SqlxError(#[from] sqlx::Error),
    #[error("Any error: {0:?}")]
    Anyhow(#[from] anyhow::Error),
}

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        log::error!("error: {}", self);

        match self {
            Self::ValidationError(err) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "error": err })),
            ),
            Self::AuthenticationError(err) => (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": err.to_string() })),
            ),
            Self::JwtError(err) => (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": err.to_string() })),
            ),
            Self::ForbiddenError(err) => (StatusCode::FORBIDDEN, Json(json!({ "error": err }))),
            Self::SqlxError(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": err.to_string() })),
            ),
            Self::Anyhow(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": err.to_string() })),
            ),
        }
        .into_response()
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;
