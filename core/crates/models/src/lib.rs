//! Shared domain models for InfoMatrix core crates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

/// Supported feed types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedType {
    /// RSS 2.0 feed.
    Rss,
    /// Atom feed.
    Atom,
    /// JSON Feed.
    JsonFeed,
    /// Type could not be confidently inferred.
    Unknown,
}

/// Discovery source classification for a feed candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscoverySource {
    /// Candidate came from `<link rel="alternate">`.
    Autodiscovery,
    /// Candidate came from fallback probing.
    CommonPath,
    /// Candidate was directly entered by user.
    Manual,
}

/// Structured feed candidate returned by discovery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveredFeed {
    /// Absolute feed URL.
    pub url: Url,
    /// Optional title from feed metadata or link title.
    pub title: Option<String>,
    /// Inferred or validated feed type.
    pub feed_type: FeedType,
    /// Confidence score in range [0.0, 1.0].
    pub confidence: f32,
    /// Origin of candidate.
    pub source: DiscoverySource,
}

/// Result of a full site discovery run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryResult {
    /// Normalized site URL used as discovery entry.
    pub normalized_site_url: Url,
    /// Validated feed candidates.
    pub discovered_feeds: Vec<DiscoveredFeed>,
    /// Optional HTML site title.
    pub site_title: Option<String>,
    /// Icon URL candidates discovered on site HTML.
    pub site_icon_candidates: Vec<Url>,
    /// Non-fatal diagnostics.
    pub warnings: Vec<String>,
}

/// Normalized feed metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedFeed {
    /// Stable internal feed ID.
    pub id: String,
    /// Feed URL.
    pub feed_url: Url,
    /// Site URL if known.
    pub site_url: Option<Url>,
    /// Human-readable title.
    pub title: Option<String>,
    /// Feed type.
    pub feed_type: FeedType,
    /// Description/subtitle.
    pub description: Option<String>,
}

/// Dedup reasoning metadata for an item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DedupReason {
    /// Dedup by stable GUID / ID.
    ExternalId,
    /// Dedup by canonical URL.
    CanonicalUrl,
    /// Dedup by normalized URL + publish timestamp.
    UrlAndPublishedAt,
    /// Fallback dedup by title/date hash.
    TitleDateFallback,
}

/// A normalized feed item for persistence and UI rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedItem {
    /// Stable internal item ID.
    pub id: String,
    /// Internal source feed ID.
    pub source_feed_id: String,
    /// Feed-provided item ID/guid.
    pub external_item_id: Option<String>,
    /// Canonical item URL.
    pub canonical_url: Option<Url>,
    /// Item title.
    pub title: String,
    /// Optional author string.
    pub author: Option<String>,
    /// Summary/excerpt.
    pub summary: Option<String>,
    /// Sanitized content HTML.
    pub content_html: Option<String>,
    /// Plaintext content representation.
    pub content_text: Option<String>,
    /// Published timestamp.
    pub published_at: Option<DateTime<Utc>>,
    /// Updated timestamp.
    pub updated_at: Option<DateTime<Utc>>,
    /// Raw payload hash.
    pub raw_hash: String,
    /// Dedup reason if deduplication matched.
    pub dedup_reason: Option<DedupReason>,
    /// Existing item ID if this was deduped.
    pub duplicate_of_item_id: Option<String>,
}

/// Unified content kinds stored in the inbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// RSS/Atom/JSON Feed article.
    Article,
    /// Web bookmark / read-it-later capture.
    Bookmark,
    /// Freeform note.
    Note,
    /// Saved quote or highlight.
    Quote,
}

/// Source categories for unified inbox entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntrySourceKind {
    /// Entry originated from a feed subscription.
    Feed,
    /// Entry was captured from the web.
    Web,
    /// Entry was entered manually.
    Manual,
    /// Entry was imported from another system.
    Import,
    /// Entry was produced by sync.
    Sync,
}

/// Source metadata attached to an entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntrySource {
    /// Source category.
    pub source_kind: EntrySourceKind,
    /// Stable source identifier when available.
    pub source_id: Option<String>,
    /// Source URL when the entry came from a URL.
    pub source_url: Option<Url>,
    /// Human-readable source title.
    pub source_title: Option<String>,
}

