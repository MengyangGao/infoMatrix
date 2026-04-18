//! SQLite storage and persistence APIs.

use chrono::{DateTime, Utc};
use models::{
    DigestPolicy, EntryKind, EntrySource, EntrySourceKind, FeedHealthState, FeedType,
    GlobalNotificationSettings, ItemScopeCounts, ItemState, ItemStatePatch, NewEntry, NewFeed,
    NormalizedItem, NotificationDeliveryState, NotificationDigest, NotificationEvent,
    NotificationMode, NotificationSettings, PushEndpointRegistration, QuietHours, RefreshSettings,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::cell::Cell;
use thiserror::Error;
use url::Url;
use uuid::Uuid;

/// Feed row used by API/query layers.
#[derive(Debug, Clone)]
pub struct FeedRow {
    /// Internal feed id.
    pub id: String,
    /// Feed URL.
    pub feed_url: Url,
    /// Optional site URL.
    pub site_url: Option<Url>,
    /// Feed title.
    pub title: Option<String>,
    /// Feed type.
    pub feed_type: FeedType,
    /// Whether the UI should auto-fetch full text for articles from this feed.
    pub auto_full_text: bool,
    /// Last ETag value.
    pub etag: Option<String>,
    /// Last-Modified header value.
    pub last_modified: Option<String>,
    /// Last fetch timestamp.
    pub last_fetch_at: Option<String>,
    /// Last successful fetch timestamp.
    pub last_success_at: Option<String>,
    /// Last HTTP status.
    pub last_http_status: Option<i64>,
    /// Consecutive fetch failures.
    pub failure_count: i64,
    /// Next scheduled fetch timestamp.
    pub next_scheduled_fetch_at: Option<String>,
    /// Average response time in ms.
    pub avg_response_time_ms: Option<i64>,
    /// Current feed health state.
    pub health_state: FeedHealthState,
}

/// Feed group row.
#[derive(Debug, Clone)]
pub struct FeedGroupRow {
    /// Internal group id.
    pub id: String,
    /// Group display name.
    pub name: String,
}

/// Feed icon row.
#[derive(Debug, Clone)]
pub struct FeedIconRow {
    /// Feed id this icon belongs to.
    pub feed_id: String,
    /// Icon source URL.
    pub source_url: String,
}

/// Per-feed notification settings row.
#[derive(Debug, Clone)]
pub struct FeedNotificationSettingsRow {
    /// Feed identifier.
    pub feed_id: String,
    /// Notification settings.
    pub settings: NotificationSettings,
    /// Last update timestamp.
    pub updated_at: String,
}

/// Refresh-job state row.
#[derive(Debug, Clone)]
pub struct RefreshJobRow {
    /// Feed identifier.
    pub feed_id: String,
    /// Last attempt timestamp.
    pub last_attempt_at: Option<String>,
    /// Last successful refresh timestamp.
    pub last_success_at: Option<String>,
    /// Consecutive failure count.
    pub failure_count: i64,
    /// Next scheduled attempt.
    pub next_attempt_at: Option<String>,
    /// Last HTTP status.
    pub last_http_status: Option<i64>,
    /// Last error message.
    pub last_error_message: Option<String>,
    /// Last reason.
    pub last_reason: Option<String>,
    /// Update timestamp.
    pub updated_at: String,
}

/// Refresh settings row for a specific scope.
#[derive(Debug, Clone)]
pub struct RefreshSettingsRow {
    /// Scope identifier such as a feed id or group id.
    pub scope_id: String,
    /// Refresh settings.
    pub settings: RefreshSettings,
    /// Last update timestamp.
    pub updated_at: String,
}

/// Notification signature row.
#[derive(Debug, Clone)]
pub struct NotificationSignatureRow {
    /// Row identifier.
    pub id: String,
    /// Feed identifier if known.
    pub feed_id: Option<String>,
    /// Entry identifier if known.
    pub entry_id: Option<String>,
    /// Canonical identity key.
    pub canonical_key: String,
    /// Content fingerprint.
    pub content_fingerprint: String,
    /// First time this item was seen.
    pub first_seen_at: String,
    /// Last time this item was seen.
    pub last_seen_at: String,
    /// Last time a notification was emitted.
    pub last_notification_at: Option<String>,
}

/// Notification event row.
#[derive(Debug, Clone)]
pub struct NotificationEventRow {
    /// Event identifier.
    pub id: String,
    /// Feed identifier if known.
    pub feed_id: Option<String>,
    /// Entry identifier if known.
    pub entry_id: Option<String>,
    /// Canonical identity key.
    pub canonical_key: String,
    /// Content fingerprint.
    pub content_fingerprint: String,
    /// Delivery mode.
    pub mode: NotificationMode,
    /// Delivery state.
    pub delivery_state: NotificationDeliveryState,
    /// Title.
    pub title: String,
    /// Body.
    pub body: String,
    /// Decision reason.
    pub reason: String,
    /// Optional digest id.
    pub digest_id: Option<String>,
    /// Created timestamp.
    pub created_at: String,
    /// Ready timestamp.
    pub ready_at: Option<String>,
    /// Delivered timestamp.
    pub delivered_at: Option<String>,
    /// Suppressed timestamp.
    pub suppressed_at: Option<String>,
    /// Optional metadata.
    pub metadata_json: Option<String>,
}

/// Notification digest row.
#[derive(Debug, Clone)]
pub struct NotificationDigestRow {
    /// Digest identifier.
    pub id: String,
    /// Feed identifier if known.
    pub feed_id: Option<String>,
    /// Entry identifiers JSON blob.
    pub entry_ids_json: String,
    /// Entry count.
    pub entry_count: i64,
    /// Title.
    pub title: String,
    /// Body.
    pub body: String,
    /// Created timestamp.
    pub created_at: String,
    /// Ready timestamp.
    pub ready_at: Option<String>,
    /// Delivered timestamp.
    pub delivered_at: Option<String>,
}

/// Push endpoint row.
#[derive(Debug, Clone)]
pub struct PushEndpointRow {
    /// Identifier.
    pub id: String,
    /// Platform/transport.
    pub platform: String,
    /// Endpoint payload.
    pub endpoint: String,
    /// Active flag.
    pub enabled: bool,
    /// Created timestamp.
    pub created_at: String,
    /// Updated timestamp.
    pub updated_at: String,
}

/// Discovery cache row.
#[derive(Debug, Clone)]
pub struct DiscoveryCacheRow {
    /// Original site URL used to build the cache.
    pub site_url: String,
    /// Normalized site URL used as the cache lookup key.
    pub normalized_site_url: String,
    /// Serialized discovery payload.
    pub result_json: String,
    /// Cache creation timestamp.
    pub created_at: String,
    /// Optional expiration timestamp.
    pub expires_at: Option<String>,
}

/// Sync event row used by queue inspection APIs.
#[derive(Debug, Clone)]
pub struct SyncEventRow {
    /// Event ID.
    pub id: String,
    /// Logical entity type (`item_state`, `feed`, etc).
    pub entity_type: String,
    /// Entity identifier.
    pub entity_id: String,
    /// Event kind (`created`, `updated`, `deleted`).
    pub event_type: String,
    /// JSON payload blob.
    pub payload_json: String,
    /// Event creation timestamp.
    pub created_at: String,
    /// Processing timestamp if acknowledged.
    pub processed_at: Option<String>,
}

/// Sync event payload accepted from remote adapters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncEventRecord {
    /// Event identifier.
    pub id: String,
    /// Logical entity type.
    pub entity_type: String,
    /// Entity identifier.
    pub entity_id: String,
    /// Event kind.
    pub event_type: String,
    /// JSON payload blob.
    pub payload_json: String,
    /// Event creation timestamp.
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncFeedPayload {
    feed_url: String,
    site_url: Option<String>,
    title: Option<String>,
    feed_type: FeedType,
    auto_full_text: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncFeedGroupPayload {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncFeedMembershipPayload {
    feed_url: String,
    group_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncItemStatePayload {
    item_id: Option<String>,
    canonical_url: Option<String>,
    external_item_id: Option<String>,
    is_read: bool,
    is_starred: bool,
    is_saved_for_later: bool,
    is_archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncGlobalNotificationSettingsPayload {
    settings: GlobalNotificationSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncFeedNotificationSettingsPayload {
    feed_url: String,
    settings: NotificationSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncFeedRefreshSettingsPayload {
    feed_url: String,
    settings: RefreshSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncGroupRefreshSettingsPayload {
    group_name: String,
    settings: RefreshSettings,
}

#[derive(Debug, Clone)]
struct EntrySyncIdentity {
    canonical_url: Option<String>,
    external_item_id: Option<String>,
}

/// Item summary row for list views.
#[derive(Debug, Clone)]
pub struct ItemSummaryRow {
    /// Internal item id.
    pub id: String,
    /// Unified item kind.
    pub kind: EntryKind,
    /// Source kind.
    pub source_kind: EntrySourceKind,
    /// Source identifier if available.
    pub source_id: Option<String>,
    /// Source URL if available.
    pub source_url: Option<Url>,
    /// Item title.
    pub title: String,
    /// Optional canonical URL.
    pub canonical_url: Option<Url>,
    /// Published timestamp string.
    pub published_at: Option<String>,
    /// Preview snippet for list UI.
    pub summary_preview: Option<String>,
    /// State flags.
    pub is_read: bool,
    /// Star state flag.
    pub is_starred: bool,
    /// Read-later flag.
    pub is_saved_for_later: bool,
    /// Archived flag.
    pub is_archived: bool,
}

/// Item detail row for detail views.
#[derive(Debug, Clone)]
pub struct ItemDetailRow {
    /// Internal item id.
    pub id: String,
    /// Unified item kind.
    pub kind: EntryKind,
    /// Source kind.
    pub source_kind: EntrySourceKind,
    /// Source identifier if available.
    pub source_id: Option<String>,
    /// Source URL if available.
    pub source_url: Option<Url>,
    /// Item title.
    pub title: String,
    /// Optional canonical URL.
    pub canonical_url: Option<Url>,
    /// Published timestamp string.
    pub published_at: Option<String>,
    /// Optional item summary.
    pub summary: Option<String>,
    /// Optional sanitized html.
    pub content_html: Option<String>,
    /// Optional text content.
    pub content_text: Option<String>,
    /// State flags.
    pub is_read: bool,
    /// Star state flag.
    pub is_starred: bool,
    /// Read-later flag.
    pub is_saved_for_later: bool,
    /// Archived flag.
    pub is_archived: bool,
}

/// Storage-layer errors.
#[derive(Debug, Error)]
pub enum StorageError {
    /// SQLite error.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// Serialization or conversion error.
    #[error("serialization error: {0}")]
    Serialization(String),
    /// Not found.
    #[error("record not found")]
    NotFound,
}

struct SyncEventSuppressionGuard {
    storage: *const Storage,
    previous: bool,
}

impl Drop for SyncEventSuppressionGuard {
    fn drop(&mut self) {
        unsafe {
            (*self.storage).sync_events_suppressed.set(self.previous);
        }
    }
}

/// SQLite-backed storage service.
pub struct Storage {
    conn: Connection,
    sync_events_suppressed: Cell<bool>,
}

/// Global item list filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemListFilter {
    /// Include non-archived items.
    All,
    /// Include unread and non-archived items.
    Unread,
    /// Include starred items.
    Starred,
    /// Include read-later items.
    Later,
    /// Include archived items only.
    Archive,
}

impl Storage {
    /// Open database from path.
    pub fn open(path: &str) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(Self { conn, sync_events_suppressed: Cell::new(false) })
    }

    /// Open in-memory database (mostly for tests).
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(Self { conn, sync_events_suppressed: Cell::new(false) })
    }

    /// Apply schema migrations.
    pub fn migrate(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(include_str!("../migrations/0001_init.sql"))?;
        self.conn.execute_batch(include_str!("../migrations/0002_notifications.sql"))?;
        self.conn.execute_batch(include_str!("../migrations/0004_refresh_rules.sql"))?;
        self.ensure_notification_settings_digest_max_items_column()?;
        self.ensure_feed_auto_full_text_column()?;
        self.conn.execute(
            "INSERT INTO item_search (item_id, title, summary, content_text)
             SELECT i.id, i.title, i.summary, c.content_text
             FROM items i
             LEFT JOIN item_contents c ON c.item_id = i.id
             WHERE NOT EXISTS (SELECT 1 FROM item_search s WHERE s.item_id = i.id)",
            [],
        )?;
        self.conn.execute(
            "INSERT OR IGNORE INTO entries (
                id, kind, external_item_id, canonical_url, title, author, summary,
                published_at, updated_at, raw_hash, dedup_reason, duplicate_of_entry_id,
                created_at, row_updated_at
             )
             SELECT
                i.id,
                'article',
                i.external_item_id,
                i.canonical_url,
                i.title,
                i.author,
                i.summary,
                i.published_at,
                i.updated_at,
                i.raw_hash,
                i.dedup_reason,
                i.duplicate_of_item_id,
                i.created_at,
                i.row_updated_at
             FROM items i
             WHERE NOT EXISTS (SELECT 1 FROM entries e WHERE e.id = i.id)",
            [],
        )?;
        self.conn.execute(
            "INSERT OR IGNORE INTO entry_contents (
                entry_id, content_html, content_text, created_at, updated_at
             )
             SELECT
                c.item_id,
                c.content_html,
                c.content_text,
                c.created_at,
                c.updated_at
             FROM item_contents c
             WHERE NOT EXISTS (SELECT 1 FROM entry_contents ec WHERE ec.entry_id = c.item_id)",
            [],
        )?;
        self.conn.execute(
            "INSERT OR IGNORE INTO entry_states (
                entry_id, is_read, is_starred, is_saved_for_later, is_archived,
                read_at, starred_at, saved_for_later_at, created_at, updated_at
             )
             SELECT
                s.item_id,
                s.is_read,
                s.is_starred,
                s.is_saved_for_later,
                s.is_archived,
                s.read_at,
                s.starred_at,
                s.saved_for_later_at,
                s.created_at,
                s.updated_at
             FROM item_states s
             WHERE NOT EXISTS (SELECT 1 FROM entry_states es WHERE es.entry_id = s.item_id)",
            [],
        )?;
        self.conn.execute(
            "INSERT OR IGNORE INTO entry_sources (
                id, entry_id, source_kind, source_id, source_url, source_title, created_at, updated_at
             )
             SELECT
                'src_' || i.id,
                i.id,
                'feed',
                i.feed_id,
                f.feed_url,
                f.title,
                i.created_at,
                i.row_updated_at
             FROM items i
             INNER JOIN feeds f ON f.id = i.feed_id
             WHERE NOT EXISTS (SELECT 1 FROM entry_sources es WHERE es.entry_id = i.id)",
            [],
        )?;
        self.conn.execute(
            "INSERT OR IGNORE INTO entry_search (entry_id, title, summary, content_text)
             SELECT e.id, e.title, e.summary, ec.content_text
             FROM entries e
             LEFT JOIN entry_contents ec ON ec.entry_id = e.id
             WHERE NOT EXISTS (SELECT 1 FROM entry_search s WHERE s.entry_id = e.id)",
            [],
        )?;
        Ok(())
    }

    fn resolve_feed_id_by_url(&self, feed_url: &str) -> Result<Option<String>, StorageError> {
        self.conn
            .query_row(
                "SELECT id FROM feeds WHERE feed_url = ?1 LIMIT 1",
                params![feed_url],
                |row| row.get(0),
            )
            .optional()
            .map_err(StorageError::Sqlite)
    }

    fn resolve_feed_url_by_id(&self, feed_id: &str) -> Result<Option<String>, StorageError> {
        self.conn
            .query_row(
                "SELECT feed_url FROM feeds WHERE id = ?1 LIMIT 1",
                params![feed_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(StorageError::Sqlite)
    }

    fn resolve_group_id_by_name(&self, group_name: &str) -> Result<Option<String>, StorageError> {
        self.conn
            .query_row(
                "SELECT id FROM feed_groups WHERE lower(name) = lower(?1) LIMIT 1",
                params![group_name],
                |row| row.get(0),
            )
            .optional()
            .map_err(StorageError::Sqlite)
    }

    fn resolve_entry_id_by_identity(
        &self,
        item_id: Option<&str>,
        canonical_url: Option<&str>,
        external_item_id: Option<&str>,
    ) -> Result<Option<String>, StorageError> {
        if let Some(item_id) = item_id {
            let existing = self
                .conn
                .query_row(
                    "SELECT id FROM entries WHERE id = ?1 LIMIT 1",
                    params![item_id],
                    |row| row.get(0),
                )
                .optional()?;
            if existing.is_some() {
                return Ok(existing);
            }
        }

        if let Some(canonical_url) = canonical_url {
            let existing = self
                .conn
                .query_row(
                    "SELECT id FROM entries WHERE canonical_url = ?1 LIMIT 1",
                    params![canonical_url],
                    |row| row.get(0),
                )
                .optional()?;
            if existing.is_some() {
                return Ok(existing);
            }
        }

        if let Some(external_item_id) = external_item_id {
            let existing = self
                .conn
                .query_row(
                    "SELECT id FROM entries WHERE external_item_id = ?1 LIMIT 1",
                    params![external_item_id],
                    |row| row.get(0),
                )
                .optional()?;
            if existing.is_some() {
                return Ok(existing);
            }
        }

        Ok(None)
    }

    fn get_entry_sync_identity(
        &self,
        item_id: &str,
    ) -> Result<Option<EntrySyncIdentity>, StorageError> {
        self.conn
            .query_row(
                "SELECT e.canonical_url, e.external_item_id
                 FROM entries e
                 WHERE e.id = ?1
                 LIMIT 1",
                params![item_id],
                |row| {
                    Ok(EntrySyncIdentity {
                        canonical_url: row.get(0)?,
                        external_item_id: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(StorageError::Sqlite)
    }

    fn is_syncable_user_entry(&self, entry_id: &str) -> Result<bool, StorageError> {
        let result = self
            .conn
            .query_row(
                "SELECT e.kind, src.source_kind
                 FROM entries e
                 LEFT JOIN entry_sources src ON src.entry_id = e.id
                 WHERE e.id = ?1
                 LIMIT 1",
                params![entry_id],
                |row| {
                    let kind_raw: String = row.get(0)?;
                    let source_kind_raw: String = row.get(1)?;
                    Ok(kind_raw != "article"
                        || entry_source_kind_from_str(&source_kind_raw) != EntrySourceKind::Feed)
                },
            )
            .optional()?;
        Ok(result.unwrap_or(false))
    }

    fn append_sync_event_if_enabled(
        &self,
        entity_type: &str,
        entity_id: &str,
        event_type: &str,
        payload_json: &str,
    ) -> Result<(), StorageError> {
        if self.sync_events_suppressed.get() {
            return Ok(());
        }
        self.append_sync_event(entity_type, entity_id, event_type, payload_json)
    }

    fn suppress_sync_events(&self) -> SyncEventSuppressionGuard {
        let previous = self.sync_events_suppressed.replace(true);
        SyncEventSuppressionGuard { storage: self as *const Storage, previous }
    }

    fn ensure_notification_settings_digest_max_items_column(&self) -> Result<(), StorageError> {
        let has_column = self.conn.query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM pragma_table_info('notification_settings')
                WHERE name = 'digest_max_items'
            )",
            [],
            |row| row.get::<_, i64>(0),
        )? != 0;

        if !has_column {
            self.conn.execute(
                "ALTER TABLE notification_settings
                 ADD COLUMN digest_max_items INTEGER NOT NULL DEFAULT 20",
                [],
            )?;
        }

        Ok(())
    }

    fn ensure_feed_auto_full_text_column(&self) -> Result<(), StorageError> {
        let has_column = self.conn.query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM pragma_table_info('feeds')
                WHERE name = 'auto_full_text'
            )",
            [],
            |row| row.get::<_, i64>(0),
        )? != 0;

        if !has_column {
            self.conn.execute(
                "ALTER TABLE feeds
                 ADD COLUMN auto_full_text INTEGER NOT NULL DEFAULT 1",
                [],
            )?;
        }

        Ok(())
    }

    /// Upsert feed and return feed id.
    pub fn upsert_feed(&self, new_feed: &NewFeed) -> Result<String, StorageError> {
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM feeds WHERE feed_url = ?1",
                params![new_feed.feed_url.as_str()],
                |row| row.get(0),
            )
            .optional()?;

        let now = Utc::now().to_rfc3339();

        if let Some(feed_id) = existing {
            let current_auto_full_text = self
                .conn
                .query_row(
                    "SELECT auto_full_text FROM feeds WHERE id = ?1 LIMIT 1",
                    params![&feed_id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap_or(1);
            self.conn.execute(
                "UPDATE feeds
                 SET site_url = COALESCE(?1, site_url),
                     title = COALESCE(?2, title),
                     feed_type = ?3,
                     updated_at = ?4
                 WHERE id = ?5",
                params![
                    new_feed.site_url.as_ref().map(|v| v.as_str()),
                    new_feed.title.as_deref(),
                    feed_type_to_str(new_feed.feed_type),
                    now,
                    feed_id,
                ],
            )?;
            let payload_json = serde_json::to_string(&SyncFeedPayload {
                feed_url: new_feed.feed_url.as_str().to_owned(),
                site_url: new_feed.site_url.as_ref().map(|v| v.as_str().to_owned()),
                title: new_feed.title.clone(),
                feed_type: new_feed.feed_type,
                auto_full_text: Some(current_auto_full_text != 0),
            })
            .map_err(|err| StorageError::Serialization(err.to_string()))?;
            self.append_sync_event_if_enabled(
                "feed",
                new_feed.feed_url.as_str(),
                "updated",
                &payload_json,
            )?;
            return Ok(feed_id);
        }

        let feed_id = Uuid::now_v7().to_string();
        let next_scheduled_fetch_at = compute_next_scheduled_fetch(
            Utc::now(),
            true,
            FeedHealthState::Healthy,
            0,
            self.get_global_notification_settings()?.background_refresh_interval_minutes.max(1),
        )
        .to_rfc3339();
        self.conn.execute(
            "INSERT INTO feeds (
                id, feed_url, site_url, title, feed_type, auto_full_text,
                next_scheduled_fetch_at, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8)",
            params![
                feed_id,
                new_feed.feed_url.as_str(),
                new_feed.site_url.as_ref().map(|v| v.as_str()),
                new_feed.title.as_deref(),
                feed_type_to_str(new_feed.feed_type),
                next_scheduled_fetch_at,
                now,
                now,
            ],
        )?;
        let payload_json = serde_json::to_string(&SyncFeedPayload {
            feed_url: new_feed.feed_url.as_str().to_owned(),
            site_url: new_feed.site_url.as_ref().map(|v| v.as_str().to_owned()),
            title: new_feed.title.clone(),
            feed_type: new_feed.feed_type,
            auto_full_text: Some(true),
        })
        .map_err(|err| StorageError::Serialization(err.to_string()))?;
        self.append_sync_event_if_enabled(
            "feed",
            new_feed.feed_url.as_str(),
            "created",
            &payload_json,
        )?;

        Ok(feed_id)
    }

    /// Fetch one feed row by id.
    pub fn get_feed(&self, feed_id: &str) -> Result<FeedRow, StorageError> {
        self.conn
            .query_row(
                "SELECT id, feed_url, site_url, title, feed_type, auto_full_text, etag,
                        last_modified, last_fetch_at, last_success_at, last_http_status,
                        failure_count, next_scheduled_fetch_at, avg_response_time_ms, health_state
                 FROM feeds WHERE id = ?1",
                params![feed_id],
                |row| {
                    let feed_url: String = row.get(1)?;
                    let site_url: Option<String> = row.get(2)?;
                    let feed_type_raw: String = row.get(4)?;
                    let health_state_raw: String = row.get(14)?;

                    Ok(FeedRow {
                        id: row.get(0)?,
                        feed_url: Url::parse(&feed_url).map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                1,
                                rusqlite::types::Type::Text,
                                Box::new(err),
                            )
                        })?,
                        site_url: site_url.and_then(|value| Url::parse(&value).ok()),
                        title: row.get(3)?,
                        feed_type: feed_type_from_str(&feed_type_raw),
                        auto_full_text: int_to_bool(row.get::<_, Option<i64>>(5)?.unwrap_or(1)),
                        etag: row.get(6)?,
                        last_modified: row.get(7)?,
                        last_fetch_at: row.get(8)?,
                        last_success_at: row.get(9)?,
                        last_http_status: row.get(10)?,
                        failure_count: row.get(11)?,
                        next_scheduled_fetch_at: row.get(12)?,
                        avg_response_time_ms: row.get(13)?,
                        health_state: feed_health_state_from_str(&health_state_raw),
                    })
                },
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound,
                _ => StorageError::Sqlite(err),
            })
    }

    /// List all feeds ordered by creation time.
    pub fn list_feeds(&self) -> Result<Vec<FeedRow>, StorageError> {
        let mut statement = self.conn.prepare(
            "SELECT id, feed_url, site_url, title, feed_type, auto_full_text, etag, last_modified,
                    last_fetch_at, last_success_at, last_http_status, failure_count,
                    next_scheduled_fetch_at, avg_response_time_ms, health_state
             FROM feeds
             ORDER BY created_at ASC",
        )?;

        let rows = statement.query_map([], |row| {
            let feed_url: String = row.get(1)?;
            let site_url: Option<String> = row.get(2)?;
            let feed_type_raw: String = row.get(4)?;
            let health_state_raw: String = row.get(14)?;

            Ok(FeedRow {
                id: row.get(0)?,
                feed_url: Url::parse(&feed_url).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?,
                site_url: site_url.and_then(|value| Url::parse(&value).ok()),
                title: row.get(3)?,
                feed_type: feed_type_from_str(&feed_type_raw),
                auto_full_text: int_to_bool(row.get::<_, Option<i64>>(5)?.unwrap_or(1)),
                etag: row.get(6)?,
                last_modified: row.get(7)?,
                last_fetch_at: row.get(8)?,
                last_success_at: row.get(9)?,
                last_http_status: row.get(10)?,
                failure_count: row.get(11)?,
                next_scheduled_fetch_at: row.get(12)?,
                avg_response_time_ms: row.get(13)?,
                health_state: feed_health_state_from_str(&health_state_raw),
            })
        })?;

        let mut feeds = Vec::new();
        for row in rows {
            feeds.push(row?);
        }
        Ok(feeds)
    }

    /// Delete one feed by id.
    pub fn delete_feed(&self, feed_id: &str) -> Result<(), StorageError> {
        let feed_url: Option<String> = self
            .conn
            .query_row(
                "SELECT feed_url FROM feeds WHERE id = ?1 LIMIT 1",
                params![feed_id],
                |row| row.get(0),
            )
            .optional()?;
        self.conn.execute(
            "DELETE FROM entries
             WHERE id IN (
                SELECT entry_id
                FROM entry_sources
                WHERE source_kind = 'feed'
                  AND source_id = ?1
             )",
            params![feed_id],
        )?;
        let affected = self.conn.execute("DELETE FROM feeds WHERE id = ?1", params![feed_id])?;
        if affected == 0 {
            return Err(StorageError::NotFound);
        }
        if let Some(feed_url) = feed_url {
            let payload_json = json!({ "feed_url": feed_url }).to_string();
            self.append_sync_event_if_enabled("feed", &feed_url, "deleted", &payload_json)?;
        }
        Ok(())
    }

    /// Delete a unified entry and its dependent rows.
    pub fn delete_entry(&self, entry_id: &str) -> Result<(), StorageError> {
        let affected = self.conn.execute("DELETE FROM entries WHERE id = ?1", params![entry_id])?;
        if affected == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    /// List feeds whose next scheduled refresh is due.
    pub fn list_due_feeds(&self, limit: usize) -> Result<Vec<FeedRow>, StorageError> {
        let globals = self.get_global_notification_settings()?;
        let global_enabled = bool_to_i64(globals.background_refresh_enabled);
        let mut statement = self.conn.prepare(
            "SELECT f.id, f.feed_url, f.site_url, f.title, f.feed_type, f.auto_full_text, f.etag, f.last_modified,
                    f.last_fetch_at, f.last_success_at, f.last_http_status, f.failure_count,
                    f.next_scheduled_fetch_at, f.avg_response_time_ms, f.health_state
             FROM feeds f
             LEFT JOIN feed_refresh_settings fr ON fr.feed_id = f.id
             LEFT JOIN group_memberships gm ON gm.feed_id = f.id
             LEFT JOIN group_refresh_settings gr ON gr.group_id = gm.group_id
             WHERE COALESCE(fr.enabled, gr.enabled, ?1) = 1
               AND (f.next_scheduled_fetch_at IS NULL
                OR f.next_scheduled_fetch_at <= ?2)
             ORDER BY COALESCE(f.next_scheduled_fetch_at, f.created_at) ASC, f.created_at ASC
             LIMIT ?3",
        )?;
        let now = Utc::now().to_rfc3339();
        let rows = statement.query_map(params![global_enabled, now, limit as i64], |row| {
            let feed_url: String = row.get(1)?;
            let site_url: Option<String> = row.get(2)?;
            let feed_type_raw: String = row.get(4)?;
            let health_state_raw: String = row.get(14)?;

            Ok(FeedRow {
                id: row.get(0)?,
                feed_url: Url::parse(&feed_url).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?,
                site_url: site_url.and_then(|value| Url::parse(&value).ok()),
                title: row.get(3)?,
                feed_type: feed_type_from_str(&feed_type_raw),
                auto_full_text: int_to_bool(row.get::<_, Option<i64>>(5)?.unwrap_or(1)),
                etag: row.get(6)?,
                last_modified: row.get(7)?,
                last_fetch_at: row.get(8)?,
                last_success_at: row.get(9)?,
                last_http_status: row.get(10)?,
                failure_count: row.get(11)?,
                next_scheduled_fetch_at: row.get(12)?,
                avg_response_time_ms: row.get(13)?,
                health_state: feed_health_state_from_str(&health_state_raw),
            })
        })?;
        let mut feeds = Vec::new();
        for row in rows {
            feeds.push(row?);
        }
        Ok(feeds)
    }

    /// Update feed title.
    pub fn update_feed_title(
        &self,
        feed_id: &str,
        title: Option<&str>,
    ) -> Result<(), StorageError> {
        let affected = self.conn.execute(
            "UPDATE feeds SET title = NULLIF(TRIM(?1), ''), updated_at = ?2 WHERE id = ?3",
            params![title, Utc::now().to_rfc3339(), feed_id],
        )?;
        if affected == 0 {
            return Err(StorageError::NotFound);
        }
        if let Some(feed_url) = self.resolve_feed_url_by_id(feed_id)? {
            let feed = self.get_feed(feed_id)?;
            let payload_json = serde_json::to_string(&SyncFeedPayload {
                feed_url: feed.feed_url.as_str().to_owned(),
                site_url: feed.site_url.as_ref().map(|value| value.as_str().to_owned()),
                title: feed.title.clone(),
                feed_type: feed.feed_type,
                auto_full_text: Some(feed.auto_full_text),
            })
            .map_err(|err| StorageError::Serialization(err.to_string()))?;
            self.append_sync_event_if_enabled("feed", &feed_url, "updated", &payload_json)?;
        }
        Ok(())
    }

    /// Update whether a feed should auto-fetch full text.
    pub fn update_feed_auto_full_text(
        &self,
        feed_id: &str,
        enabled: bool,
    ) -> Result<(), StorageError> {
        let affected = self.conn.execute(
            "UPDATE feeds SET auto_full_text = ?1, updated_at = ?2 WHERE id = ?3",
            params![bool_to_i64(enabled), Utc::now().to_rfc3339(), feed_id],
        )?;
        if affected == 0 {
            return Err(StorageError::NotFound);
        }
        if let Some(feed_url) = self.resolve_feed_url_by_id(feed_id)? {
            let feed = self.get_feed(feed_id)?;
            let payload_json = serde_json::to_string(&SyncFeedPayload {
                feed_url: feed.feed_url.as_str().to_owned(),
                site_url: feed.site_url.as_ref().map(|value| value.as_str().to_owned()),
                title: feed.title.clone(),
                feed_type: feed.feed_type,
                auto_full_text: Some(enabled),
            })
            .map_err(|err| StorageError::Serialization(err.to_string()))?;
            self.append_sync_event_if_enabled("feed", &feed_url, "updated", &payload_json)?;
        }
        Ok(())
    }

    /// List all feed groups.
    pub fn list_groups(&self) -> Result<Vec<FeedGroupRow>, StorageError> {
        let mut statement = self
            .conn
            .prepare("SELECT id, name FROM feed_groups ORDER BY name COLLATE NOCASE ASC")?;
        let rows = statement
            .query_map([], |row| Ok(FeedGroupRow { id: row.get(0)?, name: row.get(1)? }))?;
        let mut groups = Vec::new();
        for row in rows {
            groups.push(row?);
        }
        Ok(groups)
    }

    /// Create (or return existing) group by name.
    pub fn create_group(&self, name: &str) -> Result<FeedGroupRow, StorageError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(StorageError::Serialization("group name is empty".to_owned()));
        }

        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM feed_groups WHERE lower(name) = lower(?1)",
                params![trimmed],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            return Ok(FeedGroupRow { id, name: trimmed.to_owned() });
        }

        let now = Utc::now().to_rfc3339();
        let id = Uuid::now_v7().to_string();
        self.conn.execute(
            "INSERT INTO feed_groups (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![id, trimmed, now, now],
        )?;
        let payload_json = json!({ "name": trimmed }).to_string();
        self.append_sync_event_if_enabled("feed_group", &id, "created", &payload_json)?;
        Ok(FeedGroupRow { id, name: trimmed.to_owned() })
    }

    /// Delete a feed group.
    pub fn delete_group(&self, group_id: &str) -> Result<(), StorageError> {
        let affected_feeds = self.list_feed_ids_for_group(group_id)?;
        let affected =
            self.conn.execute("DELETE FROM feed_groups WHERE id = ?1", params![group_id])?;
        if affected == 0 {
            return Err(StorageError::NotFound);
        }
        for feed_id in affected_feeds {
            let _ = self.recalculate_feed_refresh_schedule(&feed_id);
        }
        Ok(())
    }

    /// Assign feed into a single group (or clear group assignment).
    pub fn set_feed_group(
        &mut self,
        feed_id: &str,
        group_id: Option<&str>,
    ) -> Result<(), StorageError> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM group_memberships WHERE feed_id = ?1", params![feed_id])?;
        if let Some(group_id) = group_id {
            tx.execute(
                "INSERT INTO group_memberships (id, group_id, feed_id, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![Uuid::now_v7().to_string(), group_id, feed_id, Utc::now().to_rfc3339()],
            )?;
        }
        tx.commit()?;
        let feed_url = self
            .conn
            .query_row(
                "SELECT feed_url FROM feeds WHERE id = ?1 LIMIT 1",
                params![feed_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let group_name = if let Some(group_id) = group_id {
            self.conn
                .query_row(
                    "SELECT name FROM feed_groups WHERE id = ?1 LIMIT 1",
                    params![group_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
        } else {
            None
        };
        if let Some(feed_url) = feed_url {
            let payload_json = json!({
                "feed_url": feed_url,
                "group_name": group_name,
            })
            .to_string();
            self.append_sync_event_if_enabled(
                "feed_membership",
                &feed_url,
                "updated",
                &payload_json,
            )?;
        }
        let _ = self.recalculate_feed_refresh_schedule(feed_id);
        Ok(())
    }

    /// List groups for one feed.
    pub fn list_groups_for_feed(&self, feed_id: &str) -> Result<Vec<FeedGroupRow>, StorageError> {
        let mut statement = self.conn.prepare(
            "SELECT g.id, g.name
             FROM feed_groups g
             INNER JOIN group_memberships m ON m.group_id = g.id
             WHERE m.feed_id = ?1
             ORDER BY g.name COLLATE NOCASE ASC",
        )?;
        let rows = statement.query_map(params![feed_id], |row| {
            Ok(FeedGroupRow { id: row.get(0)?, name: row.get(1)? })
        })?;
        let mut groups = Vec::new();
        for row in rows {
            groups.push(row?);
        }
        Ok(groups)
    }

    /// Store per-feed auto-refresh settings.
    pub fn upsert_feed_refresh_settings(
        &mut self,
        feed_id: &str,
        settings: &RefreshSettings,
    ) -> Result<RefreshSettingsRow, StorageError> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO feed_refresh_settings (
                feed_id, enabled, interval_minutes, updated_at
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(feed_id) DO UPDATE SET
                enabled = excluded.enabled,
                interval_minutes = excluded.interval_minutes,
                updated_at = excluded.updated_at",
            params![
                feed_id,
                bool_to_i64(settings.enabled),
                settings.interval_minutes.max(1) as i64,
                now,
            ],
        )?;
        let row = self.get_feed_refresh_settings(feed_id)?.ok_or(StorageError::NotFound)?;
        if let Some(feed_url) = self.resolve_feed_url_by_id(feed_id)? {
            let payload_json = json!({
                "feed_url": feed_url,
                "settings": settings,
            })
            .to_string();
            self.append_sync_event_if_enabled(
                "feed_refresh_settings",
                &feed_url,
                "updated",
                &payload_json,
            )?;
        }
        let _ = self.recalculate_feed_refresh_schedule(feed_id);
        Ok(row)
    }

    /// Delete explicit per-feed auto-refresh settings so the feed falls back to inheritance.
    pub fn delete_feed_refresh_settings(&mut self, feed_id: &str) -> Result<(), StorageError> {
        let affected = self
            .conn
            .execute("DELETE FROM feed_refresh_settings WHERE feed_id = ?1", params![feed_id])?;
        if affected == 0 {
            return Err(StorageError::NotFound);
        }
        if let Some(feed_url) = self.resolve_feed_url_by_id(feed_id)? {
            let payload_json = json!({ "feed_url": feed_url }).to_string();
            self.append_sync_event_if_enabled(
                "feed_refresh_settings",
                &feed_url,
                "deleted",
                &payload_json,
            )?;
        }
        let _ = self.recalculate_feed_refresh_schedule(feed_id);
        Ok(())
    }

    /// Load per-feed auto-refresh settings.
    pub fn get_feed_refresh_settings(
        &self,
        feed_id: &str,
    ) -> Result<Option<RefreshSettingsRow>, StorageError> {
        self.conn
            .query_row(
                "SELECT feed_id, enabled, interval_minutes, updated_at
                 FROM feed_refresh_settings
                 WHERE feed_id = ?1",
                params![feed_id],
                |row| {
                    Ok(RefreshSettingsRow {
                        scope_id: row.get(0)?,
                        settings: RefreshSettings {
                            enabled: int_to_bool(row.get::<_, i64>(1)?),
                            interval_minutes: row.get::<_, i64>(2)?.max(1) as u32,
                        },
                        updated_at: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(StorageError::Sqlite)
    }

    /// Store per-group auto-refresh settings.
    pub fn upsert_group_refresh_settings(
        &mut self,
        group_id: &str,
        settings: &RefreshSettings,
    ) -> Result<RefreshSettingsRow, StorageError> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO group_refresh_settings (
                group_id, enabled, interval_minutes, updated_at
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(group_id) DO UPDATE SET
                enabled = excluded.enabled,
                interval_minutes = excluded.interval_minutes,
                updated_at = excluded.updated_at",
            params![
                group_id,
                bool_to_i64(settings.enabled),
                settings.interval_minutes.max(1) as i64,
                now,
            ],
        )?;
        let row = self.get_group_refresh_settings(group_id)?.ok_or(StorageError::NotFound)?;
        for feed_id in self.list_feed_ids_for_group(group_id)? {
            let _ = self.recalculate_feed_refresh_schedule(&feed_id);
        }
        Ok(row)
    }

    /// Delete explicit per-group auto-refresh settings so the group falls back to inheritance.
    pub fn delete_group_refresh_settings(&mut self, group_id: &str) -> Result<(), StorageError> {
        let affected_feeds = self.list_feed_ids_for_group(group_id)?;
        let group_name: Option<String> = self
            .conn
            .query_row(
                "SELECT name FROM feed_groups WHERE id = ?1 LIMIT 1",
                params![group_id],
                |row| row.get(0),
            )
            .optional()?;
        let affected = self
            .conn
            .execute("DELETE FROM group_refresh_settings WHERE group_id = ?1", params![group_id])?;
        if affected == 0 {
            return Err(StorageError::NotFound);
        }
        if let Some(group_name) = group_name {
            let payload_json = json!({ "group_name": group_name }).to_string();
            self.append_sync_event_if_enabled(
                "group_refresh_settings",
                &group_name,
                "deleted",
                &payload_json,
            )?;
        }
        for feed_id in affected_feeds {
            let _ = self.recalculate_feed_refresh_schedule(&feed_id);
        }
        Ok(())
    }

    /// Load per-group auto-refresh settings.
    pub fn get_group_refresh_settings(
        &self,
        group_id: &str,
    ) -> Result<Option<RefreshSettingsRow>, StorageError> {
        self.conn
            .query_row(
                "SELECT group_id, enabled, interval_minutes, updated_at
                 FROM group_refresh_settings
                 WHERE group_id = ?1",
                params![group_id],
                |row| {
                    Ok(RefreshSettingsRow {
                        scope_id: row.get(0)?,
                        settings: RefreshSettings {
                            enabled: int_to_bool(row.get::<_, i64>(1)?),
                            interval_minutes: row.get::<_, i64>(2)?.max(1) as u32,
                        },
                        updated_at: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(StorageError::Sqlite)
    }

    /// Resolve the effective auto-refresh settings for a feed.
    pub fn resolve_effective_refresh_settings(
        &self,
        feed_id: &str,
    ) -> Result<RefreshSettings, StorageError> {
        if let Some(feed_settings) = self.get_feed_refresh_settings(feed_id)? {
            return Ok(feed_settings.settings);
        }
        if let Some(group_id) = self
            .conn
            .query_row(
                "SELECT group_id FROM group_memberships WHERE feed_id = ?1 ORDER BY created_at ASC LIMIT 1",
                params![feed_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            if let Some(group_settings) = self.get_group_refresh_settings(&group_id)? {
                return Ok(group_settings.settings);
            }
        }
        let globals = self.get_global_notification_settings()?;
        Ok(RefreshSettings {
            enabled: globals.background_refresh_enabled,
            interval_minutes: globals.background_refresh_interval_minutes.max(1),
        })
    }

    fn list_feed_ids_for_group(&self, group_id: &str) -> Result<Vec<String>, StorageError> {
        let mut statement = self.conn.prepare(
            "SELECT feed_id
             FROM group_memberships
             WHERE group_id = ?1
             ORDER BY created_at ASC",
        )?;
        let rows = statement.query_map(params![group_id], |row| row.get(0))?;
        let mut feed_ids = Vec::new();
        for row in rows {
            feed_ids.push(row?);
        }
        Ok(feed_ids)
    }

    fn recalculate_feed_refresh_schedule(&self, feed_id: &str) -> Result<(), StorageError> {
        let feed = self.get_feed(feed_id)?;
        let settings = self.resolve_effective_refresh_settings(feed_id)?;
        let next_scheduled_fetch_at = if settings.enabled {
            Some(
                compute_next_scheduled_fetch(
                    Utc::now(),
                    feed.last_fetch_at.is_none(),
                    feed.health_state,
                    feed.failure_count as u32,
                    settings.interval_minutes.max(1),
                )
                .to_rfc3339(),
            )
        } else {
            None
        };
        self.set_feed_next_scheduled_fetch_at(feed_id, next_scheduled_fetch_at.as_deref())
    }

    fn recalculate_all_feed_refresh_schedules(&self) -> Result<(), StorageError> {
        for feed in self.list_feeds()? {
            let _ = self.recalculate_feed_refresh_schedule(&feed.id);
        }
        Ok(())
    }

    /// Set one primary icon URL for a feed.
    pub fn set_feed_icon(&mut self, feed_id: &str, source_url: &str) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM icons WHERE feed_id = ?1", params![feed_id])?;
        tx.execute(
            "INSERT INTO icons (
                id, feed_id, source_url, mime_type, byte_size, sha256, local_path,
                fetched_at, status, created_at, updated_at
             ) VALUES (?1, ?2, ?3, NULL, NULL, NULL, NULL, ?4, 'resolved', ?4, ?4)",
            params![Uuid::now_v7().to_string(), feed_id, source_url, now],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Store icon metadata and cached asset path for a feed.
    pub fn set_feed_icon_asset(
        &mut self,
        feed_id: &str,
        source_url: &str,
        mime_type: Option<&str>,
        byte_size: i64,
        sha256: &str,
        local_path: &str,
    ) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM icons WHERE feed_id = ?1", params![feed_id])?;
        tx.execute(
            "INSERT INTO icons (
                id, feed_id, source_url, mime_type, byte_size, sha256, local_path,
                fetched_at, status, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'resolved', ?8, ?8)",
            params![
                Uuid::now_v7().to_string(),
                feed_id,
                source_url,
                mime_type,
                byte_size,
                sha256,
                local_path,
                now,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Persist a discovery cache snapshot for later subscription reuse.
    pub fn upsert_discovery_cache(
        &self,
        site_url: &str,
        normalized_site_url: &str,
        result_json: &str,
        expires_at: Option<&str>,
    ) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO discovery_cache (
                id, site_url, normalized_site_url, result_json, created_at, expires_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                Uuid::now_v7().to_string(),
                site_url,
                normalized_site_url,
                result_json,
                now,
                expires_at,
            ],
        )?;
        Ok(())
    }

    /// Fetch the newest discovery cache snapshot for a normalized site URL.
    pub fn get_latest_discovery_cache(
        &self,
        normalized_site_url: &str,
    ) -> Result<Option<DiscoveryCacheRow>, StorageError> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .query_row(
                "SELECT site_url, normalized_site_url, result_json, created_at, expires_at
                 FROM discovery_cache
                 WHERE (normalized_site_url = ?1 OR site_url = ?1)
                   AND expires_at IS NOT NULL
                   AND expires_at > ?2
                 ORDER BY created_at DESC
                 LIMIT 1",
                params![normalized_site_url, now],
                |row| {
                    Ok(DiscoveryCacheRow {
                        site_url: row.get(0)?,
                        normalized_site_url: row.get(1)?,
                        result_json: row.get(2)?,
                        created_at: row.get(3)?,
                        expires_at: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(StorageError::Sqlite)
    }

    /// Override the next scheduled fetch timestamp for a feed.
    pub fn set_feed_next_scheduled_fetch_at(
        &self,
        feed_id: &str,
        next_scheduled_fetch_at: Option<&str>,
    ) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE feeds
             SET next_scheduled_fetch_at = ?1,
                 updated_at = ?2
             WHERE id = ?3",
            params![next_scheduled_fetch_at, now, feed_id],
        )?;
        Ok(())
    }

    /// Return the most recent icon URL for a feed.
    pub fn get_feed_icon(&self, feed_id: &str) -> Result<Option<FeedIconRow>, StorageError> {
        self.conn
            .query_row(
                "SELECT feed_id, source_url
                 FROM icons
                 WHERE feed_id = ?1
                 ORDER BY fetched_at DESC, updated_at DESC
                 LIMIT 1",
                params![feed_id],
                |row| Ok(FeedIconRow { feed_id: row.get(0)?, source_url: row.get(1)? }),
            )
            .optional()
            .map_err(StorageError::Sqlite)
    }

    /// Store a per-feed notification preference row.
    pub fn upsert_notification_settings(
        &mut self,
        feed_id: &str,
        settings: &NotificationSettings,
    ) -> Result<FeedNotificationSettingsRow, StorageError> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO notification_settings (
                feed_id, enabled, mode, digest_interval_minutes, quiet_hours_enabled,
                quiet_hours_start_minute, quiet_hours_end_minute, minimum_interval_minutes,
                high_priority, digest_max_items, keyword_include_json, keyword_exclude_json, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(feed_id) DO UPDATE SET
                enabled = excluded.enabled,
                mode = excluded.mode,
                digest_interval_minutes = excluded.digest_interval_minutes,
                quiet_hours_enabled = excluded.quiet_hours_enabled,
                quiet_hours_start_minute = excluded.quiet_hours_start_minute,
                quiet_hours_end_minute = excluded.quiet_hours_end_minute,
                minimum_interval_minutes = excluded.minimum_interval_minutes,
                high_priority = excluded.high_priority,
                digest_max_items = excluded.digest_max_items,
                keyword_include_json = excluded.keyword_include_json,
                keyword_exclude_json = excluded.keyword_exclude_json,
                updated_at = excluded.updated_at",
            params![
                feed_id,
                bool_to_i64(settings.enabled),
                notification_mode_to_str(settings.mode),
                settings.digest_policy.interval_minutes as i64,
                bool_to_i64(settings.quiet_hours.enabled),
                settings.quiet_hours.start_minute as i64,
                settings.quiet_hours.end_minute as i64,
                settings.minimum_interval_minutes as i64,
                bool_to_i64(settings.high_priority),
                settings.digest_policy.max_items as i64,
                serde_json::to_string(&settings.keyword_include)
                    .map_err(|err| StorageError::Serialization(err.to_string()))?,
                serde_json::to_string(&settings.keyword_exclude)
                    .map_err(|err| StorageError::Serialization(err.to_string()))?,
                now,
            ],
        )?;
        if let Some(feed_url) = self.resolve_feed_url_by_id(feed_id)? {
            let payload_json = json!({
                "feed_url": feed_url,
                "settings": settings,
            })
            .to_string();
            self.append_sync_event_if_enabled(
                "notification_settings",
                &feed_url,
                "updated",
                &payload_json,
            )?;
        }
        self.get_notification_settings(feed_id)?.ok_or(StorageError::NotFound)
    }

    /// Load per-feed notification preferences.
    pub fn get_notification_settings(
        &self,
        feed_id: &str,
    ) -> Result<Option<FeedNotificationSettingsRow>, StorageError> {
        self.conn
            .query_row(
                "SELECT feed_id, enabled, mode, digest_interval_minutes, quiet_hours_enabled,
                        quiet_hours_start_minute, quiet_hours_end_minute, minimum_interval_minutes,
                        high_priority, digest_max_items, keyword_include_json, keyword_exclude_json, updated_at
                 FROM notification_settings
                 WHERE feed_id = ?1",
                params![feed_id],
                |row| {
                    let mode_raw: String = row.get(2)?;
                    let digest_max_items = row.get::<_, Option<i64>>(9)?.unwrap_or(20);
                    let keyword_include_json: String = row.get(10)?;
                    let keyword_exclude_json: String = row.get(11)?;
                    Ok(FeedNotificationSettingsRow {
                        feed_id: row.get(0)?,
                        settings: NotificationSettings {
                            enabled: int_to_bool(row.get::<_, i64>(1)?),
                            mode: notification_mode_from_str(&mode_raw),
                            digest_policy: DigestPolicy {
                                enabled: matches!(
                                    notification_mode_from_str(&mode_raw),
                                    NotificationMode::Digest
                                ),
                                interval_minutes: row.get::<_, i64>(3)? as u32,
                                max_items: digest_max_items.max(0) as u32,
                            },
                            quiet_hours: QuietHours {
                                enabled: int_to_bool(row.get::<_, i64>(4)?),
                                start_minute: row.get::<_, i64>(5)? as u16,
                                end_minute: row.get::<_, i64>(6)? as u16,
                            },
                            minimum_interval_minutes: row.get::<_, i64>(7)? as u32,
                            high_priority: int_to_bool(row.get::<_, i64>(8)?),
                            keyword_include: serde_json::from_str(&keyword_include_json)
                                .unwrap_or_default(),
                            keyword_exclude: serde_json::from_str(&keyword_exclude_json)
                                .unwrap_or_default(),
                        },
                        updated_at: row.get(12)?,
                    })
                },
            )
            .optional()
            .map_err(StorageError::Sqlite)
    }

    /// Persist global notification defaults in app settings.
    pub fn set_global_notification_settings(
        &self,
        settings: &GlobalNotificationSettings,
    ) -> Result<(), StorageError> {
        self.set_app_setting_json("notification_globals", settings)?;
        let payload_json = json!({ "settings": settings }).to_string();
        self.append_sync_event_if_enabled(
            "notification_globals",
            "notification_globals",
            "updated",
            &payload_json,
        )?;
        let _ = self.recalculate_all_feed_refresh_schedules();
        Ok(())
    }

    /// Load global notification defaults from app settings.
    pub fn get_global_notification_settings(
        &self,
    ) -> Result<GlobalNotificationSettings, StorageError> {
        Ok(self.get_app_setting_json("notification_globals")?.unwrap_or_default())
    }

    /// Record a notification signature and report whether it is brand new.
    pub fn record_notification_signature(
        &self,
        feed_id: &str,
        entry_id: &str,
        canonical_key: &str,
        content_fingerprint: &str,
        seen_at: &str,
        notified_at: Option<&str>,
    ) -> Result<bool, StorageError> {
        let inserted = self.conn.execute(
            "INSERT OR IGNORE INTO notification_signatures (
                id, feed_id, entry_id, canonical_key, content_fingerprint,
                first_seen_at, last_seen_at, last_notification_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7)",
            params![
                Uuid::now_v7().to_string(),
                feed_id,
                entry_id,
                canonical_key,
                content_fingerprint,
                seen_at,
                notified_at,
            ],
        )?;
        if inserted == 0 {
            self.conn.execute(
                "UPDATE notification_signatures
                 SET last_seen_at = ?1
                 WHERE canonical_key = ?2
                   AND content_fingerprint = ?3",
                params![seen_at, canonical_key, content_fingerprint],
            )?;
            Ok(false)
        } else {
            Ok(true)
        }
    }

    /// Update the last notification timestamp for a signature.
    pub fn touch_notification_signature(
        &self,
        canonical_key: &str,
        content_fingerprint: &str,
        notified_at: &str,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE notification_signatures
             SET last_notification_at = ?1,
                 last_seen_at = ?1
             WHERE canonical_key = ?2
               AND content_fingerprint = ?3",
            params![notified_at, canonical_key, content_fingerprint],
        )?;
        Ok(())
    }

    /// Persist a queued or audited notification event.
    pub fn insert_notification_event(
        &self,
        event: &NotificationEvent,
        metadata_json: Option<&str>,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO notification_events (
                id, feed_id, entry_id, canonical_key, content_fingerprint, mode,
                delivery_state, title, body, reason, digest_id, created_at, ready_at,
                delivered_at, suppressed_at, metadata_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                &event.id,
                &event.feed_id,
                &event.entry_id,
                &event.canonical_key,
                &event.content_fingerprint,
                notification_mode_to_str(event.mode),
                notification_delivery_state_to_str(event.delivery_state),
                &event.title,
                &event.body,
                &event.reason,
                &event.digest_id,
                event.created_at.to_rfc3339(),
                event.ready_at.map(|v| v.to_rfc3339()),
                event.delivered_at.map(|v| v.to_rfc3339()),
                event.suppressed_at.map(|v| v.to_rfc3339()),
                metadata_json,
            ],
        )?;
        Ok(())
    }

    /// Persist a notification digest batch.
    pub fn insert_notification_digest(
        &self,
        digest: &NotificationDigest,
        entry_ids_json: &str,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO notification_digests (
                id, feed_id, entry_ids_json, entry_count, title, body,
                created_at, ready_at, delivered_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &digest.id,
                &digest.feed_id,
                entry_ids_json,
                digest.entry_count as i64,
                &digest.title,
                &digest.body,
                digest.created_at.to_rfc3339(),
                digest.ready_at.map(|v| v.to_rfc3339()),
                digest.delivered_at.map(|v| v.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    /// List pending notification events for shell delivery.
    pub fn list_pending_notification_events(
        &self,
        limit: usize,
    ) -> Result<Vec<NotificationEventRow>, StorageError> {
        let mut statement = self.conn.prepare(
            "SELECT id, feed_id, entry_id, canonical_key, content_fingerprint, mode,
                    delivery_state, title, body, reason, digest_id, created_at, ready_at,
                    delivered_at, suppressed_at, metadata_json
             FROM notification_events
             WHERE delivery_state = 'pending'
               AND (ready_at IS NULL OR ready_at <= ?1)
             ORDER BY created_at ASC
             LIMIT ?2",
        )?;
        let rows = statement.query_map(params![Utc::now().to_rfc3339(), limit as i64], |row| {
            Ok(NotificationEventRow {
                id: row.get(0)?,
                feed_id: row.get(1)?,
                entry_id: row.get(2)?,
                canonical_key: row.get(3)?,
                content_fingerprint: row.get(4)?,
                mode: notification_mode_from_str(&row.get::<_, String>(5)?),
                delivery_state: notification_delivery_state_from_str(&row.get::<_, String>(6)?),
                title: row.get(7)?,
                body: row.get(8)?,
                reason: row.get(9)?,
                digest_id: row.get(10)?,
                created_at: row.get(11)?,
                ready_at: row.get(12)?,
                delivered_at: row.get(13)?,
                suppressed_at: row.get(14)?,
                metadata_json: row.get(15)?,
            })
        })?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    /// Mark notification events as delivered.
    pub fn acknowledge_notification_events(
        &self,
        event_ids: &[String],
    ) -> Result<usize, StorageError> {
        if event_ids.is_empty() {
            return Ok(0);
        }
        let delivered_at = Utc::now().to_rfc3339();
        let mut affected = 0usize;
        for event_id in event_ids {
            affected += self.conn.execute(
                "UPDATE notification_events
                 SET delivery_state = 'delivered',
                     delivered_at = ?1
                 WHERE id = ?2
                   AND delivery_state = 'pending'",
                params![delivered_at, event_id],
            )?;
        }
        Ok(affected)
    }

    /// Record a refresh attempt and mirror the state in the refresh job table.
    pub fn record_refresh_attempt(
        &mut self,
        attempt: &notifications::RefreshAttempt,
    ) -> Result<(), StorageError> {
        let current_feed = self.get_feed(&attempt.feed_id)?;
        let tx = self.conn.transaction()?;
        let failure_count = current_feed.failure_count;
        tx.execute(
            "INSERT INTO refresh_attempts (
                id, feed_id, reason, started_at, finished_at, success,
                http_status, item_count, error_message, next_attempt_at, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                Uuid::now_v7().to_string(),
                &attempt.feed_id,
                refresh_reason_to_str(attempt.reason),
                attempt.started_at.to_rfc3339(),
                attempt.finished_at.to_rfc3339(),
                bool_to_i64(attempt.success),
                attempt.http_status.map(|value| value as i64),
                attempt.item_count as i64,
                &attempt.error_message,
                attempt.next_attempt_at.map(|value| value.to_rfc3339()),
                Utc::now().to_rfc3339(),
            ],
        )?;
        tx.execute(
            "INSERT INTO refresh_jobs (
                feed_id, last_attempt_at, last_success_at, failure_count, next_attempt_at,
                last_http_status, last_error_message, last_reason, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(feed_id) DO UPDATE SET
                last_attempt_at = excluded.last_attempt_at,
                last_success_at = excluded.last_success_at,
                failure_count = excluded.failure_count,
                next_attempt_at = excluded.next_attempt_at,
                last_http_status = excluded.last_http_status,
                last_error_message = excluded.last_error_message,
                last_reason = excluded.last_reason,
                updated_at = excluded.updated_at",
            params![
                &attempt.feed_id,
                attempt.finished_at.to_rfc3339(),
                if attempt.success {
                    Some(attempt.finished_at.to_rfc3339())
                } else {
                    None::<String>
                },
                failure_count,
                attempt.next_attempt_at.map(|value| value.to_rfc3339()),
                attempt.http_status.map(|value| value as i64),
                &attempt.error_message,
                refresh_reason_to_str(attempt.reason),
                Utc::now().to_rfc3339(),
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Return the latest signature for an exact canonical key + fingerprint pair.
    pub fn get_notification_signature(
        &self,
        canonical_key: &str,
        content_fingerprint: &str,
    ) -> Result<Option<NotificationSignatureRow>, StorageError> {
        self.conn
            .query_row(
                "SELECT id, feed_id, entry_id, canonical_key, content_fingerprint,
                        first_seen_at, last_seen_at, last_notification_at
                 FROM notification_signatures
                 WHERE canonical_key = ?1
                   AND content_fingerprint = ?2
                 LIMIT 1",
                params![canonical_key, content_fingerprint],
                |row| {
                    Ok(NotificationSignatureRow {
                        id: row.get(0)?,
                        feed_id: row.get(1)?,
                        entry_id: row.get(2)?,
                        canonical_key: row.get(3)?,
                        content_fingerprint: row.get(4)?,
                        first_seen_at: row.get(5)?,
                        last_seen_at: row.get(6)?,
                        last_notification_at: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(StorageError::Sqlite)
    }

    /// Return the latest notification timestamp for a feed.
    pub fn get_latest_notification_at_for_feed(
        &self,
        feed_id: &str,
    ) -> Result<Option<DateTime<Utc>>, StorageError> {
        let value = self
            .conn
            .query_row(
                "SELECT last_notification_at
                 FROM notification_signatures
                 WHERE feed_id = ?1 AND last_notification_at IS NOT NULL
                 ORDER BY last_notification_at DESC
                 LIMIT 1",
                params![feed_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?;
        Ok(value.flatten().and_then(parse_datetime))
    }

    /// Upsert a hosted push endpoint registration.
    pub fn upsert_push_endpoint(
        &self,
        endpoint: &PushEndpointRegistration,
    ) -> Result<PushEndpointRegistration, StorageError> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO push_endpoints (id, platform, endpoint, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                platform = excluded.platform,
                endpoint = excluded.endpoint,
                enabled = excluded.enabled,
                updated_at = excluded.updated_at",
            params![
                &endpoint.id,
                &endpoint.platform,
                &endpoint.endpoint,
                bool_to_i64(endpoint.enabled),
                now,
                now,
            ],
        )?;
        Ok(endpoint.clone())
    }

    /// List registered push endpoints.
    pub fn list_push_endpoints(&self) -> Result<Vec<PushEndpointRegistration>, StorageError> {
        let mut statement = self.conn.prepare(
            "SELECT id, platform, endpoint, enabled, created_at, updated_at
             FROM push_endpoints
             ORDER BY created_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(PushEndpointRegistration {
                id: row.get(0)?,
                platform: row.get(1)?,
                endpoint: row.get(2)?,
                enabled: int_to_bool(row.get::<_, i64>(3)?),
                created_at: parse_datetime(row.get::<_, String>(4)?).unwrap_or_else(Utc::now),
                updated_at: parse_datetime(row.get::<_, String>(5)?).unwrap_or_else(Utc::now),
            })
        })?;
        let mut endpoints = Vec::new();
        for row in rows {
            endpoints.push(row?);
        }
        Ok(endpoints)
    }

    /// Delete a push endpoint registration by id.
    pub fn delete_push_endpoint(&self, endpoint_id: &str) -> Result<(), StorageError> {
        self.conn.execute("DELETE FROM push_endpoints WHERE id = ?1", params![endpoint_id])?;
        Ok(())
    }

    /// Store arbitrary app settings as JSON.
    pub fn set_app_setting_json<T: serde::Serialize>(
        &self,
        key: &str,
        value: &T,
    ) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        let value_json = serde_json::to_string(value)
            .map_err(|err| StorageError::Serialization(err.to_string()))?;
        self.conn.execute(
            "INSERT INTO app_settings (key, value_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![key, value_json, now],
        )?;
        Ok(())
    }

    /// Load an app setting stored as JSON.
    pub fn get_app_setting_json<T: DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<Option<T>, StorageError> {
        let value = self
            .conn
            .query_row("SELECT value_json FROM app_settings WHERE key = ?1", params![key], |row| {
                row.get::<_, String>(0)
            })
            .optional()?;
        match value {
            Some(value_json) => serde_json::from_str(&value_json)
                .map(Some)
                .map_err(|err| StorageError::Serialization(err.to_string())),
            None => Ok(None),
        }
    }

    /// Persist normalized RSS items by converting them into unified entries.
    pub fn upsert_items(&mut self, items: &[NormalizedItem]) -> Result<(), StorageError> {
        let entries: Vec<NewEntry> = items
            .iter()
            .map(|item| NewEntry {
                id: Some(item.id.clone()),
                kind: EntryKind::Article,
                source: EntrySource {
                    source_kind: EntrySourceKind::Feed,
                    source_id: Some(item.source_feed_id.clone()),
                    source_url: None,
                    source_title: None,
                },
                external_item_id: item.external_item_id.clone(),
                canonical_url: item.canonical_url.clone(),
                title: item.title.clone(),
                author: item.author.clone(),
                summary: item.summary.clone(),
                content_html: item.content_html.clone(),
                content_text: item.content_text.clone(),
                published_at: item.published_at,
                updated_at: item.updated_at,
                raw_hash: item.raw_hash.clone(),
                dedup_reason: item.dedup_reason.clone(),
                duplicate_of_entry_id: item.duplicate_of_item_id.clone(),
            })
            .collect();
        self.upsert_entries(&entries)
    }

    /// Persist unified entries and create default state rows if needed.
    pub fn upsert_entries(&mut self, entries: &[NewEntry]) -> Result<(), StorageError> {
        let tx = self.conn.transaction()?;
        let mut sync_events = Vec::new();

        for entry in entries {
            let now = Utc::now().to_rfc3339();
            let entry_id = entry.id.clone().unwrap_or_else(|| Uuid::now_v7().to_string());
            let existed = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM entries WHERE id = ?1)",
                params![&entry_id],
                |row| row.get::<_, i64>(0),
            )? != 0;

            tx.execute(
                "INSERT INTO entries (
                    id, kind, external_item_id, canonical_url, title, author, summary,
                    published_at, updated_at, raw_hash, dedup_reason, duplicate_of_entry_id,
                    created_at, row_updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                 ON CONFLICT(id) DO UPDATE SET
                   kind = excluded.kind,
                   external_item_id = excluded.external_item_id,
                   canonical_url = excluded.canonical_url,
                   title = excluded.title,
                   author = excluded.author,
                   summary = excluded.summary,
                   published_at = excluded.published_at,
                   updated_at = excluded.updated_at,
                   raw_hash = excluded.raw_hash,
                   dedup_reason = excluded.dedup_reason,
                   duplicate_of_entry_id = excluded.duplicate_of_entry_id,
                   row_updated_at = excluded.row_updated_at",
                params![
                    entry_id,
                    entry_kind_to_str(entry.kind),
                    entry.external_item_id.as_deref(),
                    entry.canonical_url.as_ref().map(|u| u.as_str()),
                    entry.title,
                    entry.author,
                    entry.summary,
                    entry.published_at.map(|v| v.to_rfc3339()),
                    entry.updated_at.map(|v| v.to_rfc3339()),
                    entry.raw_hash,
                    entry.dedup_reason.as_ref().map(|v| format!("{v:?}")),
                    entry.duplicate_of_entry_id,
                    now,
                    now,
                ],
            )?;

            tx.execute(
                "INSERT INTO entry_contents (entry_id, content_html, content_text, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(entry_id) DO UPDATE SET
                   content_html = excluded.content_html,
                   content_text = excluded.content_text,
                   updated_at = excluded.updated_at",
                params![entry_id, entry.content_html, entry.content_text, now, now],
            )?;
            tx.execute("DELETE FROM entry_search WHERE entry_id = ?1", params![entry_id])?;
            tx.execute(
                "INSERT INTO entry_search (entry_id, title, summary, content_text)
                 VALUES (?1, ?2, ?3, ?4)",
                params![entry_id, entry.title, entry.summary, entry.content_text],
            )?;

            tx.execute(
                "INSERT OR IGNORE INTO entry_states (
                    entry_id, is_read, is_starred, is_saved_for_later, is_archived,
                    read_at, starred_at, saved_for_later_at, created_at, updated_at
                 ) VALUES (?1, 0, 0, 0, 0, NULL, NULL, NULL, ?2, ?3)",
                params![entry_id, now, now],
            )?;

            tx.execute(
                "INSERT OR REPLACE INTO entry_sources (
                    id, entry_id, source_kind, source_id, source_url, source_title, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, COALESCE((SELECT created_at FROM entry_sources WHERE entry_id = ?2 LIMIT 1), ?7), ?7)",
                params![
                    format!("src_{entry_id}"),
                    entry_id,
                    entry_source_kind_to_str(entry.source.source_kind),
                    entry.source.source_id.as_deref(),
                    entry.source.source_url.as_ref().map(|v| v.as_str()),
                    entry.source.source_title.as_deref(),
                    now,
                ],
            )?;

            if entry.kind != EntryKind::Article || entry.source.source_kind != EntrySourceKind::Feed
            {
                let payload_json = serde_json::to_string(&NewEntry {
                    id: Some(entry_id.clone()),
                    ..entry.clone()
                })
                .map_err(|err| StorageError::Serialization(err.to_string()))?;
                sync_events.push((
                    "entry".to_owned(),
                    entry_id,
                    if existed { "updated" } else { "created" }.to_owned(),
                    payload_json,
                ));
            }
        }

        tx.commit()?;
        for (entity_type, entity_id, event_type, payload_json) in sync_events {
            self.append_sync_event_if_enabled(
                &entity_type,
                &entity_id,
                &event_type,
                &payload_json,
            )?;
        }
        Ok(())
    }

    /// Count items across the primary inbox scopes.
    pub fn count_item_scopes(&self) -> Result<ItemScopeCounts, StorageError> {
        self.conn
            .query_row(
                "SELECT
                    SUM(CASE WHEN COALESCE(s.is_archived, 0) = 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN COALESCE(s.is_read, 0) = 0 AND COALESCE(s.is_archived, 0) = 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN COALESCE(s.is_starred, 0) = 1 AND COALESCE(s.is_archived, 0) = 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN COALESCE(s.is_saved_for_later, 0) = 1 AND COALESCE(s.is_archived, 0) = 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN e.kind = 'note' AND COALESCE(s.is_archived, 0) = 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN COALESCE(s.is_archived, 0) = 1 THEN 1 ELSE 0 END)
                 FROM entries e
                 LEFT JOIN entry_states s ON s.entry_id = e.id",
                [],
                |row| {
                    Ok(ItemScopeCounts {
                        all: row.get::<_, Option<i64>>(0)?.unwrap_or(0).max(0) as usize,
                        unread: row.get::<_, Option<i64>>(1)?.unwrap_or(0).max(0) as usize,
                        starred: row.get::<_, Option<i64>>(2)?.unwrap_or(0).max(0) as usize,
                        later: row.get::<_, Option<i64>>(3)?.unwrap_or(0).max(0) as usize,
                        notes: row.get::<_, Option<i64>>(4)?.unwrap_or(0).max(0) as usize,
                        archive: row.get::<_, Option<i64>>(5)?.unwrap_or(0).max(0) as usize,
                    })
                },
            )
            .map_err(StorageError::Sqlite)
    }

    /// List items for a feed sorted by publish time.
    pub fn list_items_for_feed(
        &self,
        feed_id: &str,
        limit: usize,
        search_query: Option<&str>,
    ) -> Result<Vec<ItemSummaryRow>, StorageError> {
        self.query_items(Some(feed_id), ItemListFilter::All, limit, search_query, None)
    }

    /// List items across all feeds by filter.
    pub fn list_items_global(
        &self,
        limit: usize,
        search_query: Option<&str>,
        filter: ItemListFilter,
    ) -> Result<Vec<ItemSummaryRow>, StorageError> {
        self.query_items(None, filter, limit, search_query, None)
    }

    /// List items across all feeds by filter and content kind.
    pub fn list_items_global_with_kind(
        &self,
        limit: usize,
        search_query: Option<&str>,
        filter: ItemListFilter,
        kind: Option<EntryKind>,
    ) -> Result<Vec<ItemSummaryRow>, StorageError> {
        self.query_items(None, filter, limit, search_query, kind)
    }

    fn query_items(
        &self,
        feed_id: Option<&str>,
        filter: ItemListFilter,
        limit: usize,
        search_query: Option<&str>,
        kind: Option<EntryKind>,
    ) -> Result<Vec<ItemSummaryRow>, StorageError> {
        let pattern = to_fts_query(search_query);
        let filter_label = match filter {
            ItemListFilter::All => "all",
            ItemListFilter::Unread => "unread",
            ItemListFilter::Starred => "starred",
            ItemListFilter::Later => "later",
            ItemListFilter::Archive => "archive",
        };
        let kind_label = kind.map(entry_kind_to_str);
        let mut statement = self.conn.prepare(
            "SELECT e.id,
                    e.kind,
                    src.source_kind,
                    src.source_id,
                    src.source_url,
                    e.title,
                    e.canonical_url,
                    e.published_at,
                    COALESCE(NULLIF(TRIM(e.summary), ''), NULLIF(TRIM(c.content_text), '')),
                    s.is_read,
                    s.is_starred,
                    s.is_saved_for_later,
                    s.is_archived
             FROM entries e
             LEFT JOIN entry_contents c ON c.entry_id = e.id
             LEFT JOIN entry_states s ON s.entry_id = e.id
             LEFT JOIN entry_sources src ON src.entry_id = e.id
             WHERE (
                    (?1 IS NULL OR (src.source_kind = 'feed' AND src.source_id = ?1))
                    AND (
                         (?2 = 'all' AND COALESCE(s.is_archived, 0) = 0)
                      OR (?2 = 'unread' AND COALESCE(s.is_read, 0) = 0 AND COALESCE(s.is_archived, 0) = 0)
                      OR (?2 = 'starred' AND COALESCE(s.is_starred, 0) = 1 AND COALESCE(s.is_archived, 0) = 0)
                      OR (?2 = 'later' AND COALESCE(s.is_saved_for_later, 0) = 1 AND COALESCE(s.is_archived, 0) = 0)
                      OR (?2 = 'archive' AND COALESCE(s.is_archived, 0) = 1)
                    )
               )
               AND (?4 IS NULL OR e.kind = ?4)
               AND (
                 ?3 IS NULL
                 OR EXISTS(
                    SELECT 1
                    FROM entry_search
                    WHERE entry_search.entry_id = e.id
                      AND entry_search MATCH ?3
                 )
               )
             ORDER BY e.published_at DESC NULLS LAST, e.row_updated_at DESC
             LIMIT ?5",
        )?;

        let rows = statement.query_map(
            params![feed_id, filter_label, pattern, kind_label, limit as i64],
            map_item_summary_row,
        )?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    }

    /// Get full detail payload for one item.
    pub fn get_item_detail(&self, item_id: &str) -> Result<ItemDetailRow, StorageError> {
        self.conn
            .query_row(
                "SELECT e.id,
                        e.kind,
                        src.source_kind,
                        src.source_id,
                        src.source_url,
                        e.title,
                        e.canonical_url,
                        e.published_at,
                        e.summary,
                        c.content_html,
                        c.content_text,
                        s.is_read,
                        s.is_starred,
                        s.is_saved_for_later,
                        s.is_archived
                 FROM entries e
                 LEFT JOIN entry_contents c ON c.entry_id = e.id
                 LEFT JOIN entry_states s ON s.entry_id = e.id
                 LEFT JOIN entry_sources src ON src.entry_id = e.id
                 WHERE e.id = ?1",
                params![item_id],
                |row| {
                    let canonical_url: Option<String> = row.get(6)?;
                    let source_url: Option<String> = row.get(4)?;
                    Ok(ItemDetailRow {
                        id: row.get(0)?,
                        kind: entry_kind_from_str(&row.get::<_, String>(1)?),
                        source_kind: entry_source_kind_from_str(&row.get::<_, String>(2)?),
                        source_id: row.get(3)?,
                        source_url: source_url.and_then(|value| Url::parse(&value).ok()),
                        title: row.get(5)?,
                        canonical_url: canonical_url.and_then(|value| Url::parse(&value).ok()),
                        published_at: row.get(7)?,
                        summary: row.get(8)?,
                        content_html: row.get(9)?,
                        content_text: row.get(10)?,
                        is_read: int_to_bool(row.get::<_, Option<i64>>(11)?.unwrap_or(0)),
                        is_starred: int_to_bool(row.get::<_, Option<i64>>(12)?.unwrap_or(0)),
                        is_saved_for_later: int_to_bool(
                            row.get::<_, Option<i64>>(13)?.unwrap_or(0),
                        ),
                        is_archived: int_to_bool(row.get::<_, Option<i64>>(14)?.unwrap_or(0)),
                    })
                },
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound,
                _ => StorageError::Sqlite(err),
            })
    }

    /// Upsert detail content payload for an item.
    pub fn upsert_item_content(
        &mut self,
        item_id: &str,
        content_html: Option<&str>,
        content_text: Option<&str>,
    ) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO entry_contents (entry_id, content_html, content_text, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(entry_id) DO UPDATE SET
               content_html = COALESCE(?2, entry_contents.content_html),
               content_text = COALESCE(?3, entry_contents.content_text),
               updated_at = ?5",
            params![item_id, content_html, content_text, now, now],
        )?;
        tx.execute("UPDATE entries SET row_updated_at = ?1 WHERE id = ?2", params![now, item_id])?;
        tx.execute("DELETE FROM entry_search WHERE entry_id = ?1", params![item_id])?;
        tx.execute(
            "INSERT INTO entry_search (entry_id, title, summary, content_text)
             SELECT e.id, e.title, e.summary, COALESCE(?1, c.content_text)
             FROM entries e
             LEFT JOIN entry_contents c ON c.entry_id = e.id
             WHERE e.id = ?2",
            params![content_text, item_id],
        )?;
        tx.commit()?;
        if self.is_syncable_user_entry(item_id)? {
            let payload_json = json!({
                "entry_id": item_id,
                "content_html": content_html,
                "content_text": content_text,
            })
            .to_string();
            self.append_sync_event_if_enabled("entry_content", item_id, "updated", &payload_json)?;
        }
        Ok(())
    }

    /// Update fetch metadata and append fetch log in one transaction.
    pub fn record_fetch_result(
        &mut self,
        feed_id: &str,
        http_status: u16,
        etag: Option<&str>,
        last_modified: Option<&str>,
        duration_ms: u128,
        error_message: Option<&str>,
    ) -> Result<(), StorageError> {
        let now = Utc::now();
        let now_rfc3339 = now.to_rfc3339();
        let success = (200..400).contains(&(http_status as i32)) && error_message.is_none();
        let current_feed = self.get_feed(feed_id)?;
        let refresh_settings = self.resolve_effective_refresh_settings(feed_id)?;
        let failure_count = if success { 0 } else { current_feed.failure_count.saturating_add(1) };
        let health_state = if success {
            FeedHealthState::Healthy
        } else if failure_count >= 3 {
            FeedHealthState::Failing
        } else {
            FeedHealthState::Stale
        };
        let next_scheduled_fetch_at = if refresh_settings.enabled {
            Some(
                compute_next_scheduled_fetch(
                    now,
                    current_feed.last_fetch_at.is_none(),
                    health_state,
                    failure_count as u32,
                    refresh_settings.interval_minutes.max(1),
                )
                .to_rfc3339(),
            )
        } else {
            None
        };

        let tx = self.conn.transaction()?;

        tx.execute(
            "UPDATE feeds
             SET last_fetch_at = ?1,
                 last_success_at = CASE WHEN ?2 THEN ?1 ELSE last_success_at END,
                 last_http_status = ?3,
                 failure_count = ?4,
                 next_scheduled_fetch_at = ?5,
                 avg_response_time_ms = ?6,
                 health_state = ?7,
                 etag = COALESCE(?8, etag),
                 last_modified = COALESCE(?9, last_modified),
                 updated_at = ?1
             WHERE id = ?10",
            params![
                now_rfc3339,
                success,
                http_status as i64,
                failure_count,
                next_scheduled_fetch_at,
                duration_ms as i64,
                feed_health_state_to_str(health_state),
                etag,
                last_modified,
                feed_id,
            ],
        )?;

        tx.execute(
            "INSERT INTO fetch_logs (
                id, feed_id, fetched_at, duration_ms, http_status, error_message, etag, last_modified
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                Uuid::now_v7().to_string(),
                feed_id,
                now_rfc3339,
                duration_ms as i64,
                http_status as i64,
                error_message,
                etag,
                last_modified,
            ],
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Update item state flags with timestamp semantics.
    pub fn patch_item_state(
        &self,
        item_id: &str,
        patch: &ItemStatePatch,
    ) -> Result<ItemState, StorageError> {
        let mut current = self.get_item_state(item_id)?;
        let previous = current.clone();
        let now = Utc::now();

        if let Some(value) = patch.is_read {
            if current.is_read != value {
                current.is_read = value;
                current.read_at = if value { Some(now) } else { None };
            }
        }
        if let Some(value) = patch.is_starred {
            if current.is_starred != value {
                current.is_starred = value;
                current.starred_at = if value { Some(now) } else { None };
            }
        }
        if let Some(value) = patch.is_saved_for_later {
            if current.is_saved_for_later != value {
                current.is_saved_for_later = value;
                current.saved_for_later_at = if value { Some(now) } else { None };
            }
        }
        if let Some(value) = patch.is_archived {
            if current.is_archived != value {
                current.is_archived = value;
            }
        }

        let state_changed = previous != current;
        if !state_changed {
            return Ok(current);
        }
        let updated_at = now.to_rfc3339();
        self.conn.execute(
            "UPDATE entry_states
             SET is_read = ?1,
                 is_starred = ?2,
                 is_saved_for_later = ?3,
                 is_archived = ?4,
                 read_at = ?5,
                 starred_at = ?6,
                 saved_for_later_at = ?7,
                 updated_at = ?8
             WHERE entry_id = ?9",
            params![
                bool_to_i64(current.is_read),
                bool_to_i64(current.is_starred),
                bool_to_i64(current.is_saved_for_later),
                bool_to_i64(current.is_archived),
                current.read_at.as_ref().map(|v| v.to_rfc3339()),
                current.starred_at.as_ref().map(|v| v.to_rfc3339()),
                current.saved_for_later_at.as_ref().map(|v| v.to_rfc3339()),
                updated_at,
                item_id,
            ],
        )?;

        if state_changed {
            let sync_identity = self.get_entry_sync_identity(item_id)?;
            let payload_json = json!({
                "item_id": item_id,
                "canonical_url": sync_identity.as_ref().and_then(|value| value.canonical_url.clone()),
                "external_item_id": sync_identity.as_ref().and_then(|value| value.external_item_id.clone()),
                "is_read": current.is_read,
                "is_starred": current.is_starred,
                "is_saved_for_later": current.is_saved_for_later,
                "is_archived": current.is_archived,
                "updated_at": updated_at,
            })
            .to_string();
            self.append_sync_event_if_enabled("item_state", item_id, "updated", &payload_json)?;
        }

        self.get_item_state(item_id)
    }

    /// Apply a batch of remote sync events to the local database.
    pub fn apply_sync_events(&mut self, events: &[SyncEventRecord]) -> Result<usize, StorageError> {
        if events.is_empty() {
            return Ok(0);
        }

        let _guard = self.suppress_sync_events();
        let mut applied = 0usize;

        for event in events {
            match event.entity_type.as_str() {
                "feed" => {
                    let payload: SyncFeedPayload = serde_json::from_str(&event.payload_json)
                        .map_err(|err| StorageError::Serialization(err.to_string()))?;
                    if event.event_type == "deleted" {
                        if let Some(feed_id) = self.resolve_feed_id_by_url(&payload.feed_url)? {
                            self.delete_feed(&feed_id)?;
                            applied += 1;
                        }
                        continue;
                    }

                    let feed_id = self.upsert_feed(&NewFeed {
                        feed_url: Url::parse(&payload.feed_url)
                            .map_err(|err| StorageError::Serialization(err.to_string()))?,
                        site_url: payload
                            .site_url
                            .as_deref()
                            .and_then(|value| Url::parse(value).ok()),
                        title: payload.title.clone(),
                        feed_type: payload.feed_type,
                    })?;
                    if let Some(auto_full_text) = payload.auto_full_text {
                        self.update_feed_auto_full_text(&feed_id, auto_full_text)?;
                    }
                    applied += 1;
                }
                "feed_group" => {
                    let payload: SyncFeedGroupPayload =
                        serde_json::from_str(&event.payload_json)
                            .map_err(|err| StorageError::Serialization(err.to_string()))?;
                    if event.event_type == "deleted" {
                        if let Some(group_id) = self.resolve_group_id_by_name(&payload.name)? {
                            self.delete_group(&group_id)?;
                            applied += 1;
                        }
                        continue;
                    }
                    let _ = self.create_group(&payload.name)?;
                    applied += 1;
                }
                "feed_membership" => {
                    let payload: SyncFeedMembershipPayload =
                        serde_json::from_str(&event.payload_json)
                            .map_err(|err| StorageError::Serialization(err.to_string()))?;
                    let feed_id =
                        if let Some(feed_id) = self.resolve_feed_id_by_url(&payload.feed_url)? {
                            feed_id
                        } else {
                            self.upsert_feed(&NewFeed {
                                feed_url: Url::parse(&payload.feed_url)
                                    .map_err(|err| StorageError::Serialization(err.to_string()))?,
                                site_url: None,
                                title: None,
                                feed_type: FeedType::Unknown,
                            })?
                        };
                    let group_id = if let Some(group_name) = payload.group_name.as_deref() {
                        Some(self.create_group(group_name)?.id)
                    } else {
                        None
                    };
                    self.set_feed_group(&feed_id, group_id.as_deref())?;
                    applied += 1;
                }
                "entry" => {
                    let entry: NewEntry = serde_json::from_str(&event.payload_json)
                        .map_err(|err| StorageError::Serialization(err.to_string()))?;
                    if event.event_type == "deleted" {
                        if let Some(entry_id) =
                            entry.id.as_deref().or(Some(event.entity_id.as_str()))
                        {
                            self.delete_entry(entry_id)?;
                            applied += 1;
                        }
                        continue;
                    }
                    self.upsert_entries(&[entry])?;
                    applied += 1;
                }
                "entry_content" => {
                    let payload: serde_json::Value = serde_json::from_str(&event.payload_json)
                        .map_err(|err| StorageError::Serialization(err.to_string()))?;
                    let entry_id = payload
                        .get("entry_id")
                        .and_then(|value| value.as_str())
                        .unwrap_or(event.entity_id.as_str());
                    self.upsert_item_content(
                        entry_id,
                        payload.get("content_html").and_then(|value| value.as_str()),
                        payload.get("content_text").and_then(|value| value.as_str()),
                    )?;
                    applied += 1;
                }
                "item_state" => {
                    let payload: SyncItemStatePayload =
                        serde_json::from_str(&event.payload_json)
                            .map_err(|err| StorageError::Serialization(err.to_string()))?;
                    let entry_id = self.resolve_entry_id_by_identity(
                        payload.item_id.as_deref(),
                        payload.canonical_url.as_deref(),
                        payload.external_item_id.as_deref(),
                    )?;
                    if let Some(entry_id) = entry_id {
                        self.patch_item_state(
                            &entry_id,
                            &ItemStatePatch {
                                is_read: Some(payload.is_read),
                                is_starred: Some(payload.is_starred),
                                is_saved_for_later: Some(payload.is_saved_for_later),
                                is_archived: Some(payload.is_archived),
                            },
                        )?;
                        applied += 1;
                    }
                }
                "notification_settings" => {
                    let payload: SyncFeedNotificationSettingsPayload =
                        serde_json::from_str(&event.payload_json)
                            .map_err(|err| StorageError::Serialization(err.to_string()))?;
                    let feed_id =
                        if let Some(feed_id) = self.resolve_feed_id_by_url(&payload.feed_url)? {
                            feed_id
                        } else {
                            self.upsert_feed(&NewFeed {
                                feed_url: Url::parse(&payload.feed_url)
                                    .map_err(|err| StorageError::Serialization(err.to_string()))?,
                                site_url: None,
                                title: None,
                                feed_type: FeedType::Unknown,
                            })?
                        };
                    let _ = self.upsert_notification_settings(&feed_id, &payload.settings)?;
                    applied += 1;
                }
                "notification_globals" => {
                    let payload: SyncGlobalNotificationSettingsPayload =
                        serde_json::from_str(&event.payload_json)
                            .map_err(|err| StorageError::Serialization(err.to_string()))?;
                    self.set_global_notification_settings(&payload.settings)?;
                    applied += 1;
                }
                "feed_refresh_settings" => {
                    let payload: SyncFeedRefreshSettingsPayload =
                        serde_json::from_str(&event.payload_json)
                            .map_err(|err| StorageError::Serialization(err.to_string()))?;
                    let feed_id =
                        if let Some(feed_id) = self.resolve_feed_id_by_url(&payload.feed_url)? {
                            feed_id
                        } else if event.event_type == "deleted" {
                            continue;
                        } else {
                            self.upsert_feed(&NewFeed {
                                feed_url: Url::parse(&payload.feed_url)
                                    .map_err(|err| StorageError::Serialization(err.to_string()))?,
                                site_url: None,
                                title: None,
                                feed_type: FeedType::Unknown,
                            })?
                        };
                    if event.event_type == "deleted" {
                        self.conn.execute(
                            "DELETE FROM feed_refresh_settings WHERE feed_id = ?1",
                            params![feed_id],
                        )?;
                    } else {
                        let _ = self.upsert_feed_refresh_settings(&feed_id, &payload.settings)?;
                    }
                    applied += 1;
                }
                "group_refresh_settings" => {
                    let payload: SyncGroupRefreshSettingsPayload =
                        serde_json::from_str(&event.payload_json)
                            .map_err(|err| StorageError::Serialization(err.to_string()))?;
                    if event.event_type == "deleted" {
                        if let Some(group_id) =
                            self.resolve_group_id_by_name(&payload.group_name)?
                        {
                            self.conn.execute(
                                "DELETE FROM group_refresh_settings WHERE group_id = ?1",
                                params![group_id],
                            )?;
                        }
                    } else {
                        let group_id = self.create_group(&payload.group_name)?.id;
                        let _ = self.upsert_group_refresh_settings(&group_id, &payload.settings)?;
                    }
                    applied += 1;
                }
                _ => {}
            }
        }

        Ok(applied)
    }

    /// Return pending sync events ordered by creation timestamp.
    pub fn list_pending_sync_events(
        &self,
        limit: usize,
    ) -> Result<Vec<SyncEventRow>, StorageError> {
        let mut statement = self.conn.prepare(
            "SELECT id, entity_type, entity_id, event_type, payload_json, created_at, processed_at
             FROM sync_events
             WHERE processed_at IS NULL
             ORDER BY created_at ASC
             LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit as i64], |row| {
            Ok(SyncEventRow {
                id: row.get(0)?,
                entity_type: row.get(1)?,
                entity_id: row.get(2)?,
                event_type: row.get(3)?,
                payload_json: row.get(4)?,
                created_at: row.get(5)?,
                processed_at: row.get(6)?,
            })
        })?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    /// Mark sync events as processed and return number of rows acknowledged.
    pub fn acknowledge_sync_events(&self, event_ids: &[String]) -> Result<usize, StorageError> {
        if event_ids.is_empty() {
            return Ok(0);
        }

        let processed_at = Utc::now().to_rfc3339();
        let mut affected = 0usize;
        for event_id in event_ids {
            affected += self.conn.execute(
                "UPDATE sync_events
                 SET processed_at = ?1
                 WHERE id = ?2
                   AND processed_at IS NULL",
                params![processed_at, event_id],
            )?;
        }
        Ok(affected)
    }

    /// Fetch item state by item id.
    pub fn get_item_state(&self, item_id: &str) -> Result<ItemState, StorageError> {
        self.conn
            .query_row(
                "SELECT entry_id, is_read, is_starred, is_saved_for_later, is_archived,
                        read_at, starred_at, saved_for_later_at
                 FROM entry_states WHERE entry_id = ?1",
                params![item_id],
                |row| {
                    Ok(ItemState {
                        item_id: row.get(0)?,
                        is_read: int_to_bool(row.get::<_, i64>(1)?),
                        is_starred: int_to_bool(row.get::<_, i64>(2)?),
                        is_saved_for_later: int_to_bool(row.get::<_, i64>(3)?),
                        is_archived: int_to_bool(row.get::<_, i64>(4)?),
                        read_at: row.get::<_, Option<String>>(5)?.and_then(parse_datetime),
                        starred_at: row.get::<_, Option<String>>(6)?.and_then(parse_datetime),
                        saved_for_later_at: row
                            .get::<_, Option<String>>(7)?
                            .and_then(parse_datetime),
                    })
                },
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound,
                _ => StorageError::Sqlite(err),
            })
    }

    /// List item ids for a feed sorted by published timestamp desc.
    pub fn list_item_ids_for_feed(
        &self,
        feed_id: &str,
        limit: usize,
    ) -> Result<Vec<String>, StorageError> {
        let mut statement = self.conn.prepare(
            "SELECT e.id FROM entries e
             INNER JOIN entry_sources src ON src.entry_id = e.id
             WHERE src.source_kind = 'feed'
               AND src.source_id = ?1
               AND e.kind = 'article'
             ORDER BY e.published_at DESC NULLS LAST, e.row_updated_at DESC
             LIMIT ?2",
        )?;
        let rows = statement.query_map(params![feed_id, limit as i64], |row| row.get(0))?;
        let mut output = Vec::new();
        for row in rows {
            output.push(row?);
        }
        Ok(output)
    }

    fn append_sync_event(
        &self,
        entity_type: &str,
        entity_id: &str,
        event_type: &str,
        payload_json: &str,
    ) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO sync_events (
                id, entity_type, entity_id, event_type, payload_json, created_at, processed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
            params![
                Uuid::now_v7().to_string(),
                entity_type,
                entity_id,
                event_type,
                payload_json,
                now
            ],
        )?;
        Ok(())
    }
}

fn compute_next_scheduled_fetch(
    now: chrono::DateTime<chrono::Utc>,
    is_new_feed: bool,
    health_state: FeedHealthState,
    failure_count: u32,
    interval_minutes: u32,
) -> chrono::DateTime<chrono::Utc> {
    let base_minutes = interval_minutes.max(1);
    let base = if is_new_feed {
        chrono::Duration::minutes(base_minutes as i64)
    } else {
        match health_state {
            FeedHealthState::Healthy => chrono::Duration::minutes(base_minutes as i64),
            FeedHealthState::Stale => {
                chrono::Duration::minutes(base_minutes.saturating_mul(3) as i64)
            }
            FeedHealthState::Failing => {
                chrono::Duration::minutes(base_minutes.saturating_mul(6) as i64)
            }
        }
    };
    let multiplier = 2u32.saturating_pow(failure_count.min(6));
    let capped_seconds =
        (base.num_seconds() as u64).saturating_mul(multiplier as u64).min(24 * 60 * 60);
    now + chrono::Duration::seconds(capped_seconds as i64)
}

fn feed_health_state_to_str(value: FeedHealthState) -> &'static str {
    match value {
        FeedHealthState::Healthy => "healthy",
        FeedHealthState::Stale => "stale",
        FeedHealthState::Failing => "failing",
    }
}

fn feed_health_state_from_str(value: &str) -> FeedHealthState {
    match value {
        "stale" => FeedHealthState::Stale,
        "failing" => FeedHealthState::Failing,
        _ => FeedHealthState::Healthy,
    }
}

fn entry_kind_to_str(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Article => "article",
        EntryKind::Bookmark => "bookmark",
        EntryKind::Note => "note",
        EntryKind::Quote => "quote",
    }
}

fn entry_kind_from_str(value: &str) -> EntryKind {
    match value {
        "bookmark" => EntryKind::Bookmark,
        "note" => EntryKind::Note,
        "quote" => EntryKind::Quote,
        _ => EntryKind::Article,
    }
}

fn entry_source_kind_to_str(kind: EntrySourceKind) -> &'static str {
    match kind {
        EntrySourceKind::Feed => "feed",
        EntrySourceKind::Web => "web",
        EntrySourceKind::Manual => "manual",
        EntrySourceKind::Import => "import",
        EntrySourceKind::Sync => "sync",
    }
}

fn entry_source_kind_from_str(value: &str) -> EntrySourceKind {
    match value {
        "web" => EntrySourceKind::Web,
        "manual" => EntrySourceKind::Manual,
        "import" => EntrySourceKind::Import,
        "sync" => EntrySourceKind::Sync,
        _ => EntrySourceKind::Feed,
    }
}

fn bool_to_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

fn to_fts_query(search_query: Option<&str>) -> Option<String> {
    let trimmed = search_query.map(str::trim).filter(|query| !query.is_empty())?;
    let tokens = trimmed
        .split_whitespace()
        .map(|token| {
            let sanitized = token
                .chars()
                .filter(|ch| !ch.is_control())
                .collect::<String>()
                .replace('"', "\"\"");
            format!("\"{sanitized}\"")
        })
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.is_empty() { None } else { Some(tokens.join(" AND ")) }
}

#[cfg(test)]
mod search_tests {
    use super::to_fts_query;

    #[test]
    fn quotes_tokens_with_punctuation() {
        assert_eq!(
            to_fts_query(Some("blog.samaltman.com/posts.atom")),
            Some("\"blog.samaltman.com/posts.atom\"".to_owned())
        );
    }

    #[test]
    fn joins_multiple_tokens_safely() {
        assert_eq!(to_fts_query(Some("Sam Altman")), Some("\"Sam\" AND \"Altman\"".to_owned()));
    }
}

fn map_item_summary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ItemSummaryRow> {
    let canonical_url: Option<String> = row.get(6)?;
    let source_url: Option<String> = row.get(4)?;
    Ok(ItemSummaryRow {
        id: row.get(0)?,
        kind: entry_kind_from_str(&row.get::<_, String>(1)?),
        source_kind: entry_source_kind_from_str(&row.get::<_, String>(2)?),
        source_id: row.get(3)?,
        source_url: source_url.and_then(|value| Url::parse(&value).ok()),
        title: row.get(5)?,
        canonical_url: canonical_url.and_then(|value| Url::parse(&value).ok()),
        published_at: row.get(7)?,
        summary_preview: row.get(8)?,
        is_read: int_to_bool(row.get::<_, Option<i64>>(9)?.unwrap_or(0)),
        is_starred: int_to_bool(row.get::<_, Option<i64>>(10)?.unwrap_or(0)),
        is_saved_for_later: int_to_bool(row.get::<_, Option<i64>>(11)?.unwrap_or(0)),
        is_archived: int_to_bool(row.get::<_, Option<i64>>(12)?.unwrap_or(0)),
    })
}

fn int_to_bool(value: i64) -> bool {
    value != 0
}

fn parse_datetime(value: String) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(&value).map(|v| v.with_timezone(&chrono::Utc)).ok()
}

fn feed_type_to_str(feed_type: FeedType) -> &'static str {
    match feed_type {
        FeedType::Rss => "rss",
        FeedType::Atom => "atom",
        FeedType::JsonFeed => "jsonfeed",
        FeedType::Unknown => "unknown",
    }
}

fn feed_type_from_str(value: &str) -> FeedType {
    match value {
        "rss" => FeedType::Rss,
        "atom" => FeedType::Atom,
        "jsonfeed" => FeedType::JsonFeed,
        _ => FeedType::Unknown,
    }
}

fn notification_mode_to_str(mode: NotificationMode) -> &'static str {
    match mode {
        NotificationMode::Immediate => "immediate",
        NotificationMode::Digest => "digest",
    }
}

fn notification_mode_from_str(value: &str) -> NotificationMode {
    match value {
        "digest" => NotificationMode::Digest,
        _ => NotificationMode::Immediate,
    }
}

fn notification_delivery_state_to_str(value: NotificationDeliveryState) -> &'static str {
    match value {
        NotificationDeliveryState::Pending => "pending",
        NotificationDeliveryState::Delivered => "delivered",
        NotificationDeliveryState::Suppressed => "suppressed",
    }
}

