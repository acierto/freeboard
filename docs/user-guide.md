# Freeboard user guide

Freeboard watches the free space on your disk and helps you get it back without breaking anything. This guide covers installing it, what each screen does, and how cleanup stays safe.

## Install

Download the installer for your system from the [latest release](https://github.com/acierto/freeboard/releases/latest). Compare its SHA-256 with `SHA256SUMS.txt` on the same page.

Preview builds are not yet signed with an Apple Developer ID or a Windows code-signing certificate, so each system asks you to confirm the first launch.

### macOS (11 or later, Apple silicon and Intel)

1. Open `Freeboard-<version>-macOS-universal.dmg`, accept the MIT license, and drag Freeboard to Applications.
2. Open Freeboard. macOS says it can't check the app for malware.
3. Go to **System Settings → Privacy & Security**, scroll down and click **Open Anyway**. Or run:
   ```bash
   xattr -dr com.apple.quarantine /Applications/Freeboard.app
   ```

### Windows (10 or 11, x64 or Arm)

1. Run `Freeboard-<version>-windows-x64-setup.exe` (or the Arm installer, or the `.msi`). It installs for your user and needs no administrator rights.
2. If SmartScreen says "Windows protected your PC", click **More info**, then **Run anyway**.
3. Freeboard lives in the notification area. If its icon is hidden, click the **^** arrow on the taskbar.

### Linux (Ubuntu 22.04 or newer, Debian 12, or similar; x86_64 or arm64)

```bash
# Debian / Ubuntu
sudo apt install ./Freeboard-<version>-linux-x86_64.deb

# AppImage
chmod +x Freeboard-<version>-linux-x86_64.AppImage
./Freeboard-<version>-linux-x86_64.AppImage
```

Tested on GNOME (with the AppIndicator extension that Ubuntu includes) and KDE Plasma.

In apt the package is called `freeboard-disk` (`sudo apt remove freeboard-disk` uninstalls it). Ubuntu's own `freeboard` package is an unrelated IoT dashboard.

## First run

The first-run setup explains what Freeboard reads and asks which folders to scan for projects and worktrees. Freeboard asks the operating system for a permission only when a feature needs it. On macOS, for example, Full Disk Access is needed only if you want protected locations scanned.

## How Freeboard watches your disk

Freeboard monitors your home volume, plus any other fixed volumes you add. It samples free space every 5 minutes and keeps one sample per hour.

| Status | Default rule | Tray icon | Notification |
| --- | --- | --- | --- |
| Healthy | 20 GB free or more | Neutral ring | None |
| Low | Below 20 GB | Amber ring | Once when it enters Low |
| Critical | Below 8 GB | Red ring | When it enters Critical, then every 6 hours while unchanged |

A status improves only after free space rises 2 GB above its threshold, so you aren't alerted repeatedly around the limit. Clicking a notification opens Suggestions.

The **forecast** ("About N days left") appears only when there are at least 3 days of samples and free space is falling. Past cleanups are excluded from the estimate.

Closing the window hides Freeboard to the tray. Monitoring and scheduled scans keep running; the interface is released after 60 seconds hidden. The tray menu offers **Open Freeboard**, **Scan Now**, **Pause Notifications** and **Quit**.

## The screens

| Screen | What it is for |
| --- | --- |
| **Home** | Dinghy, the boat that rides higher as free space grows, free space and status, the forecast, your goal, **Where from**, and the pile of crates the main button will remove. |
| **Quick questions** | One card at a time, biggest first: what the item is, its size, why it is listed, where it lives, when it was last used, the action, and how to recover it. |
| **Everything I found** | All suggestions, grouped by source (Docker, Git worktrees, dependencies, caches, files), with a checkbox, **Remove now** and **Keep** per item. |
| **Disk map** | Where the space goes, by area; what Freeboard can't explain and why, with a fix for each; what grew fastest; which places were checked. |
| **Logbook** | History of cleanups: date, result, space freed, amount held in the Trash and skipped items. |
| **Settings** | Behind the gear button: warning levels, scan folders, excluded folders, scan schedule, startup, notifications. |

## Making room

Pick a goal (20 GB, 50 GB, 100 GB or **All I can**) and, optionally, **Where from**. Freeboard fills the pile with **Safe** items, largest first, until the goal is met. Items you added by hand come first.

- **Review** and **Risky** items are never added automatically.
- Take a crate out and the pile switches to manual mode and won't refill it. Pick a size again to refill.
- **Keep** marks an item as one you never want suggested. **Stop keeping** brings it back.
- **Choose which** lets you pick individual projects, models, emulators, apps, backups or versions inside an item.
- If Safe items fall short of your goal, Freeboard says by how much and points you to the items below.

## Risk levels

| Label | Meaning | Default action | Recovery |
| --- | --- | --- | --- |
| **Safe** | Regenerable data: caches, build output | Tool's own cleanup command, or deleting regenerable files | Rebuild with the tool that owns it |
| **Review** | Usually fine, worth a look: unused Docker images, idle projects' dependencies, merged worktrees | Moves to the Trash where possible; Docker and Git items use their own commands | Restore from the Trash, or recreate |
| **Risky** | Can't be recovered, such as Docker volumes | Blocked, or confirmed individually by name | Stated before you confirm, often "not recoverable" |

Moving something to the Trash does not free space until you empty the Trash. The plan shows this.

## Confirming a cleanup

Before anything runs, you see a plan with three totals:

1. **Freed right away.**
2. **Held in the Trash** (freed when you empty it).
3. **Cannot be recovered.** Each such item needs its own confirmation.

Then, for each item, one at a time:

1. Freeboard checks it again. If it changed since the scan (a worktree got a new commit, a container started, a folder was modified), it is **skipped** with the reason shown.
2. It runs the action.
3. The result is recorded in the Logbook.

You can cancel between items; an item already running is never interrupted. If one item fails, the rest continue. Afterwards Freeboard measures free space again and reports the measured change next to the expected one. A plan expires after 30 minutes, or sooner if a new scan runs.

## Keep me tidy

Optional rules, **off by default**, limited to named Safe detectors:

| Rule | Limit |
| --- | --- |
| Clear package caches over 30 days old when space is Low | Named Safe cache detectors only |
| Clear unused Docker build cache every week | `docker.build_cache` only |

## What Freeboard never does

- Removes anything without a plan you confirmed.
- Touches system or application folders, your home folder's main folders, or `.ssh`.
- Deletes a repository root or anything inside `.git`.
- Follows symbolic links out of the folder being cleaned.
- Uses force flags, except Docker's own prune commands after you confirm.
- Runs commands through a shell.
- Sends data anywhere. There is no account, telemetry or network access.

The full list of rules, and the test that covers each, is in [Safety traceability](safety-traceability.md).

## Default settings

| Setting | Default |
| --- | --- |
| Low warning | 20 GB |
| Critical warning | 8 GB |
| Goal | 50 GB |
| Where from | Everywhere |
| Quick scan | Every hour |
| Deep scan | Only on request |
| Inactive project | 30 days |
| Old installer | 90 days |
| Keep me tidy rules | Off |
| Pause deep scans on battery | On |
| Start at login | On |
| Notifications | On |

Quick scans read tool reports and known cache locations (under 10 seconds). Deep scans also walk your scan folders for worktrees and project dependencies; they run at low priority and never follow symbolic links.

## FAQ

**Why does a size say "about"?** Some data is shared (hard links, as with pnpm) or unreadable. Freeboard reports only the space removal would actually free, and marks the number as an estimate when it can't be exact.

**Why was an item skipped?** It changed after the scan, or Freeboard couldn't confirm its state (a tool was missing or timed out). Skipped items stay in Suggestions with the reason.

**Why didn't my free space grow after cleanup?** Items sent to the Trash free space only when the Trash is emptied.

**Does it work with WSL?** WSL paths are skipped during scans.

**Something looks wrong.** Please open an issue on [GitHub](https://github.com/acierto/freeboard/issues).
