#!/usr/bin/env bash
set -Eeuo pipefail

on_error() {
  local exit_code="$1"
  local line_no="$2"
  local command="$3"
  local target_label="${CURRENT_BUILD_TARGET:-host}"
  echo "::error file=${BASH_SOURCE[0]},line=${line_no}::Command failed with exit code ${exit_code} on ${target_label}: ${command}" >&2
}

trap 'on_error "$?" "${BASH_LINENO[0]}" "$BASH_COMMAND"' ERR

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CORE_DIR="$ROOT_DIR/core"

TARGET="${1:-}"
PROFILE="${2:-debug}"

if [[ -z "${CARGO_HOME:-}" ]]; then
  export CARGO_HOME="$ROOT_DIR/.cargo-home"
fi
mkdir -p "$CARGO_HOME"

build_args=(build -p ffi_bridge)
if [[ -n "$TARGET" ]]; then
  build_args+=(--target "$TARGET")
fi
if [[ "$PROFILE" == "release" ]]; then
  build_args+=(--release)
  export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
fi
if [[ "$TARGET" == *android* ]]; then
  build_args+=(-vv)
  export CARGO_BUILD_JOBS=1
  if [[ "$PROFILE" == "release" ]]; then
    export CARGO_PROFILE_RELEASE_OPT_LEVEL=1
    export CARGO_PROFILE_RELEASE_PANIC=abort
  fi
fi

configure_android_linker() {
  local target_triple="$1"
  local env_name
  env_name="$(printf '%s' "$target_triple" | tr 'abcdefghijklmnopqrstuvwxyz-' 'ABCDEFGHIJKLMNOPQRSTUVWXYZ_')"

  local ndk_root="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
  if [[ -z "$ndk_root" ]]; then
    local sdk_root="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
    if [[ -n "$sdk_root" ]]; then
      local candidate
      for candidate in "$sdk_root/ndk/"*; do
        if [[ -d "$candidate/toolchains/llvm/prebuilt" ]]; then
          ndk_root="$candidate"
          break
        fi
      done
    fi
  fi
  if [[ -z "$ndk_root" ]]; then
    echo "Android target ${target_triple} requires ANDROID_NDK_HOME, ANDROID_NDK_ROOT, or an SDK root with ndk/ installed." >&2
    exit 1
  fi

  local clang_name
  case "$target_triple" in
    aarch64-linux-android) clang_name="aarch64-linux-android21-clang" ;;
    armv7-linux-androideabi) clang_name="armv7a-linux-androideabi21-clang" ;;
    x86_64-linux-android) clang_name="x86_64-linux-android21-clang" ;;
    i686-linux-android) clang_name="i686-linux-android21-clang" ;;
    *)
      echo "Unsupported Android Rust target: $target_triple" >&2
      exit 1
      ;;
  esac

  local linker_path
  linker_path="$(
    find "$ndk_root/toolchains/llvm/prebuilt" -type f -name "$clang_name" | head -n 1
  )"
  if [[ -z "$linker_path" ]]; then
    echo "Unable to locate $clang_name under $ndk_root/toolchains/llvm/prebuilt" >&2
    exit 1
  fi

  local tool_dir
  tool_dir="$(dirname "$linker_path")"

  ANDROID_CARGO_CC_HYPHENATED="$linker_path"
  ANDROID_CARGO_CC_UNDERSCORED="$linker_path"
  ANDROID_CARGO_AR_HYPHENATED="${tool_dir}/llvm-ar"
  ANDROID_CARGO_AR_UNDERSCORED="${tool_dir}/llvm-ar"

  export "CARGO_TARGET_${env_name}_LINKER=$linker_path"
  export "CARGO_TARGET_${env_name}_CC=$linker_path"
  export "CC_${env_name}=$linker_path"
  export "CARGO_TARGET_${env_name}_AR=${tool_dir}/llvm-ar"
  export "AR_${env_name}=${tool_dir}/llvm-ar"
}

if [[ "$TARGET" == *android* ]]; then
  configure_android_linker "$TARGET"
fi

CURRENT_BUILD_TARGET="${TARGET:-host}"

cargo_log="$(mktemp)"
if [[ "$TARGET" == *android* ]]; then
  cargo_cmd=(
    env
    "CC_${TARGET}=${ANDROID_CARGO_CC_HYPHENATED:-}"
    "CC_${TARGET//-/_}=${ANDROID_CARGO_CC_UNDERSCORED:-}"
    "AR_${TARGET}=${ANDROID_CARGO_AR_HYPHENATED:-}"
    "AR_${TARGET//-/_}=${ANDROID_CARGO_AR_UNDERSCORED:-}"
    cargo
    "${build_args[@]}"
  )
else
  cargo_cmd=(cargo "${build_args[@]}")
fi

if ! (
  cd "$CORE_DIR"
  "${cargo_cmd[@]}"
) >"${cargo_log}" 2>&1; then
  while IFS= read -r line; do
    echo "::error file=${BASH_SOURCE[0]},line=${BASH_LINENO[0]}::${line}" >&2
  done < <(tail -n 80 "${cargo_log}")
  rm -f "${cargo_log}"
  exit 101
fi
rm -f "${cargo_log}"

if [[ -n "$TARGET" ]]; then
  output_base="$CORE_DIR/target/$TARGET"
  target_key="$TARGET"
else
  output_base="$CORE_DIR/target"
  target_key="$(uname -s | tr '[:upper:]' '[:lower:]')"
fi

if [[ "$PROFILE" == "release" ]]; then
  output_dir="$output_base/release"
else
  output_dir="$output_base/debug"
fi

case "$target_key" in
  *windows*|mingw*|msys*) lib_name="ffi_bridge.dll" ;;
  *linux*) lib_name="libffi_bridge.so" ;;
  *darwin*|*apple*) lib_name="libffi_bridge.dylib" ;;
  *android*) lib_name="libffi_bridge.so" ;;
  *)
    echo "Unsupported target OS in target key: $target_key" >&2
    exit 1
    ;;
esac

lib_path="$output_dir/$lib_name"
if [[ ! -f "$lib_path" ]]; then
  echo "Build finished but output not found: $lib_path" >&2
  exit 1
fi

echo "Built Rust FFI library: $lib_path"
