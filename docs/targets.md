# Product Targets

## Baseline

InfoMatrix is guided by the goal of being a fast, privacy-respecting, local-first RSS reader with a polished reading flow and strong saved-item workflows.

## Direction

1. Direct feed subscription should be the fastest path.
2. Website URL discovery should be reliable and explainable.
3. Reading and triage should feel smooth on desktop and touch devices.
4. Local state should remain deterministic, inspectable, and sync-ready.
5. The core should stay testable and maintainable over time.

## Guardrails

1. Avoid opaque ranking in the core inbox.
2. Avoid cloud dependence in MVP.
3. Avoid binding view code directly to networking or parsing logic.
4. Avoid hardcoding per-site shortcuts as the only discovery path.