/// Unified inbox entry stored in SQLite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Stable internal entry ID.
    pub id: String,
    /// Unified content kind.
    pub kind: EntryKind,
    /// Source metadata.
    pub source: EntrySource,
    /// Feed-provided item ID/guid when applicable.
    pub external_item_id: Option<String>,
    /// Canonical item URL.
    pub canonical_url: Option<Url>,
    /// Entry title.
    pub title: String,
    /// Optional author string.
    pub author: Option<String>,
    /// Summary/excerpt.
    pub summary: Option<String>,
    /// Sanitized content HTML.
    pub content_html: Option<String>,
    /// Plaintext content representation.
    pub content_text: Option<String>,
    /// Published timestamp.
    pub published_at: Option<DateTime<Utc>>,
    /// Updated timestamp.
    pub updated_at: Option<DateTime<Utc>>,
    /// Raw payload hash.
    pub raw_hash: String,
    /// Dedup reason if deduplication matched.
    pub dedup_reason: Option<DedupReason>,
    /// Existing entry ID if this was deduped.
    pub duplicate_of_entry_id: Option<String>,
}

/// Mutable read/star/later/archive state for an entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntryState {
    /// Entry ID.
    pub item_id: String,
    /// Read flag.
    pub is_read: bool,
    /// Starred flag.
    pub is_starred: bool,
    /// Read-it-later flag.
    pub is_saved_for_later: bool,
    /// Archived flag.
    pub is_archived: bool,
    /// Timestamp when marked read.
    pub read_at: Option<DateTime<Utc>>,
    /// Timestamp when starred.
    pub starred_at: Option<DateTime<Utc>>,
    /// Timestamp when saved for later.
    pub saved_for_later_at: Option<DateTime<Utc>>,
}

/// Alias for legacy RSS-specific code paths.
pub type ItemState = EntryState;

/// New feed input model for storage APIs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewFeed {
    /// Feed URL.
    pub feed_url: Url,
    /// Optional site URL.
    pub site_url: Option<Url>,
    /// Optional title.
    pub title: Option<String>,
    /// Feed type.
    pub feed_type: FeedType,
}

/// Item-state mutation patch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EntryStatePatch {
    /// Optional read flag mutation.
    pub is_read: Option<bool>,
    /// Optional starred flag mutation.
    pub is_starred: Option<bool>,
    /// Optional saved-for-later flag mutation.
    pub is_saved_for_later: Option<bool>,
    /// Optional archived flag mutation.
    pub is_archived: Option<bool>,
}

/// Alias for legacy RSS-specific code paths.
pub type ItemStatePatch = EntryStatePatch;

/// New unified entry input model for storage APIs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewEntry {
    /// Stable entry ID when the caller already has one.
    pub id: Option<String>,
    /// Unified content kind.
    pub kind: EntryKind,
    /// Source metadata.
    pub source: EntrySource,
    /// Feed-provided item ID/guid when applicable.
    pub external_item_id: Option<String>,
    /// Canonical item URL.
    pub canonical_url: Option<Url>,
    /// Entry title.
    pub title: String,
    /// Optional author string.
    pub author: Option<String>,
    /// Summary/excerpt.
    pub summary: Option<String>,
    /// Sanitized content HTML.
    pub content_html: Option<String>,
    /// Plaintext content representation.
    pub content_text: Option<String>,
    /// Published timestamp.
    pub published_at: Option<DateTime<Utc>>,
    /// Updated timestamp.
    pub updated_at: Option<DateTime<Utc>>,
    /// Raw payload hash.
    pub raw_hash: String,
    /// Dedup reason if deduplication matched.
    pub dedup_reason: Option<DedupReason>,
    /// Existing entry ID if this was deduped.
    pub duplicate_of_entry_id: Option<String>,
}

/// Feed health classification used by fetch scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedHealthState {
    /// Feed is healthy.
    Healthy,
    /// Feed appears stale or low-activity.
    Stale,
    /// Feed repeatedly fails.
    Failing,
}

/// Aggregated counts for the primary item scopes exposed by the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ItemScopeCounts {
    /// Non-archived items.
    pub all: usize,
    /// Unread and non-archived items.
    pub unread: usize,
    /// Starred and non-archived items.
    pub starred: usize,
    /// Saved-for-later and non-archived items.
    pub later: usize,
    /// Note entries that are not archived.
    pub notes: usize,
    /// Archived items.
    pub archive: usize,
}

/// Delivery mode for notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationMode {
    /// Notify as soon as a new item is detected.
    Immediate,
    /// Hold items for digest batching.
    Digest,
}

/// Quiet-hours window represented in minutes after midnight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuietHours {
    /// Whether quiet hours are active.
    pub enabled: bool,
    /// Start minute after midnight in local time.
    pub start_minute: u16,
    /// End minute after midnight in local time.
    pub end_minute: u16,
}

impl Default for QuietHours {
    fn default() -> Self {
        Self { enabled: false, start_minute: 22 * 60, end_minute: 7 * 60 }
    }
}

