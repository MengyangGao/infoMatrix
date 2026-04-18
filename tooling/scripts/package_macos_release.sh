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
DIST_DIR="${ROOT_DIR}/dist"
RELEASE_DIR="${DIST_DIR}/releases/macos"
STAGE_DIR="${ROOT_DIR}/.derivedData/macos-release"
APPLE_PROJECT="${ROOT_DIR}/apps/apple/XcodeGen/InfoMatrix.xcodeproj"
APP_NAME="InfoMatrix"
APP_BUNDLE="${APP_NAME}.app"
APP_DIR="${DIST_DIR}/${APP_BUNDLE}"
XCODE_BUILD_DIR="${STAGE_DIR}/xcodebuild"

mkdir -p "${DIST_DIR}" "${RELEASE_DIR}"
rm -rf "${RELEASE_DIR:?}/"*
rm -rf "${APP_DIR}" "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}"

VERSION="${INFOMATRIX_VERSION:-0.1.0}"
VERSION="${VERSION#v}"
BUILD_NUMBER="${INFOMATRIX_BUILD_NUMBER:-$(date +%Y%m%d%H%M%S)}"
MACOS_SIGNING_IDENTITY="${INFOMATRIX_MACOS_SIGNING_IDENTITY:-}"
MACOS_NOTARIZE="${INFOMATRIX_MACOS_NOTARIZE:-0}"
MACOS_NOTARY_PROFILE="${INFOMATRIX_MACOS_NOTARY_PROFILE:-}"
MACOS_NOTARY_APPLE_ID="${INFOMATRIX_MACOS_NOTARY_APPLE_ID:-}"
MACOS_NOTARY_APP_PASSWORD="${INFOMATRIX_MACOS_NOTARY_APPLE_PASSWORD:-}"
MACOS_NOTARY_TEAM_ID="${INFOMATRIX_MACOS_NOTARY_TEAM_ID:-}"
MACOS_REQUIRE_SIGNED_RELEASE="${INFOMATRIX_REQUIRE_SIGNED_RELEASE:-0}"
MACOS_REQUIRE_NOTARIZED_RELEASE="${INFOMATRIX_REQUIRE_NOTARIZED_RELEASE:-0}"
MACOS_REQUIRE_DMG_RELEASE="${INFOMATRIX_REQUIRE_DMG_RELEASE:-0}"

if [[ "${MACOS_REQUIRE_SIGNED_RELEASE}" == "1" && -z "${MACOS_SIGNING_IDENTITY}" ]]; then
  echo "macOS official releases require INFOMATRIX_MACOS_SIGNING_IDENTITY." >&2
  exit 1
fi

if [[ "${MACOS_NOTARIZE}" == "1" && -z "${MACOS_SIGNING_IDENTITY}" ]]; then
  echo "INFOMATRIX_MACOS_NOTARIZE=1 requires INFOMATRIX_MACOS_SIGNING_IDENTITY." >&2
  exit 1
fi

if [[ "${MACOS_REQUIRE_NOTARIZED_RELEASE}" == "1" && "${MACOS_NOTARIZE}" != "1" ]]; then
  echo "macOS official releases require INFOMATRIX_MACOS_NOTARIZE=1." >&2
  exit 1
fi

sign_item() {
  local item="$1"
  if [[ -n "${MACOS_SIGNING_IDENTITY}" ]]; then
    codesign --force --options runtime --timestamp --sign "${MACOS_SIGNING_IDENTITY}" "${item}"
  else
    codesign --force --sign - "${item}"
  fi
}

notary_submit() {
  local archive_path="$1"
  if [[ -n "${MACOS_NOTARY_PROFILE}" ]]; then
    xcrun notarytool submit "${archive_path}" --wait --keychain-profile "${MACOS_NOTARY_PROFILE}"
    return
  fi

  if [[ -n "${MACOS_NOTARY_APPLE_ID}" && -n "${MACOS_NOTARY_APP_PASSWORD}" && -n "${MACOS_NOTARY_TEAM_ID}" ]]; then
    xcrun notarytool submit "${archive_path}" --wait \
      --apple-id "${MACOS_NOTARY_APPLE_ID}" \
      --password "${MACOS_NOTARY_APP_PASSWORD}" \
      --team-id "${MACOS_NOTARY_TEAM_ID}"
    return
  fi

  echo "macOS notarization requested but no notarytool credentials were provided." >&2
  echo "Set INFOMATRIX_MACOS_NOTARY_PROFILE or the Apple ID/password/team variables." >&2
  exit 1
}

