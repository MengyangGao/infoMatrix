# Feed Discovery

## Pipeline

1. Normalize input URL
2. Fetch entry HTML page (best effort, tolerate failures)
3. Parse feed autodiscovery links (`rss`, `atom`, `json`)
4. Resolve relative candidate URLs
5. Parse heuristic anchor links that look like feed links (`atom`, `rss`, `feed`)
6. Validate discovered candidates with parser probes
7. Probe fallback feed paths on both site root and current path context (`/feed`, `/rss`, `/rss.xml`, `/feed.xml`, `/atom.xml`, `/index.xml`, `/posts.atom`, plus relative variants like `atom.xml`)
8. If entry page fails (4xx/5xx/network), continue probing common feed paths directly instead of hard-failing
9. Persist diagnostics and warnings
10. Validate discovered candidates concurrently so slow or missing feeds do not block the entire discovery result
11. Rank validated candidates deterministically using explicit heuristics
   - source priority (`autodiscovery` > `heuristic_link` > `commonpath`)
   - URL/title penalties for comment/podcast-only feeds
   - URL bonuses for common primary-feed patterns (`/feed`, `rss`, `/index.xml`)
12. Cache the validated discovery snapshot locally so a later subscription can reuse the parsed feed result instead of refetching the same body

## Direct Feed Guardrails

- If user input is a direct feed URL, the payload must pass parser validation before it is saved as a subscription.
- Content type hints alone must not be trusted (for example, generic `text/xml` responses).

## Subscription Side Effects

- `POST /api/v1/subscribe` accepts either website URL or direct feed URL.
- For direct feed URLs, backend validates parser success first, then stores the subscription.
- For website URLs, backend runs discovery and auto-selects the highest-confidence valid feed.
- The first parsed feed snapshot is persisted during subscription so the initial article list is available immediately after the write completes.
- Flutter FFI bridge now provides `subscribe_input` with the same strategy:
  - direct feed probe and parser validation first
  - discovery fallback when direct probe is not a feed
- Apple SwiftUI shell now uses explicit candidate selection when discovery returns multiple valid feeds:
  - run `POST /api/v1/discover`
  - present discovered candidates to the user
  - subscribe chosen candidate via `POST /api/v1/subscriptions`
- Shell clients keep a direct-feed fallback path for ambiguous or discovery-failing URLs so subscription still works when a feed URL is not obvious from the path alone.
- Discovery no longer requires the entry page to be reachable; it can still succeed from fallback probes.
- After subscription is created, backend still resolves a feed icon candidate from site HTML, but icon caching is now treated as background work so subscription completion is not blocked on icon downloads.
- Discovery cache rows are used to avoid a second fetch when a user subscribes to a feed that was already validated during a discovery flow.

## Discovery Output

- `normalized_site_url`
- `discovered_feeds[]` with:
  - `url`
  - `title`
  - `type`
  - `confidence`
  - `score`
  - `source`
- `site_title`
- `site_icon_candidates[]`
- `warnings[]`

## Operational Constraints

- redirect loop protection
- timeout and retry policy
- gzip/br/deflate support
- explicit User-Agent
- relative URL resolution
