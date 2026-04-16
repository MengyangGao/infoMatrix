# Contributing to InfoMatrix

Thanks for helping improve InfoMatrix.

## Development workflow

1. Make focused, reviewable changes.
2. Keep core business logic in Rust crates, not UI layers.
3. Add or update tests for behavior changes.
4. Run Rust tests before opening a PR.
5. Update `docs/` when architecture, schema, or discovery behavior changes.

## Local checks

From `core/`:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Product constraints

- Local-first by default.
- No telemetry/analytics by default.
- No cloud dependency required for MVP functionality.
- Treat feed/article content as untrusted input.

For full project conventions, see `AGENTS.md`.
