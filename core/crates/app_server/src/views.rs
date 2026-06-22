use serde::{Deserialize, Serialize};

use models::{EntryKind, EntrySourceKind, FeedType};

#[derive(Debug, Serialize)]
pub(crate) struct HealthResponse {
    pub(crate) status: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct MetaResponse {
    pub(crate) api_version: u16,
    pub(crate) app_version: &'static str,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DiscoverRequest {
    pub(crate) site_url: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct DiscoverResponse {
    pub(crate) normalized_site_url: String,
    pub(crate) discovered_feeds: Vec<DiscoveredFeedView>,
    pub(crate) site_title: Option<String>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DiscoveredFeedView {
    pub(crate) url: String,
    pub(crate) title: Option<String>,
    pub(crate) feed_type: FeedType,
    pub(crate) confidence: f32,
    pub(crate) source: String,
    pub(crate) score: i32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddSubscriptionRequest {
    pub(crate) feed_url: String,
    pub(crate) title: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AddSubscriptionResponse {
    pub(crate) feed_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UnifiedSubscribeRequest {
    pub(crate) input_url: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct UnifiedSubscribeResponse {
    pub(crate) feed_id: String,
    pub(crate) resolved_feed_url: String,
    pub(crate) subscription_source: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct FeedView {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) feed_url: String,
    pub(crate) site_url: Option<String>,
    pub(crate) feed_type: FeedType,
    pub(crate) auto_full_text: bool,
    pub(crate) icon_url: Option<String>,
    pub(crate) groups: Vec<FeedGroupView>,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct FeedGroupView {
    pub(crate) id: String,
    pub(crate) name: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateGroupRequest {
    pub(crate) name: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateFeedRequest {
    pub(crate) title: Option<String>,
    pub(crate) auto_full_text: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateFeedGroupRequest {
    pub(crate) group_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AckNotificationEventsRequest {
    pub(crate) event_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListNotificationEventsQuery {
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ItemView {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) source_kind: String,
    pub(crate) source_id: Option<String>,
    pub(crate) source_url: Option<String>,
    pub(crate) title: String,
    pub(crate) canonical_url: Option<String>,
    pub(crate) published_at: Option<String>,
    pub(crate) summary_preview: Option<String>,
    pub(crate) is_read: bool,
    pub(crate) is_starred: bool,
    pub(crate) is_saved_for_later: bool,
    pub(crate) is_archived: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListItemsQuery {
    pub(crate) limit: Option<usize>,
    pub(crate) q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListAllItemsQuery {
    pub(crate) limit: Option<usize>,
    pub(crate) q: Option<String>,
    pub(crate) filter: Option<String>,
    pub(crate) kind: Option<EntryKind>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PatchItemStateRequest {
    pub(crate) is_read: Option<bool>,
    pub(crate) is_starred: Option<bool>,
    pub(crate) is_saved_for_later: Option<bool>,
    pub(crate) is_archived: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateEntryRequest {
    pub(crate) id: Option<String>,
    pub(crate) kind: Option<EntryKind>,
    pub(crate) title: String,
    pub(crate) source_kind: Option<EntrySourceKind>,
    pub(crate) source_id: Option<String>,
    pub(crate) source_url: Option<String>,
    pub(crate) source_title: Option<String>,
    pub(crate) canonical_url: Option<String>,
    pub(crate) summary: Option<String>,
    pub(crate) content_html: Option<String>,
    pub(crate) content_text: Option<String>,
    pub(crate) raw_hash: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PatchItemStateResponse {
    pub(crate) item_id: String,
    pub(crate) is_read: bool,
    pub(crate) is_starred: bool,
    pub(crate) is_saved_for_later: bool,
    pub(crate) is_archived: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListSyncEventsQuery {
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SyncEventView {
    pub(crate) id: String,
    pub(crate) entity_type: String,
    pub(crate) entity_id: String,
    pub(crate) event_type: String,
    pub(crate) payload_json: String,
    pub(crate) created_at: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AckSyncEventsRequest {
    pub(crate) event_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AckSyncEventsResponse {
    pub(crate) acknowledged: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SyncEventInput {
    pub(crate) id: String,
    pub(crate) entity_type: String,
    pub(crate) entity_id: String,
    pub(crate) event_type: String,
    pub(crate) payload_json: String,
    pub(crate) created_at: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ApplySyncEventsRequest {
    pub(crate) events: Vec<SyncEventInput>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ApplySyncEventsResponse {
    pub(crate) applied: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpmlImportRequest {
    pub(crate) opml_xml: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct OpmlImportResponse {
    pub(crate) parsed_feed_count: usize,
    pub(crate) unique_feed_count: usize,
    pub(crate) grouped_feed_count: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct OpmlExportResponse {
    pub(crate) opml_xml: String,
    pub(crate) feed_count: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct RefreshResponse {
    pub(crate) feed_id: String,
    pub(crate) status: String,
    pub(crate) fetched_http_status: u16,
    pub(crate) item_count: usize,
    pub(crate) notification_count: usize,
    pub(crate) suppressed_notification_count: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct DueFeedView {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) feed_url: String,
    pub(crate) next_scheduled_fetch_at: Option<String>,
    pub(crate) failure_count: i64,
    pub(crate) health_state: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RefreshDueQuery {
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RefreshDueResponse {
    pub(crate) refreshed_count: usize,
    pub(crate) total_item_count: usize,
    pub(crate) results: Vec<RefreshResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ItemDetailView {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) source_kind: String,
    pub(crate) source_id: Option<String>,
    pub(crate) source_url: Option<String>,
    pub(crate) title: String,
    pub(crate) canonical_url: Option<String>,
    pub(crate) published_at: Option<String>,
    pub(crate) summary: Option<String>,
    pub(crate) content_html: Option<String>,
    pub(crate) content_text: Option<String>,
    pub(crate) is_read: bool,
    pub(crate) is_starred: bool,
    pub(crate) is_saved_for_later: bool,
    pub(crate) is_archived: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct FullTextResponse {
    pub(crate) item_id: String,
    pub(crate) content_text: String,
    pub(crate) source: String,
}
