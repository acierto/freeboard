# Freeboard

**Know before your disk is full.** Freeboard is a free, open-source desktop app for macOS, Windows and Linux. It watches your free space, warns you early, shows where your space went, and what you can remove safely: Docker data, package caches, old dependencies, build output, stale Git worktrees, unused AI models, old device backups and more.

[Website and downloads](https://acierto.github.io/freeboard/) · [Documentation](docs/README.md) · [User guide](docs/user-guide.md) · [Releases](https://github.com/acierto/freeboard/releases)

## What it does

- **Warns early.** One notification when you drop below your Low or Critical level, plus a forecast of when the disk fills at the current rate.
- **Finds developer clutter.** Docker build cache, images, containers and volumes; npm, pnpm, pip, Cargo, Gradle, Homebrew, JetBrains, Playwright and chat-app caches; Xcode data and simulators; `node_modules` and build output in idle projects; old Git worktrees; Ollama, Hugging Face and LM Studio models; old iPhone backups and Android emulators; Node versions and Rust toolchains no project uses; apps not opened for a year; exact duplicates and big videos you no longer open.
- **Explains the disk.** A disk map assigns used space to areas, says how much it could not explain, and why.
- **Explains every item.** Why it is listed, the exact command or folder action that would run, and how to get it back.
- **Never acts on its own.** You review a cleanup plan. Each item is checked again just before it runs, and anything that changed since the scan is skipped. The only exception is a Keep me tidy rule you turn on yourself, limited to named Safe caches.
- **Stays local.** No account, no telemetry. It goes online only when you click the version number to check for an update.

Every item carries a risk label:

| Label | Meaning |
| --- | --- |
| **Safe** | Regenerable data (caches, build output). Preselected. |
| **Review** | Usually fine to remove, but worth a look. Never preselected automatically. |
| **Risky** | Cannot be recovered, such as Docker volumes. Never preselected; confirmed one by one. |

## Install

Download the installer for your system from the [latest release](https://github.com/acierto/freeboard/releases/latest) (`.dmg`, `.exe`/`.msi`, `.deb` or `.AppImage`), and verify it against `SHA256SUMS.txt`. Preview builds are not yet signed or notarized, so macOS and Windows will ask you to confirm the first launch. Step-by-step instructions are in the [user guide](docs/user-guide.md#install).

## Opening Freeboard the first time

Until releases are signed and notarized, your system may block the first launch. This is normal for a new app downloaded from the internet.

**macOS**

1. Open Freeboard once. macOS says it can't verify the app and won't open it. Click **Done**.
2. Open **System Settings → Privacy & Security**.
3. Scroll down to **Security** and click **Open Anyway** next to "Freeboard was blocked to protect your Mac". The button shows for a while after your first try.
4. Click **Open**, and enter your Mac password if asked. From then on Freeboard opens normally.

On macOS 15 and later, Control-click → Open no longer gets around this block. On a Mac managed by your company, **Open Anyway** may be missing; ask IT to allow Freeboard. Never turn Gatekeeper off to get around this.

**Windows:** on "Windows protected your PC", click **More info**, then **Run anyway**.

**Linux (AppImage):** `chmod +x Freeboard*.AppImage`, then double-click it. The `.deb` needs nothing extra.

On a company-managed computer these options may be blocked; ask IT to allow Freeboard.

## Build from source

Freeboard is a [Tauri 2](https://tauri.app/) app: a Rust core does all monitoring, scanning and cleanup, and a React interface displays results.

```bash
npm ci
npx tauri dev        # run the app with hot reload
npx tauri build      # unsigned installers
cd src-tauri && cargo test --no-default-features   # core tests
```

Prerequisites, all checks and the project layout are in the [development guide](docs/development.md).

## Documentation

| Read this | If you want to |
| --- | --- |
| [User guide](docs/user-guide.md) | Install and use Freeboard |
| [Development guide](docs/development.md) | Build, test and contribute |
| [Architecture](docs/architecture.md) | Understand the Rust core, the UI and the safety boundary |
| [Safety traceability](docs/safety-traceability.md) | See which test covers each safety rule |
| [Detectors](docs/detectors.md) | See what is scanned and how |
| [Cleanup](docs/cleanup.md) | Follow a plan from creation to history |
| [Release builds](docs/release.md) | Sign, notarize and publish |
| [Decisions](docs/decisions.md) | Learn why things are the way they are |
| [Product specification](docs/reference/FREEBOARD_SPEC.md) | Read the full behavior contract |

## License

[MIT](LICENSE). Bundled fonts are under the SIL Open Font License; see `site/fonts/`.
