#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FLUTTER_DIR="${ROOT_DIR}/apps/flutter"
CORE_DIR="${ROOT_DIR}/core"
DIST_DIR="${ROOT_DIR}/dist"
RELEASE_DIR="${DIST_DIR}/releases/linux"
STAGE_DIR="${ROOT_DIR}/.derivedData/linux-release"
BUILD_DIR="${FLUTTER_DIR}/build/linux/x64/release/bundle"
RUST_TARGET="x86_64-unknown-linux-gnu"

mkdir -p "${DIST_DIR}" "${RELEASE_DIR}" "${STAGE_DIR}"
rm -rf "${RELEASE_DIR:?}/"*
rm -rf "${STAGE_DIR:?}/"*

echo "[1/3] Building Flutter Linux app..."
(
  cd "${FLUTTER_DIR}"
  flutter build linux --release
)

echo "[2/3] Building Rust FFI library..."
(
  cd "${ROOT_DIR}"
  tooling/scripts/build_flutter_ffi.sh "${RUST_TARGET}" release >/dev/null
)

APP_DIR="${STAGE_DIR}/InfoMatrix-linux"
mkdir -p "${APP_DIR}"
cp -R "${BUILD_DIR}/." "${APP_DIR}/"
cp "${CORE_DIR}/target/${RUST_TARGET}/release/libffi_bridge.so" "${APP_DIR}/libffi_bridge.so"

EXE_PATH="${APP_DIR}/infomatrix_shell"
if [[ ! -f "${EXE_PATH}" ]]; then
  echo "Unable to locate the Flutter Linux executable in ${APP_DIR}" >&2
  exit 1
fi

mv "${EXE_PATH}" "${APP_DIR}/InfoMatrix"
chmod +x "${APP_DIR}/InfoMatrix"

mkdir -p "${STAGE_DIR}/debroot/DEBIAN"
mkdir -p "${STAGE_DIR}/debroot/opt/InfoMatrix"
mkdir -p "${STAGE_DIR}/debroot/usr/bin"
mkdir -p "${STAGE_DIR}/debroot/usr/share/applications"

cp -R "${APP_DIR}/." "${STAGE_DIR}/debroot/opt/InfoMatrix/"
ln -sf /opt/InfoMatrix/InfoMatrix "${STAGE_DIR}/debroot/usr/bin/infomatrix"

cat > "${STAGE_DIR}/debroot/usr/share/applications/infomatrix.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=InfoMatrix
Exec=/opt/InfoMatrix/InfoMatrix
Icon=infomatrix
Categories=Network;Office;
Terminal=false
DESKTOP

LINUX_VERSION="${INFOMATRIX_VERSION:-0.1.0}"
LINUX_VERSION="${LINUX_VERSION#v}"

cat > "${STAGE_DIR}/debroot/DEBIAN/control" <<CONTROL
Package: infomatrix
Version: ${LINUX_VERSION}
Section: net
Priority: optional
Architecture: amd64
Maintainer: InfoMatrix <noreply@example.com>
Description: InfoMatrix local-first reader
CONTROL

DEB_PATH="${RELEASE_DIR}/InfoMatrix-linux-x64.deb"
rm -f "${DEB_PATH}"
dpkg-deb --build "${STAGE_DIR}/debroot" "${DEB_PATH}" >/dev/null

echo "Done: ${APP_DIR}"
echo "Done: ${DEB_PATH}"
