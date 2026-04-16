PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS feeds (
  id TEXT PRIMARY KEY,
  feed_url TEXT NOT NULL UNIQUE,
  site_url TEXT,
  title TEXT,
  feed_type TEXT NOT NULL,
  auto_full_text INTEGER NOT NULL DEFAULT 1,
  etag TEXT,
  last_modified TEXT,
  last_fetch_at TEXT,
  last_success_at TEXT,
  last_http_status INTEGER,
  failure_count INTEGER NOT NULL DEFAULT 0,
  next_scheduled_fetch_at TEXT,
  avg_response_time_ms INTEGER,
  health_state TEXT NOT NULL DEFAULT 'healthy',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS feed_groups (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS group_memberships (
  id TEXT PRIMARY KEY,
  group_id TEXT NOT NULL,
  feed_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(group_id, feed_id),
  FOREIGN KEY(group_id) REFERENCES feed_groups(id) ON DELETE CASCADE,
  FOREIGN KEY(feed_id) REFERENCES feeds(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS items (
  id TEXT PRIMARY KEY,
  feed_id TEXT NOT NULL,
  external_item_id TEXT,
  canonical_url TEXT,
  title TEXT NOT NULL,
  author TEXT,
  summary TEXT,
  published_at TEXT,
  updated_at TEXT,
  raw_hash TEXT NOT NULL,
  dedup_reason TEXT,
  duplicate_of_item_id TEXT,
  created_at TEXT NOT NULL,
  row_updated_at TEXT NOT NULL,
  FOREIGN KEY(feed_id) REFERENCES feeds(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS item_contents (
  item_id TEXT PRIMARY KEY,
  content_html TEXT,
  content_text TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(item_id) REFERENCES items(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS item_states (
  item_id TEXT PRIMARY KEY,
  is_read INTEGER NOT NULL DEFAULT 0,
  is_starred INTEGER NOT NULL DEFAULT 0,
  is_saved_for_later INTEGER NOT NULL DEFAULT 0,
  is_archived INTEGER NOT NULL DEFAULT 0,
  read_at TEXT,
  starred_at TEXT,
  saved_for_later_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(item_id) REFERENCES items(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS item_enclosures (
  id TEXT PRIMARY KEY,
  item_id TEXT NOT NULL,
  url TEXT NOT NULL,
  mime_type TEXT,
  byte_length INTEGER,
  created_at TEXT NOT NULL,
  FOREIGN KEY(item_id) REFERENCES items(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS icons (
  id TEXT PRIMARY KEY,
  feed_id TEXT,
  source_url TEXT NOT NULL,
  mime_type TEXT,
  byte_size INTEGER,
  sha256 TEXT,
  local_path TEXT,
  fetched_at TEXT,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(feed_id) REFERENCES feeds(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS discovery_cache (
  id TEXT PRIMARY KEY,
  site_url TEXT NOT NULL,
  normalized_site_url TEXT NOT NULL,
  result_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT
);

CREATE TABLE IF NOT EXISTS fetch_logs (
  id TEXT PRIMARY KEY,
  feed_id TEXT NOT NULL,
  fetched_at TEXT NOT NULL,
  duration_ms INTEGER,
  http_status INTEGER,
  error_message TEXT,
  etag TEXT,
  last_modified TEXT,
  FOREIGN KEY(feed_id) REFERENCES feeds(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS sync_events (
  id TEXT PRIMARY KEY,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  processed_at TEXT
);

CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS opml_import_jobs (
  id TEXT PRIMARY KEY,
  source_path TEXT,
  status TEXT NOT NULL,
  imported_count INTEGER NOT NULL DEFAULT 0,
  failed_count INTEGER NOT NULL DEFAULT 0,
  diagnostics_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_items_feed_published ON items(feed_id, published_at DESC);
CREATE INDEX IF NOT EXISTS idx_item_states_read ON item_states(is_read);
CREATE INDEX IF NOT EXISTS idx_item_states_starred ON item_states(is_starred);
CREATE INDEX IF NOT EXISTS idx_item_states_saved ON item_states(is_saved_for_later);
CREATE INDEX IF NOT EXISTS idx_items_canonical_url ON items(canonical_url);
CREATE INDEX IF NOT EXISTS idx_items_external_item_id ON items(external_item_id);
CREATE INDEX IF NOT EXISTS idx_fetch_logs_feed_fetched ON fetch_logs(feed_id, fetched_at DESC);
CREATE INDEX IF NOT EXISTS idx_sync_events_pending_created ON sync_events(processed_at, created_at);
CREATE INDEX IF NOT EXISTS idx_feeds_next_scheduled_fetch ON feeds(next_scheduled_fetch_at);
CREATE INDEX IF NOT EXISTS idx_discovery_cache_normalized_created
  ON discovery_cache(normalized_site_url, created_at DESC);

CREATE VIRTUAL TABLE IF NOT EXISTS item_search USING fts5(
  item_id UNINDEXED,
  title,
  summary,
  content_text
);

CREATE TRIGGER IF NOT EXISTS trg_items_delete_item_search
AFTER DELETE ON items
BEGIN
  DELETE FROM item_search WHERE item_id = OLD.id;
END;

CREATE TABLE IF NOT EXISTS entries (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  external_item_id TEXT,
  canonical_url TEXT,
  title TEXT NOT NULL,
  author TEXT,
  summary TEXT,
  published_at TEXT,
  updated_at TEXT,
  raw_hash TEXT NOT NULL,
  dedup_reason TEXT,
  duplicate_of_entry_id TEXT,
  created_at TEXT NOT NULL,
  row_updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS entry_contents (
  entry_id TEXT PRIMARY KEY,
  content_html TEXT,
  content_text TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(entry_id) REFERENCES entries(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS entry_states (
  entry_id TEXT PRIMARY KEY,
  is_read INTEGER NOT NULL DEFAULT 0,
  is_starred INTEGER NOT NULL DEFAULT 0,
  is_saved_for_later INTEGER NOT NULL DEFAULT 0,
  is_archived INTEGER NOT NULL DEFAULT 0,
  read_at TEXT,
  starred_at TEXT,
  saved_for_later_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(entry_id) REFERENCES entries(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS entry_sources (
  id TEXT PRIMARY KEY,
  entry_id TEXT NOT NULL,
  source_kind TEXT NOT NULL,
  source_id TEXT,
  source_url TEXT,
  source_title TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(entry_id) REFERENCES entries(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_entries_kind_published ON entries(kind, published_at DESC);
CREATE INDEX IF NOT EXISTS idx_entries_canonical_url ON entries(canonical_url);
CREATE INDEX IF NOT EXISTS idx_entries_external_item_id ON entries(external_item_id);
CREATE INDEX IF NOT EXISTS idx_entry_states_read ON entry_states(is_read);
CREATE INDEX IF NOT EXISTS idx_entry_states_starred ON entry_states(is_starred);
CREATE INDEX IF NOT EXISTS idx_entry_states_saved ON entry_states(is_saved_for_later);
CREATE INDEX IF NOT EXISTS idx_entry_sources_kind_source ON entry_sources(source_kind, source_id);

CREATE VIRTUAL TABLE IF NOT EXISTS entry_search USING fts5(
  entry_id UNINDEXED,
  title,
  summary,
  content_text
);

CREATE TRIGGER IF NOT EXISTS trg_entries_delete_entry_search
AFTER DELETE ON entries
BEGIN
  DELETE FROM entry_search WHERE entry_id = OLD.id;
END;

CREATE TABLE IF NOT EXISTS notification_settings (
  feed_id TEXT PRIMARY KEY,
  enabled INTEGER NOT NULL DEFAULT 0,
  mode TEXT NOT NULL DEFAULT 'immediate',
  digest_interval_minutes INTEGER NOT NULL DEFAULT 60,
  quiet_hours_enabled INTEGER NOT NULL DEFAULT 0,
  quiet_hours_start_minute INTEGER NOT NULL DEFAULT 1320,
  quiet_hours_end_minute INTEGER NOT NULL DEFAULT 420,
  minimum_interval_minutes INTEGER NOT NULL DEFAULT 60,
  high_priority INTEGER NOT NULL DEFAULT 0,
  keyword_include_json TEXT NOT NULL DEFAULT '[]',
  keyword_exclude_json TEXT NOT NULL DEFAULT '[]',
  updated_at TEXT NOT NULL,
  FOREIGN KEY(feed_id) REFERENCES feeds(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS refresh_jobs (
  feed_id TEXT PRIMARY KEY,
  last_attempt_at TEXT,
  last_success_at TEXT,
  failure_count INTEGER NOT NULL DEFAULT 0,
  next_attempt_at TEXT,
  last_http_status INTEGER,
  last_error_message TEXT,
  last_reason TEXT,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(feed_id) REFERENCES feeds(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS refresh_attempts (
  id TEXT PRIMARY KEY,
  feed_id TEXT NOT NULL,
  reason TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT NOT NULL,
  success INTEGER NOT NULL,
  http_status INTEGER,
  item_count INTEGER NOT NULL DEFAULT 0,
  error_message TEXT,
  next_attempt_at TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY(feed_id) REFERENCES feeds(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS notification_signatures (
  id TEXT PRIMARY KEY,
  feed_id TEXT,
  entry_id TEXT,
  canonical_key TEXT NOT NULL,
  content_fingerprint TEXT NOT NULL,
  first_seen_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  last_notification_at TEXT,
  UNIQUE(canonical_key, content_fingerprint)
);

CREATE TABLE IF NOT EXISTS notification_events (
  id TEXT PRIMARY KEY,
  feed_id TEXT,
  entry_id TEXT,
  canonical_key TEXT NOT NULL,
  content_fingerprint TEXT NOT NULL,
  mode TEXT NOT NULL,
  delivery_state TEXT NOT NULL,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  reason TEXT NOT NULL,
  digest_id TEXT,
  created_at TEXT NOT NULL,
  ready_at TEXT,
  delivered_at TEXT,
  suppressed_at TEXT,
  metadata_json TEXT
);

CREATE TABLE IF NOT EXISTS notification_digests (
  id TEXT PRIMARY KEY,
  feed_id TEXT,
  entry_ids_json TEXT NOT NULL,
  entry_count INTEGER NOT NULL DEFAULT 0,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  created_at TEXT NOT NULL,
  ready_at TEXT,
  delivered_at TEXT
);

CREATE TABLE IF NOT EXISTS push_endpoints (
  id TEXT PRIMARY KEY,
  platform TEXT NOT NULL,
  endpoint TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_refresh_jobs_next_attempt ON refresh_jobs(next_attempt_at);
CREATE INDEX IF NOT EXISTS idx_refresh_attempts_feed_started ON refresh_attempts(feed_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_notification_signatures_identity ON notification_signatures(canonical_key, content_fingerprint);
CREATE INDEX IF NOT EXISTS idx_notification_events_pending
  ON notification_events(delivery_state, ready_at, created_at);
