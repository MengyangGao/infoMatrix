# Product Targets

## Baseline

InfoMatrix is guided by the goal of being a fast, privacy-respecting, local-first RSS reader with a polished reading flow and strong saved-item workflows.

## Direction

1. Direct feed subscription should be the fastest path.
2. Website URL discovery should be reliable and explainable.
3. Reading and triage should feel smooth on desktop and touch devices.
4. Local state should remain deterministic, inspectable, and sync-ready.
5. The core should stay testable and maintainable over time.
6. A user should be able to install a pre-built binary from GitHub Releases without compiling the project first.
7. macOS users should also be able to install through Homebrew.
8. Apple users should be able to opt into CloudKit sync while Android, Linux, and Windows continue to work locally.
9. Auto-refresh should be configurable by global default, folder, and feed scope.

## Pre-Built Binary Target

The release pipeline should keep the public download story simple and platform-specific:

- macOS users install `InfoMatrix-macos.dmg`.
- Windows users install `InfoMatrix-windows-x64.msix`.
- Linux users install `InfoMatrix-linux-x64.deb`.
- Android remains available through manual smoke builds and Play Console preparation, but is not part of the current tagged GitHub Release download set.
- iOS simulator users can use `InfoMatrix-iOS-simulator.zip`; device builds remain signing-dependent.
- macOS users can also install `infomatrix` through the project-owned Homebrew cask.

## Guardrails

1. Avoid opaque ranking in the core inbox.
2. Avoid cloud dependence in MVP.
3. Avoid binding view code directly to networking or parsing logic.
4. Avoid hardcoding per-site shortcuts as the only discovery path.
