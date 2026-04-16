//! InfoMatrix local HTTP server.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use ammonia::Builder as HtmlSanitizerBuilder;
use app_core::{AppCoreError, InfoMatrixAppCore};
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use chrono::{DateTime, Duration, Local, Utc};
use fetcher::{FetchError, FetchOutcome, FetchRequest, FetchService, ReqwestFeedClient};
use icon::extract_icon_candidates;
use models::{
    EntryKind, EntrySource, EntrySourceKind, FeedHealthState, FeedType, GlobalNotificationSettings,
    ItemScopeCounts, ItemStatePatch, NewEntry, NewFeed, NormalizedItem, NotificationDeliveryState,
    NotificationDigest, NotificationEvent, NotificationMode, NotificationSettings,
};
use notifications::{
    NotificationCandidate, NotificationDecision, RefreshAttempt, RefreshReason, build_digest_batch,
    canonical_identity_key, compute_next_refresh_at, content_fingerprint,
};
use opml::{OpmlFeed, export_opml, import_opml};
use parser::{ParsedFeed, parse_feed};
use pulldown_cmark::{Options as MarkdownOptions, Parser as MarkdownParser, html as markdown_html};
use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use storage::{ItemListFilter, Storage, StorageError};
use tokio::task::JoinSet;
use tracing::{info, warn};
use url::Url;
use uuid::Uuid;

#[derive(Clone)]
struct AppContext {
    db_path: String,
    user_agent: String,
    timeout_secs: u64,
    migrate_on_open: bool,
}

struct FeedSnapshotFetchMetadata {
    http_status: u16,
    etag: Option<String>,
    last_modified: Option<String>,
    duration_ms: u128,
}

const DISCOVERY_CACHE_TTL_HOURS: i64 = 24;

#[derive(Debug, Clone)]
struct RuntimeConfig {
    db_path: String,
    bind_addr: SocketAddr,
}

#[derive(Debug, thiserror::Error)]
enum ApiError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
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

