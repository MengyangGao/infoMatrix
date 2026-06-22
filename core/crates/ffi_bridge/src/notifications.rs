use models::{GlobalNotificationSettings, NotificationSettings, RefreshSettings};
use serde::Deserialize;
use shared_api::notifications::notification_event_from_row;

use crate::storage::open_storage;

#[derive(Debug, Deserialize)]
pub struct FeedNotificationSettingsInput {
    pub db_path: Option<String>,
    pub feed_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFeedNotificationSettingsInput {
    pub db_path: Option<String>,
    pub feed_id: String,
    pub settings: NotificationSettings,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGlobalNotificationSettingsInput {
    pub db_path: Option<String>,
    pub settings: GlobalNotificationSettings,
}

#[derive(Debug, Deserialize)]
pub struct RefreshSettingsInput {
    pub db_path: Option<String>,
    pub feed_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFeedRefreshSettingsInput {
    pub db_path: Option<String>,
    pub feed_id: String,
    pub settings: RefreshSettings,
}

#[derive(Debug, Deserialize)]
pub struct GroupRefreshSettingsInput {
    pub db_path: Option<String>,
    pub group_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGroupRefreshSettingsInput {
    pub db_path: Option<String>,
    pub group_id: String,
    pub settings: RefreshSettings,
}

#[derive(Debug, Deserialize)]
pub struct PendingNotificationEventsInput {
    pub db_path: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct AckNotificationEventsInput {
    pub db_path: Option<String>,
    pub event_ids: Vec<String>,
}

pub fn get_global_notification_settings(
    db_path: &Option<String>,
) -> Result<GlobalNotificationSettings, String> {
    let storage = open_storage(db_path)?;
    storage.get_global_notification_settings().map_err(|err| err.to_string())
}

pub fn update_global_notification_settings(
    db_path: &Option<String>,
    settings: &GlobalNotificationSettings,
) -> Result<GlobalNotificationSettings, String> {
    let storage = open_storage(db_path)?;
    storage.set_global_notification_settings(settings).map_err(|err| err.to_string())?;
    storage.get_global_notification_settings().map_err(|err| err.to_string())
}

pub fn get_feed_notification_settings(
    db_path: &Option<String>,
    feed_id: &str,
) -> Result<NotificationSettings, String> {
    let storage = open_storage(db_path)?;
    let globals = storage.get_global_notification_settings().map_err(|err| err.to_string())?;
    Ok(storage
        .get_notification_settings(feed_id)
        .map_err(|err| err.to_string())?
        .map(|row| row.settings)
        .unwrap_or_else(|| globals.default_feed_settings.clone()))
}

pub fn update_feed_notification_settings(
    db_path: &Option<String>,
    feed_id: &str,
    settings: &NotificationSettings,
) -> Result<NotificationSettings, String> {
    let mut storage = open_storage(db_path)?;
    storage.upsert_notification_settings(feed_id, settings).map_err(|err| err.to_string())?;
    storage
        .get_notification_settings(feed_id)
        .map_err(|err| err.to_string())?
        .map(|row| row.settings)
        .ok_or_else(|| "notification settings not found after update".to_owned())
}

pub fn get_feed_refresh_settings(
    db_path: &Option<String>,
    feed_id: &str,
) -> Result<RefreshSettings, String> {
    let storage = open_storage(db_path)?;
    storage.resolve_effective_refresh_settings(feed_id).map_err(|err| err.to_string())
}

pub fn update_feed_refresh_settings(
    db_path: &Option<String>,
    feed_id: &str,
    settings: &RefreshSettings,
) -> Result<RefreshSettings, String> {
    let mut storage = open_storage(db_path)?;
    storage.upsert_feed_refresh_settings(feed_id, settings).map_err(|err| err.to_string())?;
    storage.resolve_effective_refresh_settings(feed_id).map_err(|err| err.to_string())
}

pub fn delete_feed_refresh_settings(
    db_path: &Option<String>,
    feed_id: &str,
) -> Result<RefreshSettings, String> {
    let mut storage = open_storage(db_path)?;
    storage.delete_feed_refresh_settings(feed_id).map_err(|err| err.to_string())?;
    storage.resolve_effective_refresh_settings(feed_id).map_err(|err| err.to_string())
}

pub fn get_group_refresh_settings(
    db_path: &Option<String>,
    group_id: &str,
) -> Result<RefreshSettings, String> {
    let storage = open_storage(db_path)?;
    let globals = storage.get_global_notification_settings().map_err(|err| err.to_string())?;
    Ok(storage
        .get_group_refresh_settings(group_id)
        .map_err(|err| err.to_string())?
        .map(|row| row.settings)
        .unwrap_or_else(|| RefreshSettings {
            enabled: globals.background_refresh_enabled,
            interval_minutes: globals.background_refresh_interval_minutes.max(1),
        }))
}

pub fn update_group_refresh_settings(
    db_path: &Option<String>,
    group_id: &str,
    settings: &RefreshSettings,
) -> Result<RefreshSettings, String> {
    let mut storage = open_storage(db_path)?;
    storage.upsert_group_refresh_settings(group_id, settings).map_err(|err| err.to_string())?;
    storage
        .get_group_refresh_settings(group_id)
        .map_err(|err| err.to_string())?
        .map(|row| row.settings)
        .ok_or_else(|| "group refresh settings not found after update".to_owned())
}

pub fn delete_group_refresh_settings(
    db_path: &Option<String>,
    group_id: &str,
) -> Result<RefreshSettings, String> {
    let mut storage = open_storage(db_path)?;
    storage.delete_group_refresh_settings(group_id).map_err(|err| err.to_string())?;
    let globals = storage.get_global_notification_settings().map_err(|err| err.to_string())?;
    Ok(storage
        .get_group_refresh_settings(group_id)
        .map_err(|err| err.to_string())?
        .map(|row| row.settings)
        .unwrap_or_else(|| RefreshSettings {
            enabled: globals.background_refresh_enabled,
            interval_minutes: globals.background_refresh_interval_minutes.max(1),
        }))
}

pub fn list_pending_notification_events(
    db_path: &Option<String>,
    limit: Option<usize>,
) -> Result<Vec<models::NotificationEvent>, String> {
    let storage = open_storage(db_path)?;
    let rows = storage
        .list_pending_notification_events(limit.unwrap_or(50))
        .map_err(|err| err.to_string())?;
    let events = rows.into_iter().map(notification_event_from_row).collect::<Vec<_>>();
    Ok(events)
}

pub fn ack_notification_events(
    db_path: &Option<String>,
    event_ids: &[String],
) -> Result<usize, String> {
    let storage = open_storage(db_path)?;
    storage.acknowledge_notification_events(event_ids).map_err(|err| err.to_string())
}
