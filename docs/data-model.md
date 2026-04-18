# Data Model (Schema v0)

InfoMatrix stores core product state in SQLite and uses stable internal IDs.

## Core Entities

- `feeds`: subscription metadata and fetch health fields
- `feed_groups`: user-created groups/folders
- `group_memberships`: feed-group many-to-many relation
- `entries`: unified inbox rows for articles, bookmarks, notes, quotes
- `entry_contents`: content payload split from list metadata
- `entry_states`: read/starred/later/archive state machine
- `entry_sources`: source metadata for each entry
- `entry_search`: FTS5 index for unified entry search
- `items`: legacy compatibility view/path for RSS article code
- `item_contents`: legacy alias path during migration
- `item_states`: legacy alias path during migration
- `item_enclosures`: attachment/enclosure metadata
- `icons`: feed/site icon metadata and cache pointers
- `discovery_cache`: cached discovery snapshots and resolved candidates, bounded by `expires_at`
- `fetch_logs`: per-fetch operational trace
- `sync_events`: append-only sync-ready event log
- `app_settings`: application key-value settings
- `opml_import_jobs`: OPML import diagnostics and status

## Notification State

Notification and background refresh state now has dedicated tables so delivery can be audited independently from article state:

- `notification_settings`: per-feed notification preferences, including digest interval and digest batch size
- `refresh_jobs`: durable refresh job state and next-run metadata
- `refresh_attempts`: per-refresh attempt history
- `notification_signatures`: canonical identity / content fingerprint dedup markers
- `notification_events`: queued, suppressed, delivered, and audited notification rows
- `notification_digests`: digest batches and suppression windows
- `push_endpoints`: optional device-token / endpoint registrations for a future hosted push bridge

Global notification defaults are stored in `app_settings` under `notification_globals` so feed overrides can fall back to a single application-level source of truth.

## Identifier Policy

- Internal IDs are UUID strings.
- External feed/item identifiers are stored separately.
- URLs are never used as primary keys.

## Index Strategy

Key indexes include:

- `entries(kind, published_at DESC)`
- `entry_states(is_read)`
- `entry_states(is_starred)`
- `entry_states(is_saved_for_later)`
- `entries(canonical_url)`
- `entries(external_item_id)`
- `fetch_logs(feed_id, fetched_at DESC)`
- `sync_events(processed_at, created_at)`
- `feeds(next_scheduled_fetch_at)`
- `discovery_cache(normalized_site_url, created_at DESC)`

Search is implemented with SQLite FTS5 (`entry_search`) and populated from `entries` + `entry_contents` so unified inbox search avoids table scans on larger local datasets.

Item scope counts used by the shell are derived from `entries` and `entry_states` at read time; they are not stored as separate counters yet so the data model stays source-of-truth driven.

## Sync Readiness

Each syncable row includes `created_at` and `updated_at`. `sync_events` acts as a local event journal for future adapters.

Current `sync_events` usage:

- `entity_type`: mutation domain (`item_state`, `feed`, `feed_group`, `feed_membership`, `entry`, `entry_content`, `notification_settings`, `notification_globals`)
- `entity_id`: stable internal entity identifier or stable URL identity
- `event_type`: mutation kind (`created`, `updated`, `deleted`)
- `payload_json`: structured replay payload shaped for the target adapter
- `processed_at`: nullable queue marker used for local ack semantics

The current replay payloads stay intentionally compact:

- feed events carry feed URL, site URL, title, feed type, and auto full text preference
- feed group events carry the group name
- feed membership events carry feed URL and optional group name
- entry events carry the serialized `NewEntry`
- item state events carry item identity plus the current boolean state snapshot
- notification settings events carry the feed URL and current settings blob
- global notification settings events carry the current global settings blob

Refresh attempts are also written to the dedicated refresh tables so next-run/backoff state remains inspectable even when the background refresh loop restarts.

## Refresh State

The `feeds` table also stores refresh scheduling and health fields so background refresh behavior stays inspectable and local:

- `last_fetch_at`
- `last_success_at`
- `last_http_status`
- `failure_count`
- `next_scheduled_fetch_at`
- `avg_response_time_ms`
- `health_state`

These values are updated by fetch completion handling and used by `list_due_feeds` and the refresh-due APIs.

Notification-specific delivery state is tracked separately:

- `notification_signatures.last_seen_at`
- `notification_signatures.last_notification_at`
- `notification_events.delivery_state`
- `notification_events.ready_at`
- `notification_digests.window_start` / `window_end`

This separation lets InfoMatrix suppress duplicates, batch digests, and keep a visible audit trail without mutating read/unread state as a side effect of delivery.

## Icon Cache State

The `icons` table keeps both the logical icon source URL and the cached asset details:

- `source_url`
- `mime_type`
- `byte_size`
- `sha256`
- `local_path`
- `fetched_at`
- `status`

This lets the UI use a stable icon reference while keeping the underlying asset local and inspectable.

## Discovery Cache State

`discovery_cache` stores the most recent discovery snapshot for a site and is used to avoid refetching the chosen feed immediately after discovery. The stored payload keeps:

- `site_url`
- `normalized_site_url`
- serialized discovery result JSON
- `created_at`
- optional `expires_at`

The cache is read when a discovered feed is subscribed later while `expires_at` is still in the future, so the initial item set can be written to SQLite without an extra network round-trip.
