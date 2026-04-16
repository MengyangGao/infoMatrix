#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${INFOMATRIX_BASE_URL:-http://127.0.0.1:3199}"
SITE_URL=""
FEED_URL=""
FEED_ID=""

usage() {
  cat <<'EOF'
Usage:
  measure_reader_latency.sh [--base-url URL] [--site-url URL] [--feed-url URL] [--feed-id ID]

Examples:
  tooling/scripts/measure_reader_latency.sh --site-url https://blog.example.com
  tooling/scripts/measure_reader_latency.sh --feed-url https://blog.example.com/feed.xml
  tooling/scripts/measure_reader_latency.sh --feed-id <existing-feed-id>
EOF
}

measure() {
  local label="$1"
  shift
  local seconds
  seconds="$(curl -sS -o /dev/null -w '%{time_total}' "$@")"
  printf '%-16s %s s\n' "$label:" "$seconds"
}

measure_with_body() {
  local label="$1"
  local body_file
  body_file="$(mktemp)"
  shift
  local seconds
  seconds="$(curl -sS -o "${body_file}" -w '%{time_total}' "$@")"
  printf '%-16s %s s\n' "$label:" "$seconds" >&2
  cat "${body_file}"
  rm -f "${body_file}"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --base-url)
      BASE_URL="$2"
      shift 2
      ;;
    --site-url)
      SITE_URL="$2"
      shift 2
      ;;
    --feed-url)
      FEED_URL="$2"
      shift 2
      ;;
    --feed-id)
      FEED_ID="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

echo "base_url: ${BASE_URL}"
measure health "${BASE_URL}/api/v1/health"

if [[ -n "${SITE_URL}" ]]; then
  measure discover \
    -X POST \
    -H 'Content-Type: application/json' \
    --data "{\"site_url\":\"${SITE_URL}\"}" \
    "${BASE_URL}/api/v1/discover"
fi

if [[ -n "${FEED_URL}" ]]; then
  subscribe_body="$(measure_with_body subscribe \
    -X POST \
    -H 'Content-Type: application/json' \
    --data "{\"feed_url\":\"${FEED_URL}\"}" \
    "${BASE_URL}/api/v1/subscriptions")"
  feed_id="$(python -c 'import json,sys; print(json.load(sys.stdin)["feed_id"])' <<<"${subscribe_body}")"
  if [[ -n "${feed_id}" ]]; then
    measure items "${BASE_URL}/api/v1/feeds/${feed_id}/items?limit=5"
  fi
fi

if [[ -n "${FEED_ID}" ]]; then
  measure refresh \
    -X POST \
    "${BASE_URL}/api/v1/refresh/${FEED_ID}"
fi
