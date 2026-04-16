# iOS Target Notes

SwiftUI package scaffolding is in `apps/apple/Shared` and links the Rust core through `apps/apple/Frameworks/InfoMatrixCore.xcframework`.

## Release Notes

- Keep the iOS shell behavior aligned with macOS and the shared Rust contract.
- Rebuild the Apple XCFramework with `tooling/scripts/build_apple_xcframework.sh` whenever the Rust FFI surface changes.
- Use `tooling/scripts/release_check.sh` from the repository root to validate the combined release path on this workstation.
- For a focused simulator build, use `xcodebuild -project apps/apple/XcodeGen/InfoMatrix.xcodeproj -scheme InfoMatrix-iOS -configuration Release -sdk iphonesimulator build`.
- For a simulator package, run `tooling/scripts/package_ios_release.sh`; it leaves `dist/InfoMatrix-iOS.app` in place and zips the same bundle to `dist/releases/ios/InfoMatrix-iOS-simulator.zip`.
- Set `INFOMATRIX_IOS_DEVICE_RELEASE=1` together with `INFOMATRIX_IOS_TEAM_ID` and `INFOMATRIX_IOS_EXPORT_METHOD` to produce a device archive and export an IPA for TestFlight or App Store Connect.
