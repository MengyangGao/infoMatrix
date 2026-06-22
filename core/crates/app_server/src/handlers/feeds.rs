use axum::Json;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use models::FeedHealthState;
use notifications::RefreshReason;
use shared_api::url::feed_icon_url;

use crate::config::AppContext;
use crate::error::ApiError;
use crate::services::refresh::{refresh_due_feeds_once, refresh_feed_by_id};
use crate::state::{open_app_core, open_storage};
use crate::views::{
    DueFeedView, FeedGroupView, FeedView, RefreshDueQuery, RefreshDueResponse, RefreshResponse,
    UpdateFeedGroupRequest, UpdateFeedRequest,
};

pub(crate) async fn list_feeds(
    State(context): State<AppContext>,
) -> Result<Json<Vec<FeedView>>, ApiError> {
    let storage = open_storage(&context)?;
    let core = open_app_core(&context)?;
    let mut feeds = Vec::new();
    for feed in core.list_feeds()? {
        let groups = storage
            .list_groups_for_feed(&feed.id)?
            .into_iter()
            .map(|group| FeedGroupView { id: group.id, name: group.name })
            .collect();
        let icon_url = storage
            .get_feed_icon(&feed.id)?
            .map(|icon| icon.source_url)
            .or_else(|| feed_icon_url(feed.site_url.as_ref(), &feed.feed_url));
        feeds.push(FeedView {
            id: feed.id,
            title: feed
                .title
                .unwrap_or_else(|| feed.feed_url.host_str().unwrap_or("Untitled").to_owned()),
            feed_url: feed.feed_url.to_string(),
            site_url: feed.site_url.map(|v| v.to_string()),
            feed_type: feed.feed_type,
            auto_full_text: feed.auto_full_text,
            icon_url,
            groups,
        });
    }
    Ok(Json(feeds))
}

pub(crate) async fn update_feed(
    State(context): State<AppContext>,
    AxumPath(feed_id): AxumPath<String>,
    Json(payload): Json<UpdateFeedRequest>,
) -> Result<StatusCode, ApiError> {
    let core = open_app_core(&context)?;
    if let Some(title) = payload.title.as_deref() {
        core.rename_feed(&feed_id, Some(title))?;
    }
    if let Some(auto_full_text) = payload.auto_full_text {
        core.set_feed_auto_full_text(&feed_id, auto_full_text)?;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn update_feed_group(
    State(context): State<AppContext>,
    AxumPath(feed_id): AxumPath<String>,
    Json(payload): Json<UpdateFeedGroupRequest>,
) -> Result<StatusCode, ApiError> {
    let mut core = open_app_core(&context)?;
    core.set_feed_group(&feed_id, payload.group_id.as_deref())?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn delete_feed(
    State(context): State<AppContext>,
    AxumPath(feed_id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    let core = open_app_core(&context)?;
    core.delete_feed(&feed_id)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn refresh_feed(
    State(context): State<AppContext>,
    AxumPath(feed_id): AxumPath<String>,
) -> Result<Json<RefreshResponse>, ApiError> {
    let mut storage = open_storage(&context)?;
    let result =
        refresh_feed_by_id(&context, &mut storage, &feed_id, RefreshReason::Manual).await?;
    Ok(Json(result))
}

pub(crate) async fn refresh_due_feeds(
    State(context): State<AppContext>,
    Query(query): Query<RefreshDueQuery>,
) -> Result<Json<RefreshDueResponse>, ApiError> {
    let result = refresh_due_feeds_once(
        &context,
        query.limit.unwrap_or(20).clamp(1, 100),
        RefreshReason::Periodic,
    )
    .await?;
    Ok(Json(result))
}

pub(crate) async fn list_due_feeds(
    State(context): State<AppContext>,
    Query(query): Query<RefreshDueQuery>,
) -> Result<Json<Vec<DueFeedView>>, ApiError> {
    let storage = open_storage(&context)?;
    let due_feeds = storage.list_due_feeds(query.limit.unwrap_or(20).clamp(1, 100))?;
    let views = due_feeds
        .into_iter()
        .map(|feed| DueFeedView {
            id: feed.id,
            title: feed
                .title
                .unwrap_or_else(|| feed.feed_url.host_str().unwrap_or("Untitled").to_owned()),
            feed_url: feed.feed_url.to_string(),
            next_scheduled_fetch_at: feed.next_scheduled_fetch_at,
            failure_count: feed.failure_count,
            health_state: feed_health_state_label(feed.health_state).to_owned(),
        })
        .collect();
    Ok(Json(views))
}

pub(crate) fn feed_health_state_label(value: FeedHealthState) -> &'static str {
    match value {
        FeedHealthState::Healthy => "healthy",
        FeedHealthState::Stale => "stale",
        FeedHealthState::Failing => "failing",
    }
}