#[derive(Debug, Serialize)]
struct ApiMessage {
    message: String,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct MetaResponse {
    api_version: u16,
    app_version: &'static str,
}

#[derive(Debug, Deserialize)]
struct DiscoverRequest {
    site_url: String,
}

#[derive(Debug, Serialize)]
struct DiscoverResponse {
    normalized_site_url: String,
    discovered_feeds: Vec<DiscoveredFeedView>,
    site_title: Option<String>,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiscoveryCachePayload {
    normalized_site_url: String,
    site_title: Option<String>,
    warnings: Vec<String>,
    candidates: Vec<DiscoveryCacheCandidate>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiscoveryCacheCandidate {
    url: String,
    title: Option<String>,
    feed_type: FeedType,
    confidence: f32,
    source: String,
    score: i32,
    http_status: u16,
    etag: Option<String>,
    last_modified: Option<String>,
    duration_ms: u128,
    parsed_feed: ParsedFeed,
}

#[derive(Debug, Serialize)]
struct DiscoveredFeedView {
    url: String,
    title: Option<String>,
    feed_type: FeedType,
    confidence: f32,
    source: String,
    score: i32,
}

#[derive(Debug, Clone)]
struct CandidateHint {
    url: Url,
    fallback_type: FeedType,
    fallback_title: Option<String>,
    source: String,
    order_found: usize,
}

#[derive(Debug)]
struct DiscoveryRun {
    response: DiscoverResponse,
    cache: DiscoveryCachePayload,
}

#[derive(Debug, Deserialize)]
struct AddSubscriptionRequest {
    feed_url: String,
    title: Option<String>,
}

#[derive(Debug, Serialize)]
struct AddSubscriptionResponse {
    feed_id: String,
}

#[derive(Debug, Deserialize)]
struct UnifiedSubscribeRequest {
    input_url: String,
}

#[derive(Debug, Serialize)]
struct UnifiedSubscribeResponse {
    feed_id: String,
    resolved_feed_url: String,
    subscription_source: String,
}

#[derive(Debug, Serialize)]
struct FeedView {
    id: String,
    title: String,
    feed_url: String,
    site_url: Option<String>,
    feed_type: FeedType,
    auto_full_text: bool,
    icon_url: Option<String>,
    groups: Vec<FeedGroupView>,
}

#[derive(Debug, Serialize, Clone)]
struct FeedGroupView {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct CreateGroupRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct UpdateFeedRequest {
    title: Option<String>,
    auto_full_text: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct UpdateFeedGroupRequest {
    group_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AckNotificationEventsRequest {
    event_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ListNotificationEventsQuery {
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct ItemView {
    id: String,
    kind: String,
    source_kind: String,
    source_id: Option<String>,
    source_url: Option<String>,
    title: String,
    canonical_url: Option<String>,
    published_at: Option<String>,
    summary_preview: Option<String>,
    is_read: bool,
    is_starred: bool,
    is_saved_for_later: bool,
    is_archived: bool,
}

#[derive(Debug, Deserialize)]
struct ListItemsQuery {
    limit: Option<usize>,
    q: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListAllItemsQuery {
    limit: Option<usize>,
    q: Option<String>,
    filter: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PatchItemStateRequest {
    is_read: Option<bool>,
    is_starred: Option<bool>,
    is_saved_for_later: Option<bool>,
    is_archived: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateEntryRequest {
    id: Option<String>,
    kind: Option<EntryKind>,
    title: String,
    source_kind: Option<EntrySourceKind>,
    source_id: Option<String>,
    source_url: Option<String>,
    source_title: Option<String>,
    canonical_url: Option<String>,
    summary: Option<String>,
    content_html: Option<String>,
    content_text: Option<String>,
    raw_hash: Option<String>,
}

#[derive(Debug, Serialize)]
struct PatchItemStateResponse {
    item_id: String,
    is_read: bool,
    is_starred: bool,
    is_saved_for_later: bool,
    is_archived: bool,
}

#[derive(Debug, Deserialize)]
struct ListSyncEventsQuery {
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct SyncEventView {
    id: String,
    entity_type: String,
    entity_id: String,
    event_type: String,
    payload_json: String,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct AckSyncEventsRequest {
    event_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct AckSyncEventsResponse {
    acknowledged: usize,
}

#[derive(Debug, Deserialize)]
struct OpmlImportRequest {
    opml_xml: String,
}

#[derive(Debug, Serialize)]
struct OpmlImportResponse {
    parsed_feed_count: usize,
    unique_feed_count: usize,
    grouped_feed_count: usize,
}

#[derive(Debug, Serialize)]
struct OpmlExportResponse {
    opml_xml: String,
    feed_count: usize,
}

#[derive(Debug, Serialize)]
struct RefreshResponse {
    feed_id: String,
    status: String,
    fetched_http_status: u16,
    item_count: usize,
    notification_count: usize,
    suppressed_notification_count: usize,
}

#[derive(Debug, Serialize)]
struct DueFeedView {
    id: String,
    title: String,
    feed_url: String,
    next_scheduled_fetch_at: Option<String>,
    failure_count: i64,
    health_state: String,
}

#[derive(Debug, Deserialize)]
struct RefreshDueQuery {
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct RefreshDueResponse {
    refreshed_count: usize,
    total_item_count: usize,
    results: Vec<RefreshResponse>,
}

#[derive(Debug, Serialize)]
struct ItemDetailView {
    id: String,
    kind: String,
    source_kind: String,
    source_id: Option<String>,
    source_url: Option<String>,
    title: String,
    canonical_url: Option<String>,
    published_at: Option<String>,
    summary: Option<String>,
    content_html: Option<String>,
    content_text: Option<String>,
    is_read: bool,
    is_starred: bool,
    is_saved_for_later: bool,
    is_archived: bool,
}

#[derive(Debug, Serialize)]
struct FullTextResponse {
    item_id: String,
    content_text: String,
    source: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = resolve_runtime_config(std::env::args().skip(1))?;
    ensure_parent_dir(&config.db_path)?;

    let storage = Storage::open(&config.db_path)?;
    storage.migrate()?;

    let context = AppContext {
        db_path: config.db_path,
        user_agent: "InfoMatrix/0.1 (+https://github.com/MengyangGao/infoMatrix)".to_owned(),
        timeout_secs: 20,
        migrate_on_open: false,
    };
    spawn_background_refresh_task(context.clone());
    let app = app_router(context);

    info!("infomatrix app server listening on http://{}", config.bind_addr);
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn app_router(context: AppContext) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/meta", get(meta))
        .route("/api/v1/discover", post(discover_site))
        .route("/api/v1/subscriptions", post(add_subscription))
        .route("/api/v1/subscribe", post(subscribe_input))
        .route("/api/v1/feeds", get(list_feeds))
        .route("/api/v1/groups", get(list_groups).post(create_group))
        .route("/api/v1/feeds/{feed_id}", patch(update_feed).delete(delete_feed))
        .route("/api/v1/feeds/{feed_id}/group", patch(update_feed_group))
        .route("/api/v1/feeds/{feed_id}/items", get(list_items))
        .route("/api/v1/items", get(list_all_items))
        .route("/api/v1/items/counts", get(item_counts))
        .route("/api/v1/entries", get(list_all_items).post(create_entry))
        .route("/api/v1/entries/counts", get(item_counts))
        .route("/api/v1/items/{item_id}", get(get_item_detail))
        .route("/api/v1/entries/{item_id}", get(get_item_detail))
        .route("/api/v1/items/{item_id}/fulltext", post(fetch_item_fulltext))
        .route("/api/v1/entries/{item_id}/fulltext", post(fetch_item_fulltext))
        .route("/api/v1/refresh/{feed_id}", post(refresh_feed))
        .route("/api/v1/refresh/due", post(refresh_due_feeds))
        .route("/api/v1/feeds/due", get(list_due_feeds))
        .route(
            "/api/v1/feeds/{feed_id}/notifications",
            get(get_feed_notification_settings).put(update_feed_notification_settings),
        )
        .route(
            "/api/v1/notifications/settings",
            get(get_global_notification_settings).put(update_global_notification_settings),
        )
        .route("/api/v1/notifications/pending", get(list_pending_notification_events))
        .route("/api/v1/notifications/pending/ack", post(ack_notification_events))
        .route("/api/v1/items/{item_id}/state", patch(patch_item_state))
        .route("/api/v1/entries/{item_id}/state", patch(patch_item_state))
        .route("/api/v1/sync/events", get(list_sync_events))
        .route("/api/v1/sync/events/ack", post(ack_sync_events))
        .route("/api/v1/opml/export", get(export_opml_subscriptions))
        .route("/api/v1/opml/import", post(import_opml_subscriptions))
        .with_state(context)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn meta() -> Json<MetaResponse> {
    Json(MetaResponse { api_version: 2, app_version: env!("CARGO_PKG_VERSION") })
}

#[axum::debug_handler]
async fn discover_site(
    State(context): State<AppContext>,
    Json(payload): Json<DiscoverRequest>,
) -> Result<Json<DiscoverResponse>, ApiError> {
    let normalized_input_site_url = normalize_site_url(&payload.site_url)?;
    let run = discover_site_with_reqwest(&context, &payload.site_url).await?;
    let storage = open_storage(&context)?;
    let cache_json =
        serde_json::to_string(&run.cache).map_err(|err| ApiError::Internal(err.to_string()))?;
    let expires_at = (Utc::now() + Duration::hours(DISCOVERY_CACHE_TTL_HOURS)).to_rfc3339();
    storage.upsert_discovery_cache(
        normalized_input_site_url.as_str(),
        &run.cache.normalized_site_url,
        &cache_json,
        Some(expires_at.as_str()),
    )?;
    let response = run.response;
    Ok(Json(response))
}

async fn add_subscription(
    State(context): State<AppContext>,
    Json(payload): Json<AddSubscriptionRequest>,
) -> Result<Json<AddSubscriptionResponse>, ApiError> {
    let feed_url = Url::parse(&payload.feed_url)
        .map_err(|err| ApiError::BadRequest(format!("invalid feed url: {err}")))?;
    enforce_web_url(&feed_url, "feed url")?;
    let guessed_site_url = derive_site_url_from_feed(&feed_url);
    let title_fallback = payload.title.clone();
    let probe_start = std::time::Instant::now();
    let direct_probe = {
        let client = build_http_client(&context)?;
        match client.get(feed_url.clone()).send().await {
            Ok(response) if response.status().is_success() => {
                let probe_url = response.url().clone();
                let probe_status = response.status().as_u16();
                let probe_content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned);
                let probe_etag = response
                    .headers()
                    .get(reqwest::header::ETAG)
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned);
                let probe_last_modified = response
                    .headers()
                    .get(reqwest::header::LAST_MODIFIED)
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned);
                let probe_bytes =
                    response.bytes().await.map_err(|err| ApiError::Internal(err.to_string()))?;
                parse_feed("add_subscription", &probe_bytes, probe_content_type.as_deref())
                    .ok()
                    .and_then(|parsed| {
                        if parsed.feed_type == FeedType::Unknown {
                            None
                        } else {
                            Some((probe_url, probe_status, probe_etag, probe_last_modified, parsed))
                        }
                    })
            }
            _ => None,
        }
    };

    if let Some((probe_url, probe_status, probe_etag, probe_last_modified, parsed)) = direct_probe {
        let site_url_for_icon =
            parsed.site_url.clone().or_else(|| derive_site_url_from_feed(&probe_url));
        let mut storage = open_storage(&context)?;
        let feed_id = persist_parsed_feed_snapshot(
            &mut storage,
            &probe_url,
            title_fallback.clone(),
            site_url_for_icon.clone(),
            parsed,
            FeedSnapshotFetchMetadata {
                http_status: probe_status,
                etag: probe_etag,
                last_modified: probe_last_modified,
                duration_ms: probe_start.elapsed().as_millis(),
            },
        )?;
        if let Some(site_url) = site_url_for_icon {
            if let Some(icon_url) = feed_icon_url(Some(&site_url), &probe_url) {
                let _ = storage.set_feed_icon(&feed_id, &icon_url);
            }
            spawn_background_icon_refresh(context.clone(), feed_id.clone(), site_url);
        }
        return Ok(Json(AddSubscriptionResponse { feed_id }));
    }

    let icon_feed_url = feed_url.clone();
    let mut storage = open_storage(&context)?;
    let feed_id = storage.upsert_feed(&NewFeed {
        feed_url: feed_url.clone(),
        site_url: guessed_site_url.clone(),
        title: title_fallback,
        feed_type: FeedType::Unknown,
    })?;

    if let Some(site_url) = guessed_site_url {
        hydrate_feed_from_discovery_cache(&mut storage, &feed_id, &feed_url, &site_url)?;
        if let Some(icon_url) = feed_icon_url(Some(&site_url), &icon_feed_url) {
            let _ = storage.set_feed_icon(&feed_id, &icon_url);
        }
        spawn_background_icon_refresh(context.clone(), feed_id.clone(), site_url);
    }

    Ok(Json(AddSubscriptionResponse { feed_id }))
}

async fn subscribe_input(
    State(context): State<AppContext>,
    Json(payload): Json<UnifiedSubscribeRequest>,
) -> Result<Json<UnifiedSubscribeResponse>, ApiError> {
    let normalized = normalize_site_url(&payload.input_url)?;
    let client = build_http_client(&context)?;
    let probe_candidates = candidate_site_urls(&normalized);
    let probe_start = std::time::Instant::now();

    let discovered_seed_url =
        match fetch_first_successful_response(&client, &probe_candidates).await {
            Ok(probe) => {
                let probe_url = probe.url().clone();
                let probe_status = probe.status().as_u16();
                let probe_content_type = probe
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned);
                let probe_etag = probe
                    .headers()
                    .get(reqwest::header::ETAG)
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned);
                let probe_last_modified = probe
                    .headers()
                    .get(reqwest::header::LAST_MODIFIED)
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned);
                let probe_bytes =
                    probe.bytes().await.map_err(|err| ApiError::Internal(err.to_string()))?;
                let parsed_direct =
                    parse_feed("direct_probe", &probe_bytes, probe_content_type.as_deref()).ok();
                if let Some(parsed) = parsed_direct {
                    let site_url_for_icon =
                        parsed.site_url.clone().or_else(|| derive_site_url_from_feed(&probe_url));
                    let mut storage = open_storage(&context)?;
                    let duration_ms = probe_start.elapsed().as_millis();
                    let feed_id = persist_parsed_feed_snapshot(
                        &mut storage,
                        &probe_url,
                        parsed.title.clone(),
                        site_url_for_icon.clone(),
                        parsed,
                        FeedSnapshotFetchMetadata {
                            http_status: probe_status,
                            etag: probe_etag,
                            last_modified: probe_last_modified,
                            duration_ms,
                        },
                    )?;
                    if let Some(site_url) = site_url_for_icon {
                        if let Some(icon_url) = feed_icon_url(Some(&site_url), &probe_url) {
                            let _ = storage.set_feed_icon(&feed_id, &icon_url);
                        }
                        spawn_background_icon_refresh(context.clone(), feed_id.clone(), site_url);
                    }
                    return Ok(Json(UnifiedSubscribeResponse {
                        feed_id,
                        resolved_feed_url: probe_url.to_string(),
                        subscription_source: "direct_feed".to_owned(),
                    }));
                }
                probe_url.to_string()
            }
            Err(_) => normalized.to_string(),
        };

    let discovered = discover_site_with_client(&client, discovered_seed_url.as_ref()).await?;
    let candidate = discovered
        .response
        .discovered_feeds
        .iter()
        .max_by(|a, b| a.score.cmp(&b.score).then_with(|| a.confidence.total_cmp(&b.confidence)))
        .ok_or_else(|| {
            ApiError::BadRequest("could not discover a valid feed from input url".to_owned())
        })?;
    let feed_url = Url::parse(&candidate.url)
        .map_err(|err| ApiError::Internal(format!("discovered invalid feed url: {err}")))?;
    enforce_web_url(&feed_url, "discovered feed url")?;

    let mut storage = open_storage(&context)?;
    let site_url = Url::parse(&discovered.response.normalized_site_url).ok();
    if let Some(cache_candidate) =
        cached_discovery_candidate_for_feed_url(&discovered.cache, &feed_url)
    {
        let feed_id = persist_parsed_feed_snapshot(
            &mut storage,
            &feed_url,
            candidate.title.clone(),
            site_url.clone(),
            cache_candidate.parsed_feed.clone(),
            FeedSnapshotFetchMetadata {
                http_status: cache_candidate.http_status,
                etag: cache_candidate.etag.clone(),
                last_modified: cache_candidate.last_modified.clone(),
                duration_ms: cache_candidate.duration_ms,
            },
        )?;
        if let Some(site_url) = site_url.clone() {
            if let Some(icon_url) = feed_icon_url(Some(&site_url), &feed_url) {
                let _ = storage.set_feed_icon(&feed_id, &icon_url);
            }
            spawn_background_icon_refresh(context.clone(), feed_id.clone(), site_url);
        }
        return Ok(Json(UnifiedSubscribeResponse {
            feed_id,
            resolved_feed_url: feed_url.to_string(),
            subscription_source: "discovery".to_owned(),
        }));
    }

    let feed_id = storage.upsert_feed(&NewFeed {
        feed_url: feed_url.clone(),
        site_url: site_url.clone(),
        title: candidate.title.clone(),
        feed_type: candidate.feed_type,
    })?;
    if let Some(site_url) = site_url {
        if discover_and_cache_site_icon(&context, &mut storage, &feed_id, &site_url).await.is_none()
        {
            if let Some(icon_url) = feed_icon_url(Some(&site_url), &feed_url) {
                let _ = storage.set_feed_icon(&feed_id, &icon_url);
            }
        }
    }

    Ok(Json(UnifiedSubscribeResponse {
        feed_id,
        resolved_feed_url: feed_url.to_string(),
        subscription_source: "discovery".to_owned(),
    }))
}

async fn list_feeds(State(context): State<AppContext>) -> Result<Json<Vec<FeedView>>, ApiError> {
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

async fn list_groups(
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

async fn create_group(
    State(context): State<AppContext>,
    Json(payload): Json<CreateGroupRequest>,
) -> Result<Json<FeedGroupView>, ApiError> {
    let core = open_app_core(&context)?;
    let group = core.create_group(&payload.name)?;
    Ok(Json(FeedGroupView { id: group.id, name: group.name }))
}

async fn update_feed(
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

async fn update_feed_group(
    State(context): State<AppContext>,
    AxumPath(feed_id): AxumPath<String>,
    Json(payload): Json<UpdateFeedGroupRequest>,
) -> Result<StatusCode, ApiError> {
    let mut core = open_app_core(&context)?;
    core.set_feed_group(&feed_id, payload.group_id.as_deref())?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_feed(
    State(context): State<AppContext>,
    AxumPath(feed_id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    let core = open_app_core(&context)?;
    core.delete_feed(&feed_id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_items(
    State(context): State<AppContext>,
    AxumPath(feed_id): AxumPath<String>,
    Query(query): Query<ListItemsQuery>,
) -> Result<Json<Vec<ItemView>>, ApiError> {
    let core = open_app_core(&context)?;
    let items = core
        .search_items_for_feed(&feed_id, query.limit.unwrap_or(100), query.q.as_deref())?
        .into_iter()
        .map(item_to_view)
        .collect();
    Ok(Json(items))
}

async fn list_all_items(
    State(context): State<AppContext>,
    Query(query): Query<ListAllItemsQuery>,
) -> Result<Json<Vec<ItemView>>, ApiError> {
    let core = open_app_core(&context)?;
    let filter = parse_item_filter(query.filter.as_deref());
    let items = core
        .search_all_items(query.limit.unwrap_or(200), query.q.as_deref(), filter, None)?
        .into_iter()
        .map(item_to_view)
        .collect();
    Ok(Json(items))
}

async fn create_entry(
    State(context): State<AppContext>,
    Json(payload): Json<CreateEntryRequest>,
) -> Result<Json<ItemDetailView>, ApiError> {
    let entry_id = payload.id.unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
    let source_url = payload
        .source_url
        .as_deref()
        .map(|value| Url::parse(value).map_err(|err| ApiError::BadRequest(err.to_string())))
        .transpose()?;
    let kind = payload.kind.unwrap_or_else(|| {
        if source_url.is_some() { EntryKind::Bookmark } else { EntryKind::Note }
    });
    let source_kind = payload.source_kind.unwrap_or_else(|| {
        if source_url.is_some() { EntrySourceKind::Web } else { EntrySourceKind::Manual }
    });
    let is_web_capture = source_url.is_some() || matches!(kind, EntryKind::Bookmark);
    let client = if is_web_capture { Some(build_http_client(&context)?) } else { None };
    let fallback_source_title = source_url.as_ref().and_then(webpage_fallback_title);
    let webpage_capture = if let (Some(client), Some(url)) = (client.as_ref(), source_url.as_ref())
    {
        match capture_webpage_snapshot(client, url).await {
            Ok(capture) => Some(capture),
            Err(err) => {
                warn!("webpage capture failed for {}: {}", url, err);
                None
            }
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
        } else if let Some(url) = source_url.as_ref() {
            webpage_fallback_title(url).unwrap_or_else(|| "Untitled".to_owned())
        } else {
            "未命名随想".to_owned()
        }
    };
    let canonical_url = payload
        .canonical_url
        .as_deref()
        .map(|value| Url::parse(value).map_err(|err| ApiError::BadRequest(err.to_string())))
        .transpose()?
        .or_else(|| webpage_capture.as_ref().map(|capture| capture.final_url.clone()))
        .or_else(|| source_url.clone());
    let content_html = webpage_capture
        .as_ref()
        .and_then(|capture| capture.content_html.clone())
        .or(payload.content_html);
    let content_text = webpage_capture
        .as_ref()
        .and_then(|capture| capture.content_text.clone())
        .or(payload.content_text);
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

    let mut core = open_app_core(&context)?;
    core.create_entry(NewEntry {
        id: Some(entry_id.clone()),
        kind,
        source: EntrySource {
            source_kind,
            source_id: payload.source_id,
            source_url,
            source_title: webpage_capture
                .as_ref()
                .and_then(|capture| capture.title.clone())
                .or(payload.source_title)
                .or(fallback_source_title),
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
    })?;

    let detail = core.item_detail(&entry_id)?;
    let prepared_html = prepare_display_html(
        detail.content_html.as_deref(),
        detail.content_text.as_deref(),
        detail.summary.as_deref(),
    );
    Ok(Json(ItemDetailView {
        id: detail.id,
        kind: entry_kind_label(detail.kind).to_owned(),
        source_kind: entry_source_kind_label(detail.source_kind).to_owned(),
        source_id: detail.source_id,
        source_url: detail.source_url.map(|url| url.to_string()),
        title: detail.title,
        canonical_url: detail.canonical_url.map(|url| url.to_string()),
        published_at: detail.published_at,
        summary: detail.summary,
        content_html: prepared_html,
        content_text: detail.content_text,
        is_read: detail.is_read,
        is_starred: detail.is_starred,
        is_saved_for_later: detail.is_saved_for_later,
        is_archived: detail.is_archived,
    }))
}

async fn item_counts(State(context): State<AppContext>) -> Result<Json<ItemScopeCounts>, ApiError> {
    let core = open_app_core(&context)?;
    Ok(Json(core.item_counts()?))
}

async fn get_item_detail(
    State(context): State<AppContext>,
    AxumPath(item_id): AxumPath<String>,
) -> Result<Json<ItemDetailView>, ApiError> {
    let core = open_app_core(&context)?;
    let detail = core.item_detail(&item_id)?;
    let prepared_html = prepare_display_html(
        detail.content_html.as_deref(),
        detail.content_text.as_deref(),
        detail.summary.as_deref(),
    );
    Ok(Json(ItemDetailView {
        id: detail.id,
        kind: entry_kind_label(detail.kind).to_owned(),
        source_kind: entry_source_kind_label(detail.source_kind).to_owned(),
        source_id: detail.source_id,
        source_url: detail.source_url.map(|url| url.to_string()),
        title: detail.title,
        canonical_url: detail.canonical_url.map(|url| url.to_string()),
        published_at: detail.published_at,
        summary: detail.summary,
        content_html: prepared_html,
        content_text: detail.content_text,
        is_read: detail.is_read,
        is_starred: detail.is_starred,
        is_saved_for_later: detail.is_saved_for_later,
        is_archived: detail.is_archived,
    }))
}

async fn fetch_item_fulltext(
    State(context): State<AppContext>,
    AxumPath(item_id): AxumPath<String>,
) -> Result<Json<FullTextResponse>, ApiError> {
    let mut storage = open_storage(&context)?;
    let detail = storage.get_item_detail(&item_id)?;

    let canonical_url = detail
        .canonical_url
        .ok_or_else(|| ApiError::BadRequest("item has no canonical url for fulltext".to_owned()))?;
    enforce_web_url(&canonical_url, "article url")?;

    let client = build_http_client(&context)?;
    let response = client
        .get(canonical_url.clone())
        .send()
        .await
        .map_err(|err| ApiError::Internal(err.to_string()))?;

    if !response.status().is_success() {
        return Err(ApiError::BadRequest(format!(
            "fulltext fetch failed with HTTP {}",
            response.status().as_u16()
        )));
    }

    let html = response.text().await.map_err(|err| ApiError::Internal(err.to_string()))?;
    let extracted = extract_full_content(&html)
        .or_else(|| {
            detail.content_text.as_deref().and_then(|source_text| {
                if looks_like_markdown(source_text) {
                    let rendered_html = markdown_to_safe_html(source_text);
                    Some(ExtractedContent {
                        content_html: Some(rendered_html),
                        content_text: source_text.to_owned(),
                        source: "feed_markdown",
                    })
                } else {
                    None
                }
            })
        })
        .ok_or_else(|| {
            ApiError::BadRequest(
                "unable to extract meaningful fulltext from article html".to_owned(),
            )
        })?;
    let sanitized_html = extracted.content_html.as_deref().map(sanitize_html_fragment);

    storage.upsert_item_content(
        &item_id,
        sanitized_html.as_deref(),
        Some(&extracted.content_text),
    )?;

    Ok(Json(FullTextResponse {
        item_id,
        content_text: extracted.content_text,
        source: extracted.source.to_owned(),
    }))
}

async fn refresh_feed(
    State(context): State<AppContext>,
    AxumPath(feed_id): AxumPath<String>,
) -> Result<Json<RefreshResponse>, ApiError> {
    let mut storage = open_storage(&context)?;
    let result =
        refresh_feed_by_id(&context, &mut storage, &feed_id, RefreshReason::Manual).await?;
    Ok(Json(result))
}

async fn refresh_due_feeds(
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

async fn list_due_feeds(
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

async fn get_global_notification_settings(
    State(context): State<AppContext>,
) -> Result<Json<GlobalNotificationSettings>, ApiError> {
    let storage = open_storage(&context)?;
    Ok(Json(storage.get_global_notification_settings()?))
}

async fn update_global_notification_settings(
    State(context): State<AppContext>,
    Json(payload): Json<GlobalNotificationSettings>,
) -> Result<Json<GlobalNotificationSettings>, ApiError> {
    let storage = open_storage(&context)?;
    storage.set_global_notification_settings(&payload)?;
    Ok(Json(storage.get_global_notification_settings()?))
}

async fn get_feed_notification_settings(
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

async fn update_feed_notification_settings(
    State(context): State<AppContext>,
    AxumPath(feed_id): AxumPath<String>,
    Json(payload): Json<NotificationSettings>,
) -> Result<Json<NotificationSettings>, ApiError> {
    let mut storage = open_storage(&context)?;
    let row = storage.upsert_notification_settings(&feed_id, &payload)?;
    Ok(Json(row.settings))
}

async fn list_pending_notification_events(
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

async fn ack_notification_events(
    State(context): State<AppContext>,
    Json(payload): Json<AckNotificationEventsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let storage = open_storage(&context)?;
    let acknowledged = storage.acknowledge_notification_events(&payload.event_ids)?;
    Ok(Json(serde_json::json!({ "acknowledged": acknowledged })))
}

async fn patch_item_state(
    State(context): State<AppContext>,
    AxumPath(item_id): AxumPath<String>,
    Json(payload): Json<PatchItemStateRequest>,
) -> Result<Json<PatchItemStateResponse>, ApiError> {
    let core = open_app_core(&context)?;
    let state = core.patch_item_state(
        &item_id,
        &ItemStatePatch {
            is_read: payload.is_read,
            is_starred: payload.is_starred,
            is_saved_for_later: payload.is_saved_for_later,
            is_archived: payload.is_archived,
        },
    )?;

    Ok(Json(PatchItemStateResponse {
        item_id: state.item_id,
        is_read: state.is_read,
        is_starred: state.is_starred,
        is_saved_for_later: state.is_saved_for_later,
        is_archived: state.is_archived,
    }))
}

async fn list_sync_events(
    State(context): State<AppContext>,
    Query(query): Query<ListSyncEventsQuery>,
) -> Result<Json<Vec<SyncEventView>>, ApiError> {
    let storage = open_storage(&context)?;
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let events = storage
        .list_pending_sync_events(limit)?
        .into_iter()
        .map(|event| SyncEventView {
            id: event.id,
            entity_type: event.entity_type,
            entity_id: event.entity_id,
            event_type: event.event_type,
            payload_json: event.payload_json,
            created_at: event.created_at,
        })
        .collect();
    Ok(Json(events))
}

async fn ack_sync_events(
    State(context): State<AppContext>,
    Json(payload): Json<AckSyncEventsRequest>,
) -> Result<Json<AckSyncEventsResponse>, ApiError> {
    let storage = open_storage(&context)?;
    let acknowledged = storage.acknowledge_sync_events(&payload.event_ids)?;
    Ok(Json(AckSyncEventsResponse { acknowledged }))
}

async fn refresh_due_feeds_once(
    context: &AppContext,
    limit: usize,
    reason: RefreshReason,
) -> Result<RefreshDueResponse, ApiError> {
    let mut storage = open_storage(context)?;
    let due_feeds = storage.list_due_feeds(limit)?;
    let mut results = Vec::with_capacity(due_feeds.len());
    let mut total_item_count = 0usize;

    for feed in due_feeds {
        let result = refresh_feed_by_id(context, &mut storage, &feed.id, reason).await?;
        total_item_count = total_item_count.saturating_add(result.item_count);
        results.push(result);
    }

    Ok(RefreshDueResponse { refreshed_count: results.len(), total_item_count, results })
}

async fn refresh_feed_by_id(
    context: &AppContext,
    storage: &mut Storage,
    feed_id: &str,
    reason: RefreshReason,
) -> Result<RefreshResponse, ApiError> {
    let feed = storage.get_feed(feed_id)?;
    let feed_url = feed.feed_url.clone();
    let site_url = feed.site_url.clone();
    let started_at = Utc::now();

    let client = ReqwestFeedClient::new(context.timeout_secs, &context.user_agent)
        .map_err(|err| ApiError::Internal(err.to_string()))?;
    let service = FetchService::new(client);
    let request =
        FetchRequest { url: feed_url.clone(), etag: feed.etag, last_modified: feed.last_modified };

    let outcome = match service.fetch_and_parse(feed_id, &request).await {
        Ok(outcome) => outcome,
        Err(err) => {
            let (http_status, error_message, api_error) = match err {
                FetchError::UnexpectedStatus(status) => (
                    status,
                    format!("feed fetch returned HTTP {status}"),
                    ApiError::BadRequest(format!("feed refresh failed: HTTP {status}")),
                ),
                FetchError::Parse(message) => (
                    422,
                    format!("feed parse failed: {message}"),
                    ApiError::BadRequest(format!("feed refresh parse failed: {message}")),
                ),
                FetchError::Network(message) => (
                    599,
                    format!("feed network error: {message}"),
                    ApiError::Internal(format!("feed refresh network error: {message}")),
                ),
            };
            let _ = storage.record_fetch_result(
                feed_id,
                http_status,
                None,
                None,
                0,
                Some(&error_message),
            );
            if let Ok(updated_feed) = storage.get_feed(feed_id) {
                let finished_at = Utc::now();
                let next_attempt_at = compute_next_refresh_at(
                    finished_at,
                    reason,
                    updated_feed.health_state,
                    updated_feed.failure_count as u32,
                    feed_id,
                );
                let _ = storage.record_refresh_attempt(&RefreshAttempt {
                    feed_id: feed_id.to_owned(),
                    reason,
                    started_at,
                    finished_at,
                    http_status: Some(http_status),
                    success: false,
                    item_count: 0,
                    error_message: Some(error_message.clone()),
                    next_attempt_at: Some(next_attempt_at),
                });
            }
            return Err(api_error);
        }
    };

    match outcome {
        FetchOutcome::NotModified(response) => {
            storage.record_fetch_result(
                feed_id,
                response.status,
                response.etag.as_deref(),
                response.last_modified.as_deref(),
                response.duration_ms,
                None,
            )?;
            let updated_feed = storage.get_feed(feed_id)?;
            let finished_at = Utc::now();
            let next_attempt_at = compute_next_refresh_at(
                finished_at,
                reason,
                updated_feed.health_state,
                updated_feed.failure_count as u32,
                feed_id,
            );
            storage.record_refresh_attempt(&RefreshAttempt {
                feed_id: feed_id.to_owned(),
                reason,
                started_at,
                finished_at,
                http_status: Some(response.status),
                success: true,
                item_count: 0,
                error_message: None,
                next_attempt_at: Some(next_attempt_at),
            })?;

            Ok(RefreshResponse {
                feed_id: feed_id.to_owned(),
                status: "not_modified".to_owned(),
                fetched_http_status: response.status,
                item_count: 0,
                notification_count: 0,
                suppressed_notification_count: 0,
            })
        }
        FetchOutcome::Updated { response, parsed } => {
            storage.upsert_items(&parsed.items)?;
            let discovered_site_url = parsed.site_url.clone();
            let refreshed_feed_id = storage.upsert_feed(&NewFeed {
                feed_url: feed_url.clone(),
                site_url: parsed.site_url,
                title: parsed.title,
                feed_type: parsed.feed_type,
            })?;

            if let Some(site_url) = discovered_site_url {
                if storage.get_feed_icon(&refreshed_feed_id)?.is_none() {
                    if let Some(icon_url) = feed_icon_url(Some(&site_url), &feed_url) {
                        let _ = storage.set_feed_icon(&refreshed_feed_id, &icon_url);
                    }
                }
                spawn_background_icon_refresh(context.clone(), refreshed_feed_id, site_url);
            } else if storage.get_feed_icon(&refreshed_feed_id)?.is_none() {
                if let Some(icon_url) = feed_icon_url(site_url.as_ref(), &feed_url) {
                    let _ = storage.set_feed_icon(&refreshed_feed_id, &icon_url);
                }
            }

            storage.record_fetch_result(
                feed_id,
                response.status,
                response.etag.as_deref(),
                response.last_modified.as_deref(),
                response.duration_ms,
                None,
            )?;
            let notification_result =
                process_refresh_notifications(storage, feed_id, &parsed.items)?;
            let updated_feed = storage.get_feed(feed_id)?;
            let finished_at = Utc::now();
            let next_attempt_at = compute_next_refresh_at(
                finished_at,
                reason,
                updated_feed.health_state,
                updated_feed.failure_count as u32,
                feed_id,
            );
            storage.record_refresh_attempt(&RefreshAttempt {
                feed_id: feed_id.to_owned(),
                reason,
                started_at,
                finished_at,
                http_status: Some(response.status),
                success: true,
                item_count: parsed.items.len(),
                error_message: None,
                next_attempt_at: Some(next_attempt_at),
            })?;

            Ok(RefreshResponse {
                feed_id: feed_id.to_owned(),
                status: "updated".to_owned(),
                fetched_http_status: response.status,
                item_count: parsed.items.len(),
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
    items: &[NormalizedItem],
) -> Result<NotificationProcessingResult, ApiError> {
    let globals = storage.get_global_notification_settings()?;
    let settings = storage
        .get_notification_settings(feed_id)?
        .map(|row| row.settings)
        .unwrap_or_else(|| globals.default_feed_settings.clone());

    let now = Utc::now();
    let local_offset_minutes = Local::now().offset().local_minus_utc() / 60;
    let local_minute_of_day = notifications::policy::local_minute_of_day(now, local_offset_minutes);
    let mut last_notification_at = storage.get_latest_notification_at_for_feed(feed_id)?;

    let mut sent_count = 0usize;
    let mut suppressed_count = 0usize;
    let mut digest_drafts = Vec::new();

    for item in items {
        let candidate = notification_candidate_from_item(feed_id, item);
        let canonical_key = canonical_identity_key(item);
        let fingerprint = content_fingerprint(item);
        let duplicate = storage.get_notification_signature(&canonical_key, &fingerprint)?.is_some();
        let seen_at = now.to_rfc3339();
        let _ = storage.record_notification_signature(
            feed_id,
            &candidate.entry_id,
            &canonical_key,
            &fingerprint,
            &seen_at,
            None,
        )?;
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
                let event_id = Uuid::now_v7().to_string();
                let event = NotificationEvent {
                    id: event_id,
                    feed_id: Some(feed_id.to_owned()),
                    entry_id: Some(draft.entry_id.clone()),
                    canonical_key: draft.canonical_key.clone(),
                    content_fingerprint: draft.content_fingerprint.clone(),
                    title: draft.title.clone(),
                    body: draft.body.clone(),
                    mode: draft.mode,
                    delivery_state: NotificationDeliveryState::Pending,
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
                storage.insert_notification_event(&event, Some(&metadata_json))?;
                storage.touch_notification_signature(&canonical_key, &fingerprint, &seen_at)?;
                last_notification_at = Some(now);
                sent_count = sent_count.saturating_add(1);
            }
            NotificationDecision::QueueDigest(draft) => {
                digest_drafts.push(draft);
            }
            NotificationDecision::Suppress(reason) => {
                let event = NotificationEvent {
                    id: Uuid::now_v7().to_string(),
                    feed_id: Some(feed_id.to_owned()),
                    entry_id: Some(candidate.entry_id.clone()),
                    canonical_key: canonical_key.clone(),
                    content_fingerprint: fingerprint.clone(),
                    title: candidate.title.clone(),
                    body: candidate.body_text(),
                    mode: settings.mode,
                    delivery_state: NotificationDeliveryState::Suppressed,
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
                storage.insert_notification_event(&event, Some(&metadata_json))?;
                suppressed_count = suppressed_count.saturating_add(1);
            }
        }
    }

    if !digest_drafts.is_empty() {
        let digest_batch = build_digest_batch(feed_id.to_owned(), &digest_drafts, now)
            .ok_or_else(|| ApiError::Internal("unable to build notification digest".to_owned()))?;
        let digest = NotificationDigest {
            id: Uuid::now_v7().to_string(),
            feed_id: Some(feed_id.to_owned()),
            entry_count: digest_batch.entry_ids.len(),
            title: digest_batch.title.clone(),
            body: digest_batch.body.clone(),
            created_at: digest_batch.created_at,
            ready_at: digest_batch.ready_at,
            delivered_at: None,
        };
        let entry_ids_json = serde_json::to_string(&digest_batch.entry_ids)
            .map_err(|err| ApiError::Internal(err.to_string()))?;
        storage.insert_notification_digest(&digest, &entry_ids_json)?;
        let digest_event = NotificationEvent {
            id: Uuid::now_v7().to_string(),
            feed_id: Some(feed_id.to_owned()),
            entry_id: digest_batch.entry_ids.first().cloned(),
            canonical_key: format!("digest:{feed_id}:{}", digest.id),
            content_fingerprint: digest.id.clone(),
            title: digest.title.clone(),
            body: digest.body.clone(),
            mode: NotificationMode::Digest,
            delivery_state: NotificationDeliveryState::Pending,
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
        storage.insert_notification_event(&digest_event, Some(&metadata_json))?;
        for draft in digest_drafts {
            storage.touch_notification_signature(
                &draft.canonical_key,
                &draft.content_fingerprint,
                &now.to_rfc3339(),
            )?;
        }
        sent_count = sent_count.saturating_add(1);
    }

    Ok(NotificationProcessingResult { sent_count, suppressed_count })
}

fn notification_candidate_from_item(feed_id: &str, item: &NormalizedItem) -> NotificationCandidate {
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
    DateTime::parse_from_rfc3339(&value).ok().map(|value| value.with_timezone(&Utc))
}

async fn export_opml_subscriptions(
    State(context): State<AppContext>,
) -> Result<Json<OpmlExportResponse>, ApiError> {
    let storage = open_storage(&context)?;
    let feeds = storage.list_feeds()?;
    let mut opml_feeds = Vec::with_capacity(feeds.len());

    for feed in feeds {
        let group = storage.list_groups_for_feed(&feed.id)?.into_iter().next().map(|row| row.name);
        opml_feeds.push(OpmlFeed {
            title: feed.title,
            xml_url: feed.feed_url,
            html_url: feed.site_url,
            group,
        });
    }

    let feed_count = opml_feeds.len();
    let opml_xml = export_opml(&opml_feeds, "InfoMatrix Subscriptions")
        .map_err(|err| ApiError::Internal(err.to_string()))?;
    Ok(Json(OpmlExportResponse { opml_xml, feed_count }))
}

async fn import_opml_subscriptions(
    State(context): State<AppContext>,
    Json(payload): Json<OpmlImportRequest>,
) -> Result<Json<OpmlImportResponse>, ApiError> {
    let opml_xml = payload.opml_xml.trim();
    if opml_xml.is_empty() {
        return Err(ApiError::BadRequest("opml_xml is empty".to_owned()));
    }

    let feeds = import_opml(opml_xml).map_err(|err| ApiError::BadRequest(err.to_string()))?;
    let parsed_feed_count = feeds.len();
    let mut unique_urls = HashSet::new();
    let mut grouped_feed_count = 0usize;

    let mut storage = open_storage(&context)?;
    for feed in feeds {
        unique_urls.insert(feed.xml_url.to_string());
        let feed_id = storage.upsert_feed(&NewFeed {
            feed_url: feed.xml_url,
            site_url: feed.html_url,
            title: feed.title,
            feed_type: FeedType::Unknown,
        })?;

        if let Some(group_name) =
            feed.group.map(|name| name.trim().to_owned()).filter(|name| !name.is_empty())
        {
            let group = storage.create_group(&group_name)?;
            storage.set_feed_group(&feed_id, Some(&group.id))?;
            grouped_feed_count = grouped_feed_count.saturating_add(1);
        }
    }

    Ok(Json(OpmlImportResponse {
        parsed_feed_count,
        unique_feed_count: unique_urls.len(),
        grouped_feed_count,
    }))
}

async fn discover_site_with_reqwest(
    context: &AppContext,
    site_url: &str,
) -> Result<DiscoveryRun, ApiError> {
    let client = build_http_client(context)?;
    discover_site_with_client(&client, site_url).await
}

fn item_to_view(item: storage::ItemSummaryRow) -> ItemView {
    ItemView {
        id: item.id,
        kind: entry_kind_label(item.kind).to_owned(),
        source_kind: entry_source_kind_label(item.source_kind).to_owned(),
        source_id: item.source_id,
        source_url: item.source_url.map(|url| url.to_string()),
        title: item.title,
        canonical_url: item.canonical_url.map(|url| url.to_string()),
        published_at: item.published_at,
        summary_preview: item.summary_preview.as_deref().and_then(clean_preview_text),
        is_read: item.is_read,
        is_starred: item.is_starred,
        is_saved_for_later: item.is_saved_for_later,
        is_archived: item.is_archived,
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

fn feed_icon_url(site_url: Option<&Url>, feed_url: &Url) -> Option<String> {
    let host = site_url
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
        .or_else(|| feed_url.host_str().map(ToOwned::to_owned))?;
    Some(format!("https://{host}/favicon.ico"))
}

fn derive_site_url_from_feed(feed_url: &Url) -> Option<Url> {
    let host = feed_url.host_str()?;
    let scheme = feed_url.scheme();
    Url::parse(&format!("{scheme}://{host}")).ok()
}

async fn discover_and_cache_site_icon(
    context: &AppContext,
    storage: &mut Storage,
    feed_id: &str,
    site_url: &Url,
) -> Option<Url> {
    let client = build_http_client(context).ok()?;
    let response = client.get(site_url.clone()).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    let final_url = response.url().clone();
    let html = response.text().await.ok()?;
    let candidates = extract_icon_candidates(&final_url, &html, None);
    let cache_dir = icon_cache_dir(&context.db_path);
    let _ = std::fs::create_dir_all(&cache_dir);

    for candidate in candidates {
        let response = client.get(candidate.url.clone()).send().await.ok()?;
        if !response.status().is_success() {
            continue;
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let bytes = response.bytes().await.ok()?.to_vec();
        if bytes.len() > 512 * 1024 {
            continue;
        }
        if let Some(content_type_value) = content_type.as_deref() {
            if !content_type_value.to_ascii_lowercase().starts_with("image/") {
                continue;
            }
        }

        let sha256 = Sha256::digest(&bytes);
        let sha256_hex = hex_encode(sha256);
        let local_path = cache_dir.join(format!(
            "{}{}",
            sha256_hex,
            icon_file_extension(content_type.as_deref())
        ));
        if std::fs::write(&local_path, &bytes).is_err() {
            continue;
        }

        let _ = storage.set_feed_icon_asset(
            feed_id,
            candidate.url.as_str(),
            content_type.as_deref(),
            bytes.len() as i64,
            &sha256_hex,
            local_path.to_string_lossy().as_ref(),
        );
        return Some(candidate.url);
    }

    let fallback_url = final_url.join("/favicon.ico").ok()?;
    let response = client.get(fallback_url.clone()).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let bytes = response.bytes().await.ok()?.to_vec();
    if bytes.len() > 512 * 1024 {
        return None;
    }
    if let Some(content_type_value) = content_type.as_deref() {
        if !content_type_value.to_ascii_lowercase().starts_with("image/") {
            return None;
        }
    }

    let sha256 = Sha256::digest(&bytes);
    let sha256_hex = hex_encode(sha256);
    let local_path =
        cache_dir.join(format!("{}{}", sha256_hex, icon_file_extension(content_type.as_deref())));
    if std::fs::write(&local_path, &bytes).is_err() {
        return None;
    }

    let _ = storage.set_feed_icon_asset(
        feed_id,
        fallback_url.as_str(),
        content_type.as_deref(),
        bytes.len() as i64,
        &sha256_hex,
        local_path.to_string_lossy().as_ref(),
    );
    Some(fallback_url)
}

fn prepare_display_html(
    content_html: Option<&str>,
    content_text: Option<&str>,
    summary: Option<&str>,
) -> Option<String> {
    if let Some(content_html) = content_html.map(str::trim).filter(|value| !value.is_empty()) {
        return Some(sanitize_html_fragment(content_html));
    }

    if let Some(summary_html) = summary.map(str::trim).filter(|value| value.contains('<')) {
        return Some(sanitize_html_fragment(summary_html));
    }

    let text = content_text.map(str::trim).filter(|value| !value.is_empty())?;
    if looks_like_markdown(text) {
        return Some(markdown_to_safe_html(text));
    }
    None
}

fn clean_preview_text(value: &str) -> Option<String> {
    let plain = normalize_preview_whitespace(&strip_html_tags(value));
    if plain.is_empty() {
        return None;
    }
    Some(plain.chars().take(180).collect())
}

fn strip_html_tags(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    output
}

fn normalize_preview_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn looks_like_markdown(text: &str) -> bool {
    if text.contains("```") || text.contains("~~~") || text.contains("<!--") || text.contains("![")
    {
        return true;
    }

    let mut markdown_signals = 0usize;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if line.starts_with("    ") || line.starts_with('\t') {
            return true;
        }
        if trimmed.starts_with('#')
            || trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("+ ")
            || trimmed.starts_with("> ")
            || trimmed.starts_with("1. ")
            || trimmed.starts_with("2. ")
            || trimmed.starts_with("3. ")
        {
            markdown_signals += 1;
        }
        if trimmed.starts_with('|') && trimmed.contains('|') {
            markdown_signals += 1;
        }
    }

    markdown_signals >= 2 || text.contains("](") || text.contains("**") || text.contains("__")
}

fn markdown_to_safe_html(markdown: &str) -> String {
    let mut options = MarkdownOptions::empty();
    options.insert(MarkdownOptions::ENABLE_TABLES);
    options.insert(MarkdownOptions::ENABLE_STRIKETHROUGH);
    options.insert(MarkdownOptions::ENABLE_TASKLISTS);
    options.insert(MarkdownOptions::ENABLE_FOOTNOTES);

    let parser = MarkdownParser::new_ext(markdown, options);
    let mut rendered = String::new();
    markdown_html::push_html(&mut rendered, parser);
    sanitize_html_fragment(&rendered)
}

fn sanitize_html_fragment(html: &str) -> String {
    HtmlSanitizerBuilder::default()
        .add_tags([
            "article",
            "aside",
            "b",
            "br",
            "body",
            "div",
            "em",
            "section",
            "footer",
            "figure",
            "figcaption",
            "header",
            "h1",
            "h2",
            "h3",
            "h4",
            "h5",
            "h6",
            "hr",
            "i",
            "img",
            "iframe",
            "li",
            "main",
            "nav",
            "ol",
            "p",
            "picture",
            "pre",
            "blockquote",
            "code",
            "small",
            "summary",
            "source",
            "span",
            "strong",
            "sub",
            "sup",
            "details",
            "table",
            "tbody",
            "td",
            "tfoot",
            "th",
            "thead",
            "time",
            "tr",
            "ul",
            "video",
        ])
        .add_tag_attributes("a", ["href", "title", "target"])
        .add_tag_attributes(
            "img",
            ["src", "alt", "title", "srcset", "sizes", "width", "height", "loading", "decoding"],
        )
        .add_tag_attributes("iframe", ["src", "title", "width", "height", "allow", "loading"])
        .add_tag_attributes(
            "video",
            ["src", "poster", "controls", "autoplay", "muted", "loop", "playsinline"],
        )
        .add_tag_attributes("source", ["src", "type", "media", "srcset", "sizes"])
        .add_tag_attributes("time", ["datetime"])
        .clean(html)
        .to_string()
}

fn feed_health_state_label(value: FeedHealthState) -> &'static str {
    match value {
        FeedHealthState::Healthy => "healthy",
        FeedHealthState::Stale => "stale",
        FeedHealthState::Failing => "failing",
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

fn icon_cache_dir(db_path: &str) -> PathBuf {
    let db_path = Path::new(db_path);
    db_path
        .parent()
        .map(|parent| parent.join("icon-cache"))
        .unwrap_or_else(|| PathBuf::from("icon-cache"))
}

fn icon_file_extension(content_type: Option<&str>) -> &'static str {
    match content_type.map(|value| value.to_ascii_lowercase()).as_deref() {
        Some(value) if value.contains("png") => ".png",
        Some(value) if value.contains("jpeg") || value.contains("jpg") => ".jpg",
        Some(value) if value.contains("webp") => ".webp",
        Some(value) if value.contains("svg") => ".svg",
        Some(value) if value.contains("x-icon") || value.contains("icon") => ".ico",
        _ => ".bin",
    }
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

async fn discover_site_with_client(
    client: &Client,
    site_url: &str,
) -> Result<DiscoveryRun, ApiError> {
    let normalized_site_url = normalize_site_url(site_url)?;
    let page_candidates = candidate_site_urls(&normalized_site_url);
    let mut warnings = Vec::new();
    let (final_site_url, site_title, mut candidates) =
        match fetch_first_successful_response(client, &page_candidates).await {
            Ok(page_response) => {
                let final_site_url = page_response.url().clone();
                let html = page_response
                    .text()
                    .await
                    .map_err(|err| ApiError::Internal(err.to_string()))?;
                let (site_title, candidates) =
                    collect_html_feed_candidates(&html, &final_site_url)?;
                (final_site_url, site_title, candidates)
            }
            Err(err) => {
                warnings.push(format!("entry page fetch failed: {err}"));
                (normalized_site_url.clone(), None, Vec::new())
            }
        };

    for candidate_url in fallback_candidate_urls(&normalized_site_url) {
        if !candidates.iter().any(|candidate| candidate.url == candidate_url) {
            candidates.push(CandidateHint {
                url: candidate_url,
                fallback_type: FeedType::Unknown,
                fallback_title: None,
                source: "commonpath".to_owned(),
                order_found: candidates.len().saturating_add(1),
            });
        }
    }

    for candidate_url in fallback_candidate_urls(&final_site_url) {
        if !candidates.iter().any(|candidate| candidate.url == candidate_url) {
            candidates.push(CandidateHint {
                url: candidate_url,
                fallback_type: FeedType::Unknown,
                fallback_title: None,
                source: "commonpath".to_owned(),
                order_found: candidates.len().saturating_add(1),
            });
        }
    }

    let mut discovered_map: HashMap<String, DiscoveryCacheCandidate> = HashMap::new();
    let mut join_set = JoinSet::new();

    for candidate in candidates {
        let candidate_client = client.clone();
        join_set.spawn(async move {
            probe_discovered_feed_candidate(candidate_client, candidate).await
        });
    }

    while let Some(join_result) = join_set.join_next().await {
        match join_result {
            Ok(Ok(Some(discovered))) => {
                let key = discovered.url.clone();
                match discovered_map.get_mut(&key) {
                    Some(existing) => {
                        if discovered.score > existing.score {
                            *existing = discovered;
                        } else if existing.title.is_none() && discovered.title.is_some() {
                            existing.title = discovered.title.clone();
                        }
                    }
                    None => {
                        discovered_map.insert(key, discovered);
                    }
                }
            }
            Ok(Ok(None)) => {}
            Ok(Err(warning)) => warnings.push(warning),
            Err(err) => warnings.push(format!("candidate task failed: {err}")),
        }
    }

    let mut discovered_candidates: Vec<DiscoveryCacheCandidate> =
        discovered_map.into_values().collect();
    discovered_candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.confidence.total_cmp(&a.confidence))
            .then_with(|| a.url.cmp(&b.url))
    });

    let discovered_feeds = discovered_candidates
        .iter()
        .map(|candidate| DiscoveredFeedView {
            url: candidate.url.clone(),
            title: candidate.title.clone(),
            feed_type: candidate.feed_type,
            confidence: candidate.confidence,
            source: candidate.source.clone(),
            score: candidate.score,
        })
        .collect();

    let response = DiscoverResponse {
        normalized_site_url: final_site_url.to_string(),
        discovered_feeds,
        site_title,
        warnings,
    };
    let cache = DiscoveryCachePayload {
        normalized_site_url: response.normalized_site_url.clone(),
        site_title: response.site_title.clone(),
        warnings: response.warnings.clone(),
        candidates: discovered_candidates,
    };

    Ok(DiscoveryRun { response, cache })
}

async fn probe_discovered_feed_candidate(
    client: Client,
    candidate: CandidateHint,
) -> Result<Option<DiscoveryCacheCandidate>, String> {
    let start = std::time::Instant::now();
    let response = client
        .get(candidate.url.clone())
        .send()
        .await
        .map_err(|_| format!("candidate fetch failed: {}", candidate.url))?;
    let duration_ms = start.elapsed().as_millis();
    let response_status = response.status().as_u16();
    if response_status >= 400 {
        return Ok(None);
    }

    let final_url = response.url().clone();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let last_modified = response
        .headers()
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let bytes =
        response.bytes().await.map_err(|err| format!("candidate body read failed: {err}"))?;
    let parsed_feed = parse_feed("discovery_probe", &bytes, content_type.as_deref())
        .map_err(|err| err.to_string())?;
    let feed_type = if parsed_feed.feed_type == FeedType::Unknown {
        candidate.fallback_type
    } else {
        parsed_feed.feed_type
    };
    if feed_type == FeedType::Unknown {
        return Ok(None);
    }

    let parsed_title = parsed_feed.title.clone().or(candidate.fallback_title.clone());
    let score = feed_candidate_score(
        final_url.as_str(),
        parsed_title.as_deref(),
        &candidate.source,
        candidate.order_found,
    );
    Ok(Some(DiscoveryCacheCandidate {
        url: final_url.to_string(),
        title: parsed_title,
        feed_type,
        confidence: confidence_from_score(score),
        source: candidate.source,
        score,
        http_status: response_status,
        etag,
        last_modified,
        duration_ms,
        parsed_feed,
    }))
}

fn spawn_background_icon_refresh(context: AppContext, feed_id: String, site_url: Url) {
    tokio::task::spawn_blocking(move || {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else {
            warn!("background icon refresh skipped: failed to build runtime");
            return;
        };

        runtime.block_on(async move {
            let Ok(mut storage) = open_storage(&context) else {
                warn!("background icon refresh skipped: could not open storage");
                return;
            };
            let _ = discover_and_cache_site_icon(&context, &mut storage, &feed_id, &site_url).await;
        });
    });
}

fn rebind_items_to_feed(feed_id: &str, items: &[NormalizedItem]) -> Vec<NormalizedItem> {
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
    parsed_feed: ParsedFeed,
    fetch_metadata: FeedSnapshotFetchMetadata,
) -> Result<String, ApiError> {
    let feed_id = storage.upsert_feed(&NewFeed {
        feed_url: feed_url.clone(),
        site_url: parsed_feed.site_url.clone().or(site_url_fallback),
        title: parsed_feed.title.clone().or(title_fallback),
        feed_type: parsed_feed.feed_type,
    })?;

    let rebased_items = rebind_items_to_feed(&feed_id, &parsed_feed.items);
    storage.upsert_items(&rebased_items)?;
    storage.record_fetch_result(
        &feed_id,
        fetch_metadata.http_status,
        fetch_metadata.etag.as_deref(),
        fetch_metadata.last_modified.as_deref(),
        fetch_metadata.duration_ms,
        None,
    )?;
    Ok(feed_id)
}

fn cached_discovery_candidate_for_feed_url<'a>(
    cache: &'a DiscoveryCachePayload,
    feed_url: &Url,
) -> Option<&'a DiscoveryCacheCandidate> {
    cache.candidates.iter().find(|candidate| candidate.url == feed_url.as_str())
}

fn hydrate_feed_from_discovery_cache(
    storage: &mut Storage,
    feed_id: &str,
    feed_url: &Url,
    site_url: &Url,
) -> Result<(), ApiError> {
    let Some(cache_row) = storage.get_latest_discovery_cache(site_url.as_str())? else {
        return Ok(());
    };
    let cache: DiscoveryCachePayload = serde_json::from_str(&cache_row.result_json)
        .map_err(|err| ApiError::Internal(err.to_string()))?;
    let Some(candidate) = cached_discovery_candidate_for_feed_url(&cache, feed_url) else {
        return Ok(());
    };

    let parsed_feed = candidate.parsed_feed.clone();
    let rebased_items = rebind_items_to_feed(feed_id, &parsed_feed.items);
    storage.upsert_feed(&NewFeed {
        feed_url: feed_url.clone(),
        site_url: parsed_feed.site_url.clone().or_else(|| Some(site_url.clone())),
        title: parsed_feed.title.clone().or(candidate.title.clone()),
        feed_type: parsed_feed.feed_type,
    })?;
    storage.upsert_items(&rebased_items)?;
    storage.record_fetch_result(
        feed_id,
        candidate.http_status,
        candidate.etag.as_deref(),
        candidate.last_modified.as_deref(),
        candidate.duration_ms,
        None,
    )?;
    Ok(())
}

fn collect_html_feed_candidates(
    html: &str,
    base_url: &Url,
) -> Result<(Option<String>, Vec<CandidateHint>), ApiError> {
    let document = Html::parse_document(html);
    let site_title = Selector::parse("title")
        .ok()
        .and_then(|selector| {
            document
                .select(&selector)
                .next()
                .map(|node| node.text().collect::<String>().trim().to_owned())
        })
        .filter(|value| !value.is_empty());

    let link_selector = Selector::parse("link[rel][href]")
        .map_err(|_| ApiError::Internal("selector parse failed".to_owned()))?;
    let mut candidates: Vec<CandidateHint> = Vec::new();
    let mut order_found = 0usize;

    for node in document.select(&link_selector) {
        let rel = node.value().attr("rel").unwrap_or_default().to_ascii_lowercase();
        if !rel.split_whitespace().any(|value| value == "alternate") {
            continue;
        }
        let feed_type =
            match node.value().attr("type").unwrap_or_default().to_ascii_lowercase().as_str() {
                "application/rss+xml" => FeedType::Rss,
                "application/atom+xml" => FeedType::Atom,
                "application/feed+json" | "application/json" => FeedType::JsonFeed,
                _ => continue,
            };

        let Some(href) = node.value().attr("href") else {
            continue;
        };
        let Ok(url) = base_url.join(href) else {
            continue;
        };
        let title = node.value().attr("title").map(ToOwned::to_owned);
        order_found = order_found.saturating_add(1);
        candidates.push(CandidateHint {
            url,
            fallback_type: feed_type,
            fallback_title: title,
            source: "autodiscovery".to_owned(),
            order_found,
        });
    }

    if let Ok(anchor_selector) = Selector::parse("a[href]") {
        for node in document.select(&anchor_selector) {
            let Some(href) = node.value().attr("href") else {
                continue;
            };
            let href_lower = href.to_ascii_lowercase();
            let looks_like_feed = href_lower.contains("atom")
                || href_lower.contains("rss")
                || href_lower.contains("feed");
            if !looks_like_feed {
                continue;
            }
            let Ok(url) = base_url.join(href) else {
                continue;
            };
            order_found = order_found.saturating_add(1);
            candidates.push(CandidateHint {
                url,
                fallback_type: FeedType::Unknown,
                fallback_title: None,
                source: "heuristic_link".to_owned(),
                order_found,
            });
        }
    }

    Ok((site_title, candidates))
}

fn source_base_score(source: &str) -> i32 {
    match source {
        "manual" | "direct_feed" => 1000,
        "autodiscovery" => 50,
        "heuristic_link" => 15,
        "commonpath" => 10,
        _ => 0,
    }
}

fn feed_candidate_score(url: &str, title: Option<&str>, source: &str, order_found: usize) -> i32 {
    let mut score = source_base_score(source);
    score -= (order_found.saturating_sub(1) as i32) * 5;

    let url_lower = url.to_ascii_lowercase();
    if url_lower.contains("comments") {
        score -= 10;
    }
    if url_lower.contains("podcast") {
        score -= 10;
    }
    if url_lower.contains("rss") {
        score += 5;
    }
    if url_lower.ends_with("/index.xml") {
        score += 5;
    }
    if url_lower.ends_with("/feed/") {
        score += 5;
    }
    if url_lower.ends_with("/feed") {
        score += 4;
    }
    if url_lower.contains("json") {
        score += 3;
    }

    if let Some(title) = title.map(|value| value.to_ascii_lowercase()) {
        if title.contains("comments") {
            score -= 10;
        }
    }

    score
}

fn confidence_from_score(score: i32) -> f32 {
    ((score + 30) as f32 / 120.0).clamp(0.15, 0.99)
}

fn build_http_client(context: &AppContext) -> Result<Client, ApiError> {
    Client::builder()
        .user_agent(context.user_agent.clone())
        .timeout(std::time::Duration::from_secs(context.timeout_secs))
        .redirect(reqwest::redirect::Policy::limited(8))
        .build()
        .map_err(|err| ApiError::Internal(err.to_string()))
}

fn candidate_site_urls(base: &Url) -> Vec<Url> {
    let mut output = Vec::new();
    let mut seen = HashSet::new();

    let mut push_unique = |url: Url| {
        let key = url.to_string();
        if seen.insert(key) {
            output.push(url);
        }
    };

    push_unique(base.clone());

    if base.scheme() == "https" {
        let mut alt = base.clone();
        if alt.set_scheme("http").is_ok() {
            push_unique(alt);
        }
    } else if base.scheme() == "http" {
        let mut alt = base.clone();
        if alt.set_scheme("https").is_ok() {
            push_unique(alt);
        }
    }

    if let Some(domain) = base.domain() {
        if let Some(stripped) = domain.strip_prefix("www.") {
            let mut alt = base.clone();
            if alt.set_host(Some(stripped)).is_ok() {
                push_unique(alt);
            }
        } else {
            let mut alt = base.clone();
            if alt.set_host(Some(&format!("www.{domain}"))).is_ok() {
                push_unique(alt);
            }
        }
    }

    output
}

async fn fetch_first_successful_response(
    client: &Client,
    candidates: &[Url],
) -> Result<reqwest::Response, ApiError> {
    let mut errors = Vec::new();

    for candidate in candidates {
        match client.get(candidate.clone()).send().await {
            Ok(response) if response.status().is_success() => return Ok(response),
            Ok(response) => {
                errors.push(format!("{} (HTTP {})", candidate, response.status().as_u16()));
            }
            Err(err) => {
                errors.push(format!("{} ({})", candidate, summarize_reqwest_error(&err)));
            }
        }
    }

    let detail = if errors.is_empty() {
        "no candidate url attempted".to_owned()
    } else {
        errors.into_iter().take(3).collect::<Vec<_>>().join("; ")
    };
    Err(ApiError::BadRequest(format!(
        "could not reach website url. tried alternate urls and all failed: {detail}"
    )))
}

fn summarize_reqwest_error(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        return "request timeout".to_owned();
    }
    if err.is_connect() {
        return "connection failure".to_owned();
    }
    if err.is_builder() {
        return "request build failure".to_owned();
    }
    if err.is_request() {
        return "request failure".to_owned();
    }
    err.to_string()
}

struct ExtractedContent {
    content_html: Option<String>,
    content_text: String,
    source: &'static str,
}

#[derive(Debug, Clone)]
struct WebpageCapture {
    final_url: Url,
    title: Option<String>,
    content_html: Option<String>,
    content_text: Option<String>,
}

async fn capture_webpage_snapshot(client: &Client, url: &Url) -> Result<WebpageCapture, ApiError> {
    let response =
        client.get(url.clone()).send().await.map_err(|err| ApiError::Internal(err.to_string()))?;

    if !response.status().is_success() {
        return Err(ApiError::BadRequest(format!(
            "webpage capture failed with HTTP {}",
            response.status().as_u16()
        )));
    }

    let final_url = response.url().clone();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase());
    if let Some(content_type) = content_type.as_deref() {
        let looks_like_html = content_type.contains("html")
            || content_type.contains("xml")
            || content_type.contains("xhtml");
        if !looks_like_html {
            return Err(ApiError::BadRequest(format!(
                "webpage capture only supports html content, got: {content_type}"
            )));
        }
    }

    let html = response.text().await.map_err(|err| ApiError::Internal(err.to_string()))?;
    let document = Html::parse_document(&html);
    let title = extract_webpage_title(&document)
        .or_else(|| webpage_fallback_title(&final_url))
        .filter(|value| !value.trim().is_empty());
    let body_html =
        extract_webpage_body_html(&document).map(|value| sanitize_html_fragment(&value));
    let body_text = extract_webpage_body_text(&document);

    Ok(WebpageCapture { final_url, title, content_html: body_html, content_text: body_text })
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

fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fallback_candidate_urls(site_url: &Url) -> Vec<Url> {
    const ROOT_PATHS: &[&str] = &[
        "/feed",
        "/rss",
        "/rss.xml",
        "/feed.xml",
        "/atom.xml",
        "/index.xml",
        "/posts.atom",
        "/blog/feed",
        "/blog/rss",
        "/blog/rss.xml",
        "/blog/feed.xml",
        "/blog/atom.xml",
        "/blog/index.xml",
        "/blog/posts.atom",
    ];
    const RELATIVE_PATHS: &[&str] =
        &["feed", "rss", "rss.xml", "feed.xml", "atom.xml", "index.xml", "posts.atom"];

    let mut output = Vec::new();
    let mut relative_bases = vec![site_url.clone()];
    if !site_url.path().ends_with('/') {
        let mut with_slash = site_url.clone();
        with_slash.set_path(&format!("{}/", site_url.path()));
        relative_bases.push(with_slash);
    }

    for path in ROOT_PATHS {
        if let Ok(url) = site_url.join(path) {
            output.push(url);
        }
    }
    for base in relative_bases {
        for path in RELATIVE_PATHS {
            if let Ok(url) = base.join(path) {
                output.push(url);
            }
        }
    }
    output
}

fn normalize_site_url(input: &str) -> Result<Url, ApiError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ApiError::BadRequest("site url is empty".to_owned()));
    }
    let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    let mut url = Url::parse(&with_scheme)
        .map_err(|err| ApiError::BadRequest(format!("invalid url: {err}")))?;
    url.set_fragment(None);
    if let Some(host) = url.host_str().map(|value| value.to_ascii_lowercase()) {
        let _ = url.set_host(Some(&host));
    }
    enforce_web_url(&url, "site url")?;
    Ok(url)
}

fn enforce_web_url(url: &Url, label: &str) -> Result<(), ApiError> {
    match url.scheme() {
        "http" | "https" => Ok(()),
        other => Err(ApiError::BadRequest(format!(
            "{label} must use http or https scheme, got: {other}"
        ))),
    }
}

fn open_storage(context: &AppContext) -> Result<Storage, ApiError> {
    let storage = Storage::open(&context.db_path)?;
    if context.migrate_on_open {
        storage.migrate()?;
    }
    Ok(storage)
}

fn open_app_core(context: &AppContext) -> Result<InfoMatrixAppCore, ApiError> {
    Ok(InfoMatrixAppCore::new(open_storage(context)?))
}

fn resolve_runtime_config(
    args: impl IntoIterator<Item = String>,
) -> Result<RuntimeConfig, Box<dyn std::error::Error>> {
    let mut db_path = std::env::var("INFOMATRIX_DB_PATH").unwrap_or_else(|_| default_db_path());
    let mut bind_addr = std::env::var("INFOMATRIX_BIND_ADDR")
        .ok()
        .and_then(|value| value.parse::<SocketAddr>().ok())
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 3199)));

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--db-path" => {
                let value = iter.next().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "--db-path requires a value",
                    )
                })?;
                db_path = value;
            }
            "--bind-addr" => {
                let value = iter.next().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "--bind-addr requires a value",
                    )
                })?;
                bind_addr = value.parse::<SocketAddr>()?;
            }
            "--port" => {
                let value = iter.next().ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "--port requires a value")
                })?;
                let port = value.parse::<u16>()?;
                bind_addr = SocketAddr::from(([127, 0, 0, 1], port));
            }
            _ => {}
        }
    }

    Ok(RuntimeConfig { db_path, bind_addr })
}

fn default_db_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
    PathBuf::from(home).join(".infomatrix").join("infomatrix.db").to_string_lossy().to_string()
}

fn background_refresh_limit() -> usize {
    std::env::var("INFOMATRIX_BACKGROUND_REFRESH_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20)
        .clamp(1, 100)
}

fn spawn_background_refresh_task(context: AppContext) {
    tokio::spawn(async move {
        let limit = background_refresh_limit();
        let globals = open_storage(&context)
            .ok()
            .and_then(|storage| storage.get_global_notification_settings().ok());
        let Some(settings) = globals else {
            warn!("background refresh settings unavailable");
            return;
        };
        if !settings.background_refresh_enabled {
            info!("background refresh disabled");
            return;
        }
        let interval = std::time::Duration::from_secs(
            settings.background_refresh_interval_minutes.max(1) as u64 * 60,
        );

        if let Err(err) = refresh_due_feeds_once(&context, limit, RefreshReason::CatchUp).await {
            warn!(error = %err, "background catch-up refresh failed");
        }

        let mut ticker = tokio::time::interval(interval);
        ticker.tick().await;

        loop {
            ticker.tick().await;
            match refresh_due_feeds_once(&context, limit, RefreshReason::Periodic).await {
                Ok(result) => {
                    if result.refreshed_count > 0 {
                        info!(
                            refreshed_count = result.refreshed_count,
                            total_item_count = result.total_item_count,
                            "background refresh completed"
                        );
                    }
                }
                Err(err) => {
                    warn!(error = %err, "background refresh failed");
                }
            }
        }
    });
}

fn ensure_parent_dir(db_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = Path::new(db_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use axum::response::Html;
    use axum::routing::get;
    use chrono::Utc;
    use http_body_util::BodyExt;
    use models::{
        DigestPolicy, FeedType, GlobalNotificationSettings, ItemStatePatch, NewFeed,
        NormalizedItem, NotificationDeliveryState, NotificationEvent, NotificationMode,
        NotificationSettings, QuietHours,
    };
    use serde_json::json;
    use storage::Storage;
    use tower::ServiceExt;
    use url::Url;

    use super::{
        AppContext, app_router, candidate_site_urls, clean_preview_text, confidence_from_score,
        extract_full_content, fallback_candidate_urls, feed_candidate_score, prepare_display_html,
        resolve_runtime_config,
    };

    fn sample_item(feed_id: &str, item_id: &str) -> NormalizedItem {
        NormalizedItem {
            id: item_id.to_owned(),
            source_feed_id: feed_id.to_owned(),
            external_item_id: Some("ext-1".to_owned()),
            canonical_url: Some(Url::parse("https://example.com/post").expect("url parse")),
            title: "Sample Item".to_owned(),
            author: Some("Author".to_owned()),
            summary: Some("Summary".to_owned()),
            content_html: Some("<p>content</p>".to_owned()),
            content_text: Some("content".to_owned()),
            published_at: None,
            updated_at: None,
            raw_hash: "abc".to_owned(),
            dedup_reason: None,
            duplicate_of_item_id: None,
        }
    }

    async fn spawn_webpage_server(html: &'static str) -> Url {
        let app = axum::Router::new().route("/", get(move || async move { Html(html) }));
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind test webpage server");
        let addr = listener.local_addr().expect("listener addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test webpage");
        });
        Url::parse(&format!("http://{addr}/")).expect("test webpage url")
    }

    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");
        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn add_subscription_then_list_feeds() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");
        let feed_server = spawn_webpage_server(
            r#"<?xml version="1.0" encoding="UTF-8"?>
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
</rss>"#,
        )
        .await;
        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
        });

        let add_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/subscriptions")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"feed_url": feed_server.as_str()}).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(add_response.status(), StatusCode::OK);
        let add_body = add_response.into_body().collect().await.expect("body bytes").to_bytes();
        let add_json: serde_json::Value = serde_json::from_slice(&add_body).expect("json");
        let feed_id = add_json["feed_id"].as_str().expect("feed id").to_owned();

        let storage = Storage::open(db.to_string_lossy().as_ref()).expect("open storage");
        let feed = storage.get_feed(&feed_id).expect("load feed");
        assert_eq!(feed.title.as_deref(), Some("Local Example Feed"));
        let items = storage.list_items_for_feed(&feed_id, 10, None).expect("list items");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Hello");
    }

    #[tokio::test]
    async fn due_feeds_endpoint_returns_ready_feeds() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");

        let mut storage = Storage::open(db.to_string_lossy().as_ref()).expect("open storage");
        storage.migrate().expect("migrate");
        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: Url::parse("https://example.com/feed.xml").expect("url parse"),
                site_url: Some(Url::parse("https://example.com").expect("url parse")),
                title: Some("Example".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("upsert feed");
        storage
            .set_feed_next_scheduled_fetch_at(&feed_id, Some("2000-01-01T00:00:00Z"))
            .expect("make feed due");

        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/feeds/due?limit=10")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.expect("body").to_bytes();
        let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(payload.as_array().map(|rows| rows.len()), Some(1));
        assert_eq!(payload[0]["id"], feed_id);
        assert_eq!(payload[0]["health_state"], "healthy");
    }

    #[tokio::test]
    async fn item_counts_endpoint_returns_scope_totals() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");

        let mut storage = Storage::open(db.to_string_lossy().as_ref()).expect("open storage");
        storage.migrate().expect("migrate");
        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: Url::parse("https://example.com/feed.xml").expect("url parse"),
                site_url: Some(Url::parse("https://example.com").expect("url parse")),
                title: Some("Example".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("upsert feed");
        storage
            .upsert_items(&[
                sample_item(&feed_id, "item-1"),
                NormalizedItem {
                    id: "item-2".to_owned(),
                    source_feed_id: feed_id.clone(),
                    external_item_id: Some("ext-2".to_owned()),
                    canonical_url: Some(
                        Url::parse("https://example.com/post-2").expect("url parse"),
                    ),
                    title: "Item Two".to_owned(),
                    author: Some("Author".to_owned()),
                    summary: Some("Summary".to_owned()),
                    content_html: Some("<p>content</p>".to_owned()),
                    content_text: Some("content".to_owned()),
                    published_at: None,
                    updated_at: None,
                    raw_hash: "def".to_owned(),
                    dedup_reason: None,
                    duplicate_of_item_id: None,
                },
            ])
            .expect("upsert items");
        storage
            .patch_item_state(
                "item-1",
                &ItemStatePatch {
                    is_read: Some(true),
                    is_starred: Some(true),
                    is_saved_for_later: Some(true),
                    is_archived: Some(false),
                },
            )
            .expect("patch state");
        storage
            .patch_item_state(
                "item-2",
                &ItemStatePatch {
                    is_read: Some(true),
                    is_starred: Some(false),
                    is_saved_for_later: Some(false),
                    is_archived: Some(true),
                },
            )
            .expect("patch state");

        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/entries/counts")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.expect("body").to_bytes();
        let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(payload["all"], 1);
        assert_eq!(payload["unread"], 0);
        assert_eq!(payload["starred"], 1);
        assert_eq!(payload["later"], 1);
        assert_eq!(payload["archive"], 1);
    }

    #[tokio::test]
    async fn create_entry_endpoint_persists_bookmark_entry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");
        let webpage_url = spawn_webpage_server(
            r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta property="og:title" content="Example Site" />
    <title>Example Site</title>
  </head>
  <body>
    <main>
      <h1>Welcome</h1>
      <p>Captured content from the homepage.</p>
    </main>
  </body>
</html>"#,
        )
        .await;
        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
        });

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/entries")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "title": "",
                            "source_url": webpage_url.as_str(),
                            "canonical_url": webpage_url.as_str(),
                            "summary": "Saved for later"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(create_response.status(), StatusCode::OK);
        let create_body =
            create_response.into_body().collect().await.expect("body bytes").to_bytes();
        let payload: serde_json::Value = serde_json::from_slice(&create_body).expect("json");
        let entry_id = payload["id"].as_str().expect("entry id").to_owned();
        assert_eq!(payload["kind"], "bookmark");
        assert_eq!(payload["source_kind"], "web");
        assert_eq!(payload["source_url"], webpage_url.as_str());
        assert_eq!(payload["title"], "Example Site");
        assert!(
            payload["content_html"]
                .as_str()
                .expect("content html")
                .contains("Captured content from the homepage")
        );

