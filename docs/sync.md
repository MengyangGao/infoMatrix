# Sync Foundation

Sync is not shipped in MVP but its interface is defined immediately.

## Principles

- local writes are represented as events
- syncable state carries monotonic `updated_at`
- delete/clear style actions should preserve semantic intent via tombstones/events

## Components

- `sync_events` table: append-only event ledger
- `sync` crate:
  - event model
  - adapter trait
  - local-only no-op adapter and future conflict-resolution hooks

## Local Event Queue Semantics

- Item state writes (`read`, `starred`, `saved_for_later`, `archived`) append an `item_state` `updated` event when state actually changes.
- Event payload stores:
  - requested patch fields
  - previous state snapshot
  - current state snapshot
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
