# @mengyanggao/infomatrix

CLI installer and launcher for [InfoMatrix](https://github.com/MengyangGao/infoMatrix).

## Install

```bash
npm install -g @mengyanggao/infomatrix
```

The launcher downloads the correct platform artifact from GitHub Releases on first run, verifies its SHA256 checksum, and extracts it to `~/.cache/infomatrix-cli/<version>/`.

## Usage

```bash
infomatrix
```

This launches the InfoMatrix app for your platform:

- **macOS**: opens `InfoMatrix.app`
- **Linux**: runs the bundled `InfoMatrix` executable
- **Windows**: runs the bundled `InfoMatrix.exe`

## Supported platforms

- macOS (`x64` / `arm64`)
- Linux (`x64`)
- Windows (`x64`)

## Notes

- The first install downloads the release artifact from GitHub, so an internet connection is required.
- On macOS, the installer removes the `com.apple.quarantine` attribute so the app can be opened directly.
- This package does not contain the native binaries; it downloads them on demand.
