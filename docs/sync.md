# Sync Foundation

Sync is a cross-platform architecture boundary, but the first shipped remote backend is Apple-only CloudKit sync. Android, Linux, and Windows remain local-only for now.

## Principles

- local writes are represented as events
- syncable state carries monotonic `updated_at`
- delete/clear style actions should preserve semantic intent via tombstones/events
- Apple devices can exchange those events through CloudKit, while the local SQLite journal remains the source of truth

## Components

- `sync_events` table: append-only event ledger
- `sync` crate:
  - event model
  - adapter trait
  - local-only no-op adapter and future conflict-resolution hooks
- Apple shell:
  - CloudKit sync coordinator
  - account state, enablement, and retry status
  - local queue upload / remote event pull orchestration

## Local Event Queue Semantics

- Item state writes (`read`, `starred`, `saved_for_later`, `archived`) append an `item_state` `updated` event when state actually changes.
- Event payload stores:
  - `item_id`
  - stable identity hints (`canonical_url`, `external_item_id`)
  - the current boolean item state
  - mutation timestamp
- Events stay pending while `processed_at IS NULL`.
- Acknowledge is explicit: queue consumers set `processed_at` when an event is safely handled.

## Current Inspection APIs (`app_server`)

- `GET /api/v1/sync/events?limit=N`
  - lists pending events ordered by `created_at ASC`
  - default `limit=100`, clamped to `[1, 500]`
- `POST /api/v1/sync/events/ack`
  - request body: `{ "event_ids": ["..."] }`
  - response: `{ "acknowledged": <count> }`

## Related Local APIs

- `GET /api/v1/feeds/due?limit=N`
  - lists feeds whose `next_scheduled_fetch_at` is due or unset
  - feeds are ordered by schedule time, then creation time
- `POST /api/v1/refresh/due?limit=N`
  - refreshes due feeds in order and reports per-feed results
  - intended as the background-refresh entry point for future schedulers

## Planned Adapters

- local-only noop adapter
- self-hosted API adapter
- WebDAV-like document adapter
- third-party RSS sync service adapter
- Apple CloudKit adapter for the SwiftUI shell
