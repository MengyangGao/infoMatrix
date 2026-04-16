# AGENTS.md

## Project
InfoMatrix is a cross-platform, privacy-respecting RSS reader.

## Core direction
- Local-first by default.
- Rust owns discovery, parsing, fetching, persistence, icons, and sync foundations.
- SwiftUI is the Apple shell.
- Flutter is the shell for Windows, Linux, and Android.
- Important state belongs in SQLite.
- Fetching and reading behavior must stay deterministic and testable.

## When changing code
1. Inspect the repository structure and the relevant setup first.
2. Keep changes small and reviewable.
3. Update docs when architecture, discovery, or schema changes.
4. Run the relevant tests and builds.
5. Summarize the result and any follow-up risk.

## Non-negotiables
- Do not couple networking directly to view code.
- Do not keep parsing logic in UI layers.
- Do not introduce opaque ranking or telemetry by default.
- Do not block future sync with local-only shortcuts.
