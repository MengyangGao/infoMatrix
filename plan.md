# Objective
Diagnose the macOS "opens then quits" report and fix serious launch/distribution bugs that can make the packaged macOS app behave differently from local builds.

# User Value
Users should be able to launch the macOS app from the generated app/ZIP/DMG with predictable behavior, and release scripts should clearly fail or warn when they produce an artifact that is not suitable for end-user distribution.

# Constraints
- Preserve the local-first architecture and keep networking/parsing out of SwiftUI views.
- Do not add dependencies.
- Do not push or publish artifacts without explicit approval.
- Keep changes focused on macOS launch and packaging correctness.

# Assumptions
- [ASSUMPTION] The reported flash crash happens from an app bundle, ZIP, or DMG artifact rather than by running `Contents/MacOS/InfoMatrix-macOS` directly.
- [ASSUMPTION] A locally reproducible LaunchServices crash is not currently present on this machine; Debug, Release, `dist/InfoMatrix.app`, and the mounted DMG app stayed running.
- [ASSUMPTION] We should still fix release-path mismatches because they can explain user-only behavior.

# Affected Files
- `plan.md`
- `apps/apple/XcodeGen/project.yml`
- `tooling/scripts/package_macos_release.sh`
- `apps/apple/macOS/XcodeApp/MacApp.swift`

# Steps
1. Record the macOS crash investigation plan before edits.
2. Align the generated Xcode macOS target with the maintained SwiftPM macOS entrypoint so packaged Release builds use the same AppDelegate/window activation path.
3. Remove the stale Xcode-only macOS entrypoint to prevent future divergence.
4. Harden the macOS packaging script:
   - support direct `xcodegen` when `mint` is not installed
   - verify code signatures after signing
   - make Gatekeeper assessment explicit
   - warn loudly for ad-hoc local-test artifacts
   - fail Gatekeeper validation when a signed/notarized release is requested
   - copy the staged app into the DMG with `ditto` instead of `cp -R`
5. Regenerate/build the macOS Xcode target and verify the resulting app launches through `open`.
6. Verify existing release diagnostics: `codesign`, `spctl`, ZIP/DMG checks where applicable.

# Validation
- `xcodebuild -project apps/apple/XcodeGen/InfoMatrix.xcodeproj -scheme InfoMatrix-macOS -configuration Debug -destination "platform=macOS" ... build`
- `xcodebuild -project apps/apple/XcodeGen/InfoMatrix.xcodeproj -scheme InfoMatrix-macOS -configuration Release -destination "platform=macOS" ... build`
- `open -n -W <built app>`
- `codesign --verify --deep --strict --verbose=2 <app>`
- `spctl --assess --type execute --verbose <app>` as a diagnostic, with ad-hoc failures treated as expected for local artifacts.

# Risks
- Overlooked risk 1: the user's real crash may be caused by their local `~/.infomatrix/infomatrix.db`, which this machine does not reproduce.
- Overlooked risk 2: Gatekeeper/quarantine failures may look like "flash crash" to users but do not always create `.ips` crash reports.
- Overlooked risk 3: direct executable launch crashes are real for development workflows but are not the same path as Finder/`open` launching an `.app`.
- Switching XcodeGen to the shared macOS entrypoint must avoid compiling two `@main` types in the same target.
- Gatekeeper validation behavior differs between ad-hoc local builds and Developer ID notarized builds.

# Rollback Notes
- If the Xcode target fails because of package/import differences, revert the source path change and instead move the shared app delegate code into a common file without `@main`.
- If stricter packaging checks block local development, keep signature verification but limit Gatekeeper failure to `INFOMATRIX_REQUIRE_SIGNED_RELEASE=1` or `INFOMATRIX_REQUIRE_NOTARIZED_RELEASE=1`.
