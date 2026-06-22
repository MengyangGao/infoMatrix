use axum::Json;
use axum::extract::State;

use crate::config::AppContext;
use crate::error::ApiError;
use crate::state::open_app_core;
use crate::views::{CreateGroupRequest, FeedGroupView};

pub(crate) async fn list_groups(
    State(context): State<AppContext>,
) -> Result<Json<Vec<FeedGroupView>>, ApiError> {
    let core = open_app_core(&context)?;
    let groups = core
        .list_groups()?
        .into_iter()
        .map(|group| FeedGroupView { id: group.id, name: group.name })
        .collect();
    Ok(Json(groups))
}

pub(crate) async fn create_group(
    State(context): State<AppContext>,
    Json(payload): Json<CreateGroupRequest>,
) -> Result<Json<FeedGroupView>, ApiError> {
    let core = open_app_core(&context)?;
    let group = core.create_group(&payload.name)?;
    Ok(Json(FeedGroupView { id: group.id, name: group.name }))
}
