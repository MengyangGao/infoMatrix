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
APPLE_DIR="${ROOT_DIR}/apps/apple"
PROJECT="${APPLE_DIR}/XcodeGen/InfoMatrix.xcodeproj"
DIST_DIR="${ROOT_DIR}/dist"
RELEASE_DIR="${DIST_DIR}/releases/ios"
DERIVED_DATA="${ROOT_DIR}/.derivedData/ios-release"
IOS_DEVICE_RELEASE="${INFOMATRIX_IOS_DEVICE_RELEASE:-0}"
IOS_TEAM_ID="${INFOMATRIX_IOS_TEAM_ID:-}"
IOS_EXPORT_METHOD="${INFOMATRIX_IOS_EXPORT_METHOD:-}"
IOS_SIGNING_STYLE="${INFOMATRIX_IOS_SIGNING_STYLE:-automatic}"
IOS_EXPORT_OPTIONS_PLIST="${INFOMATRIX_IOS_EXPORT_OPTIONS_PLIST:-}"
IOS_REQUIRE_DEVICE_RELEASE="${INFOMATRIX_REQUIRE_DEVICE_RELEASE:-0}"

if [[ "${IOS_REQUIRE_DEVICE_RELEASE}" == "1" && "${IOS_DEVICE_RELEASE}" != "1" ]]; then
  echo "iOS official releases require INFOMATRIX_IOS_DEVICE_RELEASE=1." >&2
  exit 1
fi

mkdir -p "${DIST_DIR}" "${RELEASE_DIR}" "${DERIVED_DATA}"
rm -rf "${RELEASE_DIR:?}/"*

echo "[1/2] Building iOS simulator app..."
xcodebuild \
  -project "${PROJECT}" \
  -scheme InfoMatrix-iOS \
  -configuration Release \
  -sdk iphonesimulator \
  -derivedDataPath "${DERIVED_DATA}" \
  build >/dev/null

APP_PATH="$(
  find "${DERIVED_DATA}/Build/Products" \
    -type d \
    -name "InfoMatrix-iOS.app" \
    | head -n 1
)"

if [[ -z "${APP_PATH}" ]]; then
  echo "Unable to locate InfoMatrix-iOS.app in ${DERIVED_DATA}/Build/Products" >&2
  exit 1
fi

STAGE_APP="${DIST_DIR}/InfoMatrix-iOS.app"
rm -rf "${STAGE_APP}"
cp -R "${APP_PATH}" "${STAGE_APP}"

if [[ "${IOS_DEVICE_RELEASE}" == "1" ]]; then
  if [[ -z "${IOS_TEAM_ID}" || -z "${IOS_EXPORT_METHOD}" ]]; then
    echo "INFOMATRIX_IOS_DEVICE_RELEASE=1 requires INFOMATRIX_IOS_TEAM_ID and INFOMATRIX_IOS_EXPORT_METHOD." >&2
    exit 1
  fi

  echo "[2/3] Building iOS device archive..."
  ARCHIVE_PATH="${RELEASE_DIR}/InfoMatrix-iOS.xcarchive"
  rm -rf "${ARCHIVE_PATH}"
  xcodebuild \
    -project "${PROJECT}" \
    -scheme InfoMatrix-iOS \
    -configuration Release \
    DEVELOPMENT_TEAM="${IOS_TEAM_ID}" \
    CODE_SIGN_STYLE="${IOS_SIGNING_STYLE}" \
    -destination "generic/platform=iOS" \
    -archivePath "${ARCHIVE_PATH}" \
    -allowProvisioningUpdates \
    archive >/dev/null

  if [[ -n "${IOS_EXPORT_OPTIONS_PLIST}" ]]; then
    if [[ ! -f "${IOS_EXPORT_OPTIONS_PLIST}" ]]; then
      echo "INFOMATRIX_IOS_EXPORT_OPTIONS_PLIST points to a missing file: ${IOS_EXPORT_OPTIONS_PLIST}" >&2
      exit 1
    fi
    EXPORT_OPTIONS_PLIST="${IOS_EXPORT_OPTIONS_PLIST}"
  else
    EXPORT_OPTIONS_PLIST="${DERIVED_DATA}/InfoMatrix-iOS-export-options.plist"
    cat > "${EXPORT_OPTIONS_PLIST}" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>method</key>
  <string>${IOS_EXPORT_METHOD}</string>
  <key>signingStyle</key>
  <string>${IOS_SIGNING_STYLE}</string>
  <key>teamID</key>
  <string>${IOS_TEAM_ID}</string>
</dict>
</plist>
PLIST
  fi

  EXPORT_PATH="${DERIVED_DATA}/ios-export"
  rm -rf "${EXPORT_PATH}"
  mkdir -p "${EXPORT_PATH}"
  xcodebuild \
    -exportArchive \
    -archivePath "${ARCHIVE_PATH}" \
    -exportOptionsPlist "${EXPORT_OPTIONS_PLIST}" \
    -allowProvisioningUpdates \
    -exportPath "${EXPORT_PATH}" >/dev/null

  IPA_PATH="$(
    find "${EXPORT_PATH}" -type f -name "*.ipa" | head -n 1
  )"
  if [[ -z "${IPA_PATH}" ]]; then
    echo "Unable to locate exported IPA under ${EXPORT_PATH}" >&2
    exit 1
  fi

  FINAL_IPA="${RELEASE_DIR}/InfoMatrix-iOS.ipa"
  rm -f "${FINAL_IPA}"
  cp "${IPA_PATH}" "${FINAL_IPA}"
else
  echo "[2/3] Skipping device archive/export because INFOMATRIX_IOS_DEVICE_RELEASE is not set to 1."
fi

echo "[3/3] Creating simulator ZIP artifact..."
ZIP_PATH="${RELEASE_DIR}/InfoMatrix-iOS-simulator.zip"
rm -f "${ZIP_PATH}"
ditto -c -k --sequesterRsrc --keepParent "${STAGE_APP}" "${ZIP_PATH}"

echo "Done: ${STAGE_APP}"
echo "Done: ${ZIP_PATH}"
