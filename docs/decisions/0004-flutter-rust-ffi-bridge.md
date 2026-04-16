# ADR 0004: Flutter Uses Rust FFI Bridge for Core Access

## Status

Accepted

## Context

The Flutter targets (Android/Windows/Linux) need direct access to the same local-first core capabilities implemented in Rust:

- subscription CRUD
- discovery
- fetch + parse + persistence
- item state transitions

Relying only on an HTTP sidecar process in Flutter introduces packaging friction and lifecycle complexity, especially on Android.

## Decision

Introduce `core/crates/ffi_bridge` as a `cdylib` exposing C ABI entrypoints with JSON payloads.

- Flutter calls Rust via `dart:ffi`.
- Rust returns a stable envelope: `{ ok, data, error }`.
- Memory ownership for returned strings is explicit via `infomatrix_core_free_string`.

## Consequences

### Positive

- Single Rust core capability path reused across Flutter platforms.
- No required local HTTP sidecar in Flutter runtime.
- Easier long-term API evolution by versioning JSON envelopes.

### Trade-offs

- Runtime dynamic library loading and packaging becomes platform-specific.
- Android needs ABI-specific `.so` staging.
- FFI call surface must remain backward-compatible or version-gated.
