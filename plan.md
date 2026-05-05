# Objective
Fix ten correctness and contract-alignment issues across the InfoMatrix Rust core, FFI bridge, Swift shell, and Flutter shell.

# User Value
Users get more reliable RSS subscription, OPML import, feed parsing, search, read-it-later, and memo behavior without changing the app's local-first architecture.

# Constraints
- Keep changes tightly scoped to the planned bug fixes.
- Do not add dependencies unless a fix cannot be done safely with the existing stack.
- Preserve the Rust-owned parsing/fetching/persistence boundary.
- Keep SwiftUI and Flutter shells aligned with the existing FFI contract.
- Add focused regression tests for non-trivial behavior changes.

# Assumptions
- [ASSUMPTION] This pass prioritizes correctness and contract alignment over visual redesign.
- [ASSUMPTION] Invalid direct feed URLs should fail instead of creating empty unknown subscriptions.
- [ASSUMPTION] Website discovery remains the supported fallback path for non-feed site URLs.
- [ASSUMPTION] No dependency changes are required.

# Affected Files
- `plan.md`
- `core/crates/parser/src/lib.rs`
- `core/crates/opml/src/lib.rs`
- `core/crates/app_server/src/main.rs`
- `core/crates/ffi_bridge/src/lib.rs`
- `apps/apple/Shared/NativeReaderService.swift`
- `apps/apple/Tests/InfoMatrixShellTests/NativeReaderServiceTests.swift`
- `apps/flutter/lib/ui/reader_shell_page.dart`
- `apps/flutter/test/widget_test.dart`

# Steps
1. Replace the stale release-focused plan with this implementation plan.
2. Fix parser stability and feed metadata issues:
   - use deterministic SHA-256-derived item IDs instead of `DefaultHasher`
   - prefer Atom article links over feed/self links
   - prefer Atom home/site links for feed `site_url`
   - fall back to Atom summary text when content is absent
   - validate and escape RSS image enclosure URLs before generating HTML
3. Fix OPML and subscription validation:
   - reject non-web OPML `xmlUrl` and `htmlUrl`
   - prevent direct feed subscription from persisting unknown placeholder feeds after probe failure in both HTTP and FFI entrypoints
4. Fix FFI/shell contract gaps:
   - make `infomatrix_core_list_items_json` honor optional search query `q`
   - make Swift `addSubscription` decode `feed_id`
   - route Swift feed-scoped search through a searchable endpoint
5. Fix Flutter local-first shell behavior:
   - load special scopes even when there are no RSS subscriptions
   - keep saved bookmarks visible even when optional full-text extraction fails
6. Add focused regression tests for each changed behavior.
7. Run Rust, Swift, and Flutter validation.

# Validation
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `swift test`
- `flutter test`

# Risks
- Parser ID changes could affect existing entries whose old IDs were generated with `DefaultHasher`; Rust's default hasher was already not guaranteed stable, but this may still create duplicate rows for some previously persisted items.
- Rejecting unknown direct feed subscriptions may remove a permissive workflow that someone used for manually staging feed URLs before they were reachable.
- Full-text failure handling in Flutter must not mask failures from the actual bookmark creation or later-state mutation.
- OPML files containing local file paths or custom schemes will now fail instead of partially importing.
- Atom link preference changes could alter canonical URLs for feeds that rely on unusual link ordering.

# Rollback Notes
- If parser ID changes cause unacceptable duplicate entries, revert only the deterministic ID change and keep the other parser fixes.
- If direct placeholder subscriptions are needed, add an explicit "manual placeholder" flow instead of restoring silent fallback in FFI direct add.
- If OPML validation proves too strict, loosen only `htmlUrl` handling while keeping `xmlUrl` web-only.
