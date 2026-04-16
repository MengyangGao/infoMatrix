# ADR 0002: UI Shell Split

- Status: Accepted
- Date: 2026-03-15

## Decision
Use SwiftUI for Apple platforms and Flutter for Windows/Linux/Android.

## Rationale
SwiftUI is first-class for Apple-native workflows; Flutter provides broad non-Apple coverage with practical desktop/mobile parity.
The current shipping matrix covers macOS, iOS, and iPadOS; visionOS remains a follow-up slice until the Apple XCFramework ships a supported variant.
