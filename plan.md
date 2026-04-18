# Phase 4: Distribution, Refresh Rules, and Stability Hardening

## Objective
Deliver the next product slice that matters to users: reliable install paths for macOS/Windows/Linux/Android, a macOS Homebrew installation path, configurable auto-refresh rules with global/folder/feed inheritance, and tighter crash-resistant behavior on the main app flows.

## User Value
- Users can install InfoMatrix on macOS, Windows, Linux, and Android without building from source.
- Mac users can optionally install through Homebrew in addition to release downloads.
- Auto-refresh becomes controllable at the right scope instead of being a single global toggle.
- The app becomes safer to use because main mutation and refresh flows surface errors instead of crashing.

## Constraints
- Keep SQLite as the source of truth.
- Preserve existing release artifact generation where possible.
- Do not introduce AI/LLM work in this phase.
- Keep Android, Linux, and Windows local-only; Apple sync remains separate.
- Keep changes deterministic and testable.
- Do not couple networking directly to view code.

## Assumptions
- [ASSUMPTION] Homebrew distribution will use a project-owned tap with cask/formula metadata checked into this repo.
- [ASSUMPTION] Auto-refresh rules should inherit as `feed override > folder override > global default`.
- [ASSUMPTION] Existing global background refresh settings remain as the default layer, with new per-group and per-feed overrides added alongside them.
- [ASSUMPTION] "Memos" continue to use the existing note/memo entry model.
- [ASSUMPTION] App Store readiness stays mostly documentation/signing prep in this pass; full submission is later.

## Overlooked Risks
1. A naive refresh-rule implementation can silently diverge between storage, server scheduling, and both shells if the effective-setting logic is duplicated instead of shared.
2. Homebrew packaging can become stale quickly if the release artifact names or checksums change without a matching tap update path.
3. Crash hardening can regress user-visible behavior if error propagation changes without tests for the common paths.

## Affected Files
- `core/crates/models/src/lib.rs`
- `core/crates/storage/src/lib.rs`
- `core/crates/ffi_bridge/src/lib.rs`
- `core/crates/app_server/src/main.rs`
- `apps/apple/Shared/*.swift`
- `apps/apple/Tests/InfoMatrixShellTests/*.swift`
- `apps/flutter/lib/core/*.dart`
- `apps/flutter/lib/ui/*.dart`
- `apps/flutter/test/*.dart`
- `tooling/scripts/*.sh`
- `.github/workflows/release.yml`
- `docs/*.md`
- `Formula/*` or `Casks/*` if a Homebrew tap layout is added

## Steps
1. Add a refresh-rule model in the shared Rust domain layer, persist it in SQLite, and expose read/write APIs for global, group, and feed-level settings.
2. Teach the background refresh scheduler and both shells to compute and edit the effective refresh behavior using the new inheritance chain.
3. Add regression tests for refresh-rule persistence, inheritance, and scheduling behavior.
4. Add a Homebrew tap layout or equivalent metadata so macOS users can install the app via `brew` in addition to downloading release artifacts.
5. Review the user-facing mutation flows for obvious crash hazards and convert any reachable hard failures into recoverable errors with tests.
6. Update release and installation docs so the supported install paths and refresh behavior are explicit.

## Validation
- `cargo test --workspace`
- `swift test --disable-sandbox`
- `flutter test`
- `tooling/scripts/release_check.sh`
- Local install smoke checks for the packaged artifacts where the platform is available

## Risks
- A cross-language settings change can require coordinated updates in Rust, Swift, and Dart at the same time.
- Homebrew metadata may need release-specific URLs or checksums, so the tap may require a follow-up automation once the tap format is chosen.
- Installing refresh rules at the wrong abstraction level could make the UI confusing unless the effective inheritance is shown clearly.

## Rollback Notes
- If the refresh-rule model is too broad, keep the existing global background refresh settings and revert the new per-feed/per-group overrides.
- If the Homebrew path is unstable, remove the tap metadata and keep the GitHub Releases download path intact.
- If crash-hardening changes disrupt core flows, revert the affected UI adapters while keeping the new tests and model types isolated for a narrower follow-up.
