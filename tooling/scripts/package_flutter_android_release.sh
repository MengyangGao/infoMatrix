#!/usr/bin/env bash
set -Eeuo pipefail

on_error() {
  local exit_code="$1"
  local line_no="$2"
  local command="$3"
  echo "::error file=${BASH_SOURCE[0]},line=${line_no}::Command failed with exit code ${exit_code}: ${command}" >&2
}

trap 'on_error "$?" "${BASH_LINENO[0]}" "$BASH_COMMAND"' ERR

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FLUTTER_DIR="${ROOT_DIR}/apps/flutter"
DIST_DIR="${ROOT_DIR}/dist"
RELEASE_DIR="${DIST_DIR}/releases/android"
APK_SOURCE="${FLUTTER_DIR}/build/app/outputs/flutter-apk/app-release.apk"
AAB_SOURCE="${FLUTTER_DIR}/build/app/outputs/bundle/release/app-release.aab"

mkdir -p "${DIST_DIR}" "${RELEASE_DIR}"
rm -rf "${RELEASE_DIR:?}/"*

ANDROID_KEY_PROPERTIES="${FLUTTER_DIR}/android/key.properties"
ANDROID_REQUIRE_SIGNED_RELEASE="${INFOMATRIX_REQUIRE_SIGNED_RELEASE:-0}"
if [[ ! -f "${ANDROID_KEY_PROPERTIES}" && "${ANDROID_REQUIRE_SIGNED_RELEASE}" == "1" ]]; then
  cat >&2 <<EOF
Android official releases require release signing material.
Create ${ANDROID_KEY_PROPERTIES} from key.properties.example before running this script.
EOF
  exit 1
fi
if [[ ! -f "${ANDROID_KEY_PROPERTIES}" && "${INFOMATRIX_ANDROID_ALLOW_DEBUG_SIGNING:-0}" != "1" ]]; then
  cat >&2 <<EOF
Android release signing is required for this package.
Create ${ANDROID_KEY_PROPERTIES} from key.properties.example before running this script,
or set INFOMATRIX_ANDROID_ALLOW_DEBUG_SIGNING=1 for an explicit smoke-only build.
EOF
  exit 1
fi
if [[ ! -f "${ANDROID_KEY_PROPERTIES}" && "${INFOMATRIX_ANDROID_ALLOW_DEBUG_SIGNING:-0}" == "1" ]]; then
  echo "Using explicit smoke-only Android debug signing because INFOMATRIX_ANDROID_ALLOW_DEBUG_SIGNING=1 is set." >&2
fi

echo "[1/4] Staging Android Rust libraries..."
"${ROOT_DIR}/tooling/scripts/stage_flutter_android_ffi.sh" --all

echo "[2/4] Building Android APK..."
(
  cd "${FLUTTER_DIR}"
  flutter build apk --release
)

echo "[3/4] Building Android App Bundle..."
(
  cd "${FLUTTER_DIR}"
  flutter build appbundle --release
)

if [[ ! -f "${APK_SOURCE}" ]]; then
  echo "Unable to locate APK at ${APK_SOURCE}" >&2
  exit 1
fi

if [[ ! -f "${AAB_SOURCE}" ]]; then
  echo "Unable to locate AAB at ${AAB_SOURCE}" >&2
  exit 1
fi

APK_DEST="${RELEASE_DIR}/InfoMatrix-android.apk"
AAB_DEST="${RELEASE_DIR}/InfoMatrix-android.aab"
cp "${APK_SOURCE}" "${APK_DEST}"
cp "${AAB_SOURCE}" "${AAB_DEST}"

echo "[4/4] Android release artifacts ready."
echo "Done: ${APK_DEST}"
echo "Done: ${AAB_DEST}"
