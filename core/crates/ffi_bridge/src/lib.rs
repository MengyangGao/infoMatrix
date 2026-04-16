//! Flutter-facing Rust FFI bridge for InfoMatrix core capabilities.

use std::ffi::{CStr, CString, c_char};
use std::path::Path;

use ammonia::Builder as HtmlSanitizerBuilder;
use app_core::InfoMatrixAppCore;
use chrono::{DateTime, Local, Utc};
use discovery::{DiscoveryService, ReqwestDiscoveryClient};
use fetcher::{
    FeedHttpClient, FetchError, FetchOutcome, FetchRequest, FetchService, ReqwestFeedClient,
};
use models::{
    FeedType, GlobalNotificationSettings, ItemStatePatch, NewFeed, NotificationDeliveryState,
    NotificationEvent, NotificationMode, NotificationSettings,
};
use notifications::{
    NotificationCandidate, NotificationDecision, build_digest_batch, canonical_identity_key,
    content_fingerprint,
};
use opml::{OpmlFeed, export_opml, import_opml};
use parser::ParsedFeed;
use scraper::{Html, Selector};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use storage::{ItemListFilter, Storage};
use url::Url;

const DEFAULT_TIMEOUT_SECS: u64 = 20;
const USER_AGENT: &str = "InfoMatrix/0.1 (+https://github.com/MengyangGao/infoMatrix)";