        let detail_response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/api/v1/entries/{entry_id}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(detail_response.status(), StatusCode::OK);
        let detail_body =
            detail_response.into_body().collect().await.expect("body bytes").to_bytes();
        let detail: serde_json::Value = serde_json::from_slice(&detail_body).expect("json");
        assert_eq!(detail["id"], entry_id);
        assert_eq!(detail["kind"], "bookmark");
        assert_eq!(detail["source_kind"], "web");
        assert_eq!(detail["summary"], "Saved for later");
        assert_eq!(detail["title"], "Example Site");
        assert!(detail["content_html"].as_str().expect("detail content html").contains("Welcome"));
    }

    #[tokio::test]
    async fn create_entry_endpoint_persists_note_entry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");
        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
        });

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/entries")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "title": "Memos",
                            "kind": "note",
                            "source_kind": "manual",
                            "summary": "memos content here",
                            "content_text": "memos content here"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(create_response.status(), StatusCode::OK);
        let create_body =
            create_response.into_body().collect().await.expect("body bytes").to_bytes();
        let payload: serde_json::Value = serde_json::from_slice(&create_body).expect("json");
        let entry_id = payload["id"].as_str().expect("entry id").to_owned();
        assert_eq!(payload["kind"], "note");
        assert_eq!(payload["source_kind"], "manual");
        assert_eq!(payload["title"], "Memos");

        let counts_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/entries/counts")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(counts_response.status(), StatusCode::OK);
        let counts_body =
            counts_response.into_body().collect().await.expect("body bytes").to_bytes();
        let counts: serde_json::Value = serde_json::from_slice(&counts_body).expect("json");
        assert_eq!(counts["notes"], 1);
        assert_eq!(counts["all"], 1);

        let detail_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/api/v1/entries/{entry_id}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(detail_response.status(), StatusCode::OK);
        let detail_body =
            detail_response.into_body().collect().await.expect("body bytes").to_bytes();
        let detail: serde_json::Value = serde_json::from_slice(&detail_body).expect("json");
        assert_eq!(detail["kind"], "note");
        assert_eq!(detail["source_kind"], "manual");
        assert_eq!(detail["summary"], "memos content here");
    }

    #[tokio::test]
    async fn delete_feed_endpoint_removes_subscription() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");
        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
        });

        let add_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/subscriptions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"feed_url":"https://example.com/feed.xml"}).to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(add_response.status(), StatusCode::OK);

        let add_body = add_response.into_body().collect().await.expect("body bytes").to_bytes();
        let payload: serde_json::Value = serde_json::from_slice(&add_body).expect("json");
        let feed_id =
            payload.get("feed_id").and_then(|value| value.as_str()).expect("feed_id").to_owned();

        let delete_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::DELETE)
                    .uri(format!("/api/v1/feeds/{feed_id}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let list_response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/feeds")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(list_response.status(), StatusCode::OK);
        let body = list_response.into_body().collect().await.expect("body bytes").to_bytes();
        let text = String::from_utf8(body.to_vec()).expect("utf8 body");
        assert_eq!(text, "[]");
    }

    #[tokio::test]
    async fn notification_settings_and_pending_queue_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");

        let storage = Storage::open(db.to_string_lossy().as_ref()).expect("open storage");
        storage.migrate().expect("migrate");
        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: Url::parse("https://example.com/feed.xml").expect("url parse"),
                site_url: Some(Url::parse("https://example.com").expect("url parse")),
                title: Some("Example".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("upsert feed");

        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
        });

        let global_settings = GlobalNotificationSettings {
            background_refresh_enabled: false,
            background_refresh_interval_minutes: 30,
            digest_policy: DigestPolicy { enabled: true, interval_minutes: 90, max_items: 12 },
            default_feed_settings: NotificationSettings {
                enabled: true,
                mode: NotificationMode::Digest,
                digest_policy: DigestPolicy { enabled: true, interval_minutes: 60, max_items: 20 },
                quiet_hours: QuietHours {
                    enabled: true,
                    start_minute: 22 * 60,
                    end_minute: 7 * 60,
                },
                minimum_interval_minutes: 45,
                high_priority: false,
                keyword_include: vec!["rust".to_owned()],
                keyword_exclude: vec!["ads".to_owned()],
            },
        };

        let put_global = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/v1/notifications/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(json!(global_settings).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(put_global.status(), StatusCode::OK);

        let get_global = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/notifications/settings")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(get_global.status(), StatusCode::OK);
        let body = get_global.into_body().collect().await.expect("body bytes").to_bytes();
        let loaded: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(loaded["background_refresh_enabled"], false);
        assert_eq!(loaded["default_feed_settings"]["mode"], "digest");

        let feed_settings = NotificationSettings {
            enabled: true,
            mode: NotificationMode::Immediate,
            digest_policy: DigestPolicy { enabled: false, interval_minutes: 60, max_items: 20 },
            quiet_hours: QuietHours { enabled: false, start_minute: 22 * 60, end_minute: 7 * 60 },
            minimum_interval_minutes: 20,
            high_priority: true,
            keyword_include: vec!["rust".to_owned()],
            keyword_exclude: vec![],
        };
        let put_feed = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri(format!("/api/v1/feeds/{feed_id}/notifications"))
                    .header("content-type", "application/json")
                    .body(Body::from(json!(feed_settings).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(put_feed.status(), StatusCode::OK);

        let get_feed = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/api/v1/feeds/{feed_id}/notifications"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(get_feed.status(), StatusCode::OK);

        storage
            .insert_notification_event(
                &NotificationEvent {
                    id: "event-1".to_owned(),
                    feed_id: Some(feed_id.clone()),
                    entry_id: Some("entry-1".to_owned()),
                    canonical_key: "canonical-1".to_owned(),
                    content_fingerprint: "fingerprint-1".to_owned(),
                    title: "Test".to_owned(),
                    body: "Body".to_owned(),
                    mode: NotificationMode::Immediate,
                    delivery_state: NotificationDeliveryState::Pending,
                    reason: "new_item".to_owned(),
                    digest_id: None,
                    created_at: Utc::now(),
                    ready_at: Some(Utc::now()),
                    delivered_at: None,
                    suppressed_at: None,
                },
                Some("{\"source\":\"test\"}"),
            )
            .expect("seed event");

        let pending = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/notifications/pending?limit=10")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(pending.status(), StatusCode::OK);
        let pending_body = pending.into_body().collect().await.expect("body bytes").to_bytes();
        let pending_json: serde_json::Value = serde_json::from_slice(&pending_body).expect("json");
        assert_eq!(pending_json.as_array().map(|items| items.len()), Some(1));

        let ack = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/notifications/pending/ack")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"event_ids":["event-1"]}).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(ack.status(), StatusCode::OK);

        let pending_after = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/notifications/pending?limit=10")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let body = pending_after.into_body().collect().await.expect("body bytes").to_bytes();
        let pending_after_json: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert!(pending_after_json.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn patch_item_state_emits_sync_event_then_ack_clears_queue() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");

        let mut storage = Storage::open(db.to_string_lossy().as_ref()).expect("open storage");
        storage.migrate().expect("migrate");
        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: Url::parse("https://example.com/feed.xml").expect("url parse"),
                site_url: Some(Url::parse("https://example.com").expect("url parse")),
                title: Some("Example".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("upsert feed");
        storage.upsert_items(&[sample_item(&feed_id, "item-1")]).expect("upsert items");

        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
        });

        let patch_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PATCH)
                    .uri("/api/v1/items/item-1/state")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"is_read": true}).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(patch_response.status(), StatusCode::OK);

        let list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/sync/events?limit=10")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(list_response.status(), StatusCode::OK);
        let list_body = list_response.into_body().collect().await.expect("body bytes").to_bytes();
        let events: serde_json::Value = serde_json::from_slice(&list_body).expect("events json");
        let event_id = events
            .get(0)
            .and_then(|event| event.get("id"))
            .and_then(|id| id.as_str())
            .expect("event id")
            .to_owned();
        assert_eq!(events[0]["entity_type"], "item_state");
        assert_eq!(events[0]["entity_id"], "item-1");
        assert_eq!(events[0]["event_type"], "updated");

        let ack_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/sync/events/ack")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"event_ids":[event_id]}).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(ack_response.status(), StatusCode::OK);
        let ack_body = ack_response.into_body().collect().await.expect("body bytes").to_bytes();
        let ack_payload: serde_json::Value = serde_json::from_slice(&ack_body).expect("ack json");
        assert_eq!(ack_payload["acknowledged"], 1);

        let list_after_ack = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/sync/events?limit=10")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(list_after_ack.status(), StatusCode::OK);
        let list_after_ack_body =
            list_after_ack.into_body().collect().await.expect("body bytes").to_bytes();
        let events_after_ack: serde_json::Value =
            serde_json::from_slice(&list_after_ack_body).expect("events json");
        assert_eq!(events_after_ack, json!([]));
    }

    #[tokio::test]
    async fn import_opml_endpoint_creates_feed_and_group() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");
        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
        });

        let opml_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>Subscriptions</title></head>
  <body>
    <outline text="Tech" title="Tech">
      <outline text="Example" title="Example" type="rss" xmlUrl="https://example.com/feed.xml" htmlUrl="https://example.com" />
    </outline>
  </body>
