use app_core::AppCoreError;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use shared_api::error::SharedApiError;
use storage::StorageError;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ApiError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = Json(ApiMessage { message: self.to_string() });
        (status, body).into_response()
    }
}

impl From<StorageError> for ApiError {
    fn from(value: StorageError) -> Self {
        match value {
            StorageError::NotFound => Self::NotFound("record not found".to_owned()),
            other => Self::Internal(other.to_string()),
        }
    }
}

impl From<AppCoreError> for ApiError {
    fn from(value: AppCoreError) -> Self {
        match value {
            AppCoreError::Storage(err) => err.into(),
            AppCoreError::InvalidUrl(message) => Self::BadRequest(message),
            AppCoreError::Discovery(message) => Self::BadRequest(message),
            AppCoreError::Fetch(message) => Self::BadRequest(message),
        }
    }
}

impl From<SharedApiError> for ApiError {
    fn from(value: SharedApiError) -> Self {
        match value {
            SharedApiError::BadRequest(message) => Self::BadRequest(message),
            SharedApiError::Internal(message) => Self::Internal(message),
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct ApiMessage {
    pub(crate) message: String,
}