#[derive(Debug, Serialize)]
struct FfiEnvelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DbInput {
    db_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiscoverInput {
    db_path: Option<String>,
    site_url: String,
}

#[derive(Debug, Deserialize)]
struct AddSubscriptionInput {
    db_path: Option<String>,
    feed_url: String,
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SubscribeInput {
    db_path: Option<String>,
    input_url: String,
}

#[derive(Debug, Deserialize)]
struct OpmlImportInput {
    db_path: Option<String>,
    opml_xml: String,
}

#[derive(Debug, Deserialize)]
struct ListItemsInput {
    db_path: Option<String>,
    feed_id: String,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ListEntriesInput {
    db_path: Option<String>,
    feed_id: Option<String>,
    filter: Option<String>,
    q: Option<String>,
    limit: Option<usize>,
    kind: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RefreshInput {
    db_path: Option<String>,
    feed_id: String,
}

#[derive(Debug, Deserialize)]
struct RefreshDueInput {
    db_path: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct FeedNotificationSettingsInput {
    db_path: Option<String>,
    feed_id: String,
}

#[derive(Debug, Deserialize)]
struct UpdateFeedNotificationSettingsInput {
    db_path: Option<String>,
    feed_id: String,
    settings: NotificationSettings,
}

#[derive(Debug, Deserialize)]
struct UpdateGlobalNotificationSettingsInput {
    db_path: Option<String>,
    settings: GlobalNotificationSettings,
}

#[derive(Debug, Deserialize)]
struct PendingNotificationEventsInput {
    db_path: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct AckNotificationEventsInput {
    db_path: Option<String>,
    event_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PatchItemStateInput {
    db_path: Option<String>,
    item_id: String,
    is_read: Option<bool>,
    is_starred: Option<bool>,
    is_saved_for_later: Option<bool>,
    is_archived: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateEntryInput {
    db_path: Option<String>,
    id: Option<String>,
    title: String,
    kind: Option<String>,
    source_kind: Option<String>,
    source_id: Option<String>,
    source_url: Option<String>,
    source_title: Option<String>,
    canonical_url: Option<String>,
    summary: Option<String>,
    content_html: Option<String>,
    content_text: Option<String>,
    raw_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FullTextInput {
    db_path: Option<String>,
    item_id: String,
}

#[derive(Debug, Serialize)]
struct HealthOutput {
    status: &'static str,
    version: &'static str,
}

#[derive(Debug, Serialize)]
struct DefaultDbPathOutput {
    db_path: String,
}

#[derive(Debug, Serialize)]
struct FeedOutput {
    id: String,
    title: String,
    feed_url: String,
    site_url: Option<String>,
    feed_type: String,
    auto_full_text: bool,
    icon_url: Option<String>,
    groups: Vec<FeedGroupOutput>,
}

#[derive(Debug, Serialize)]
struct FeedGroupOutput {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct CreateGroupInput {
    db_path: Option<String>,
    name: String,
}

#[derive(Debug, Deserialize)]
struct UpdateFeedInput {
    db_path: Option<String>,
    feed_id: String,
    title: Option<String>,
    auto_full_text: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct UpdateFeedGroupInput {
    db_path: Option<String>,
    feed_id: String,
    group_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeleteFeedInput {
    db_path: Option<String>,
    feed_id: String,
}

#[derive(Debug, Serialize)]
struct EntryOutput {
    id: String,
    title: String,
    kind: String,
    source_kind: String,
    source_id: Option<String>,
    source_url: Option<String>,
    source_title: Option<String>,
    canonical_url: Option<String>,
    published_at: Option<String>,
    summary_preview: Option<String>,
    summary: Option<String>,
    content_html: Option<String>,
    content_text: Option<String>,
    is_read: bool,
    is_starred: bool,
    is_saved_for_later: bool,
    is_archived: bool,
}

#[derive(Debug, Serialize)]
struct PatchItemStateOutput {
    item_id: String,
    is_read: bool,
    is_starred: bool,
    is_saved_for_later: bool,
    is_archived: bool,
}

#[derive(Debug, Serialize)]
struct FullTextOutput {
    item_id: String,
    content_text: String,
    source: String,
}

#[derive(Debug, Serialize)]
struct RefreshOutput {
    status: String,
    fetched_http_status: u16,
    item_count: usize,
    notification_count: usize,
    suppressed_notification_count: usize,
}

#[derive(Debug, Serialize)]
struct RefreshDueOutput {
    refreshed_count: usize,
    total_item_count: usize,
}

#[derive(Debug, Serialize)]
struct DiscoverOutput {
    normalized_site_url: String,
    discovered_feeds: Vec<DiscoveredFeedOutput>,
    site_title: Option<String>,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DiscoveredFeedOutput {
    url: String,
    title: Option<String>,
    feed_type: String,
    confidence: f32,
    source: String,
    score: i32,
}

#[derive(Debug, Serialize)]
struct SubscribeOutput {
    feed_id: String,
    resolved_feed_url: String,
    subscription_source: String,
}

#[derive(Debug, Serialize)]
struct OpmlImportOutput {
    parsed_feed_count: usize,
    unique_feed_count: usize,
    grouped_feed_count: usize,
}

#[derive(Debug, Serialize)]
struct OpmlExportOutput {
    opml_xml: String,
    feed_count: usize,
}

#[derive(Debug, Serialize)]
struct AckOutput {
    ok: bool,
}

#[derive(Debug, Deserialize)]
struct ListSyncEventsInput {
    db_path: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct AckSyncEventsInput {
    db_path: Option<String>,
    event_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MetaOutput {
    api_version: i32,
    app_version: &'static str,
}

#[derive(Debug, Serialize)]
struct SyncEventOutput {
    id: String,
    entity_type: String,
    entity_id: String,
    event_type: String,
    payload_json: String,
    created_at: String,
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
    respond_ok(DefaultDbPathOutput { db_path: default_db_path() })
}

/// List all subscribed feeds.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_list_feeds_json(input: *const c_char) -> *mut c_char {
    match with_input::<DbInput, _>(input, |payload| {
        let storage = open_storage(&payload.db_path)?;
        let rows = storage.list_feeds().map_err(|err| err.to_string())?;
        let feeds: Vec<FeedOutput> = rows
            .into_iter()
            .map(|feed| {
                let groups = storage
                    .list_groups_for_feed(&feed.id)
                    .map_err(|err| err.to_string())?
                    .into_iter()
                    .map(|group| FeedGroupOutput { id: group.id, name: group.name })
                    .collect();
                let icon_url = storage
                    .get_feed_icon(&feed.id)
                    .map_err(|err| err.to_string())?
                    .map(|icon| icon.source_url)
                    .or_else(|| feed_icon_url(feed.site_url.as_ref(), &feed.feed_url));
                Ok(FeedOutput {
                    id: feed.id,
                    title: feed.title.unwrap_or_else(|| {
                        feed.feed_url.host_str().unwrap_or("Untitled").to_owned()
                    }),
                    feed_url: feed.feed_url.to_string(),
                    site_url: feed.site_url.map(|value| value.to_string()),
                    feed_type: feed_type_label(feed.feed_type).to_owned(),
                    auto_full_text: feed.auto_full_text,
                    icon_url,
                    groups,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(feeds)
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
        let storage = open_storage(&payload.db_path)?;
        let groups = storage
            .list_groups()
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|group| FeedGroupOutput { id: group.id, name: group.name })
            .collect::<Vec<_>>();
        Ok(groups)
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
        let storage = open_storage(&payload.db_path)?;
        let group = storage.create_group(&payload.name).map_err(|err| err.to_string())?;
        Ok(FeedGroupOutput { id: group.id, name: group.name })
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
        let storage = open_storage(&payload.db_path)?;
        if let Some(title) = payload.title.as_deref() {
            storage
                .update_feed_title(&payload.feed_id, Some(title))
                .map_err(|err| err.to_string())?;
        }
        if let Some(auto_full_text) = payload.auto_full_text {
            storage
                .update_feed_auto_full_text(&payload.feed_id, auto_full_text)
                .map_err(|err| err.to_string())?;
        }
        Ok(AckOutput { ok: true })
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
        let mut storage = open_storage(&payload.db_path)?;
        storage
            .set_feed_group(&payload.feed_id, payload.group_id.as_deref())
            .map_err(|err| err.to_string())?;
        Ok(AckOutput { ok: true })
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
        let storage = open_storage(&payload.db_path)?;
        storage.delete_feed(&payload.feed_id).map_err(|err| err.to_string())?;
        Ok(AckOutput { ok: true })
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
        let feed_url =
            Url::parse(&payload.feed_url).map_err(|err| format!("invalid feed url: {err}"))?;
        enforce_web_url(&feed_url, "feed url")?;
        let title_fallback = payload.title.clone();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("failed to create tokio runtime: {err}"))?;

        if let Some(direct_probe) = probe_direct_feed(&runtime, &feed_url)? {
            let mut storage = open_storage(&payload.db_path)?;
            let site_url_for_icon = direct_probe
                .parsed
                .site_url
                .clone()
                .or_else(|| derive_site_url_from_feed(&direct_probe.response.final_url));
            let feed_id = persist_parsed_feed_snapshot(
                &mut storage,
                &direct_probe.response.final_url,
                title_fallback.clone(),
                site_url_for_icon.clone(),
                &direct_probe.parsed,
                FeedSnapshotFetchMetadata {
                    http_status: direct_probe.response.status,
                    etag: direct_probe.response.etag,
                    last_modified: direct_probe.response.last_modified,
                    duration_ms: direct_probe.response.duration_ms,
                },
            )?;
            if let Some(site_url) = site_url_for_icon.as_ref() {
                if let Some(icon_url) =
                    feed_icon_url(Some(site_url), &direct_probe.response.final_url)
                {
                    let _ = storage.set_feed_icon(&feed_id, &icon_url);
                }
            }
            return Ok(Output { feed_id });
        }

        let mut storage = open_storage(&payload.db_path)?;
        let site_url = derive_site_url_from_feed(&feed_url);
        let feed_url_for_icon = feed_url.clone();
        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url,
                site_url: site_url.clone(),
                title: title_fallback,
                feed_type: FeedType::Unknown,
            })
            .map_err(|err| err.to_string())?;
        if let Some(site_url) = site_url.as_ref() {
            if let Some(icon_url) = feed_icon_url(Some(site_url), &feed_url_for_icon) {
                let _ = storage.set_feed_icon(&feed_id, &icon_url);
            }
        }
        Ok(Output { feed_id })
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
        let normalized_input = normalize_input_url(&payload.input_url)?;
        let parsed_input =
            Url::parse(&normalized_input).map_err(|err| format!("invalid input url: {err}"))?;
        enforce_web_url(&parsed_input, "input url")?;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("failed to create tokio runtime: {err}"))?;

        if let Some(direct_probe) = probe_direct_feed(&runtime, &parsed_input)? {
            let mut storage = open_storage(&payload.db_path)?;
            let site_url_fallback = derive_site_url_from_feed(&direct_probe.response.final_url);
            let feed_id = persist_parsed_feed_snapshot(
                &mut storage,
                &direct_probe.response.final_url,
                direct_probe.parsed.title.clone(),
                site_url_fallback,
                &direct_probe.parsed,
                FeedSnapshotFetchMetadata {
                    http_status: direct_probe.response.status,
                    etag: direct_probe.response.etag,
                    last_modified: direct_probe.response.last_modified,
                    duration_ms: direct_probe.response.duration_ms,
                },
            )?;

            return Ok(SubscribeOutput {
                feed_id,
                resolved_feed_url: direct_probe.response.final_url.to_string(),
                subscription_source: "direct_feed".to_owned(),
            });
        }

        let discovery_client = ReqwestDiscoveryClient::new(DEFAULT_TIMEOUT_SECS, USER_AGENT)
            .map_err(|err| format!("failed to initialize discovery client: {err}"))?;
        let discovery_service = DiscoveryService::new(discovery_client);
        let discovered = runtime
            .block_on(discovery_service.discover(&normalized_input))
            .map_err(|err| err.to_string())?;
        let candidate = discovered
            .discovered_feeds
            .first()
            .ok_or_else(|| "could not discover a valid feed from input url".to_owned())?;
        enforce_web_url(&candidate.url, "discovered feed url")?;

        let storage = open_storage(&payload.db_path)?;
        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: candidate.url.clone(),
                site_url: Some(discovered.normalized_site_url.clone()),
                title: candidate.title.clone(),
                feed_type: candidate.feed_type,
            })
            .map_err(|err| err.to_string())?;

        Ok(SubscribeOutput {
            feed_id,
            resolved_feed_url: candidate.url.to_string(),
            subscription_source: "discovery".to_owned(),
        })
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
    match with_input::<DiscoverInput, _>(input, |payload| {
        let _ = payload.db_path;
        let client = ReqwestDiscoveryClient::new(DEFAULT_TIMEOUT_SECS, USER_AGENT)
            .map_err(|err| format!("failed to initialize discovery client: {err}"))?;
        let service = DiscoveryService::new(client);

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("failed to create tokio runtime: {err}"))?;

        let discovered =
            runtime.block_on(service.discover(&payload.site_url)).map_err(|err| err.to_string())?;

        let mut discovered_feeds: Vec<DiscoveredFeedOutput> = discovered
            .discovered_feeds
            .into_iter()
            .map(|feed| {
                let score = score_from_confidence(feed.confidence);
                DiscoveredFeedOutput {
                    url: feed.url.to_string(),
                    title: feed.title,
                    feed_type: feed_type_label(feed.feed_type).to_owned(),
                    confidence: feed.confidence,
                    source: format!("{:?}", feed.source).to_ascii_lowercase(),
                    score,
                }
            })
            .collect();
        discovered_feeds.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| b.confidence.total_cmp(&a.confidence))
                .then_with(|| a.url.cmp(&b.url))
        });

        Ok(DiscoverOutput {
            normalized_site_url: discovered.normalized_site_url.to_string(),
            discovered_feeds,
            site_title: discovered.site_title,
            warnings: discovered.warnings,
        })
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
        let core = open_app_core(&payload.db_path)?;
        let feeds = core.list_feeds().map_err(|err| err.to_string())?;
        let storage = core.storage;
        let mut opml_feeds = Vec::with_capacity(feeds.len());

        for feed in feeds {
            let group = storage
                .list_groups_for_feed(&feed.id)
                .map_err(|err| err.to_string())?
                .into_iter()
                .next()
                .map(|row| row.name);
            opml_feeds.push(OpmlFeed {
                title: feed.title,
                xml_url: feed.feed_url,
                html_url: feed.site_url,
                group,
            });
        }

        let feed_count = opml_feeds.len();
        let opml_xml = export_opml(&opml_feeds, "InfoMatrix Subscriptions")
            .map_err(|err| format!("opml export failed: {err}"))?;
        Ok(OpmlExportOutput { opml_xml, feed_count })
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
        let opml_xml = payload.opml_xml.trim();
        if opml_xml.is_empty() {
            return Err("opml_xml is empty".to_owned());
        }

        let feeds = import_opml(opml_xml).map_err(|err| format!("opml import failed: {err}"))?;
        let parsed_feed_count = feeds.len();
        let mut unique_urls = std::collections::HashSet::new();
        let mut grouped_feed_count = 0usize;

        let mut storage = open_storage(&payload.db_path)?;
        for feed in feeds {
            unique_urls.insert(feed.xml_url.to_string());
            let feed_id = storage
                .upsert_feed(&NewFeed {
                    feed_url: feed.xml_url,
                    site_url: feed.html_url,
                    title: feed.title,
                    feed_type: FeedType::Unknown,
                })
                .map_err(|err| err.to_string())?;

            if let Some(group_name) =
                feed.group.map(|name| name.trim().to_owned()).filter(|name| !name.is_empty())
            {
                let group = storage.create_group(&group_name).map_err(|err| err.to_string())?;
                storage.set_feed_group(&feed_id, Some(&group.id)).map_err(|err| err.to_string())?;
                grouped_feed_count = grouped_feed_count.saturating_add(1);
            }
        }

        Ok(OpmlImportOutput {
            parsed_feed_count,
            unique_feed_count: unique_urls.len(),
            grouped_feed_count,
        })
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
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("failed to create tokio runtime: {err}"))?;
        refresh_feed_with_runtime(&runtime, &mut storage, &payload.feed_id)
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
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("failed to create tokio runtime: {err}"))?;
        let due_feeds = storage
            .list_due_feeds(payload.limit.unwrap_or(20).clamp(1, 100))
            .map_err(|err| err.to_string())?;

        let mut refreshed_count = 0usize;
        let mut total_item_count = 0usize;

        for feed in due_feeds {
            let result = refresh_feed_with_runtime(&runtime, &mut storage, &feed.id)?;
            refreshed_count = refreshed_count.saturating_add(1);
            total_item_count = total_item_count.saturating_add(result.item_count);
        }

        Ok(RefreshDueOutput { refreshed_count, total_item_count })
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

fn refresh_feed_with_runtime(
    runtime: &tokio::runtime::Runtime,
    storage: &mut Storage,
    feed_id: &str,
) -> Result<RefreshOutput, String> {
    let feed = storage.get_feed(feed_id).map_err(|err| err.to_string())?;

    let client = ReqwestFeedClient::new(DEFAULT_TIMEOUT_SECS, USER_AGENT)
        .map_err(|err| format!("failed to initialize fetch client: {err}"))?;
    let service = FetchService::new(client);
    let request = FetchRequest {
        url: feed.feed_url.clone(),
        etag: feed.etag,
        last_modified: feed.last_modified,
    };

    let outcome = runtime
        .block_on(service.fetch_and_parse(feed_id, &request))
        .map_err(|err| err.to_string())?;

    match outcome {
        FetchOutcome::NotModified(response) => {
            storage
                .record_fetch_result(
                    feed_id,
                    response.status,
                    response.etag.as_deref(),
                    response.last_modified.as_deref(),
                    response.duration_ms,
                    None,
                )
                .map_err(|err| err.to_string())?;

            Ok(RefreshOutput {
                status: "not_modified".to_owned(),
                fetched_http_status: response.status,
                item_count: 0,
                notification_count: 0,
                suppressed_notification_count: 0,
            })
        }
        FetchOutcome::Updated { response, parsed } => {
            let item_count = parsed.items.len();
            let persisted_feed_id = persist_parsed_feed_snapshot(
                storage,
                &feed.feed_url,
                parsed.title.clone(),
                derive_site_url_from_feed(&feed.feed_url),
                &parsed,
                FeedSnapshotFetchMetadata {
                    http_status: response.status,
                    etag: response.etag,
                    last_modified: response.last_modified,
                    duration_ms: response.duration_ms,
                },
            )?;

            let notification_result =
                process_refresh_notifications(storage, &persisted_feed_id, &parsed.items)?;

            Ok(RefreshOutput {
                status: "updated".to_owned(),
                fetched_http_status: response.status,
                item_count,
                notification_count: notification_result.sent_count,
                suppressed_notification_count: notification_result.suppressed_count,
            })
        }
    }
}

struct NotificationProcessingResult {
    sent_count: usize,
    suppressed_count: usize,
}

fn process_refresh_notifications(
    storage: &mut Storage,
    feed_id: &str,
    items: &[models::NormalizedItem],
) -> Result<NotificationProcessingResult, String> {
    let globals = storage.get_global_notification_settings().map_err(|err| err.to_string())?;
    let settings = storage
        .get_notification_settings(feed_id)
        .map_err(|err| err.to_string())?
        .map(|row| row.settings)
        .unwrap_or_else(|| globals.default_feed_settings.clone());

    let now = Utc::now();
    let local_offset_minutes = Local::now().offset().local_minus_utc() / 60;
    let local_minute_of_day = notifications::policy::local_minute_of_day(now, local_offset_minutes);
    let mut last_notification_at =
        storage.get_latest_notification_at_for_feed(feed_id).map_err(|err| err.to_string())?;

    let mut sent_count = 0usize;
    let mut suppressed_count = 0usize;
    let mut digest_drafts = Vec::new();

    for item in items {
        let candidate = notification_candidate_from_item(feed_id, item);
        let canonical_key = canonical_identity_key(item);
        let fingerprint = content_fingerprint(item);
        let duplicate = storage
            .get_notification_signature(&canonical_key, &fingerprint)
            .map_err(|err| err.to_string())?
            .is_some();
        let seen_at = now.to_rfc3339();
        let _ = storage
            .record_notification_signature(
                feed_id,
                &candidate.entry_id,
                &canonical_key,
                &fingerprint,
                &seen_at,
                None,
            )
            .map_err(|err| err.to_string())?;
        let decision = notifications::policy::evaluate_candidate(
            &settings,
            &candidate,
            canonical_key.clone(),
            fingerprint.clone(),
            now,
            local_minute_of_day,
            duplicate,
            last_notification_at,
        );

        match decision {
            NotificationDecision::Send(draft) => {
                let event = NotificationEvent {
                    id: uuid::Uuid::now_v7().to_string(),
                    feed_id: Some(feed_id.to_owned()),
                    entry_id: Some(draft.entry_id.clone()),
                    canonical_key: draft.canonical_key.clone(),
                    content_fingerprint: draft.content_fingerprint.clone(),
                    mode: draft.mode,
                    delivery_state: NotificationDeliveryState::Pending,
                    title: draft.title.clone(),
                    body: draft.body.clone(),
                    reason: draft.reason.clone(),
                    digest_id: None,
                    created_at: draft.created_at,
                    ready_at: draft.ready_at,
                    delivered_at: None,
                    suppressed_at: None,
                };
                let metadata_json = serde_json::json!({
                    "source": "refresh",
                    "decision": "send",
                    "feed_id": feed_id,
                    "entry_id": draft.entry_id,
                })
                .to_string();
                storage
                    .insert_notification_event(&event, Some(&metadata_json))
                    .map_err(|err| err.to_string())?;
                storage
                    .touch_notification_signature(&canonical_key, &fingerprint, &seen_at)
                    .map_err(|err| err.to_string())?;
                last_notification_at = Some(now);
                sent_count = sent_count.saturating_add(1);
            }
            NotificationDecision::QueueDigest(draft) => {
                digest_drafts.push(draft);
            }
            NotificationDecision::Suppress(reason) => {
                let event = NotificationEvent {
                    id: uuid::Uuid::now_v7().to_string(),
                    feed_id: Some(feed_id.to_owned()),
                    entry_id: Some(candidate.entry_id.clone()),
                    canonical_key: canonical_key.clone(),
                    content_fingerprint: fingerprint.clone(),
                    mode: settings.mode,
                    delivery_state: NotificationDeliveryState::Suppressed,
                    title: candidate.title.clone(),
                    body: candidate.body_text(),
                    reason: notifications::coordinator::suppression_reason_label(&reason)
                        .to_owned(),
                    digest_id: None,
                    created_at: now,
                    ready_at: None,
                    delivered_at: None,
                    suppressed_at: Some(now),
                };
                let metadata_json = serde_json::json!({
                    "source": "refresh",
                    "decision": "suppressed",
                    "reason": notifications::coordinator::suppression_reason_label(&reason),
                    "feed_id": feed_id,
                    "entry_id": candidate.entry_id,
                })
                .to_string();
                storage
                    .insert_notification_event(&event, Some(&metadata_json))
                    .map_err(|err| err.to_string())?;
                suppressed_count = suppressed_count.saturating_add(1);
            }
        }
    }

    if !digest_drafts.is_empty() {
        let digest_batch = build_digest_batch(feed_id.to_owned(), &digest_drafts, now)
            .ok_or_else(|| "unable to build notification digest".to_owned())?;
        let digest = models::NotificationDigest {
            id: uuid::Uuid::now_v7().to_string(),
            feed_id: Some(feed_id.to_owned()),
            entry_count: digest_batch.entry_ids.len(),
            title: digest_batch.title.clone(),
            body: digest_batch.body.clone(),
            created_at: digest_batch.created_at,
            ready_at: digest_batch.ready_at,
            delivered_at: None,
        };
        let entry_ids_json =
            serde_json::to_string(&digest_batch.entry_ids).map_err(|err| err.to_string())?;
        storage
            .insert_notification_digest(&digest, &entry_ids_json)
            .map_err(|err| err.to_string())?;
        let digest_event = NotificationEvent {
            id: uuid::Uuid::now_v7().to_string(),
            feed_id: Some(feed_id.to_owned()),
            entry_id: digest_batch.entry_ids.first().cloned(),
            canonical_key: format!("digest:{feed_id}:{}", digest.id),
            content_fingerprint: digest.id.clone(),
            mode: NotificationMode::Digest,
            delivery_state: NotificationDeliveryState::Pending,
            title: digest.title.clone(),
            body: digest.body.clone(),
            reason: "digest_batch".to_owned(),
            digest_id: Some(digest.id.clone()),
            created_at: digest.created_at,
            ready_at: digest.ready_at,
            delivered_at: None,
            suppressed_at: None,
        };
        let metadata_json = serde_json::json!({
            "source": "refresh",
            "decision": "digest",
            "feed_id": feed_id,
            "entry_ids": digest_batch.entry_ids,
        })
        .to_string();
        storage
            .insert_notification_event(&digest_event, Some(&metadata_json))
            .map_err(|err| err.to_string())?;
        for draft in digest_drafts {
            storage
                .touch_notification_signature(
                    &draft.canonical_key,
                    &draft.content_fingerprint,
                    &now.to_rfc3339(),
                )
                .map_err(|err| err.to_string())?;
        }
        sent_count = sent_count.saturating_add(1);
    }

    Ok(NotificationProcessingResult { sent_count, suppressed_count })
}

fn notification_candidate_from_item(
    feed_id: &str,
    item: &models::NormalizedItem,
) -> NotificationCandidate {
    NotificationCandidate {
        feed_id: feed_id.to_owned(),
        entry_id: item.id.clone(),
        title: item.title.clone(),
        canonical_url: item.canonical_url.clone(),
        summary: item.summary.clone(),
        content_text: item.content_text.clone(),
        content_html: item.content_html.clone(),
        published_at: item.published_at,
        updated_at: item.updated_at,
        raw_hash: item.raw_hash.clone(),
    }
}

fn notification_event_from_row(row: storage::NotificationEventRow) -> NotificationEvent {
    NotificationEvent {
        id: row.id,
        feed_id: row.feed_id,
        entry_id: row.entry_id,
        canonical_key: row.canonical_key,
        content_fingerprint: row.content_fingerprint,
        title: row.title,
        body: row.body,
        mode: row.mode,
        delivery_state: row.delivery_state,
        reason: row.reason,
        digest_id: row.digest_id,
        created_at: parse_datetime(row.created_at).unwrap_or_else(Utc::now),
        ready_at: row.ready_at.and_then(parse_datetime),
        delivered_at: row.delivered_at.and_then(parse_datetime),
        suppressed_at: row.suppressed_at.and_then(parse_datetime),
    }
}

fn parse_datetime(value: String) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&value).map(|value| value.with_timezone(&Utc)).ok()
}

/// List normalized item summaries for one feed.
///
/// # Safety
/// `input` must be a valid, null-terminated C string pointer owned by the caller for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_list_items_json(input: *const c_char) -> *mut c_char {
    match with_input::<ListItemsInput, _>(input, |payload| {
        let core = open_app_core(&payload.db_path)?;
        let rows = core
            .search_items_for_feed(&payload.feed_id, payload.limit.unwrap_or(100), None)
            .map_err(|err| err.to_string())?;

        let items: Vec<EntryOutput> = rows.into_iter().map(entry_summary_output_from_row).collect();

        Ok(items)
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
        let core = open_app_core(&payload.db_path)?;
        let filter = parse_item_filter(payload.filter.as_deref());
        let kind = payload.kind.as_deref().map(parse_entry_kind_label);
        let rows = if let Some(feed_id) = payload.feed_id.as_deref() {
            core.search_items_for_feed(feed_id, payload.limit.unwrap_or(200), payload.q.as_deref())
                .map_err(|err| err.to_string())?
        } else {
            core.search_all_items(payload.limit.unwrap_or(200), payload.q.as_deref(), filter, kind)
                .map_err(|err| err.to_string())?
        };

        let items: Vec<EntryOutput> = rows.into_iter().map(entry_summary_output_from_row).collect();

        Ok(items)
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
    match with_input::<DbInput, _>(input, |payload| {
        let core = open_app_core(&payload.db_path)?;
        core.item_counts().map_err(|err| err.to_string())
    }) {
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
        let core = open_app_core(&payload.db_path)?;
        let detail = core.item_detail(&payload.item_id).map_err(|err| err.to_string())?;
        Ok(entry_detail_output_from_row(detail, None, None))
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
        let mut core = open_app_core(&payload.db_path)?;
        let entry_id = payload.id.unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
        let source_url = payload
            .source_url
            .as_deref()
            .map(|value| Url::parse(value).map_err(|err| format!("invalid source_url: {err}")))
            .transpose()?;
        let canonical_url = payload
            .canonical_url
            .as_deref()
            .map(|value| Url::parse(value).map_err(|err| format!("invalid canonical_url: {err}")))
            .transpose()?;
        let kind = payload.kind.as_deref().map(parse_entry_kind_label).unwrap_or_else(|| {
            if source_url.is_some() { models::EntryKind::Bookmark } else { models::EntryKind::Note }
        });
        let source_kind =
            payload.source_kind.as_deref().map(parse_entry_source_kind_label).unwrap_or_else(
                || {
                    if source_url.is_some() {
                        models::EntrySourceKind::Web
                    } else {
                        models::EntrySourceKind::Manual
                    }
                },
            );
        let is_web_capture = source_url.is_some() || matches!(kind, models::EntryKind::Bookmark);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("failed to create tokio runtime: {err}"))?;
        let fallback_source_title = source_url.as_ref().and_then(webpage_fallback_title);
        let webpage_capture = if is_web_capture {
            if let Some(source_url) = source_url.as_ref() {
                capture_webpage_snapshot(&runtime, source_url).ok()
            } else {
                None
            }
        } else {
            None
        };
        let resolved_title = {
            let requested = payload.title.trim();
            if !requested.is_empty() {
                requested.to_owned()
            } else if let Some(capture) = webpage_capture.as_ref() {
                capture
                    .title
                    .clone()
                    .or(fallback_source_title.clone())
                    .unwrap_or_else(|| "Untitled".to_owned())
            } else if let Some(source_url) = source_url.as_ref() {
                webpage_fallback_title(source_url).unwrap_or_else(|| "Untitled".to_owned())
            } else {
                "未命名随想".to_owned()
            }
        };
        let content_html = webpage_capture
            .as_ref()
            .and_then(|capture| capture.content_html.clone())
            .or(payload.content_html);
        let content_text = webpage_capture
            .as_ref()
            .and_then(|capture| capture.content_text.clone())
            .or(payload.content_text);
        let canonical_url = canonical_url
            .or_else(|| webpage_capture.as_ref().map(|capture| capture.final_url.clone()))
            .or_else(|| source_url.clone());
        let raw_hash = payload.raw_hash.unwrap_or_else(|| {
            let mut hasher = Sha256::new();
            hasher.update(entry_kind_label(kind));
            hasher.update(resolved_title.as_bytes());
            if let Some(value) = canonical_url.as_ref() {
                hasher.update(value.as_str().as_bytes());
            }
            if let Some(value) = source_url.as_ref() {
                hasher.update(value.as_str().as_bytes());
            }
            if let Some(value) = payload.summary.as_deref() {
                hasher.update(value.as_bytes());
            }
            if let Some(value) = content_text.as_deref() {
                hasher.update(value.as_bytes());
            }
            hex_encode(hasher.finalize())
        });

        core.create_entry(models::NewEntry {
            id: Some(entry_id.clone()),
            kind,
            source: models::EntrySource {
                source_kind,
                source_id: payload.source_id,
                source_url,
                source_title: payload.source_title,
            },
            external_item_id: None,
            canonical_url,
            title: resolved_title,
            author: None,
            summary: payload.summary,
            content_html,
            content_text,
            published_at: None,
            updated_at: None,
            raw_hash,
            dedup_reason: None,
            duplicate_of_entry_id: None,
        })
        .map_err(|err| err.to_string())?;

        let detail = core.item_detail(&entry_id).map_err(|err| err.to_string())?;
        Ok(entry_detail_output_from_row(detail, None, None))
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
        let mut storage = open_storage(&payload.db_path)?;
        let detail = storage.get_item_detail(&payload.item_id).map_err(|err| err.to_string())?;

        let candidate_url = detail
            .canonical_url
            .or(detail.source_url)
            .ok_or_else(|| "entry has no source url for fulltext".to_owned())?;
        enforce_web_url(&candidate_url, "entry url")?;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("failed to create tokio runtime: {err}"))?;
        let client = ReqwestFeedClient::new(DEFAULT_TIMEOUT_SECS, USER_AGENT)
            .map_err(|err| format!("failed to initialize fetch client: {err}"))?;
        let request = FetchRequest { url: candidate_url.clone(), etag: None, last_modified: None };
        let response = runtime.block_on(client.fetch(&request)).map_err(|err| err.to_string())?;
        if response.status >= 400 {
            return Err(format!("fulltext fetch failed with HTTP {}", response.status));
        }

        let html = String::from_utf8(response.body).map_err(|err| err.to_string())?;
        let extracted = extract_full_content(&html)
            .ok_or_else(|| "unable to extract meaningful fulltext from entry html".to_owned())?;
        let sanitized_html = extracted.content_html.as_deref().map(sanitize_html_fragment);
        storage
            .upsert_item_content(
                &payload.item_id,
                sanitized_html.as_deref(),
                Some(&extracted.content_text),
            )
            .map_err(|err| err.to_string())?;

        Ok(FullTextOutput {
            item_id: payload.item_id,
            content_text: extracted.content_text,
            source: extracted.source.to_owned(),
        })
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
        let core = open_app_core(&payload.db_path)?;
        let state = core
            .patch_item_state(
                &payload.item_id,
                &ItemStatePatch {
                    is_read: payload.is_read,
                    is_starred: payload.is_starred,
                    is_saved_for_later: payload.is_saved_for_later,
                    is_archived: payload.is_archived,
                },
            )
            .map_err(|err| err.to_string())?;

        Ok(PatchItemStateOutput {
            item_id: state.item_id,
            is_read: state.is_read,
            is_starred: state.is_starred,
            is_saved_for_later: state.is_saved_for_later,
            is_archived: state.is_archived,
        })
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
        let storage = open_storage(&payload.db_path)?;
        storage.get_global_notification_settings().map_err(|err| err.to_string())
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
        let storage = open_storage(&payload.db_path)?;
        storage
            .set_global_notification_settings(&payload.settings)
            .map_err(|err| err.to_string())?;
        storage.get_global_notification_settings().map_err(|err| err.to_string())
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
        let storage = open_storage(&payload.db_path)?;
        let globals = storage.get_global_notification_settings().map_err(|err| err.to_string())?;
        Ok(storage
            .get_notification_settings(&payload.feed_id)
            .map_err(|err| err.to_string())?
            .map(|row| row.settings)
            .unwrap_or_else(|| globals.default_feed_settings.clone()))
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
        let mut storage = open_storage(&payload.db_path)?;
        storage
            .upsert_notification_settings(&payload.feed_id, &payload.settings)
            .map_err(|err| err.to_string())?;
        storage
            .get_notification_settings(&payload.feed_id)
            .map_err(|err| err.to_string())?
            .map(|row| row.settings)
            .ok_or_else(|| "notification settings not found after update".to_owned())
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
        let storage = open_storage(&payload.db_path)?;
        let rows = storage
            .list_pending_notification_events(payload.limit.unwrap_or(50))
            .map_err(|err| err.to_string())?;
        let events = rows.into_iter().map(notification_event_from_row).collect::<Vec<_>>();
        Ok(events)
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

    match with_input::<AckNotificationEventsInput, _>(input, |payload| {
        let storage = open_storage(&payload.db_path)?;
        let acknowledged = storage
            .acknowledge_notification_events(&payload.event_ids)
            .map_err(|err| err.to_string())?;
        Ok(Output { acknowledged })
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
        let storage = open_storage(&payload.db_path)?;
        let limit = payload.limit.unwrap_or(100).clamp(1, 500);
        let events: Vec<SyncEventOutput> = storage
            .list_pending_sync_events(limit)
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|event| SyncEventOutput {
                id: event.id,
                entity_type: event.entity_type,
                entity_id: event.entity_id,
                event_type: event.event_type,
                payload_json: event.payload_json,
                created_at: event.created_at,
            })
            .collect();
        Ok(events)
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
        let storage = open_storage(&payload.db_path)?;
        let acknowledged =
            storage.acknowledge_sync_events(&payload.event_ids).map_err(|err| err.to_string())?;
        Ok(Output { acknowledged })
    }) {
        Ok(ptr) => ptr,
        Err(ptr) => ptr,
    }
}

/// Free a C string returned by this library.
///
/// # Safety
/// `ptr` must be a pointer previously returned by this crate from one of the `*_json` APIs.
/// It must not be freed more than once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn infomatrix_core_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    let _ = unsafe { CString::from_raw(ptr) };
}

fn with_input<I, O>(
    input: *const c_char,
    operation: impl FnOnce(I) -> Result<O, String>,
) -> Result<*mut c_char, *mut c_char>
where
    I: DeserializeOwned,
    O: Serialize,
{
    let parsed = match parse_input::<I>(input) {
        Ok(parsed) => parsed,
        Err(error) => return Err(respond_error(error)),
    };

    match operation(parsed) {
        Ok(output) => Ok(respond_ok(output)),
        Err(error) => Err(respond_error(error)),
    }
}

fn parse_input<T: DeserializeOwned>(input: *const c_char) -> Result<T, String> {
    if input.is_null() {
        return Err("input pointer is null".to_owned());
    }

    let payload = unsafe { CStr::from_ptr(input) }
        .to_str()
        .map_err(|err| format!("input is not valid utf-8: {err}"))?;

    serde_json::from_str(payload).map_err(|err| format!("invalid input json: {err}"))
}

fn respond_ok<T: Serialize>(data: T) -> *mut c_char {
    let envelope = FfiEnvelope { ok: true, data: Some(data), error: None };
    encode_envelope(&envelope)
}

fn respond_error(error: String) -> *mut c_char {
    let envelope = FfiEnvelope::<serde_json::Value> { ok: false, data: None, error: Some(error) };
    encode_envelope(&envelope)
}

fn encode_envelope<T: Serialize>(envelope: &FfiEnvelope<T>) -> *mut c_char {
    let json = serde_json::to_string(envelope).unwrap_or_else(|err| {
        format!(
            r#"{{"ok":false,"data":null,"error":"failed to encode json: {}"}}"#,
            sanitize_error(&err.to_string())
        )
    });
    CString::new(json)
        .unwrap_or_else(|_| {
            CString::new(r#"{"ok":false,"data":null,"error":"response contained null byte"}"#)
                .expect("fallback CString")
        })
        .into_raw()
}

fn sanitize_error(message: &str) -> String {
    message.replace('"', "'")
}

fn open_storage(db_path: &Option<String>) -> Result<Storage, String> {
    let db_path = db_path.clone().unwrap_or_else(default_db_path);
    ensure_parent_dir(&db_path)?;

    let storage = Storage::open(&db_path).map_err(|err| err.to_string())?;
    storage.migrate().map_err(|err| err.to_string())?;
    Ok(storage)
}

fn open_app_core(db_path: &Option<String>) -> Result<InfoMatrixAppCore, String> {
    Ok(InfoMatrixAppCore::new(open_storage(db_path)?))
}

fn ensure_parent_dir(db_path: &str) -> Result<(), String> {
    if let Some(parent) = Path::new(db_path).parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn default_db_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
    Path::new(&home).join(".infomatrix").join("infomatrix.db").to_string_lossy().to_string()
}

fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = bytes.as_ref();
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

struct ExtractedContent {
    content_html: Option<String>,
    content_text: String,
    source: &'static str,
}

fn feed_type_label(value: FeedType) -> &'static str {
    match value {
        FeedType::Rss => "rss",
        FeedType::Atom => "atom",
        FeedType::JsonFeed => "jsonfeed",
        FeedType::Unknown => "unknown",
    }
}

fn score_from_confidence(confidence: f32) -> i32 {
    (confidence * 120.0 - 30.0).round() as i32
}

fn normalize_input_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input url is empty".to_owned());
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Ok(trimmed.to_owned());
    }
    Ok(format!("https://{trimmed}"))
}

fn enforce_web_url(url: &Url, label: &str) -> Result<(), String> {
    match url.scheme() {
        "http" | "https" => Ok(()),
        other => Err(format!("{label} must use http or https scheme, got: {other}")),
    }
}

fn parse_item_filter(value: Option<&str>) -> ItemListFilter {
    match value.unwrap_or("all").trim().to_ascii_lowercase().as_str() {
        "unread" => ItemListFilter::Unread,
        "starred" => ItemListFilter::Starred,
        "later" => ItemListFilter::Later,
        "archive" | "archived" => ItemListFilter::Archive,
        _ => ItemListFilter::All,
    }
}

fn parse_entry_kind_label(value: &str) -> models::EntryKind {
    match value.trim().to_ascii_lowercase().as_str() {
        "bookmark" => models::EntryKind::Bookmark,
        "note" => models::EntryKind::Note,
        "quote" => models::EntryKind::Quote,
        _ => models::EntryKind::Article,
    }
}

fn parse_entry_source_kind_label(value: &str) -> models::EntrySourceKind {
    match value.trim().to_ascii_lowercase().as_str() {
        "web" => models::EntrySourceKind::Web,
        "manual" => models::EntrySourceKind::Manual,
        "import" => models::EntrySourceKind::Import,
        "sync" => models::EntrySourceKind::Sync,
        _ => models::EntrySourceKind::Feed,
    }
}

fn entry_kind_label(value: models::EntryKind) -> &'static str {
    match value {
        models::EntryKind::Article => "article",
        models::EntryKind::Bookmark => "bookmark",
        models::EntryKind::Note => "note",
        models::EntryKind::Quote => "quote",
    }
}

fn entry_source_kind_label(value: models::EntrySourceKind) -> &'static str {
    match value {
        models::EntrySourceKind::Feed => "feed",
        models::EntrySourceKind::Web => "web",
        models::EntrySourceKind::Manual => "manual",
        models::EntrySourceKind::Import => "import",
        models::EntrySourceKind::Sync => "sync",
    }
}

fn entry_summary_output_from_row(row: storage::ItemSummaryRow) -> EntryOutput {
    EntryOutput {
        id: row.id,
        title: row.title,
        kind: entry_kind_label(row.kind).to_owned(),
        source_kind: entry_source_kind_label(row.source_kind).to_owned(),
        source_id: row.source_id,
        source_url: row.source_url.map(|value| value.to_string()),
        source_title: None,
        canonical_url: row.canonical_url.map(|value| value.to_string()),
        published_at: row.published_at,
        summary_preview: row.summary_preview,
        summary: None,
        content_html: None,
        content_text: None,
        is_read: row.is_read,
        is_starred: row.is_starred,
        is_saved_for_later: row.is_saved_for_later,
        is_archived: row.is_archived,
    }
}

fn entry_detail_output_from_row(
    row: storage::ItemDetailRow,
    summary_preview: Option<String>,
    content_html_override: Option<String>,
) -> EntryOutput {
    EntryOutput {
        id: row.id,
        title: row.title,
        kind: entry_kind_label(row.kind).to_owned(),
        source_kind: entry_source_kind_label(row.source_kind).to_owned(),
        source_id: row.source_id,
        source_url: row.source_url.map(|value| value.to_string()),
        source_title: None,
        canonical_url: row.canonical_url.map(|value| value.to_string()),
        published_at: row.published_at,
        summary_preview,
        summary: row.summary,
        content_html: content_html_override.or(row.content_html),
        content_text: row.content_text,
        is_read: row.is_read,
        is_starred: row.is_starred,
        is_saved_for_later: row.is_saved_for_later,
        is_archived: row.is_archived,
    }
}

fn extract_full_content(html: &str) -> Option<ExtractedContent> {
    let document = Html::parse_document(html);

    for selector in [
        "article",
        "main",
        ".article-prose",
        ".markdown-body",
        ".post-content",
        ".entry-content",
        ".post",
        "body",
    ] {
        if let Some(extracted) = extract_from_selector(&document, selector) {
            return Some(extracted);
        }
    }

    None
}

fn extract_from_selector(document: &Html, selector: &str) -> Option<ExtractedContent> {
    let selector = Selector::parse(selector).ok()?;
    let block_selector =
        Selector::parse("p,pre,li,h1,h2,h3,h4,h5,h6,blockquote,table,thead,tbody,tr,th,td").ok()?;

    for node in document.select(&selector) {
        let mut blocks = Vec::new();
        let mut saw_preformatted = false;
        for block in node.select(&block_selector) {
            let tag = block.value().name();
            let raw = block.text().collect::<String>();
            let text = if tag == "pre" {
                saw_preformatted = true;
                raw.trim().to_owned()
            } else {
                normalize_whitespace(&raw)
            };
            if text.len() >= 8 {
                blocks.push(text);
            }
        }

        let content_text = if blocks.is_empty() {
            normalize_whitespace(&node.text().collect::<String>())
        } else {
            blocks.join("\n\n")
        };

        let minimum_length = if saw_preformatted { 1 } else { 240 };
        if content_text.len() >= minimum_length {
            let content_html = Some(node.html());
            return Some(ExtractedContent { content_html, content_text, source: "web_extract" });
        }
    }

    None
}

fn sanitize_html_fragment(html: &str) -> String {
    HtmlSanitizerBuilder::new().clean(html).to_string()
}

fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn feed_icon_url(site_url: Option<&Url>, feed_url: &Url) -> Option<String> {
    let host = site_url
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
        .or_else(|| feed_url.host_str().map(ToOwned::to_owned))?;
    Some(format!("https://{host}/favicon.ico"))
}

fn derive_site_url_from_feed(feed_url: &Url) -> Option<Url> {
    let host = feed_url.host_str()?;
    Url::parse(&format!("{}://{host}", feed_url.scheme())).ok()
}

#[derive(Debug, Clone)]
struct FeedSnapshotFetchMetadata {
    http_status: u16,
    etag: Option<String>,
    last_modified: Option<String>,
    duration_ms: u128,
}

#[derive(Debug)]
struct DirectFeedProbe {
    response: fetcher::FetchResponse,
    parsed: ParsedFeed,
}

#[derive(Debug)]
struct WebpageCapture {
    final_url: Url,
    title: Option<String>,
    content_html: Option<String>,
    content_text: Option<String>,
}

fn rebind_items_to_feed(
    feed_id: &str,
    items: &[models::NormalizedItem],
) -> Vec<models::NormalizedItem> {
    items
        .iter()
        .cloned()
        .map(|mut item| {
            item.source_feed_id = feed_id.to_owned();
            item
        })
        .collect()
}

fn persist_parsed_feed_snapshot(
    storage: &mut Storage,
    feed_url: &Url,
    title_fallback: Option<String>,
    site_url_fallback: Option<Url>,
    parsed_feed: &ParsedFeed,
    fetch_metadata: FeedSnapshotFetchMetadata,
) -> Result<String, String> {
    let icon_site_url = parsed_feed.site_url.clone().or_else(|| site_url_fallback.clone());
    let feed_id = storage
        .upsert_feed(&NewFeed {
            feed_url: feed_url.clone(),
            site_url: parsed_feed.site_url.clone().or(site_url_fallback),
            title: parsed_feed.title.clone().or(title_fallback),
            feed_type: parsed_feed.feed_type,
        })
        .map_err(|err| err.to_string())?;

    let rebased_items = rebind_items_to_feed(&feed_id, &parsed_feed.items);
    storage.upsert_items(&rebased_items).map_err(|err| err.to_string())?;
    storage
        .record_fetch_result(
            &feed_id,
            fetch_metadata.http_status,
            fetch_metadata.etag.as_deref(),
            fetch_metadata.last_modified.as_deref(),
            fetch_metadata.duration_ms,
            None,
        )
        .map_err(|err| err.to_string())?;
    if let Some(site_url) = icon_site_url.as_ref() {
        if let Some(icon_url) = feed_icon_url(Some(site_url), feed_url) {
            let _ = storage.set_feed_icon(&feed_id, &icon_url);
        }
    }
    Ok(feed_id)
}

fn capture_webpage_snapshot(
    runtime: &tokio::runtime::Runtime,
    url: &Url,
) -> Result<WebpageCapture, String> {
    let client = ReqwestFeedClient::new(DEFAULT_TIMEOUT_SECS, USER_AGENT)
        .map_err(|err| format!("failed to initialize fetch client: {err}"))?;
    let request = FetchRequest { url: url.clone(), etag: None, last_modified: None };
    let response = runtime.block_on(client.fetch(&request)).map_err(|err| err.to_string())?;

    if response.status >= 400 {
        return Err(format!("webpage capture failed with HTTP {}", response.status));
    }

    let content_type = response.content_type.as_deref().map(|value| value.to_ascii_lowercase());
    if let Some(content_type) = content_type.as_deref() {
        let looks_like_html = content_type.contains("html")
            || content_type.contains("xml")
            || content_type.contains("xhtml");
        if !looks_like_html {
            return Err(format!("webpage capture only supports html content, got: {content_type}"));
        }
    }

    let html = String::from_utf8_lossy(&response.body).to_string();
    let document = Html::parse_document(&html);
    let title = extract_webpage_title(&document)
        .or_else(|| webpage_fallback_title(&response.final_url))
        .filter(|value| !value.trim().is_empty());
    let content_html =
        extract_webpage_body_html(&document).map(|value| sanitize_html_fragment(&value));
    let content_text = extract_webpage_body_text(&document);

    Ok(WebpageCapture { final_url: response.final_url, title, content_html, content_text })
}

fn extract_webpage_title(document: &Html) -> Option<String> {
    for selector in
        ["meta[property='og:title']", "meta[name='twitter:title']", "meta[name='title']"]
    {
        if let Some(value) = extract_meta_content(document, selector) {
            return Some(value);
        }
    }

    let selector = Selector::parse("title").ok()?;
    if let Some(value) = document
        .select(&selector)
        .next()
        .map(|node| node.text().collect::<String>().trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        return Some(value);
    }

    let selector = Selector::parse("h1").ok()?;
    document
        .select(&selector)
        .next()
        .map(|node| node.text().collect::<String>().trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn extract_meta_content(document: &Html, selector: &str) -> Option<String> {
    let selector = Selector::parse(selector).ok()?;
    document
        .select(&selector)
        .next()
        .and_then(|node| node.value().attr("content"))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn extract_webpage_body_html(document: &Html) -> Option<String> {
    let selector = Selector::parse("body").ok()?;
    document
        .select(&selector)
        .next()
        .map(|node| node.inner_html())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn extract_webpage_body_text(document: &Html) -> Option<String> {
    let selector = Selector::parse("body").ok()?;
    document
        .select(&selector)
        .next()
        .map(|node| normalize_whitespace(&node.text().collect::<String>()))
        .filter(|value| !value.is_empty())
}

fn webpage_fallback_title(url: &Url) -> Option<String> {
    let host = url.host_str()?.trim();
    if host.is_empty() {
        return None;
    }

    let path = url.path().trim_matches('/');
    if path.is_empty() {
        return Some(host.to_owned());
    }

    let last_segment = path
        .split('/')
        .rev()
        .find(|segment| !segment.trim().is_empty())
        .map(|segment| segment.trim().to_owned())?;
    Some(format!("{host} · {last_segment}"))
}

fn probe_direct_feed(
    runtime: &tokio::runtime::Runtime,
    candidate_url: &Url,
) -> Result<Option<DirectFeedProbe>, String> {
    let client = ReqwestFeedClient::new(DEFAULT_TIMEOUT_SECS, USER_AGENT)
        .map_err(|err| format!("failed to initialize fetch client: {err}"))?;
    let service = FetchService::new(client);
    let request = FetchRequest { url: candidate_url.clone(), etag: None, last_modified: None };

    let outcome = match runtime.block_on(service.fetch_and_parse("ffi_direct_probe", &request)) {
        Ok(outcome) => outcome,
        Err(FetchError::Parse(_)) => return Ok(None),
        Err(FetchError::UnexpectedStatus(status)) if status >= 400 => return Ok(None),
        Err(err) => return Err(err.to_string()),
    };

    match outcome {
        FetchOutcome::NotModified(_) => Ok(None),
        FetchOutcome::Updated { response, parsed } => {
            if parsed.feed_type == FeedType::Unknown {
                return Ok(None);
            }
            Ok(Some(DirectFeedProbe { response, parsed }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_envelope(output: *mut c_char) -> serde_json::Value {
        let json = unsafe { CStr::from_ptr(output) }.to_str().expect("utf8").to_owned();
        unsafe { infomatrix_core_free_string(output) };
        serde_json::from_str(&json).expect("json parse")
    }

    #[test]
    fn health_json_contains_ok() {
        let output = infomatrix_core_health_json();
        let value = decode_envelope(output);
        assert_eq!(value["ok"], true);
        assert_eq!(value["data"]["status"], "ok");
    }

    #[test]
    fn can_add_and_list_subscription() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = temp.path().to_string_lossy().to_string();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("local addr").port();
        let server_handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 2048];
                let _ = std::io::Read::read(&mut stream, &mut buffer);
                let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Local Example Feed</title>
    <link>https://example.com</link>
    <description>Local example feed</description>
    <item>
      <title>Hello</title>
      <link>https://example.com/post</link>
      <guid>item-1</guid>
      <description>Body</description>
    </item>
  </channel>
</rss>"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                let _ = std::io::Write::flush(&mut stream);
            }
        });

        let add_payload = serde_json::json!({
            "db_path": db_path,
            "feed_url": format!("http://127.0.0.1:{port}/feed.xml"),
            "title": "Example Feed"
        });

        let add_input = CString::new(add_payload.to_string()).expect("cstring");
        let add_output = unsafe { infomatrix_core_add_subscription_json(add_input.as_ptr()) };
        let add_envelope = decode_envelope(add_output);
        assert_eq!(add_envelope["ok"], true);
        let feed_id = add_envelope["data"]["feed_id"].as_str().expect("feed id").to_owned();

        let list_entries_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "feed_id": feed_id,
            "limit": 10
        });
        let list_entries_input = CString::new(list_entries_payload.to_string()).expect("cstring");
        let list_entries_output =
            unsafe { infomatrix_core_list_entries_json(list_entries_input.as_ptr()) };
        let list_entries_envelope = decode_envelope(list_entries_output);
        assert_eq!(list_entries_envelope["ok"], true);
        let entries = list_entries_envelope["data"].as_array().expect("entries array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["title"], "Hello");

        let list_payload = serde_json::json!({ "db_path": temp.path().to_string_lossy() });
        let list_input = CString::new(list_payload.to_string()).expect("cstring");
        let list_output = unsafe { infomatrix_core_list_feeds_json(list_input.as_ptr()) };
        let list_envelope = decode_envelope(list_output);

        assert_eq!(list_envelope["ok"], true);
        let feeds = list_envelope["data"].as_array().expect("array");
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0]["title"], "Local Example Feed");
        assert_eq!(feeds[0]["icon_url"], "https://example.com/favicon.ico");

        let _ = server_handle.join();
    }

    #[test]
    fn subscribe_direct_feed_returns_resolved_feed_url() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = temp.path().to_string_lossy().to_string();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("local addr").port();
        let server_handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 2048];
                let _ = std::io::Read::read(&mut stream, &mut buffer);
                let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Local Example Feed</title>
    <link>https://example.com</link>
    <description>Local example feed</description>
    <item>
      <title>Hello</title>
      <link>https://example.com/post</link>
      <guid>item-1</guid>
      <description>Body</description>
    </item>
  </channel>
</rss>"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                let _ = std::io::Write::flush(&mut stream);
            }
        });

        let feed_url = format!("http://127.0.0.1:{port}/feed.xml");
        let subscribe_payload = serde_json::json!({
            "db_path": db_path,
            "input_url": feed_url
        });
        let subscribe_input = CString::new(subscribe_payload.to_string()).expect("cstring");
        let subscribe_output =
            unsafe { infomatrix_core_subscribe_input_json(subscribe_input.as_ptr()) };
        let subscribe_envelope = decode_envelope(subscribe_output);

        assert_eq!(subscribe_envelope["ok"], true, "subscribe envelope: {subscribe_envelope}");
        assert_eq!(subscribe_envelope["data"]["resolved_feed_url"], feed_url);
        assert_eq!(subscribe_envelope["data"]["subscription_source"], "direct_feed");
        assert!(
            subscribe_envelope["data"]["feed_id"].as_str().is_some(),
            "feed id missing: {subscribe_envelope}"
        );

        let feed_id = subscribe_envelope["data"]["feed_id"].as_str().expect("feed id");
        let list_entries_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "feed_id": feed_id,
            "limit": 10
        });
        let list_entries_input = CString::new(list_entries_payload.to_string()).expect("cstring");
        let list_entries_output =
            unsafe { infomatrix_core_list_entries_json(list_entries_input.as_ptr()) };
        let list_entries_envelope = decode_envelope(list_entries_output);
        assert_eq!(list_entries_envelope["ok"], true);
        let entries = list_entries_envelope["data"].as_array().expect("entries array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["title"], "Hello");

        let list_feeds_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy()
        });
        let list_feeds_input = CString::new(list_feeds_payload.to_string()).expect("cstring");
        let list_feeds_output =
            unsafe { infomatrix_core_list_feeds_json(list_feeds_input.as_ptr()) };
        let list_feeds_envelope = decode_envelope(list_feeds_output);
        let feeds = list_feeds_envelope["data"].as_array().expect("feeds array");
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0]["icon_url"], "https://example.com/favicon.ico");

        let _ = server_handle.join();
    }

    #[test]
    fn create_entry_captures_webpage_snapshot() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = temp.path().to_string_lossy().to_string();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("local addr").port();
        let server_handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 2048];
                let _ = std::io::Read::read(&mut stream, &mut buffer);
                let body = r#"<!doctype html>
