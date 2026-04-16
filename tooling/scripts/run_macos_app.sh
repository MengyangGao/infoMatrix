#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

"${ROOT_DIR}/tooling/scripts/package_macos_app.sh"
open -W "${ROOT_DIR}/dist/InfoMatrix.app"