echo "[1/3] Building Apple XCFramework..."
"${ROOT_DIR}/tooling/scripts/build_apple_xcframework.sh"

echo "[1.5/3] Generating Xcode project..."
(cd "${ROOT_DIR}/apps/apple/XcodeGen" && xcodegen)

echo "[2/3] Building Apple macOS app bundle..."
xcode_log="$(mktemp)"
if ! (
  xcodebuild \
    -project "${APPLE_PROJECT}" \
    -scheme InfoMatrix-macOS \
    -configuration Release \
    -destination "platform=macOS" \
    -derivedDataPath "${XCODE_BUILD_DIR}" \
    MARKETING_VERSION="${VERSION}" \
    CURRENT_PROJECT_VERSION="${BUILD_NUMBER}" \
    build
) >"${xcode_log}" 2>&1; then
  while IFS= read -r line; do
    echo "::error file=${BASH_SOURCE[0]},line=${BASH_LINENO[0]}::${line}" >&2
  done < <(tail -n 80 "${xcode_log}")
  rm -f "${xcode_log}"
  exit 101
fi
rm -f "${xcode_log}"

APP_SOURCE="${XCODE_BUILD_DIR}/Build/Products/Release/InfoMatrix-macOS.app"
if [[ ! -d "${APP_SOURCE}" ]]; then
  echo "Unable to locate built macOS app bundle at ${APP_SOURCE}" >&2
  exit 1
fi

echo "Copying app bundle to release staging area..."
ditto "${APP_SOURCE}" "${APP_DIR}"

echo "[3/3] Signing macOS app bundle..."
sign_item "${APP_DIR}"

if [[ "${MACOS_NOTARIZE}" == "1" ]]; then
  echo "Notarizing macOS app bundle..."
  NOTARY_ZIP_PATH="${STAGE_DIR}/${APP_NAME}-macos-notary.zip"
  rm -f "${NOTARY_ZIP_PATH}"
  ditto -c -k --sequesterRsrc --keepParent "${APP_DIR}" "${NOTARY_ZIP_PATH}"
  notary_submit "${NOTARY_ZIP_PATH}"
  xcrun stapler staple "${APP_DIR}"
  rm -f "${NOTARY_ZIP_PATH}"
  if ! spctl --assess --type execute --verbose "${APP_DIR}" >/dev/null; then
    echo "Gatekeeper verification failed for ${APP_DIR}" >&2
    exit 1
  fi
else
  echo "Skipping notarization because INFOMATRIX_MACOS_NOTARIZE is not set to 1."
fi

echo "Creating ZIP artifact..."
ZIP_PATH="${RELEASE_DIR}/${APP_NAME}-macos.zip"
rm -f "${ZIP_PATH}"
ditto -c -k --sequesterRsrc --keepParent "${APP_DIR}" "${ZIP_PATH}"

echo "Creating DMG artifact..."
DMG_STAGE="${STAGE_DIR}/dmg-root"
rm -rf "${DMG_STAGE}"
mkdir -p "${DMG_STAGE}"
cp -R "${APP_DIR}" "${DMG_STAGE}/${APP_BUNDLE}"
ln -s /Applications "${DMG_STAGE}/Applications"
DMG_PATH="${RELEASE_DIR}/${APP_NAME}-macos.dmg"
rm -f "${DMG_PATH}"
if ! hdiutil create \
  -volname "${APP_NAME}" \
  -srcfolder "${DMG_STAGE}" \
  -ov \
  -format UDZO \
  "${DMG_PATH}" >/dev/null; then
  if [[ "${MACOS_REQUIRE_DMG_RELEASE}" == "1" ]]; then
    echo "macOS official releases require a DMG artifact, but hdiutil could not create one." >&2
    exit 1
  fi
  echo "Warning: unable to create DMG on this host; ZIP artifact remains available." >&2
  rm -f "${DMG_PATH}"
fi

echo "Done: ${APP_DIR}"
echo "Done: ${ZIP_PATH}"
if [[ -f "${DMG_PATH}" ]]; then
  echo "Done: ${DMG_PATH}"
fi
