use std::ffi::c_char;

use serde::Serialize;

use crate::TOKIO_RUNTIME;
use crate::entries::{
    CreateEntryInput, FullTextInput, ListEntriesInput, ListItemsInput, PatchItemStateInput,
};
use crate::envelope::with_input;
use crate::notifications::{
    FeedNotificationSettingsInput, GroupRefreshSettingsInput, PendingNotificationEventsInput,
    RefreshSettingsInput, UpdateFeedNotificationSettingsInput, UpdateFeedRefreshSettingsInput,
    UpdateGlobalNotificationSettingsInput, UpdateGroupRefreshSettingsInput,
};
use crate::opml::OpmlImportInput;
use crate::refresh::{RefreshDueInput, RefreshInput};
use crate::storage::open_storage;
use crate::subscriptions::{
    AddSubscriptionInput, CreateGroupInput, DbInput, DeleteFeedInput, SubscribeInput,
    UpdateFeedGroupInput, UpdateFeedInput,
};
use crate::sync::{AckSyncEventsInput, ApplySyncEventsInput, ListSyncEventsInput};

#[derive(Debug, Serialize)]
struct MetaOutput {
    api_version: i32,
    app_version: &'static str,
}

#[derive(Debug, Serialize)]
struct HealthOutput {
    status: &'static str,
    version: &'static str,
}

/// Return core metadata and API version.
#[unsafe(no_mangle)]
pub extern "C" fn infomatrix_core_meta_json() -> *mut c_char {
    respond_ok(MetaOutput { api_version: 2, app_version: env!("CARGO_PKG_VERSION") })
}

/// Return core health status.
#[unsafe(no_mangle)]
pub extern "C" fn infomatrix_core_health_json() -> *mut c_char {
    respond_ok(HealthOutput { status: "ok", version: env!("CARGO_PKG_VERSION") })
}

/// Return default database path for this host.
#[unsafe(no_mangle)]
pub extern "C" fn infomatrix_core_default_db_path_json() -> *mut c_char {
    #[derive(Debug, Serialize)]
    struct Output {
        db_path: String,
    }

    respond_ok(Output { db_path: shared_api::db::default_db_path() })
}

