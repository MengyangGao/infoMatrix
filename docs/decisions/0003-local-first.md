# ADR 0003: Local-First State Ownership

- Status: Accepted
- Date: 2026-03-15

## Decision
All essential subscription and reading state must be representable and operable locally in SQLite.

## Rationale
This preserves user ownership, privacy, offline functionality, and enables multiple future sync backends without product lock-in.
