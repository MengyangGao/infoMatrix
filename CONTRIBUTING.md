# Contributing to InfoMatrix

Thanks for helping improve InfoMatrix.

## Development workflow

1. Make focused, reviewable changes.
2. Keep core business logic in Rust crates, not UI layers.
3. Add or update tests for behavior changes.
4. Run Rust tests before opening a PR.
5. Update `docs/` when architecture, schema, or discovery behavior changes.

## Local checks

From `core/`:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

For the Apple shell, build the Rust FFI XCFramework first, then run Swift tests and verify the Xcode project builds:

```bash
tooling/scripts/build_apple_xcframework.sh
cd apps/apple
swift test --disable-sandbox

# Optional: also verify the generated Xcode project compiles
cd XcodeGen
xcodegen generate
xcodebuild build -project InfoMatrix.xcodeproj -scheme InfoMatrix-macOS -destination 'platform=macOS' CODE_SIGNING_REQUIRED=NO CODE_SIGNING_ALLOWED=NO
```

For the Homebrew cask:

```bash
brew style Casks/infomatrix.rb
brew tap-new infomatrix/ci
mkdir -p "$(brew --repo infomatrix/ci)/Casks"
cp Casks/infomatrix.rb "$(brew --repo infomatrix/ci)/Casks/infomatrix.rb"
brew audit --cask --strict infomatrix/ci/infomatrix
brew audit --cask --strict infomatrix/ci/infomatrix --online
brew install --cask infomatrix/ci/infomatrix
```

For the npm package:

```bash
cd packages/npm
npm ci
npm test
npm pack
```

For the Flutter shell (requires a network connection to resolve pub packages):

```bash
cd apps/flutter
flutter analyze
flutter test
```

## Product constraints

- Local-first by default.
- No telemetry/analytics by default.
- No cloud dependency required for MVP functionality.
- Treat feed/article content as untrusted input.

For full project conventions, see `AGENTS.md`.
