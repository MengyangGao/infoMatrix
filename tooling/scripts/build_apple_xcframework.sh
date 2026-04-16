#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CORE_DIR="${ROOT_DIR}/core"
APPLE_DIR="${ROOT_DIR}/apps/apple"
SHARED_DIR="${APPLE_DIR}/Shared"
BUILD_DIR="${ROOT_DIR}/.derivedData/xcframework"
XCFRAMEWORK_NAME="InfoMatrixCore.xcframework"
OUTPUT_PATH="${APPLE_DIR}/Frameworks/${XCFRAMEWORK_NAME}"

mkdir -p "${BUILD_DIR}"
mkdir -p "$(dirname "${OUTPUT_PATH}")"

# Targets
MACOS_ARM64="aarch64-apple-darwin"
MACOS_X86_64="x86_64-apple-darwin"
IOS_ARM64="aarch64-apple-ios"
IOS_SIM_ARM64="aarch64-apple-ios-sim"
IOS_SIM_X86_64="x86_64-apple-ios"

export MACOSX_DEPLOYMENT_TARGET="${INFOMATRIX_MACOS_DEPLOYMENT_TARGET:-14.0}"
export IPHONEOS_DEPLOYMENT_TARGET="${INFOMATRIX_IOS_DEPLOYMENT_TARGET:-17.0}"

echo "Installing Rust targets..."
rustup target add $MACOS_ARM64 $MACOS_X86_64 $IOS_ARM64 $IOS_SIM_ARM64 $IOS_SIM_X86_64

# Ensure rustup-managed toolchain takes precedence over Homebrew
RUSTUP_BIN="$(rustup which rustc | xargs dirname)"
export PATH="${RUSTUP_BIN}:${PATH}"

echo "Building Rust core for all Apple targets..."
(
    cd "${CORE_DIR}"
    cargo build --release --locked -p ffi_bridge --target $MACOS_ARM64
    cargo build --release --locked -p ffi_bridge --target $MACOS_X86_64
    cargo build --release --locked -p ffi_bridge --target $IOS_ARM64
    cargo build --release --locked -p ffi_bridge --target $IOS_SIM_ARM64
    cargo build --release --locked -p ffi_bridge --target $IOS_SIM_X86_64
)

# Prepare platform libraries
mkdir -p "${BUILD_DIR}/macOS"
lipo -create \
    "${CORE_DIR}/target/${MACOS_ARM64}/release/libffi_bridge.a" \
    "${CORE_DIR}/target/${MACOS_X86_64}/release/libffi_bridge.a" \
    -output "${BUILD_DIR}/macOS/libInfoMatrixCore.a"

mkdir -p "${BUILD_DIR}/iOS"
cp "${CORE_DIR}/target/${IOS_ARM64}/release/libffi_bridge.a" "${BUILD_DIR}/iOS/libInfoMatrixCore.a"

mkdir -p "${BUILD_DIR}/iOS-simulator"
lipo -create \
    "${CORE_DIR}/target/${IOS_SIM_ARM64}/release/libffi_bridge.a" \
    "${CORE_DIR}/target/${IOS_SIM_X86_64}/release/libffi_bridge.a" \
    -output "${BUILD_DIR}/iOS-simulator/libInfoMatrixCore.a"

# Prepare Headers
mkdir -p "${BUILD_DIR}/Headers"
cp "${SHARED_DIR}/InfoMatrixCore.h" "${BUILD_DIR}/Headers/"
cp "${SHARED_DIR}/module.modulemap" "${BUILD_DIR}/Headers/"

# Create XCFramework
rm -rf "${OUTPUT_PATH}"
xcodebuild -create-xcframework \
    -library "${BUILD_DIR}/macOS/libInfoMatrixCore.a" -headers "${BUILD_DIR}/Headers" \
    -library "${BUILD_DIR}/iOS/libInfoMatrixCore.a" -headers "${BUILD_DIR}/Headers" \
    -library "${BUILD_DIR}/iOS-simulator/libInfoMatrixCore.a" -headers "${BUILD_DIR}/Headers" \
    -output "${OUTPUT_PATH}"

echo "XCFramework created at: ${OUTPUT_PATH}"
