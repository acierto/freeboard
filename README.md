# Freeboard

**Know before your disk is full.** Freeboard is a free, open-source desktop app for macOS, Windows and Linux. It watches your free space, warns you early, and shows what you can remove safely: Docker data, package caches, old dependencies, build output and stale Git worktrees.

[Website and downloads](https://acierto.github.io/freeboard/) · [Documentation](docs/README.md) · [User guide](docs/user-guide.md) · [Releases](https://github.com/acierto/freeboard/releases)

## What it does

- **Warns early.** One notification when you drop below your Low or Critical level, plus a forecast of when the disk fills at the current rate.
- **Finds developer clutter.** Docker build cache, images, containers and volumes; npm, pnpm, pip, Cargo, Gradle and Homebrew caches; Xcode data; `node_modules` and build output in idle projects; old Git worktrees.
- **Explains every item.** Why it is listed, the exact command or folder action that would run, and how to get it back.
- **Never acts on its own.** You review a cleanup plan. Each item is checked again just before it runs, and anything that changed since the scan is skipped.
- **Stays local.** No account, no telemetry, no network calls.

Every item carries a risk label:

| Label | Meaning |
| --- | --- |
| **Safe** | Regenerable data (caches, build output). Preselected. |
| **Review** | Usually fine to remove, but worth a look. Never preselected automatically. |
| **Risky** | Cannot be recovered, such as Docker volumes. Never preselected; confirmed one by one. |

## Install

Download the installer for your system from the [latest release](https://github.com/acierto/freeboard/releases/latest) (`.dmg`, `.exe`/`.msi`, `.deb` or `.AppImage`), and verify it against `SHA256SUMS.txt`. Preview builds are not yet signed or notarized, so macOS and Windows will ask you to confirm the first launch. Step-by-step instructions are in the [user guide](docs/user-guide.md#install).

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