<html>
  <head><title>Example Page</title></head>
  <body>
    <main>
      <article><p>Story body</p></article>
    </main>
  </body>
</html>"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                let _ = std::io::Write::flush(&mut stream);
            }
        });

        let source_url = format!("http://127.0.0.1:{port}/story.html");
        let create_payload = serde_json::json!({
            "db_path": db_path,
            "title": "",
            "kind": "bookmark",
            "source_kind": "web",
            "source_url": source_url
        });
        let create_input = CString::new(create_payload.to_string()).expect("cstring");
        let create_output = unsafe { infomatrix_core_create_entry_json(create_input.as_ptr()) };
        let create_envelope = decode_envelope(create_output);
        assert_eq!(create_envelope["ok"], true, "create envelope: {create_envelope}");
        assert_eq!(create_envelope["data"]["title"], "Example Page");
        assert!(
            create_envelope["data"]["content_text"]
                .as_str()
                .expect("content_text")
                .contains("Story body")
        );

        let _ = server_handle.join();
    }

    #[test]
    fn can_manage_groups_and_feed_metadata() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = temp.path().to_string_lossy().to_string();

        let add_payload = serde_json::json!({
            "db_path": db_path,
            "feed_url": "https://example.com/feed.xml",
            "title": "Example Feed"
        });
        let add_input = CString::new(add_payload.to_string()).expect("cstring");
        let add_output = unsafe { infomatrix_core_add_subscription_json(add_input.as_ptr()) };
        let add_envelope = decode_envelope(add_output);
        assert_eq!(add_envelope["ok"], true);

        let create_group_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "name": "Tech"
        });
        let create_group_input = CString::new(create_group_payload.to_string()).expect("cstring");
        let create_group_output =
            unsafe { infomatrix_core_create_group_json(create_group_input.as_ptr()) };
        let create_group_envelope = decode_envelope(create_group_output);
        assert_eq!(create_group_envelope["ok"], true);
        assert_eq!(create_group_envelope["data"]["name"], "Tech");

        let list_groups_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy()
        });
        let list_groups_input = CString::new(list_groups_payload.to_string()).expect("cstring");
        let list_groups_output =
            unsafe { infomatrix_core_list_groups_json(list_groups_input.as_ptr()) };
        let list_groups_envelope = decode_envelope(list_groups_output);
        let groups = list_groups_envelope["data"].as_array().expect("groups array");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["name"], "Tech");

        let update_feed_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "feed_id": add_envelope["data"]["feed_id"].as_str().expect("feed id"),
            "title": "Renamed Feed"
        });
        let update_feed_input = CString::new(update_feed_payload.to_string()).expect("cstring");
        let update_feed_output =
            unsafe { infomatrix_core_update_feed_json(update_feed_input.as_ptr()) };
        let update_feed_envelope = decode_envelope(update_feed_output);
        assert_eq!(update_feed_envelope["ok"], true);

        let update_group_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "feed_id": add_envelope["data"]["feed_id"].as_str().expect("feed id"),
            "group_id": create_group_envelope["data"]["id"].as_str().expect("group id")
        });
        let update_group_input = CString::new(update_group_payload.to_string()).expect("cstring");
        let update_group_output =
            unsafe { infomatrix_core_update_feed_group_json(update_group_input.as_ptr()) };
        let update_group_envelope = decode_envelope(update_group_output);
        assert_eq!(update_group_envelope["ok"], true);

        let list_feeds_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy()
        });
        let list_feeds_input = CString::new(list_feeds_payload.to_string()).expect("cstring");
        let list_feeds_output =
            unsafe { infomatrix_core_list_feeds_json(list_feeds_input.as_ptr()) };
        let list_feeds_envelope = decode_envelope(list_feeds_output);
        let feeds = list_feeds_envelope["data"].as_array().expect("feeds array");
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0]["title"], "Renamed Feed");
        let feed_groups = feeds[0]["groups"].as_array().expect("feed groups");
        assert_eq!(feed_groups.len(), 1);
        assert_eq!(feed_groups[0]["name"], "Tech");

        let delete_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "feed_id": add_envelope["data"]["feed_id"].as_str().expect("feed id")
        });
        let delete_input = CString::new(delete_payload.to_string()).expect("cstring");
        let delete_output = unsafe { infomatrix_core_delete_feed_json(delete_input.as_ptr()) };
        let delete_envelope = decode_envelope(delete_output);
        assert_eq!(delete_envelope["ok"], true);

        let list_after_delete_output =
            unsafe { infomatrix_core_list_feeds_json(list_feeds_input.as_ptr()) };
        let list_after_delete_envelope = decode_envelope(list_after_delete_output);
        assert!(list_after_delete_envelope["data"].as_array().expect("feeds array").is_empty());
    }

    #[test]
    fn can_import_and_export_opml() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = temp.path().to_string_lossy().to_string();
        let opml_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>Subscriptions</title></head>
  <body>
    <outline text="Tech" title="Tech">
      <outline text="Example" title="Example" type="rss" xmlUrl="https://example.com/feed.xml" htmlUrl="https://example.com" />
    </outline>
  </body>
