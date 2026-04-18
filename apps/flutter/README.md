# InfoMatrix Flutter App (Android / Windows / Linux)

Flutter shell backed by the shared Rust `app_core` orchestration layer through the FFI bridge.

## Current capabilities

- Add subscription by feed URL
- Discover feed by website URL
- List feeds, groups, and items
- Refresh feed and refresh due feeds
- Create notes and read-later entries
- Mark read / starred / save-later / archive
- Local SQLite persistence through Rust core

## End-User Distribution

The Flutter shell is the production path for Windows, Linux, and Android.

- Windows users should install `InfoMatrix-windows-x64.msix`, or unpack `InfoMatrix-windows-x64.zip` if they need a portable bundle.
- Linux users should install `InfoMatrix-linux-x64.deb`.
- Android users should install `InfoMatrix-android.apk`, or use `InfoMatrix-android.aab` for Play-style distribution.

These packages are published from the GitHub Releases page and are built from the same Rust core contract as the Apple shell.

## Build Rust FFI library

From repository root:

```bash
tooling/scripts/build_flutter_ffi.sh
```

Optional target + profile:

```bash
tooling/scripts/build_flutter_ffi.sh aarch64-linux-android release
```

Stage Android ABI libraries into `android/app/src/main/jniLibs`:

```bash
tooling/scripts/stage_flutter_android_ffi.sh --all
```

## Run Flutter app

From `apps/flutter`:

```bash
flutter pub get
flutter run -d macos
```

If dynamic library path is custom, set:

```bash
export INFOMATRIX_FFI_LIB_PATH=/absolute/path/to/libffi_bridge.dylib
```

(Use `.so` on Linux/Android, `.dll` on Windows.)

## Release Notes

- Flutter behavior should mirror the Apple shell for the shared reader workflows.
- Keep `apps/flutter/lib/core/ffi_reader_backend.dart` aligned with the exported Rust FFI contract.
- Use the Rust workspace tests as the source of truth for core behavior before packaging a Flutter build.
- `tooling/scripts/release_check.sh` is the preferred local release gate because it runs Rust, Apple, and Flutter checks in one pass.
- The release workflow publishes pre-built artifacts for Windows, Linux, and Android so new users do not need to compile Flutter locally.
- For Android release signing, copy `apps/flutter/android/key.properties.example` to `apps/flutter/android/key.properties` and fill in the standard Gradle signing fields. If you need an explicit smoke-only package, set `INFOMATRIX_ANDROID_ALLOW_DEBUG_SIGNING=1`; otherwise the release build requires real signing material.
- Release packaging helpers:
  - `tooling/scripts/package_flutter_android_release.sh` creates `dist/releases/android/InfoMatrix-android.apk` and `dist/releases/android/InfoMatrix-android.aab`
  - `tooling/scripts/package_flutter_windows_release.sh` creates `dist/releases/windows/InfoMatrix-windows-x64.msix` and `dist/releases/windows/InfoMatrix-windows-x64.zip`
  - `tooling/scripts/package_flutter_linux_release.sh` creates `dist/releases/linux/InfoMatrix-linux-x64.deb`
- The Windows MSIX is the primary installer; the zip contains `InfoMatrix.exe` and the bundled runtime files for manual unpacking or fallback distribution.
- The Android APK is the direct-install artifact and the AAB is the store-style artifact.
- The Linux deb installs `InfoMatrix` under `/opt/InfoMatrix` and provides an `infomatrix` launcher.

## Platform notes

- Android build requires local Android SDK (`flutter doctor` should be green).
- Windows/Linux desktop builds need corresponding host toolchains.
- Android release packaging requires Rust Android targets + NDK toolchain available for `cargo`.
- The app attempts library load in this order:
  1. `INFOMATRIX_FFI_LIB_PATH`
  2. `core/target/debug`
  3. `core/target/release`
  4. current working directory
  5. executable directory and bundle-relative paths for packaged desktop builds
