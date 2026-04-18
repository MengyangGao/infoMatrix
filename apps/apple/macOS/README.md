# macOS Target Notes

SwiftUI shell scaffolding lives in `apps/apple/Shared` and links the Rust core through `apps/apple/Frameworks/InfoMatrixCore.xcframework`.

## Release Notes

- Keep the macOS app aligned with the shared Rust core contract.
- Rebuild the Apple XCFramework with `tooling/scripts/build_apple_xcframework.sh` whenever the Rust FFI surface changes.
- Verify subscription, refresh, and detail flows through the native bridge before shipping.
- Treat the shared SwiftUI code as the main Apple shell path for macOS, iOS, and iPadOS; visionOS remains follow-up work until the Apple XCFramework ships a supported slice.
- For a local release-style check, run `tooling/scripts/release_check.sh` from the repository root.
- For a focused macOS build, use `xcodebuild -project apps/apple/XcodeGen/InfoMatrix.xcodeproj -scheme InfoMatrix-macOS -configuration Release -destination 'platform=macOS' build`.
- For a macOS release bundle, run `tooling/scripts/package_macos_release.sh` and distribute `dist/releases/macos/InfoMatrix-macos.dmg`.
- The GitHub Releases page is the public download surface for macOS users, with `InfoMatrix-macos.dmg` as the normal install path and `InfoMatrix-macos.zip` as the fallback bundle.
- Set `INFOMATRIX_MACOS_SIGNING_IDENTITY` for Developer ID signing and `INFOMATRIX_MACOS_NOTARIZE=1` for notarized release bundles. Provide either `INFOMATRIX_MACOS_NOTARY_PROFILE` or the Apple ID / password / team variables before running an official release.
- The native bridge removes the old localhost-server dependency, but App Store delivery still needs standard Apple release work such as icons, privacy metadata, and signing/notarization validation.