</opml>"#;

        let import_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/opml/import")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "opml_xml": opml_xml }).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(import_response.status(), StatusCode::OK);
        let import_body = import_response.into_body().collect().await.expect("body").to_bytes();
        let import_payload: serde_json::Value = serde_json::from_slice(&import_body).expect("json");
        assert_eq!(import_payload["parsed_feed_count"], 1);
        assert_eq!(import_payload["unique_feed_count"], 1);
        assert_eq!(import_payload["grouped_feed_count"], 1);

        let feeds_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/feeds")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(feeds_response.status(), StatusCode::OK);
        let feeds_body = feeds_response.into_body().collect().await.expect("body").to_bytes();
        let feeds: serde_json::Value = serde_json::from_slice(&feeds_body).expect("json");
        assert_eq!(feeds.as_array().map(|rows| rows.len()), Some(1));
        assert_eq!(feeds[0]["feed_url"], "https://example.com/feed.xml");

        let groups_response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/groups")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(groups_response.status(), StatusCode::OK);
        let groups_body = groups_response.into_body().collect().await.expect("body").to_bytes();
        let groups: serde_json::Value = serde_json::from_slice(&groups_body).expect("json");
        assert!(groups.as_array().is_some_and(|rows| rows.iter().any(|row| row["name"] == "Tech")));
    }

    #[tokio::test]
    async fn export_opml_endpoint_returns_subscription_xml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");

        let mut storage = Storage::open(db.to_string_lossy().as_ref()).expect("open storage");
        storage.migrate().expect("migrate");
        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: Url::parse("https://example.com/feed.xml").expect("url parse"),
                site_url: Some(Url::parse("https://example.com").expect("url parse")),
                title: Some("Example".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("upsert feed");
        let group = storage.create_group("Tech").expect("create group");
        storage.set_feed_group(&feed_id, Some(&group.id)).expect("assign group");

        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/opml/export")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.expect("body").to_bytes();
        let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
        let xml = payload["opml_xml"].as_str().expect("opml xml");
        assert_eq!(payload["feed_count"], 1);
        assert!(xml.contains("xmlUrl=\"https://example.com/feed.xml\""));
        assert!(xml.contains("title=\"Tech\"") || xml.contains("text=\"Tech\""));
    }

    #[test]
    fn fallback_candidates_cover_domain_blog_paths() {
        let site_url = url::Url::parse("https://example.com").expect("site url");
        let candidates = fallback_candidate_urls(&site_url);
        let urls: Vec<String> = candidates.into_iter().map(|url| url.to_string()).collect();

        assert!(urls.iter().any(|url| url == "https://example.com/blog/atom.xml"));
        assert!(urls.iter().any(|url| url == "https://example.com/posts.atom"));
    }

    #[test]
    fn runtime_config_accepts_cli_overrides() {
        let config = resolve_runtime_config(vec![
            "--port".to_owned(),
            "4321".to_owned(),
            "--db-path".to_owned(),
            "/tmp/infomatrix-cli-test.db".to_owned(),
        ])
        .expect("resolve config");

        assert_eq!(config.bind_addr.port(), 4321);
        assert_eq!(config.db_path, "/tmp/infomatrix-cli-test.db");
    }

    #[test]
    fn markdown_content_is_rendered_to_html() {
        let html = prepare_display_html(
            None,
            Some("```rust\nfn main() {}\n```\n\n    let answer = 42\n"),
            None,
        )
        .expect("rendered html");
        assert!(html.contains("<pre><code"));
        assert!(html.contains("let answer = 42"));
    }

    #[test]
    fn summary_html_can_be_promoted_to_display_html() {
        let html = prepare_display_html(None, None, Some("<p>Hello <strong>RSS</strong></p>"))
            .expect("summary html");
        assert!(html.contains("<strong>RSS</strong>"));
    }

    #[test]
    fn extracted_full_content_keeps_code_blocks_once() {
        let html = r#"
            <html>
              <body>
                <main>
                  <article class="article-prose">
                    <p>Hello world</p>
                    <pre><code>let answer = 42</code></pre>
                    <p>Goodbye</p>
                  </article>
                </main>
              </body>
            </html>
        "#;

        let extracted = extract_full_content(html).expect("full content");
        assert_eq!(extracted.source, "web_extract");
        assert_eq!(extracted.content_text.matches("let answer = 42").count(), 1);
        assert!(extracted.content_text.contains("Hello world"));
        assert!(extracted.content_html.as_deref().unwrap_or_default().contains("<pre><code>"));
    }

    #[test]
    fn preview_strips_html_tags() {
        let preview = clean_preview_text("<p>Hello <strong>world</strong></p>").expect("preview");
        assert_eq!(preview, "Hello world");
    }

    #[test]
    fn candidate_urls_cover_scheme_and_www_variants() {
        let base = url::Url::parse("https://example.com/blog").expect("url");
        let candidates = candidate_site_urls(&base);
        let values: Vec<String> = candidates.into_iter().map(|value| value.to_string()).collect();
        assert!(values.iter().any(|value| value == "https://example.com/blog"));
        assert!(values.iter().any(|value| value == "http://example.com/blog"));
        assert!(values.iter().any(|value| value == "https://www.example.com/blog"));
    }

    #[test]
    fn scoring_prefers_primary_feed_over_comments_feed() {
        let main_score =
            feed_candidate_score("https://example.com/feed", Some("Main Feed"), "autodiscovery", 1);
        let comments_score = feed_candidate_score(
            "https://example.com/comments/feed",
            Some("Comments Feed"),
            "autodiscovery",
            1,
        );
        assert!(main_score > comments_score);
    }

    #[test]
    fn confidence_from_score_is_clamped() {
        assert!((0.15..=0.99).contains(&confidence_from_score(-200)));
        assert!((0.15..=0.99).contains(&confidence_from_score(500)));
    }
}
