# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.4] - 2026-05-28

### Added
- Cross-platform release pipeline for macOS, Windows, Linux, and iOS.
- Homebrew Cask distribution support.
- GitHub Actions CI with Rust, Flutter, and Apple shell test gates.
- CloudKit sync with device origin filtering and automatic retry.
- Background feed refresh with deterministic fetch behavior.
- Full-text extraction for article detail view.
- Per-feed notification settings with quiet hours and digest delivery.
- OPML import/export for subscription portability.
- Read-it-later and memo composer workflows.

### Changed
- Replaced per-call Tokio runtime creation in FFI bridge with a global shared runtime.
- Updated Linux `.deb` maintainer to project author.

### Fixed
- Fixed race condition in Apple Shell item selection that could overwrite detail with stale data.
- Eliminated unsafe raw pointer in `SyncEventSuppressionGuard` by using lifetime-bound references.
- Hardened HTTP server to bind loopback by default and require API token for remote access.
- Fixed `entries?kind=` API contract to correctly filter by entry type.

### Security
- Local HTTP server now rejects non-loopback binds unless explicitly allowed and token-protected.
- Homebrew Cask now uses version-pinned URL with SHA256 checksum verification.

## [0.1.0] - 2026-04-02

### Added
- Initial release of InfoMatrix local-first RSS reader.
- RSS, Atom, and JSON feed discovery and parsing.
- Three-pane macOS reader shell (SwiftUI).
- Flutter shell for Windows, Linux, and Android.
- SQLite-backed persistence with FTS5 search index.
- Feed grouping and smart scopes (unread, starred, later, notes, archive).
