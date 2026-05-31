//! Unified error + success envelopes.
//!
//! Success responses are `{ "code": 0, "data": <T> }`.
//! Errors are `{ "code": <nonzero>, "message": <string>, "data": null }`.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

pub type ApiResult<T> = Result<ApiOk<T>, AppError>;

/// Wraps a successful payload into the `{ code: 0, data }` envelope.
pub struct ApiOk<T>(pub T);

impl<T: Serialize> IntoResponse for ApiOk<T> {
    fn into_response(self) -> Response {
        Json(serde_json::json!({ "code": 0, "data": self.0 })).into_response()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("upstream provider error: {0}")]
    Upstream(String),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Internal(String),
}

impl AppError {
    fn parts(&self) -> (StatusCode, i32) {
        match self {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, 4040),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, 4000),
            AppError::Upstream(_) => (StatusCode::BAD_GATEWAY, 5020),
            AppError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, 5000),
            AppError::Json(_) => (StatusCode::INTERNAL_SERVER_ERROR, 5001),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, 5999),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = self.parts();
        let message = self.to_string();
        if status.is_server_error() {
            tracing::error!(%code, %message, "request failed");
        } else {
            tracing::debug!(%code, %message, "request rejected");
        }
        (
            status,
            Json(serde_json::json!({ "code": code, "message": message, "data": null })),
        )
            .into_response()
    }
}
