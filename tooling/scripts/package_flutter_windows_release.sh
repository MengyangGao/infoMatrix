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
CORE_DIR="${ROOT_DIR}/core"
DIST_DIR="${ROOT_DIR}/dist"
RELEASE_DIR="${DIST_DIR}/releases/windows"
STAGE_DIR="${ROOT_DIR}/.derivedData/windows-release"
BUILD_DIR="${FLUTTER_DIR}/build/windows/x64/runner/Release"
RUST_TARGET="x86_64-pc-windows-msvc"
MSIX_PATH="${RELEASE_DIR}/InfoMatrix-windows-x64.msix"

mkdir -p "${DIST_DIR}" "${RELEASE_DIR}" "${STAGE_DIR}"
rm -rf "${RELEASE_DIR:?}/"*
rm -rf "${STAGE_DIR:?}/"*

echo "[1/3] Building Flutter Windows app..."
(
  cd "${FLUTTER_DIR}"
  flutter build windows --release
)

echo "[2/3] Building Rust FFI library..."
(
  cd "${ROOT_DIR}"
  tooling/scripts/build_flutter_ffi.sh "${RUST_TARGET}" release >/dev/null
)

cp "${CORE_DIR}/target/${RUST_TARGET}/release/ffi_bridge.dll" "${BUILD_DIR}/ffi_bridge.dll"
cp -R "${BUILD_DIR}/." "${STAGE_DIR}/"

EXE_PATH="$(
  find "${STAGE_DIR}" -maxdepth 1 -type f -name "*.exe" | head -n 1
)"
if [[ -z "${EXE_PATH}" ]]; then
  echo "Unable to locate the Flutter Windows executable in ${STAGE_DIR}" >&2
  exit 1
fi

TARGET_EXE="${STAGE_DIR}/InfoMatrix.exe"
mv "${EXE_PATH}" "${TARGET_EXE}"

echo "[3/4] Creating MSIX artifact..."
(
  cd "${FLUTTER_DIR}"
  dart run msix:create --build-windows false --output-path "${RELEASE_DIR}" --output-name "InfoMatrix-windows-x64" >/dev/null
)

GENERATED_MSIX_PATH="$(
  find "${RELEASE_DIR}" -maxdepth 1 -type f -name "*.msix" | head -n 1
)"
if [[ -z "${GENERATED_MSIX_PATH}" ]]; then
  echo "Unable to locate a generated MSIX artifact in ${RELEASE_DIR}" >&2
  exit 1
fi
if [[ "${GENERATED_MSIX_PATH}" != "${MSIX_PATH}" ]]; then
  mv "${GENERATED_MSIX_PATH}" "${MSIX_PATH}"
fi
if [[ ! -f "${MSIX_PATH}" ]]; then
  echo "Unable to locate MSIX at ${MSIX_PATH}" >&2
  exit 1
fi

echo "[4/4] Creating ZIP artifact..."
ZIP_PATH="${RELEASE_DIR}/InfoMatrix-windows-x64.zip"
rm -f "${ZIP_PATH}"
STAGE_DIR_WIN="$(cygpath -w "${STAGE_DIR}")"
ZIP_PATH_WIN="$(cygpath -w "${ZIP_PATH}")"
powershell.exe -NoProfile -Command \
  "Compress-Archive -Path '${STAGE_DIR_WIN}\\*' -DestinationPath '${ZIP_PATH_WIN}' -Force" >/dev/null

echo "Done: ${TARGET_EXE}"
echo "Done: ${MSIX_PATH}"
echo "Done: ${ZIP_PATH}"
