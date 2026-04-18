# Objective
Fix the macOS app crash that occurs after bypassing Gatekeeper, caused by CloudKit startup initialization. The app should launch reliably even when iCloud sync is unavailable, disabled, or the app is running without a fully provisioned CloudKit release setup.

# User Value
Users can open the downloaded macOS build instead of seeing an immediate crash after choosing "Open Anyway". Apple sync remains optional and local-first behavior is preserved.

# Constraints
- Keep the fix narrowly scoped to startup and CloudKit initialization.
- Do not require CloudKit to be available at process launch.
- Preserve optional Apple sync behavior for macOS/iOS/iPadOS/visionOS.
- Avoid regressing the existing release workflow more than necessary.
- Add or update tests for the startup path.

# Assumptions
- [ASSUMPTION] The crash is triggered by eager CloudKit initialization during app startup, not by later user interaction.
- [ASSUMPTION] CloudKit sync should be lazy and best-effort, with startup falling back to local-only mode when CloudKit cannot be initialized.
- [ASSUMPTION] The current release is still intended to ship with optional Apple sync, but the app must never fail to launch just because CloudKit is unavailable.

# Affected Files
- apps/apple/Shared/CloudKitSyncManager.swift
- apps/apple/Shared/InfoMatrixApp.swift
- apps/apple/macOS/XcodeApp/MacApp.swift
- apps/apple/iOS/XcodeApp/iOSApp.swift
- apps/apple/iPadOS/XcodeApp/iPadApp.swift
- apps/apple/visionOS/XcodeApp/VisionApp.swift
- apps/apple/Tests/InfoMatrixShellTests/*

# Steps
1. Inspect the CloudKit coordinator initialization path and make it lazy or failure-tolerant.
2. Stop constructing or touching CloudKit state during app startup unless the user actually enables sync or the app explicitly refreshes sync status.
3. Add a fallback state for CloudKit initialization failures so the UI can still show local-only operation.
4. Add regression tests that verify app state can initialize without CloudKit and that sync remains optional.
5. Rebuild and run the Apple test suite plus the release gate.

# Validation
- `swift test --disable-sandbox`
- `tooling/scripts/release_check.sh`
- If possible, smoke-run the macOS app bundle or at least verify the crash logs no longer show startup termination from `CloudKitSyncCoordinator.init`.

# Risks
- CloudKit may still be unavailable on some machines even after lazy init; the fallback must prevent that from surfacing as a crash.
- Changing startup behavior may alter when sync status appears in the UI, so the UI must tolerate `nil` or disabled sync state.
- If the issue is actually an entitlement/signing problem, lazy init will fix the crash symptom but not the eventual sync feature availability.

# Rollback Notes
- If the lazy CloudKit path introduces regressions, revert only the coordinator startup changes and keep the crash report evidence for the next iteration.