/// List all subscribed feeds.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_list_feeds_json(input: *const c_char) -> *mut c_char {
    match with_input::<DbInput, _>(input, |payload| {
        crate::subscriptions::list_feeds(&payload.db_path)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// List all feed groups.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_list_groups_json(input: *const c_char) -> *mut c_char {
    match with_input::<DbInput, _>(input, |payload| {
        crate::subscriptions::list_groups(&payload.db_path)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Create or reuse a feed group.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_create_group_json(input: *const c_char) -> *mut c_char {
    match with_input::<CreateGroupInput, _>(input, |payload| {
        crate::subscriptions::create_group(&payload.db_path, &payload.name)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Update feed metadata.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_update_feed_json(input: *const c_char) -> *mut c_char {
    match with_input::<UpdateFeedInput, _>(input, |payload| {
        crate::subscriptions::update_feed(
            &payload.db_path,
            &payload.feed_id,
            payload.title.as_deref(),
            payload.auto_full_text,
        )
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Update feed group assignment.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_update_feed_group_json(
    input: *const c_char,
) -> *mut c_char {
    match with_input::<UpdateFeedGroupInput, _>(input, |payload| {
        crate::subscriptions::update_feed_group(
            &payload.db_path,
            &payload.feed_id,
            payload.group_id.as_deref(),
        )
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Delete a feed subscription.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_delete_feed_json(input: *const c_char) -> *mut c_char {
    match with_input::<DeleteFeedInput, _>(input, |payload| {
        crate::subscriptions::delete_feed(&payload.db_path, &payload.feed_id)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Add subscription by direct feed URL.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_add_subscription_json(
    input: *const c_char,
) -> *mut c_char {
    #[derive(Debug, Serialize)]
    struct Output {
        feed_id: String,
    }

    match with_input::<AddSubscriptionInput, _>(input, |payload| {
        crate::subscriptions::add_subscription(
            &payload.db_path,
            &payload.feed_url,
            payload.title.clone(),
        )
        .map(|feed_id| Output { feed_id })
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Subscribe by input URL with direct-feed probe + site discovery fallback.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_subscribe_input_json(input: *const c_char) -> *mut c_char {
    match with_input::<SubscribeInput, _>(input, |payload| {
        crate::subscriptions::subscribe_input(&payload.db_path, &payload.input_url)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Discover feeds from a website URL.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_discover_site_json(input: *const c_char) -> *mut c_char {
    match with_input::<crate::discovery::DiscoverInput, _>(input, |payload| {
        crate::discovery::discover_site(&payload.site_url, &payload.db_path)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Export subscriptions as OPML XML string.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_export_opml_json(input: *const c_char) -> *mut c_char {
    match with_input::<DbInput, _>(input, |payload| {
        crate::opml::export_opml_subscriptions(&payload.db_path)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Import subscriptions from OPML XML string.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_import_opml_json(input: *const c_char) -> *mut c_char {
    match with_input::<OpmlImportInput, _>(input, |payload| {
        crate::opml::import_opml_subscriptions(&payload.db_path, &payload.opml_xml)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Refresh feed content and persist parsed items.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_refresh_feed_json(input: *const c_char) -> *mut c_char {
    match with_input::<RefreshInput, _>(input, |payload| {
        let mut storage = open_storage(&payload.db_path)?;
        let runtime = &*TOKIO_RUNTIME;
        crate::refresh::refresh_feed_with_runtime(runtime, &mut storage, &payload.feed_id)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Refresh all due feeds and report aggregate counts.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_refresh_due_json(input: *const c_char) -> *mut c_char {
    match with_input::<RefreshDueInput, _>(input, |payload| {
        let mut storage = open_storage(&payload.db_path)?;
        let runtime = &*TOKIO_RUNTIME;
        let due_feeds = storage
            .list_due_feeds(payload.limit.unwrap_or(20).clamp(1, 100))
            .map_err(|err| err.to_string())?;

        let mut refreshed_count = 0usize;
        let mut total_item_count = 0usize;

        for feed in due_feeds {
            let result =
                crate::refresh::refresh_feed_with_runtime(runtime, &mut storage, &feed.id)?;
            refreshed_count = refreshed_count.saturating_add(1);
            total_item_count = total_item_count.saturating_add(result.item_count);
        }

        Ok(crate::refresh::RefreshDueOutput { refreshed_count, total_item_count })
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// List normalized item summaries for one feed.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_list_items_json(input: *const c_char) -> *mut c_char {
    match with_input::<ListItemsInput, _>(input, |payload| {
        crate::entries::list_items(
            &payload.db_path,
            &payload.feed_id,
            payload.limit,
            payload.q.as_deref(),
        )
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// List unified entries across the inbox and special scopes.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_list_entries_json(input: *const c_char) -> *mut c_char {
    match with_input::<ListEntriesInput, _>(input, |payload| {
        crate::entries::list_entries(
            &payload.db_path,
            payload.feed_id.as_deref(),
            payload.filter.as_deref(),
            payload.q.as_deref(),
            payload.limit,
            payload.kind.as_deref(),
        )
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Return aggregate scope counts for the unified inbox.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_item_counts_json(input: *const c_char) -> *mut c_char {
    match with_input::<DbInput, _>(input, |payload| crate::entries::item_counts(&payload.db_path)) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Fetch the current detail payload for one entry.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_get_entry_json(input: *const c_char) -> *mut c_char {
    match with_input::<FullTextInput, _>(input, |payload| {
        crate::entries::get_entry(&payload.db_path, &payload.item_id)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Create a new entry in the unified inbox.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_create_entry_json(input: *const c_char) -> *mut c_char {
    match with_input::<CreateEntryInput, _>(input, |payload| {
        crate::entries::create_entry(&payload.db_path.clone(), payload)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Fetch and persist full text for one entry.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_fetch_fulltext_json(input: *const c_char) -> *mut c_char {
    match with_input::<FullTextInput, _>(input, |payload| {
        crate::entries::fetch_fulltext(&payload.db_path, &payload.item_id)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Patch item read/star/later/archive states.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_patch_item_state_json(
    input: *const c_char,
) -> *mut c_char {
    match with_input::<PatchItemStateInput, _>(input, |payload| {
        crate::entries::patch_item_state(
            &payload.db_path,
            &payload.item_id,
            payload.is_read,
            payload.is_starred,
            payload.is_saved_for_later,
            payload.is_archived,
        )
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Return global notification defaults.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_get_global_notification_settings_json(
    input: *const c_char,
) -> *mut c_char {
    match with_input::<DbInput, _>(input, |payload| {
        crate::notifications::get_global_notification_settings(&payload.db_path)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Update global notification defaults.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_update_global_notification_settings_json(
    input: *const c_char,
) -> *mut c_char {
    match with_input::<UpdateGlobalNotificationSettingsInput, _>(input, |payload| {
        crate::notifications::update_global_notification_settings(
            &payload.db_path,
            &payload.settings,
        )
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Return per-feed notification settings, falling back to global defaults when absent.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_get_feed_notification_settings_json(
    input: *const c_char,
) -> *mut c_char {
    match with_input::<FeedNotificationSettingsInput, _>(input, |payload| {
        crate::notifications::get_feed_notification_settings(&payload.db_path, &payload.feed_id)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Update per-feed notification settings.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_update_feed_notification_settings_json(
    input: *const c_char,
) -> *mut c_char {
    match with_input::<UpdateFeedNotificationSettingsInput, _>(input, |payload| {
        crate::notifications::update_feed_notification_settings(
            &payload.db_path,
            &payload.feed_id,
            &payload.settings,
        )
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Return per-feed auto-refresh settings, falling back to the effective scope when absent.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_get_feed_refresh_settings_json(
    input: *const c_char,
) -> *mut c_char {
    match with_input::<RefreshSettingsInput, _>(input, |payload| {
        crate::notifications::get_feed_refresh_settings(&payload.db_path, &payload.feed_id)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Update per-feed auto-refresh settings.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_update_feed_refresh_settings_json(
    input: *const c_char,
) -> *mut c_char {
    match with_input::<UpdateFeedRefreshSettingsInput, _>(input, |payload| {
        crate::notifications::update_feed_refresh_settings(
            &payload.db_path,
            &payload.feed_id,
            &payload.settings,
        )
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Clear explicit per-feed auto-refresh settings.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_delete_feed_refresh_settings_json(
    input: *const c_char,
) -> *mut c_char {
    match with_input::<RefreshSettingsInput, _>(input, |payload| {
        crate::notifications::delete_feed_refresh_settings(&payload.db_path, &payload.feed_id)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Return per-group auto-refresh settings.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_get_group_refresh_settings_json(
    input: *const c_char,
) -> *mut c_char {
    match with_input::<GroupRefreshSettingsInput, _>(input, |payload| {
        crate::notifications::get_group_refresh_settings(&payload.db_path, &payload.group_id)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Update per-group auto-refresh settings.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_update_group_refresh_settings_json(
    input: *const c_char,
) -> *mut c_char {
    match with_input::<UpdateGroupRefreshSettingsInput, _>(input, |payload| {
        crate::notifications::update_group_refresh_settings(
            &payload.db_path,
            &payload.group_id,
            &payload.settings,
        )
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Clear explicit per-group auto-refresh settings.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_delete_group_refresh_settings_json(
    input: *const c_char,
) -> *mut c_char {
    match with_input::<GroupRefreshSettingsInput, _>(input, |payload| {
        crate::notifications::delete_group_refresh_settings(&payload.db_path, &payload.group_id)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// List notification events waiting for delivery.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_list_pending_notification_events_json(
    input: *const c_char,
) -> *mut c_char {
    match with_input::<PendingNotificationEventsInput, _>(input, |payload| {
        crate::notifications::list_pending_notification_events(&payload.db_path, payload.limit)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Mark pending notification events as delivered.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_ack_notification_events_json(
    input: *const c_char,
) -> *mut c_char {
    #[derive(Debug, Serialize)]
    struct Output {
        acknowledged: usize,
    }

    match with_input::<crate::notifications::AckNotificationEventsInput, _>(input, |payload| {
        crate::notifications::ack_notification_events(&payload.db_path, &payload.event_ids)
            .map(|acknowledged| Output { acknowledged })
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// List pending sync events.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_list_sync_events_json(
    input: *const c_char,
) -> *mut c_char {
    match with_input::<ListSyncEventsInput, _>(input, |payload| {
        crate::sync::list_sync_events(&payload.db_path, payload.limit)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Acknowledge sync events.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_ack_sync_events_json(input: *const c_char) -> *mut c_char {
    #[derive(Debug, Serialize)]
    struct Output {
        acknowledged: usize,
    }

    match with_input::<AckSyncEventsInput, _>(input, |payload| {
        crate::sync::ack_sync_events(&payload.db_path, &payload.event_ids)
            .map(|acknowledged| Output { acknowledged })
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Apply remote sync events.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_apply_sync_events_json(
    input: *const c_char,
) -> *mut c_char {
    match with_input::<ApplySyncEventsInput, _>(input, |payload| {
        crate::sync::apply_sync_events(&payload.db_path, payload.events)
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

fn respond_ok<T: Serialize>(data: T) -> *mut c_char {
    crate::envelope::respond_ok(data)
}
