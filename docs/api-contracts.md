# API Contracts (Internal)

## Shared Core Façade

The `app_core` crate is the canonical orchestration layer for the most common UI-shell actions:

- subscription by feed URL
- site discovery and subscription
- feed, group, item, and detail listing
- search queries across feed and inbox scopes
- item state mutations
- manual unified entry creation
- refresh coordination for feed parsing and persistence

Both `app_server` and `ffi_bridge` should map their CRUD-style endpoints to this layer so the Apple and Flutter shells stay behaviorally aligned.

## Search Contract

Input:

- `feed_id: Option<&str>`
- `limit: usize`
- `search_query: Option<&str>`
- `filter: ItemListFilter`
- optional `kind: EntryKind`

Output:

- deterministic item summaries sorted by publish time and row update time
- feed-scoped searches are constrained to the selected feed
- inbox-scoped searches respect the active filter and optional kind

## Discovery Contract

Input:

- `site_url: &str`

Output:

- `DiscoveryResult`
  - normalized site URL
  - validated feed candidates
  - site metadata
  - diagnostics/warnings

## Parser Contract

Input:

- raw bytes
- optional content-type
- source feed id

Output:

- normalized feed metadata
- normalized item list
- parse diagnostics

## Storage Contract

Operations:

- initialize/migrate schema
- upsert feed
- insert/update normalized items
- mutate item states (read/star/later/archive)
- query feeds/items with indexed sort orders
- query aggregated item scope counts
- query due feeds for background refresh
- update refresh metadata and fetch logs
- persist icon asset metadata
- list pending sync events
- acknowledge processed sync events

## Sync Queue Contract (Current Local API)

Endpoints:

- `GET /api/v1/sync/events?limit=N`
  - returns pending events (`processed_at IS NULL`) sorted by `created_at ASC`
- `POST /api/v1/sync/events/ack`
  - input: `{ "event_ids": ["event-id"] }`
  - output: `{ "acknowledged": <count> }`

## Background Refresh Contract (Current Local API)

Endpoints:

- `GET /api/v1/feeds/due?limit=N`
  - output: list of feeds with `id`, `title`, `feed_url`, `next_scheduled_fetch_at`, `failure_count`, `health_state`
- `POST /api/v1/refresh/due?limit=N`
  - output: `{ "refreshed_count": <count>, "total_item_count": <count>, "results": [...] }`
- `POST /api/v1/refresh/{feed_id}`
  - output: per-feed refresh result with `feed_id`, `status`, `fetched_http_status`, and `item_count`

Refresh results also include notification accounting:

- `notification_count`
- `suppressed_notification_count`

The server writes a refresh-attempt record and applies feed-level notification policy during the same refresh pipeline used for manual and background refreshes.

## Notification Contract (Current Local API)

Endpoints:

- `GET /api/v1/notifications/settings`
  - output: global notification defaults (`background_refresh_enabled`, `background_refresh_interval_minutes`, digest defaults, and default feed settings)
- `PUT /api/v1/notifications/settings`
  - input: global notification defaults payload
  - output: stored global notification defaults
- `GET /api/v1/feeds/{feed_id}/notifications`
  - output: effective feed notification settings with global defaults applied as fallback
- `PUT /api/v1/feeds/{feed_id}/notifications`
  - input: per-feed notification settings payload
  - output: stored per-feed notification settings
- `GET /api/v1/notifications/pending?limit=N`
  - output: queued notification events ready for shell delivery
- `POST /api/v1/notifications/pending/ack`
  - input: `{ "event_ids": ["event-id"] }`
  - output: `{ "acknowledged": <count> }`

Delivery semantics:

- events are persisted before delivery
- the shell may poll and acknowledge pending events
- quiet hours, minimum intervals, digest batching, and keyword filters are applied before an event becomes deliverable
- a separate audit trail records whether an event was sent, queued, or suppressed

## Inbox Counts Contract (Current Local API)

Endpoints:

- `GET /api/v1/entries/counts`
  - output: `{ "all": <count>, "unread": <count>, "starred": <count>, "later": <count>, "archive": <count> }`

## Unified Entry Contract (Current Local API)

Endpoints:

- `GET /api/v1/entries`
  - list unified inbox entries
- `POST /api/v1/entries`
  - input: unified entry payload (`title`, optional `kind`, `source_*`, content fields)
  - output: created entry detail
- `GET /api/v1/entries/{entry_id}`
  - output: unified entry detail
- `PATCH /api/v1/entries/{entry_id}/state`
  - input: item state patch (`is_read`, `is_starred`, `is_saved_for_later`, `is_archived`)
- `POST /api/v1/entries/{entry_id}/fulltext`
  - refresh extracted content for a unified entry

Legacy compatibility:

- `GET /api/v1/items`
- `GET /api/v1/items/{item_id}`
- `PATCH /api/v1/items/{item_id}/state`
- `POST /api/v1/items/{item_id}/fulltext`

## OPML Contract (Current Local API)

Endpoints:

- `GET /api/v1/opml/export`
  - output: `{ "opml_xml": "<xml...>", "feed_count": <count> }`
- `POST /api/v1/opml/import`
  - input: `{ "opml_xml": "<xml...>" }`
  - output:
    - `parsed_feed_count`
    - `unique_feed_count`
    - `grouped_feed_count`

FFI equivalents:

- `infomatrix_core_export_opml_json`
- `infomatrix_core_import_opml_json`
