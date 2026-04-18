# Release Checklist

## Goal
Ship InfoMatrix as a stable local-first reader with matched Apple and Flutter behavior, and publish installable release assets through GitHub Releases.

## Public Distribution

- Public downloads are published from [GitHub Releases](https://github.com/MengyangGao/infoMatrix/releases).
- macOS users can also install the app through the project-owned Homebrew cask tap:
  - `brew tap MengyangGao/infomatrix https://github.com/MengyangGao/infoMatrix`
  - `brew install --cask infomatrix`
- The release workflow publishes installable bundles for macOS, iOS, Windows, and Linux.
- Android packaging remains available for manual smoke builds and store preparation, but it is not published as part of tagged GitHub Releases.
- Artifact names are intentionally stable so users can learn one download path per platform:
  - macOS: `InfoMatrix-macos.dmg`, `InfoMatrix-macos.zip`
  - iOS simulator: `InfoMatrix-iOS-simulator.zip`
  - iOS device: `InfoMatrix-iOS.ipa` when signing inputs are present
  - Windows: `InfoMatrix-windows-x64.msix`, `InfoMatrix-windows-x64.zip`
  - Linux: `InfoMatrix-linux-x64.deb`
- Checksum files are published next to each platform bundle so downloads can be verified before installation.

## Required Checks
- `tooling/scripts/release_check.sh`
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `flutter test`
- Apple platform smoke build or run for the active shell target
- Packaging scripts for the target release artifacts:
  - `tooling/scripts/package_macos_release.sh`
  - `tooling/scripts/package_ios_release.sh`
  - `tooling/scripts/package_flutter_android_release.sh`
  - `tooling/scripts/package_flutter_windows_release.sh`
  - `tooling/scripts/package_flutter_linux_release.sh`

## Packaging Notes
- macOS release bundles are produced from the Xcode-built Apple shell and should keep the native Rust FFI bridge intact. The macOS packaging script emits:
  - `dist/releases/macos/InfoMatrix-macos.dmg`
  - `dist/releases/macos/InfoMatrix-macos.zip`
- If `INFOMATRIX_MACOS_SIGNING_IDENTITY` is set, the app bundle is signed with that identity; if `INFOMATRIX_MACOS_NOTARIZE=1` is also set, the script requires either `INFOMATRIX_MACOS_NOTARY_PROFILE` or the Apple ID/password/team variables and staples the notarization ticket before packaging.
- GitHub Actions can import Apple signing assets from the secrets `INFOMATRIX_APPLE_CERTS_P12_B64`, `INFOMATRIX_APPLE_CERTS_PASSWORD`, `INFOMATRIX_APPLE_KEYCHAIN_PASSWORD`, and `INFOMATRIX_IOS_PROVISIONING_PROFILE_B64` when you want signed macOS or iOS device release artifacts.
- The iOS packaging script emits a simulator app bundle for local testing:
  - `dist/InfoMatrix-iOS.app`
  - `dist/releases/ios/InfoMatrix-iOS-simulator.zip`
- If `INFOMATRIX_IOS_DEVICE_RELEASE=1` is set together with `INFOMATRIX_IOS_TEAM_ID` and `INFOMATRIX_IOS_EXPORT_METHOD`, the script also emits:
  - `dist/releases/ios/InfoMatrix-iOS.xcarchive`
  - `dist/releases/ios/InfoMatrix-iOS.ipa`
  If you need a custom export profile, provide `INFOMATRIX_IOS_EXPORT_OPTIONS_PLIST` and the script will use it instead of generating a minimal plist.
- Flutter release artifacts include the Rust FFI library next to the executable or in the bundle search path so the app works outside the source tree.
- Android release packaging emits:
  - `dist/releases/android/InfoMatrix-android.apk`
  - `dist/releases/android/InfoMatrix-android.aab`
  The APK is the direct-install package; the AAB is for store-style distribution. Release signing requires `apps/flutter/android/key.properties`. This path is for manual smoke builds or Play Console prep, not for tagged GitHub Releases.
- In GitHub Actions, Android signing can be supplied from `INFOMATRIX_ANDROID_KEYSTORE_B64`, `INFOMATRIX_ANDROID_KEYSTORE_PASSWORD`, `INFOMATRIX_ANDROID_KEY_ALIAS`, and `INFOMATRIX_ANDROID_KEY_PASSWORD` when you run the Android packaging path manually.
- Windows release packaging emits:
  - `dist/releases/windows/InfoMatrix-windows-x64.msix`
  - `dist/releases/windows/InfoMatrix-windows-x64.zip`
  The MSIX is the primary Windows installer; the zip contains `InfoMatrix.exe` and the bundled runtime files for manual unpacking or fallback distribution.
- Linux release packaging emits:
  - `dist/releases/linux/InfoMatrix-linux-x64.deb`
  The deb installs the app under `/opt/InfoMatrix` and provides an `infomatrix` launcher.
- Version bumps should keep the app version, release notes, and store metadata aligned.
- `tooling/scripts/release_check.sh` is the preferred local gate before tagging a release because it runs the Rust, Apple, and Flutter checks in one place.
- The Apple script path is intended to validate macOS and iOS on this workstation; visionOS remains a follow-up slice until the Rust XCFramework includes a supported visionOS variant.
- GitHub Release publication is automated by `.github/workflows/release.yml` on `v*` tags.
- Official `v*` tags prefer signed release assets when secrets are available. If the signing material is missing, the workflow falls back to smoke artifacts so GitHub Release still gets populated, but those artifacts are not equivalent to store-ready builds.
- Each platform release directory now includes a platform-specific `InfoMatrix-*.SHA256SUMS` file so users can verify downloaded artifacts before installation.

## Smoke Scenarios
- Add a direct feed subscription.
- Discover a site and subscribe to a candidate feed.
- Refresh a feed and verify updated items appear.
- Create a memo and a read-later item.
- Toggle read/star/later/archive state.
- Import and export OPML.
- Open an entry detail and fetch full text.
- Save a webpage with a blank title and confirm the title/content are auto-captured.

## Usage
### macOS
```bash
tooling/scripts/package_macos_release.sh
open dist/releases/macos/InfoMatrix-macos.dmg
```
Drag `InfoMatrix.app` into Applications after mounting the DMG.
For Homebrew installation, tap the repo and install the cask:
```bash
brew tap MengyangGao/infomatrix https://github.com/MengyangGao/infoMatrix
brew install --cask infomatrix
```
Set `INFOMATRIX_MACOS_SIGNING_IDENTITY` and `INFOMATRIX_MACOS_NOTARIZE=1` to produce a Developer ID signed and notarized bundle. Provide either `INFOMATRIX_MACOS_NOTARY_PROFILE` or `INFOMATRIX_MACOS_NOTARY_APPLE_ID` / `INFOMATRIX_MACOS_NOTARY_APPLE_PASSWORD` / `INFOMATRIX_MACOS_NOTARY_TEAM_ID`.

### iOS simulator package
```bash
tooling/scripts/package_ios_release.sh
```
The script leaves `dist/InfoMatrix-iOS.app` in place and zips the same bundle under `dist/releases/ios/InfoMatrix-iOS-simulator.zip`. Use it for simulator smoke checks, not device installation.
Set `INFOMATRIX_IOS_DEVICE_RELEASE=1` together with `INFOMATRIX_IOS_TEAM_ID` and `INFOMATRIX_IOS_EXPORT_METHOD` to also build a device archive and export an IPA for TestFlight or App Store Connect.

### Windows
```bash
tooling/scripts/package_flutter_windows_release.sh
```
Install `dist/releases/windows/InfoMatrix-windows-x64.msix` directly for the easiest setup, or unzip `dist/releases/windows/InfoMatrix-windows-x64.zip` and launch `InfoMatrix.exe` as a fallback.

### Android
```bash
tooling/scripts/package_flutter_android_release.sh
```
Use `dist/releases/android/InfoMatrix-android.apk` for direct-install smoke testing, or upload `dist/releases/android/InfoMatrix-android.aab` to Play Console later. Create `apps/flutter/android/key.properties` for release signing, or set `INFOMATRIX_ANDROID_ALLOW_DEBUG_SIGNING=1` only when you need an explicit smoke-only package.

### Linux
```bash
tooling/scripts/package_flutter_linux_release.sh
sudo dpkg -i dist/releases/linux/InfoMatrix-linux-x64.deb
```
The package installs `InfoMatrix` under `/opt/InfoMatrix` and registers `infomatrix` on the PATH.
