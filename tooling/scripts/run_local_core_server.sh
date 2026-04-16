#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DB_DIR="${HOME}/Library/Application Support/InfoMatrix"
DB_PATH="${DB_DIR}/infomatrix.db"

mkdir -p "${DB_DIR}"

cd "${ROOT_DIR}/core"
INFOMATRIX_DB_PATH="${DB_PATH}" \
INFOMATRIX_BIND_ADDR="127.0.0.1:3199" \
cargo run -p app_server --release
