use axum::Json;
use axum::extract::{Path as AxumPath, Query, State};
use models::{
    GlobalNotificationSettings, NotificationEvent, NotificationSettings, RefreshSettings,
};
use shared_api::notifications::notification_event_from_row;

use crate::config::AppContext;
use crate::error::ApiError;
use crate::state::open_storage;
use crate::views::{AckNotificationEventsRequest, ListNotificationEventsQuery};

pub(crate) async fn get_global_notification_settings(
    State(context): State<AppContext>,
) -> Result<Json<GlobalNotificationSettings>, ApiError> {
    let storage = open_storage(&context)?;
    Ok(Json(storage.get_global_notification_settings()?))
}

pub(crate) async fn update_global_notification_settings(
    State(context): State<AppContext>,
    Json(payload): Json<GlobalNotificationSettings>,
) -> Result<Json<GlobalNotificationSettings>, ApiError> {
    let storage = open_storage(&context)?;
    storage.set_global_notification_settings(&payload)?;
    Ok(Json(storage.get_global_notification_settings()?))
}

pub(crate) async fn get_feed_notification_settings(
    State(context): State<AppContext>,
    AxumPath(feed_id): AxumPath<String>,
) -> Result<Json<NotificationSettings>, ApiError> {
    let storage = open_storage(&context)?;
    let globals = storage.get_global_notification_settings()?;
    let settings = storage
        .get_notification_settings(&feed_id)?
        .map(|row| row.settings)
        .unwrap_or_else(|| globals.default_feed_settings.clone());
    Ok(Json(settings))
}

pub(crate) async fn update_feed_notification_settings(
    State(context): State<AppContext>,
    AxumPath(feed_id): AxumPath<String>,
    Json(payload): Json<NotificationSettings>,
) -> Result<Json<NotificationSettings>, ApiError> {
    let mut storage = open_storage(&context)?;
    let row = storage.upsert_notification_settings(&feed_id, &payload)?;
    Ok(Json(row.settings))
}

pub(crate) async fn get_feed_refresh_settings(
    State(context): State<AppContext>,
    AxumPath(feed_id): AxumPath<String>,
) -> Result<Json<RefreshSettings>, ApiError> {
    let storage = open_storage(&context)?;
    Ok(Json(storage.resolve_effective_refresh_settings(&feed_id)?))
}

pub(crate) async fn update_feed_refresh_settings(
    State(context): State<AppContext>,
    AxumPath(feed_id): AxumPath<String>,
    Json(payload): Json<RefreshSettings>,
) -> Result<Json<RefreshSettings>, ApiError> {
    let mut storage = open_storage(&context)?;
    let row = storage.upsert_feed_refresh_settings(&feed_id, &payload)?;
    Ok(Json(row.settings))
}

pub(crate) async fn delete_feed_refresh_settings(
    State(context): State<AppContext>,
    AxumPath(feed_id): AxumPath<String>,
) -> Result<Json<RefreshSettings>, ApiError> {
    let mut storage = open_storage(&context)?;
    storage.delete_feed_refresh_settings(&feed_id)?;
    Ok(Json(storage.resolve_effective_refresh_settings(&feed_id)?))
}

pub(crate) async fn get_group_refresh_settings(
    State(context): State<AppContext>,
    AxumPath(group_id): AxumPath<String>,
) -> Result<Json<RefreshSettings>, ApiError> {
    let storage = open_storage(&context)?;
    let globals = storage.get_global_notification_settings()?;
    let settings = storage
        .get_group_refresh_settings(&group_id)?
        .map(|row| row.settings)
        .unwrap_or_else(|| RefreshSettings {
            enabled: globals.background_refresh_enabled,
            interval_minutes: globals.background_refresh_interval_minutes.max(1),
        });
    Ok(Json(settings))
}

pub(crate) async fn update_group_refresh_settings(
    State(context): State<AppContext>,
    AxumPath(group_id): AxumPath<String>,
    Json(payload): Json<RefreshSettings>,
) -> Result<Json<RefreshSettings>, ApiError> {
    let mut storage = open_storage(&context)?;
    let row = storage.upsert_group_refresh_settings(&group_id, &payload)?;
    Ok(Json(row.settings))
}

pub(crate) async fn delete_group_refresh_settings(
    State(context): State<AppContext>,
    AxumPath(group_id): AxumPath<String>,
) -> Result<Json<RefreshSettings>, ApiError> {
    let mut storage = open_storage(&context)?;
    storage.delete_group_refresh_settings(&group_id)?;
    let globals = storage.get_global_notification_settings()?;
    let settings = storage
        .get_group_refresh_settings(&group_id)?
        .map(|row| row.settings)
        .unwrap_or_else(|| RefreshSettings {
            enabled: globals.background_refresh_enabled,
            interval_minutes: globals.background_refresh_interval_minutes.max(1),
        });
    Ok(Json(settings))
}

pub(crate) async fn list_pending_notification_events(
    State(context): State<AppContext>,
    Query(query): Query<ListNotificationEventsQuery>,
) -> Result<Json<Vec<NotificationEvent>>, ApiError> {
    let storage = open_storage(&context)?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let events = storage
        .list_pending_notification_events(limit)?
        .into_iter()
        .map(notification_event_from_row)
        .collect();
    Ok(Json(events))
}

pub(crate) async fn ack_notification_events(
    State(context): State<AppContext>,
    Json(payload): Json<AckNotificationEventsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let storage = open_storage(&context)?;
    let acknowledged = storage.acknowledge_notification_events(&payload.event_ids)?;
    Ok(Json(serde_json::json!({ "acknowledged": acknowledged })))
}