fn notification_delivery_state_from_str(value: &str) -> NotificationDeliveryState {
    match value {
        "delivered" => NotificationDeliveryState::Delivered,
        "suppressed" => NotificationDeliveryState::Suppressed,
        _ => NotificationDeliveryState::Pending,
    }
}

fn refresh_reason_to_str(reason: notifications::RefreshReason) -> &'static str {
    match reason {
        notifications::RefreshReason::Periodic => "periodic",
        notifications::RefreshReason::CatchUp => "catch_up",
        notifications::RefreshReason::Manual => "manual",
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use models::{
        DigestPolicy, FeedHealthState, FeedType, GlobalNotificationSettings, ItemStatePatch,
        NewEntry, NewFeed, NormalizedItem, NotificationDeliveryState, NotificationDigest,
        NotificationEvent, NotificationMode, NotificationSettings, QuietHours, RefreshSettings,
    };
    use rusqlite::params;
    use serde_json::Value;
    use url::Url;
    use uuid::Uuid;

    use super::{Storage, SyncEventRecord};

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

    #[test]
    fn migrates_and_persists_item_state() {
        let mut storage = Storage::open_in_memory().expect("open");
        storage.migrate().expect("migrate");

        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: Url::parse("https://example.com/feed.xml").expect("url parse"),
                site_url: Some(Url::parse("https://example.com").expect("url parse")),
                title: Some("Example".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("upsert feed");

        storage.upsert_items(&[sample_item(&feed_id, "item-1")]).expect("insert item");

        let updated = storage
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

        assert!(updated.is_read);
        assert!(updated.is_starred);
        assert!(updated.is_saved_for_later);
        assert!(updated.read_at.is_some());
        assert!(updated.starred_at.is_some());
        assert!(updated.saved_for_later_at.is_some());

        let ids = storage.list_item_ids_for_feed(&feed_id, 10).expect("query ids");
        assert_eq!(ids, vec!["item-1".to_owned()]);

        let feeds = storage.list_feeds().expect("list feeds");
        assert_eq!(feeds.len(), 1);

        storage
            .record_fetch_result(&feed_id, 200, Some("etag-1"), None, 42, None)
            .expect("record fetch");

        let items = storage.list_items_for_feed(&feed_id, 10, None).expect("list item summaries");
        assert_eq!(items.len(), 1);
        assert!(items[0].is_read);

        let detail = storage.get_item_detail("item-1").expect("get item detail");
        assert_eq!(detail.id, "item-1");
        assert_eq!(detail.content_text.as_deref(), Some("content"));
    }

    #[test]
    fn deletes_feed_and_cascades_items() {
        let mut storage = Storage::open_in_memory().expect("open");
        storage.migrate().expect("migrate");

        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: Url::parse("https://example.com/feed.xml").expect("url parse"),
                site_url: Some(Url::parse("https://example.com").expect("url parse")),
                title: Some("Example".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("upsert feed");

        storage.upsert_items(&[sample_item(&feed_id, "item-1")]).expect("insert item");
        storage.delete_feed(&feed_id).expect("delete feed");

        let feeds = storage.list_feeds().expect("list feeds");
        assert!(feeds.is_empty());

        let ids = storage.list_item_ids_for_feed(&feed_id, 10).expect("query ids");
        assert!(ids.is_empty());
    }

    #[test]
    fn migrate_is_idempotent_for_notification_settings_column() {
        let storage = Storage::open_in_memory().expect("open");
        storage.migrate().expect("first migrate");
        storage.migrate().expect("second migrate");

        let has_column: i64 = storage
            .conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('notification_settings')
                 WHERE name = 'digest_max_items'",
                [],
                |row| row.get(0),
            )
            .expect("digest_max_items column");
        assert_eq!(has_column, 1);
    }

    #[test]
    fn parse_failure_result_counts_as_fetch_failure() {
        let mut storage = Storage::open_in_memory().expect("open");
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
            .record_fetch_result(
                &feed_id,
                200,
                Some("etag-1"),
                None,
                30,
                Some("feed parse failed: unsupported format"),
            )
            .expect("record failed fetch");

        let feed = storage.get_feed(&feed_id).expect("get feed");
        assert_eq!(feed.etag.as_deref(), Some("etag-1"));

        let conn = &storage.conn;
        let failure_count: i64 = conn
            .query_row("SELECT failure_count FROM feeds WHERE id = ?1", params![feed_id], |row| {
                row.get(0)
            })
            .expect("failure_count");
        assert_eq!(failure_count, 1);
    }

    #[test]
    fn due_feed_listing_tracks_schedule_and_reschedules_after_fetch() {
        let mut storage = Storage::open_in_memory().expect("open");
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
            .conn
            .execute(
                "UPDATE feeds SET next_scheduled_fetch_at = '2000-01-01T00:00:00Z' WHERE id = ?1",
                params![feed_id],
            )
            .expect("make feed due");

        let due_before = storage.list_due_feeds(10).expect("list due feeds");
        assert_eq!(due_before.len(), 1);
        assert_eq!(due_before[0].id, feed_id);

        storage
            .record_fetch_result(&feed_id, 200, Some("etag-1"), None, 42, None)
            .expect("record fetch");

        let due_after = storage.list_due_feeds(10).expect("list due feeds");
        assert!(due_after.is_empty());

        let feed = storage.get_feed(&feed_id).expect("get feed");
        assert_eq!(feed.health_state, FeedHealthState::Healthy);
        assert_eq!(feed.failure_count, 0);
        assert_eq!(feed.last_http_status, Some(200));
        assert!(feed.last_success_at.is_some());
        assert!(feed.next_scheduled_fetch_at.is_some());
    }

    #[test]
    fn stores_and_reads_feed_icon_source_url() {
        let mut storage = Storage::open_in_memory().expect("open");
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
            .set_feed_icon(&feed_id, "https://example.com/static/favicon.png")
            .expect("set icon");

        let icon = storage.get_feed_icon(&feed_id).expect("read icon").expect("icon exists");
        assert_eq!(icon.feed_id, feed_id);
        assert_eq!(icon.source_url, "https://example.com/static/favicon.png");
    }

    #[test]
    fn upsert_feed_preserves_existing_site_url_when_refresh_has_none() {
        let storage = Storage::open_in_memory().expect("open");
        storage.migrate().expect("migrate");

        let feed_url = Url::parse("https://example.com/feed.xml").expect("url parse");
        let initial_site_url = Url::parse("https://example.com").expect("url parse");
        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: feed_url.clone(),
                site_url: Some(initial_site_url.clone()),
                title: Some("Example".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("upsert feed");

        let returned_id = storage
            .upsert_feed(&NewFeed {
                feed_url,
                site_url: None,
                title: Some("Renamed".to_owned()),
                feed_type: FeedType::Atom,
            })
            .expect("refresh feed");

        assert_eq!(returned_id, feed_id);
        let feed = storage.get_feed(&feed_id).expect("get feed");
        assert_eq!(feed.site_url.as_ref(), Some(&initial_site_url));
        assert_eq!(feed.title.as_deref(), Some("Renamed"));
        assert_eq!(feed.feed_type, FeedType::Atom);
    }

    #[test]
    fn upsert_item_content_refreshes_search_index() {
        let mut storage = Storage::open_in_memory().expect("open");
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
            .upsert_items(&[NormalizedItem {
                id: "item-1".to_owned(),
                source_feed_id: feed_id.clone(),
                external_item_id: Some("ext-1".to_owned()),
                canonical_url: Some(Url::parse("https://example.com/post").expect("url parse")),
                title: "Searchable title".to_owned(),
                author: None,
                summary: Some("Short summary".to_owned()),
                content_html: Some("<p>old body</p>".to_owned()),
                content_text: Some("old body".to_owned()),
                published_at: None,
                updated_at: None,
                raw_hash: "abc".to_owned(),
                dedup_reason: None,
                duplicate_of_item_id: None,
            }])
            .expect("upsert items");

        let before = storage
            .list_items_for_feed(&feed_id, 10, Some("needle"))
            .expect("search before update");
        assert!(before.is_empty());

        storage
            .upsert_item_content(
                "item-1",
                None,
                Some("This refreshed body contains the needle keyword"),
            )
            .expect("upsert item content");

        let after =
            storage.list_items_for_feed(&feed_id, 10, Some("needle")).expect("search after update");
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].id, "item-1");

        let detail = storage.get_item_detail("item-1").expect("item detail");
        assert!(detail.content_text.as_deref().expect("content text").contains("needle"));
    }

    #[test]
    fn discovery_cache_skips_expired_rows() {
        let storage = Storage::open_in_memory().expect("open");
        storage.migrate().expect("migrate");

        let site_url = "https://example.com";
        let normalized_site_url = "https://example.com";
        let now = Utc::now().to_rfc3339();
        let expired = (Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
        storage
            .conn
            .execute(
                "INSERT INTO discovery_cache (
                    id, site_url, normalized_site_url, result_json, created_at, expires_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    Uuid::now_v7().to_string(),
                    site_url,
                    normalized_site_url,
                    r#"{"candidates":[]}"#,
                    now.clone(),
                    expired,
                ],
            )
            .expect("insert expired cache");

        let expired_lookup =
            storage.get_latest_discovery_cache(normalized_site_url).expect("lookup expired cache");
        assert!(expired_lookup.is_none());

        storage
            .upsert_discovery_cache(
                site_url,
                normalized_site_url,
                r#"{"candidates":[{"url":"https://example.com/feed.xml"}]}"#,
                Some(&(Utc::now() + chrono::Duration::hours(24)).to_rfc3339()),
            )
            .expect("insert fresh cache");

        let fresh_lookup =
            storage.get_latest_discovery_cache(normalized_site_url).expect("lookup fresh cache");
        assert!(fresh_lookup.is_some());
    }

    #[test]
    fn refresh_attempt_failure_count_mirrors_feed_state() {
        let mut storage = Storage::open_in_memory().expect("open");
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
            .record_fetch_result(&feed_id, 500, None, None, 10, Some("network error"))
            .expect("first failure");
        storage
            .record_fetch_result(&feed_id, 500, None, None, 11, Some("network error"))
            .expect("second failure");

        let now = Utc::now();
        storage
            .record_refresh_attempt(&notifications::RefreshAttempt {
                feed_id: feed_id.clone(),
                reason: notifications::RefreshReason::Manual,
                started_at: now,
                finished_at: now,
                http_status: Some(500),
                success: false,
                item_count: 0,
                error_message: Some("network error".to_owned()),
                next_attempt_at: Some(now + chrono::Duration::minutes(15)),
            })
            .expect("record attempt");

        let failure_count: i64 = storage
            .conn
            .query_row(
                "SELECT failure_count FROM refresh_jobs WHERE feed_id = ?1",
                params![feed_id],
                |row| row.get(0),
            )
            .expect("failure count");
        assert_eq!(failure_count, 2);
    }

    #[test]
    fn patch_item_state_emits_sync_event_and_acknowledges() {
        let mut storage = Storage::open_in_memory().expect("open");
        storage.migrate().expect("migrate");

        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: Url::parse("https://example.com/feed.xml").expect("url parse"),
                site_url: Some(Url::parse("https://example.com").expect("url parse")),
                title: Some("Example".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("upsert feed");
        storage.upsert_items(&[sample_item(&feed_id, "item-1")]).expect("insert item");

        storage
            .patch_item_state(
                "item-1",
                &ItemStatePatch {
                    is_read: Some(true),
                    is_starred: Some(false),
                    is_saved_for_later: None,
                    is_archived: None,
                },
            )
            .expect("patch state");

        let pending = storage.list_pending_sync_events(10).expect("list pending");
        assert!(pending.iter().any(|event| event.entity_type == "feed"));
        let item_state_event = pending
            .iter()
            .find(|event| {
                event.entity_type == "item_state"
                    && event.entity_id == "item-1"
                    && event.event_type == "updated"
            })
            .expect("item state event");
        assert!(item_state_event.processed_at.is_none());
        assert!(!item_state_event.created_at.is_empty());

        let payload: Value =
            serde_json::from_str(&item_state_event.payload_json).expect("payload json");
        assert_eq!(payload["item_id"], "item-1");
        assert_eq!(payload["is_read"], true);
        assert_eq!(payload["is_starred"], false);

        let acknowledged = storage
            .acknowledge_sync_events(std::slice::from_ref(&item_state_event.id))
            .expect("ack sync event");
        assert_eq!(acknowledged, 1);
        let pending_after_ack = storage.list_pending_sync_events(10).expect("list pending");
        assert!(pending_after_ack.iter().all(|event| event.entity_type != "item_state"));

        storage
            .patch_item_state(
                "item-1",
                &ItemStatePatch {
                    is_read: Some(true),
                    is_starred: None,
                    is_saved_for_later: None,
                    is_archived: None,
                },
            )
            .expect("no-op patch");
        let pending_after_noop = storage.list_pending_sync_events(10).expect("list pending");
        assert!(pending_after_noop.iter().all(|event| event.entity_type != "item_state"));
    }

    #[test]
    fn notification_preferences_and_events_round_trip() {
        let mut storage = Storage::open_in_memory().expect("open");
        storage.migrate().expect("migrate");

        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: Url::parse("https://example.com/feed.xml").expect("url parse"),
                site_url: Some(Url::parse("https://example.com").expect("url parse")),
                title: Some("Example".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("upsert feed");

        let settings = NotificationSettings {
            enabled: true,
            mode: NotificationMode::Digest,
            digest_policy: DigestPolicy { enabled: true, interval_minutes: 60, max_items: 12 },
            quiet_hours: QuietHours { enabled: true, start_minute: 22 * 60, end_minute: 7 * 60 },
            minimum_interval_minutes: 30,
            high_priority: true,
            keyword_include: vec!["rust".to_owned()],
            keyword_exclude: vec!["spam".to_owned()],
        };
        let row =
            storage.upsert_notification_settings(&feed_id, &settings).expect("upsert settings");
        assert!(row.settings.enabled);
        assert_eq!(row.settings.mode, NotificationMode::Digest);
        assert_eq!(row.settings.digest_policy.max_items, 12);
        assert!(chrono::DateTime::parse_from_rfc3339(&row.updated_at).is_ok());
        let loaded_settings = storage
            .get_notification_settings(&feed_id)
            .expect("load settings")
            .expect("settings row");
        assert_eq!(loaded_settings.settings.digest_policy.max_items, 12);
        assert!(chrono::DateTime::parse_from_rfc3339(&loaded_settings.updated_at).is_ok());
        assert_eq!(loaded_settings.updated_at, row.updated_at);

        let globals = GlobalNotificationSettings::default();
        storage.set_global_notification_settings(&globals).expect("set globals");
        let loaded_globals = storage.get_global_notification_settings().expect("load globals");
        assert_eq!(loaded_globals.background_refresh_interval_minutes, 15);

        let seen_at = Utc::now().to_rfc3339();
        let inserted = storage
            .record_notification_signature(
                &feed_id,
                "entry-1",
                "canonical-1",
                "fingerprint-1",
                &seen_at,
                Some(&seen_at),
            )
            .expect("signature");
        assert!(inserted);
        let duplicate = storage
            .record_notification_signature(
                &feed_id,
                "entry-1",
                "canonical-1",
                "fingerprint-1",
                &seen_at,
                Some(&seen_at),
            )
            .expect("signature duplicate");
        assert!(!duplicate);
        let latest =
            storage.get_latest_notification_at_for_feed(&feed_id).expect("latest notification");
        assert!(latest.is_some());

        let event = NotificationEvent {
            id: "event-1".to_owned(),
            feed_id: Some(feed_id.clone()),
            entry_id: Some("entry-1".to_owned()),
            canonical_key: "canonical-1".to_owned(),
            content_fingerprint: "fingerprint-1".to_owned(),
            title: "Rust update".to_owned(),
            body: "Rust update body".to_owned(),
            mode: NotificationMode::Immediate,
            delivery_state: NotificationDeliveryState::Pending,
            reason: "new_item".to_owned(),
            digest_id: None,
            created_at: Utc::now(),
            ready_at: Some(Utc::now()),
            delivered_at: None,
            suppressed_at: None,
        };
        storage
            .insert_notification_event(&event, Some("{\"source\":\"test\"}"))
            .expect("insert event");
        let pending = storage.list_pending_notification_events(10).expect("pending events");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "event-1");
        let acknowledged =
            storage.acknowledge_notification_events(&[pending[0].id.clone()]).expect("ack event");
        assert_eq!(acknowledged, 1);

        let digest = NotificationDigest {
            id: "digest-1".to_owned(),
            feed_id: Some(feed_id.clone()),
            entry_count: 1,
            title: "Digest".to_owned(),
            body: "Digest body".to_owned(),
            created_at: Utc::now(),
            ready_at: Some(Utc::now()),
            delivered_at: None,
        };
        storage.insert_notification_digest(&digest, "[\"entry-1\"]").expect("insert digest");
    }

    #[test]
    fn refresh_settings_inherit_and_override_by_scope() {
        let mut storage = Storage::open_in_memory().expect("open");
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

        let globals = GlobalNotificationSettings {
            background_refresh_enabled: true,
            background_refresh_interval_minutes: 30,
            digest_policy: DigestPolicy::default(),
            default_feed_settings: NotificationSettings::default(),
        };
        storage.set_global_notification_settings(&globals).expect("set globals");

        let effective =
            storage.resolve_effective_refresh_settings(&feed_id).expect("resolve effective");
        assert!(effective.enabled);
        assert_eq!(effective.interval_minutes, 30);

        let group_settings = RefreshSettings { enabled: true, interval_minutes: 90 };
        storage
            .upsert_group_refresh_settings(&group.id, &group_settings)
            .expect("upsert group refresh settings");
        let effective_after_group = storage
            .resolve_effective_refresh_settings(&feed_id)
            .expect("resolve effective after group");
        assert_eq!(effective_after_group.interval_minutes, 90);

        let feed_settings = RefreshSettings { enabled: false, interval_minutes: 5 };
        storage
            .upsert_feed_refresh_settings(&feed_id, &feed_settings)
            .expect("upsert feed refresh settings");
        let effective_after_feed = storage
            .resolve_effective_refresh_settings(&feed_id)
            .expect("resolve effective after feed");
        assert!(!effective_after_feed.enabled);
        assert_eq!(effective_after_feed.interval_minutes, 5);

        let due_feeds = storage.list_due_feeds(10).expect("list due feeds");
        assert!(due_feeds.is_empty());
    }

    #[test]
    fn apply_sync_events_replays_feeds_groups_and_notes() {
        let mut source = Storage::open_in_memory().expect("open");
        source.migrate().expect("migrate");

        let feed_id = source
            .upsert_feed(&NewFeed {
                feed_url: Url::parse("https://example.com/feed.xml").expect("url parse"),
                site_url: Some(Url::parse("https://example.com").expect("url parse")),
                title: Some("Example".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("upsert feed");
        let group = source.create_group("Tech").expect("create group");
        source.set_feed_group(&feed_id, Some(&group.id)).expect("set group");
        source
            .upsert_entries(&[NewEntry {
                id: Some("note-1".to_owned()),
                kind: models::EntryKind::Note,
                source: models::EntrySource {
                    source_kind: models::EntrySourceKind::Manual,
                    source_id: None,
                    source_url: None,
                    source_title: Some("manual".to_owned()),
                },
                external_item_id: None,
                canonical_url: None,
                title: "Sync me".to_owned(),
                author: None,
                summary: Some("Sync summary".to_owned()),
                content_html: None,
                content_text: Some("Sync body".to_owned()),
                published_at: None,
                updated_at: None,
                raw_hash: "note-1".to_owned(),
                dedup_reason: None,
                duplicate_of_entry_id: None,
            }])
            .expect("upsert note");
        source
            .patch_item_state(
                "note-1",
                &ItemStatePatch {
                    is_read: Some(true),
                    is_starred: Some(true),
                    is_saved_for_later: Some(true),
                    is_archived: Some(false),
                },
            )
            .expect("patch note");

        let events = source.list_pending_sync_events(100).expect("list events");
        assert!(!events.is_empty());

        let mut target = Storage::open_in_memory().expect("open target");
        target.migrate().expect("migrate target");
        let applied = target
            .apply_sync_events(
                &events
                    .iter()
                    .map(|event| SyncEventRecord {
                        id: event.id.clone(),
                        entity_type: event.entity_type.clone(),
                        entity_id: event.entity_id.clone(),
                        event_type: event.event_type.clone(),
                        payload_json: event.payload_json.clone(),
                        created_at: event.created_at.clone(),
                    })
                    .collect::<Vec<_>>(),
            )
            .expect("apply sync events");
        assert!(applied >= 3);

        let feeds = target.list_feeds().expect("list feeds");
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].feed_url.as_str(), "https://example.com/feed.xml");

        let groups = target.list_groups().expect("list groups");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "Tech");

        let group_members =
            target.list_groups_for_feed(&feeds[0].id).expect("list groups for feed");
        assert_eq!(group_members.len(), 1);
        assert_eq!(group_members[0].name, "Tech");

        let detail = target.get_item_detail("note-1").expect("note detail");
        assert_eq!(detail.title, "Sync me");
        let state = target.get_item_state("note-1").expect("note state");
        assert!(state.is_read);
        assert!(state.is_starred);
        assert!(state.is_saved_for_later);
    }
}
