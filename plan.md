# Objective
Ship a P0/P1-ready macOS-first InfoMatrix build that works as a local-first RSS reader, read-later queue, and memos tool, while fixing confirmed safety, API-contract, sync, and project hygiene issues.

# User Value
Users should be able to install and use the macOS app immediately for the three core workflows: subscribe/read feeds, save webpages for later, and write memos. Apple users should be able to opt into a usable CloudKit sync path without weakening the local-first model.

# Constraints
- Preserve the local-first architecture and keep networking, parsing, persistence, and sync plumbing out of SwiftUI views.
- Rust remains the owner of discovery, parsing, fetching, persistence, icons, and sync-ready event state.
- Do not add new dependencies unless a blocker appears.
- Do not push or publish artifacts without explicit approval.
- Keep Windows, Linux, Android, and Flutter behavior from regressing while prioritizing macOS.

# Assumptions
- [ASSUMPTION] This round treats "all vulnerabilities" as confirmed P0/P1 issues that affect startup, data safety, local API exposure, sync correctness, or the three core product workflows.
- [ASSUMPTION] CloudKit sync is Apple-only for this round; no new cross-platform remote sync service is introduced.
- [ASSUMPTION] Apple Developer account setup will provide matching iCloud containers for release signing.

# Affected Files
- `plan.md`
- `core/crates/app_server/src/main.rs`
- `docs/api-contracts.md`
- `docs/sync.md`
- `apps/apple/Shared/CloudKitSyncManager.swift`
- `apps/apple/XcodeGen/project.yml`
- `apps/apple/Entitlements/*.entitlements`
- `apps/apple/Tests/InfoMatrixShellTests/AppStateTests.swift`
- `apps/flutter/lib/app.dart`
- `apps/flutter/lib/ui/reader_shell_page.dart`
- `apps/flutter/test/widget_test.dart`

# Steps
1. Record this P0/P1 delivery plan before broad edits.
2. Harden the local HTTP server:
   - keep loopback as the default bind target
   - reject non-loopback binds unless `INFOMATRIX_ALLOW_REMOTE_BIND=1`
   - require `INFOMATRIX_API_TOKEN` for non-loopback binds
   - require `Authorization: Bearer <token>` for protected `/api/v1/*` routes when a token is configured
3. Fix the HTTP entries contract so `GET /api/v1/entries?kind=note|bookmark|article|quote` filters correctly, and document it.
4. Make Apple CloudKit sync usable:
   - add CloudKit entitlements for Apple app targets
   - tag uploaded records with an origin device id
   - filter local-origin records when fetching remote events
   - fetch all pages and sort remote events deterministically by `created_at`
   - refresh pending counts and sync status after sync attempts
5. Tighten the macOS three-in-one workflow by preserving current RSS, read-later, and memo flows and adding tests around the sync and entry filtering behavior.
6. Fix Flutter project hygiene:
   - remove the fake feed URL prefill
   - allow read-later title to be blank so the core can infer it
   - resolve current `flutter analyze` issues that fail the project
7. Run focused and broad validation across Rust, Apple, and Flutter.

# Validation
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `swift test`
- `flutter analyze`
- `flutter test`
- Targeted tests for remote-bind/token behavior, `entries?kind`, CloudKit coordinator behavior, and blank-title read-later.

# Risks
- Overlooked risk 1: CloudKit entitlements can compile locally but still fail at runtime if the Apple Developer container is not provisioned.
- Overlooked risk 2: adding token enforcement to remote-bound local API servers may break ad hoc scripts that intentionally bind externally without a token.
- Overlooked risk 3: CloudKit event replay can surface pre-existing malformed sync events from older local databases; tests need to cover bad or self-origin records without corrupting local state.
- Flutter analyzer behavior depends on the installed Flutter SDK version, so fixes should target the current warnings without forcing broad UI rewrites.
- Remote-bind protection must not interfere with normal loopback app/server workflows.

# Rollback Notes
- If token middleware blocks legitimate local use, restrict enforcement to non-loopback runtime configuration only and keep loopback unauthenticated.
- If CloudKit entitlements break unsigned local builds, keep the files but gate code signing settings by configuration or target.
- If CloudKit paging APIs are unavailable on the local SDK, retain origin tagging and deterministic sorting while falling back to the SDK-supported query API shape.