</opml>"#;

        let import_payload = serde_json::json!({
            "db_path": db_path,
            "opml_xml": opml_xml
        });
        let import_input = CString::new(import_payload.to_string()).expect("cstring");
        let import_output = unsafe { infomatrix_core_import_opml_json(import_input.as_ptr()) };
        let import_envelope = decode_envelope(import_output);
        assert_eq!(import_envelope["ok"], true);
        assert_eq!(import_envelope["data"]["parsed_feed_count"], 1);
        assert_eq!(import_envelope["data"]["unique_feed_count"], 1);
        assert_eq!(import_envelope["data"]["grouped_feed_count"], 1);

        let export_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy()
        });
        let export_input = CString::new(export_payload.to_string()).expect("cstring");
        let export_output = unsafe { infomatrix_core_export_opml_json(export_input.as_ptr()) };
        let export_envelope = decode_envelope(export_output);
        assert_eq!(export_envelope["ok"], true);
        assert_eq!(export_envelope["data"]["feed_count"], 1);
        let xml = export_envelope["data"]["opml_xml"].as_str().expect("xml");
        assert!(xml.contains("xmlUrl=\"https://example.com/feed.xml\""));
    }

    #[test]
    fn can_refresh_due_feeds() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = temp.path().to_string_lossy().to_string();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("local addr").port();
        let server_handle = std::thread::spawn(move || {
            for _ in 0..2 {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut buffer = [0u8; 1024];
                let _ = std::io::Read::read(&mut stream, &mut buffer);
                let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Example Feed</title>
    <link>https://example.com</link>
    <description>Example</description>
    <item>
      <title>Hello</title>
      <link>https://example.com/post</link>
      <guid>item-1</guid>
      <description>Body</description>
    </item>
  </channel>
</rss>"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                let _ = std::io::Write::flush(&mut stream);
            }
        });

        let feed_url = format!("http://127.0.0.1:{port}/feed.xml");

        let add_payload = serde_json::json!({
            "db_path": db_path,
            "feed_url": feed_url,
            "title": "Example Feed"
        });
        let add_input = CString::new(add_payload.to_string()).expect("cstring");
        let add_output = unsafe { infomatrix_core_add_subscription_json(add_input.as_ptr()) };
        let add_envelope = decode_envelope(add_output);
        let feed_id = add_envelope["data"]["feed_id"].as_str().expect("feed id");

        let mut storage =
            open_storage(&Some(temp.path().to_string_lossy().to_string())).expect("open storage");
        storage
            .set_feed_next_scheduled_fetch_at(feed_id, Some("2000-01-01T00:00:00Z"))
            .expect("make feed due");

        let refresh_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "limit": 10
        });
        let refresh_input = CString::new(refresh_payload.to_string()).expect("cstring");
        let refresh_output = unsafe { infomatrix_core_refresh_due_json(refresh_input.as_ptr()) };
        let refresh_envelope = decode_envelope(refresh_output);
        assert_eq!(refresh_envelope["ok"], true, "refresh envelope: {refresh_envelope}");
        assert_eq!(refresh_envelope["data"]["refreshed_count"], 1);

        let _ = server_handle.join();
    }

    #[test]
    fn can_create_note_and_count_notes() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = temp.path().to_string_lossy().to_string();

        let create_payload = serde_json::json!({
            "db_path": db_path,
            "title": "Quick note",
            "kind": "note",
            "source_kind": "manual",
            "content_text": "Body"
        });
        let create_input = CString::new(create_payload.to_string()).expect("cstring");
        let create_output = unsafe { infomatrix_core_create_entry_json(create_input.as_ptr()) };
        let create_envelope = decode_envelope(create_output);
        assert_eq!(create_envelope["ok"], true);
        assert_eq!(create_envelope["data"]["kind"], "note");
        let item_id = create_envelope["data"]["id"].as_str().expect("id").to_owned();

        let list_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "filter": "all",
            "limit": 20,
            "kind": "note"
        });
        let list_input = CString::new(list_payload.to_string()).expect("cstring");
        let list_output = unsafe { infomatrix_core_list_entries_json(list_input.as_ptr()) };
        let list_envelope = decode_envelope(list_output);
        assert_eq!(list_envelope["ok"], true);
        let rows = list_envelope["data"].as_array().expect("array");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], item_id);

        let counts_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy()
        });
        let counts_input = CString::new(counts_payload.to_string()).expect("cstring");
        let counts_output = unsafe { infomatrix_core_item_counts_json(counts_input.as_ptr()) };
        let counts_envelope = decode_envelope(counts_output);
        assert_eq!(counts_envelope["ok"], true);
        assert_eq!(counts_envelope["data"]["notes"], 1);
    }
}
