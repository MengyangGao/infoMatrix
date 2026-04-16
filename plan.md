# InfoMatrix Feed UX Improvements

## Objective
Improve the subscription and reading workflow so a user can subscribe with only a domain name, automatically fetch full text by default, toggle that behavior per subscription, and avoid duplicated article titles in the Apple detail view.

## User Value
- Domain-only input like `ruanyifeng.com` can resolve common RSS/Atom locations without forcing the user to guess the exact feed path.
- Full-text extraction becomes the default reading behavior, which reduces extra taps on article-heavy feeds.
- Users can opt out per feed from the sidebar context menu when a source is noisy or slow.
- The Apple reader detail pane becomes cleaner by showing the article title only once.
- The README screenshot will match the current app UI.

## Constraints
- Keep the subscription logic deterministic and testable.
- Preserve local-first storage and existing refresh behavior.
- Avoid coupling network probing to view code.
- Keep the schema change backward-compatible for existing databases.

## Assumptions
- [ASSUMPTION] The new auto-full-text preference should default to enabled for new and existing subscriptions.
- [ASSUMPTION] Apple is the primary shell for the right-click toggle, but the setting must be stored in core so other shells can honor it.
- [ASSUMPTION] The README screenshot can be replaced with the provided image asset or an equivalent capture of the current UI if the attachment is not directly accessible from disk.

## Overlooked Risks
1. Broader feed-path probing can increase network requests and slow subscription discovery on some sites.
2. Adding a feed-level preference requires schema migration and contract updates across Rust, Apple, and Flutter, so one missed decoder can break startup.
3. Auto full-text on selection may increase fetch traffic and could surface parsing edge cases for sites that block article scraping.

## Affected Files
- `README.md`
- `assets/README/macos-main.png`
- `core/crates/models/src/lib.rs`
- `core/crates/storage/src/lib.rs`
- `core/crates/storage/migrations/0001_init.sql`
- `core/crates/app_core/src/lib.rs`
- `core/crates/app_server/src/main.rs`
- `core/crates/ffi_bridge/src/lib.rs`
- `apps/apple/Shared/Models.swift`
- `apps/apple/Shared/ReaderService.swift`
- `apps/apple/Shared/NativeReaderService.swift`
- `apps/apple/Shared/AppState.swift`
- `apps/apple/Shared/ReaderShellView.swift`
- `apps/apple/Tests/InfoMatrixShellTests/AppStateTests.swift`
- `apps/flutter/lib/core/models.dart`
- `apps/flutter/lib/core/reader_backend.dart`
- `apps/flutter/lib/core/ffi_reader_backend.dart`
- `apps/flutter/lib/ui/reader_shell_page.dart`
- `apps/flutter/test/widget_test.dart`

## Steps
1. Replace the README screenshot asset and keep the README reference stable.
2. Extend feed storage and API contracts with an `auto_full_text` setting defaulting to enabled.
3. Teach subscription discovery to probe a wider set of common feed path patterns from domain-only input.
4. Wire Apple and Flutter shells to honor the new preference when loading article details, and expose a sidebar control on Apple to toggle it.
5. Remove the duplicated article title from the Apple detail header.
6. Add or update regression tests for discovery inference, auto-full-text behavior, and model decoding.

## Validation
- `cargo test --workspace --locked`
- `swift test --disable-sandbox`
- `flutter test`
- Run focused tests for the updated subscription and reader-state paths if full workspace runs expose regressions.

## Risks
- Existing databases need the new feed column to be added safely on open.
- More aggressive discovery probing may need tuning if it hits slow or rate-limited sites.
- Changing the selected-item hydration path may affect tests that currently expect manual full-text fetch only.

## Rollback Notes
- Revert the schema, API, and UI changes together if any shell stops decoding feeds or loading details correctly.
- Keep the README image swap independent so it can remain even if a code rollback is needed.
