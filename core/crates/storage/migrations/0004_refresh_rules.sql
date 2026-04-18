PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS feed_refresh_settings (
  feed_id TEXT PRIMARY KEY,
  enabled INTEGER NOT NULL DEFAULT 1,
  interval_minutes INTEGER NOT NULL DEFAULT 15,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(feed_id) REFERENCES feeds(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS group_refresh_settings (
  group_id TEXT PRIMARY KEY,
  enabled INTEGER NOT NULL DEFAULT 1,
  interval_minutes INTEGER NOT NULL DEFAULT 15,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(group_id) REFERENCES feed_groups(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_feed_refresh_settings_enabled
  ON feed_refresh_settings(enabled, interval_minutes);
CREATE INDEX IF NOT EXISTS idx_group_refresh_settings_enabled
  ON group_refresh_settings(enabled, interval_minutes);
