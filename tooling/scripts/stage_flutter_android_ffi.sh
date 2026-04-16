#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD_SCRIPT="$ROOT_DIR/tooling/scripts/build_flutter_ffi.sh"
JNI_LIBS_DIR="$ROOT_DIR/apps/flutter/android/app/src/main/jniLibs"
ANDROID_FFI_PROFILE="${INFOMATRIX_ANDROID_FFI_PROFILE:-release}"

if [[ ! -x "$BUILD_SCRIPT" ]]; then
  echo "Missing build script: $BUILD_SCRIPT" >&2
  exit 1
fi

copy_one() {
  local target="$1"
  local abi="$2"

  "$BUILD_SCRIPT" "$target" "$ANDROID_FFI_PROFILE"

  local source_lib="$ROOT_DIR/core/target/$target/release/libffi_bridge.so"
  if [[ "$ANDROID_FFI_PROFILE" != "release" ]]; then
    source_lib="$ROOT_DIR/core/target/$target/$ANDROID_FFI_PROFILE/libffi_bridge.so"
  fi
  local target_dir="$JNI_LIBS_DIR/$abi"
  mkdir -p "$target_dir"
  cp "$source_lib" "$target_dir/libffi_bridge.so"
  echo "Staged $source_lib -> $target_dir/libffi_bridge.so"
}

if [[ "${1:-}" == "--all" || "$#" -eq 0 ]]; then
  copy_one "aarch64-linux-android" "arm64-v8a"
  copy_one "armv7-linux-androideabi" "armeabi-v7a"
  copy_one "x86_64-linux-android" "x86_64"
  copy_one "i686-linux-android" "x86"
  exit 0
fi

if [[ "$#" -ne 2 ]]; then
  echo "Usage:" >&2
  echo "  $0 --all" >&2
  echo "  $0 <rust-target-triple> <android-abi-dir>" >&2
  exit 1
fi

copy_one "$1" "$2"
