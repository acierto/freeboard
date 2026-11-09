# Release builds (macOS and Linux)

Scope decision (2026-11-04): v1 ships for macOS and Linux together. Windows is out of scope for now.

## macOS

Local builds are **ad-hoc signed** (`signingIdentity: "-"` in `src-tauri/tauri.conf.json`), so they run on the machine that built them but are not distributable.

A distributable build needs a **Developer ID Application** certificate (Apple Developer Program) and notarization. Tauri reads these from the environment and overrides the ad-hoc identity:

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: <Name> (<TEAMID>)"
# Notarization with an app-specific password:
export APPLE_ID="<apple id email>"
export APPLE_PASSWORD="<app-specific password>"
export APPLE_TEAM_ID="<TEAMID>"
npx tauri build --bundles app,dmg
```

Verify the result:

```bash
codesign --verify --deep --strict --verbose=2 src-tauri/target/release/bundle/macos/Freeboard.app
spctl --assess --type execute --verbose src-tauri/target/release/bundle/macos/Freeboard.app   # "accepted, source=Notarized Developer ID"
xcrun stapler validate src-tauri/target/release/bundle/dmg/Freeboard_*.dmg
```

Hardened runtime is on. Freeboard needs no special entitlements: it starts developer tools as child processes and reads files the user can read. Full Disk Access is requested by the user in System Settings only if they want protected locations scanned.

Status: **incomplete**. This Mac has only an "Apple Development" certificate, which cannot sign apps for distribution outside the App Store.

## Linux

`.deb` and AppImage need no signing. Build on Ubuntu 22.04 (oldest supported glibc) with `scripts/linux/verify.sh`, which runs in Docker and writes installers and logs to `linux-evidence/`. Optionally sign the AppImage with `appimagetool --sign` and publish SHA-256 checksums alongside the files.

## Publishing a preview release

1. Set the version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `package.json`.
2. Tag and push: `git tag -a v0.1.0 -m "<one line: what is new>" && git push origin v0.1.0`. The tag's message is the "New: …" note in the app's update banner.
3. `.github/workflows/release.yml` builds on GitHub runners (macOS Apple silicon and Intel, Windows x64 and Arm64, Linux x86_64 and arm64), runs the core tests, and attaches `Freeboard-<version>-macOS-arm64.dmg`, `Freeboard-<version>-macOS-x64.dmg`, `Freeboard-linux-<arch>.deb`, `Freeboard-linux-<arch>.AppImage` and `SHA256SUMS.txt` to the release.
   Each build also signs its update files (`.app.tar.gz` on macOS, the setup `.exe` and `.msi`, the AppImage and the renamed `.deb`), and the publish job writes `latest.json` (`scripts/release/latest-json.mjs`), which the app's version chip reads. Without every platform's signed file the publish fails.
4. The download page (`site/`, deployed by `.github/workflows/pages.yml`) links to `releases/latest/download/<name>`, so it always serves the newest non-prerelease release.

## Update signing key

Update files are signed with a minisign key made by `npx tauri signer generate`. The public key is `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`. The private key and its password are the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; the maintainer keeps the originals in `~/.tauri/freeboard-updater.key` and `~/.tauri/freeboard-updater.password`, outside the repository. Losing the private key means installed copies can no longer update themselves (a new key needs one manual download), so keep a backup.

## Smoke test after installing

1. Launch; the window opens on first-run setup.
2. Close the window; the menu-bar/tray ring remains, and after 60 seconds the log shows `destroying hidden webview`.
3. Tray → Open Freeboard reopens the window with the same state.
4. Tray → Quit exits.
