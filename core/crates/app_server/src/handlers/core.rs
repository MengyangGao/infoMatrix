use axum::Json;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, header::AUTHORIZATION};
use axum::middleware::Next;
use axum::response::Response;

use crate::config::AppContext;
use crate::error::ApiError;
use crate::views::{HealthResponse, MetaResponse};

pub(crate) async fn require_api_token(
    State(context): State<AppContext>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let Some(expected_token) = context.api_token.as_deref() else {
        return Ok(next.run(request).await);
    };

    let expected_header = format!("Bearer {expected_token}");
    let authorized = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == expected_header);

    if authorized {
        Ok(next.run(request).await)
    } else {
        Err(ApiError::Unauthorized("missing or invalid API token".to_owned()))
    }
}

pub(crate) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

pub(crate) async fn meta() -> Json<MetaResponse> {
    Json(MetaResponse { api_version: 2, app_version: env!("CARGO_PKG_VERSION") })
}
