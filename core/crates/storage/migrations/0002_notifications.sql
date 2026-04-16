PRAGMA foreign_keys = ON;

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