/// Per-feed notification preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DigestPolicy {
    /// Whether digest batching is enabled.
    pub enabled: bool,
    /// Digest cadence in minutes when digest mode is enabled.
    pub interval_minutes: u32,
    /// Maximum number of items that should be grouped into one digest.
    pub max_items: u32,
}

impl Default for DigestPolicy {
    fn default() -> Self {
        Self { enabled: false, interval_minutes: 60, max_items: 20 }
    }
}

/// Per-feed notification preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationSettings {
    /// Whether notifications are enabled.
    pub enabled: bool,
    /// Whether to notify immediately or batch into a digest.
    pub mode: NotificationMode,
    /// Digest cadence in minutes when digest mode is enabled.
    pub digest_policy: DigestPolicy,
    /// Quiet-hours preference.
    pub quiet_hours: QuietHours,
    /// Minimum minutes between notifications for the same feed.
    pub minimum_interval_minutes: u32,
    /// Priority/watchlist hint.
    pub high_priority: bool,
    /// Optional include keywords.
    pub keyword_include: Vec<String>,
    /// Optional exclude keywords.
    pub keyword_exclude: Vec<String>,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: NotificationMode::Immediate,
            digest_policy: DigestPolicy::default(),
            quiet_hours: QuietHours::default(),
            minimum_interval_minutes: 60,
            high_priority: false,
            keyword_include: Vec::new(),
            keyword_exclude: Vec::new(),
        }
    }
}

/// Auto-refresh preference for a feed or folder scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshSettings {
    /// Whether automatic refresh is enabled for the scope.
    pub enabled: bool,
    /// Refresh cadence in minutes when enabled.
    pub interval_minutes: u32,
}

impl Default for RefreshSettings {
    fn default() -> Self {
        Self { enabled: true, interval_minutes: 15 }
    }
}

/// Global defaults for refresh and notification behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalNotificationSettings {
    /// Whether background refresh should run automatically.
    pub background_refresh_enabled: bool,
    /// Periodic refresh cadence in minutes for the app shell.
    pub background_refresh_interval_minutes: u32,
    /// Global digest defaults.
    pub digest_policy: DigestPolicy,
    /// Default feed notification preferences.
    pub default_feed_settings: NotificationSettings,
}

impl Default for GlobalNotificationSettings {
    fn default() -> Self {
        Self {
            background_refresh_enabled: true,
            background_refresh_interval_minutes: 15,
            digest_policy: DigestPolicy::default(),
            default_feed_settings: NotificationSettings::default(),
        }
    }
}

/// Delivery state for queued notification events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationDeliveryState {
    /// Waiting for delivery by the shell or a future push bridge.
    Pending,
    /// Delivered to the user.
    Delivered,
    /// Suppressed by policy or deduplication.
    Suppressed,
}

/// Persisted notification event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationEvent {
    /// Notification event id.
    pub id: String,
    /// Feed identifier.
    pub feed_id: Option<String>,
    /// Entry identifier.
    pub entry_id: Option<String>,
    /// Canonical identity used for deduplication.
    pub canonical_key: String,
    /// Content fingerprint used for deduplication.
    pub content_fingerprint: String,
    /// Entry title.
    pub title: String,
    /// Short body shown in the notification.
    pub body: String,
    /// Mode that was selected for the event.
    pub mode: NotificationMode,
    /// Current delivery state.
    pub delivery_state: NotificationDeliveryState,
    /// Decision reason, e.g. `new_item`, `duplicate`, `quiet_hours`.
    pub reason: String,
    /// Optional digest identifier.
    pub digest_id: Option<String>,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
    /// Delivery-ready timestamp.
    pub ready_at: Option<DateTime<Utc>>,
    /// Delivered timestamp.
    pub delivered_at: Option<DateTime<Utc>>,
    /// Suppressed timestamp.
    pub suppressed_at: Option<DateTime<Utc>>,
}

/// Persisted notification digest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationDigest {
    /// Digest identifier.
    pub id: String,
    /// Associated feed identifier, if any.
    pub feed_id: Option<String>,
    /// Number of entries in the digest.
    pub entry_count: usize,
    /// Digest title.
    pub title: String,
    /// Digest body preview.
    pub body: String,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
    /// Ready timestamp.
    pub ready_at: Option<DateTime<Utc>>,
    /// Delivered timestamp.
    pub delivered_at: Option<DateTime<Utc>>,
}

/// Registration metadata for future hosted push endpoints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PushEndpointRegistration {
    /// Registration identifier.
    pub id: String,
    /// Platform or transport name.
    pub platform: String,
    /// Device token or endpoint payload.
    pub endpoint: String,
    /// Whether the registration is active.
    pub enabled: bool,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Update timestamp.
    pub updated_at: DateTime<Utc>,
}
