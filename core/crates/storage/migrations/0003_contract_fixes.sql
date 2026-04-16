PRAGMA foreign_keys = ON;

ALTER TABLE notification_settings
  ADD COLUMN digest_max_items INTEGER NOT NULL DEFAULT 20;
