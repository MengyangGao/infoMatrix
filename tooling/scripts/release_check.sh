#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CORE_DIR="${ROOT_DIR}/core"
APPLE_DIR="${ROOT_DIR}/apps/apple"
FLUTTER_DIR="${ROOT_DIR}/apps/flutter"
APPLE_PROJECT="${APPLE_DIR}/XcodeGen/InfoMatrix.xcodeproj"
DERIVED_DATA="${ROOT_DIR}/.derivedData/release-check"

mkdir -p "${DERIVED_DATA}"

step() {
  echo
  echo "==> $*"
}

step "Rust format check"
(cd "${CORE_DIR}" && cargo fmt --all --check)

step "Rust clippy"
(cd "${CORE_DIR}" && cargo clippy --workspace --all-targets -- -D warnings)

step "Rust tests"
(cd "${CORE_DIR}" && cargo test --workspace)

step "Build Apple XCFramework"
"${ROOT_DIR}/tooling/scripts/build_apple_xcframework.sh"

step "Apple Swift package tests"
(cd "${APPLE_DIR}" && swift test --disable-sandbox)

step "Apple macOS Xcode build"
xcodebuild \
  -project "${APPLE_PROJECT}" \
  -scheme InfoMatrix-macOS \
  -configuration Release \
  -destination "platform=macOS" \
  -derivedDataPath "${DERIVED_DATA}" \
  build

step "Apple iOS Xcode build"
xcodebuild \
  -project "${APPLE_PROJECT}" \
  -scheme InfoMatrix-iOS \
  -configuration Release \
  -sdk iphonesimulator \
  -derivedDataPath "${DERIVED_DATA}" \
  build

step "Flutter tests"
(cd "${FLUTTER_DIR}" && flutter test)

step "Flutter macOS build"
(cd "${FLUTTER_DIR}" && flutter build macos)

step "Flutter iOS simulator build"
(cd "${FLUTTER_DIR}" && flutter build ios --simulator)

echo
echo "Release check completed."
