# Freeboard: Product and Implementation Specification

Nov 6, 2026 · @Bogdan

## Overview

Freeboard is a local Tauri 2 desktop app for macOS, Linux and Windows. It warns developers before the disk fills, explains where all used space went, and lets them choose exactly what to remove. Rebuild the approved prototype, `freeboard-prototype.html`, as the real app.

**Problem:** developer machines fill up with Docker data, worktrees, dependencies, build output, AI models and caches. Cleanup tools are slow to use, and they stay silent about anything they don't recognize.

**Primary user:** a developer on a workstation with limited disk space who needs space back quickly during normal work.

### What this rebuild must solve

| Concern | What the product does |
| --- | --- |
| Cleanup takes too many steps | Choose how much room you need, confirm once, and let Freeboard pick Safe items |
| A single button gives too little control | Choose by area, group, item, or individual folders and models inside an item |
| It may miss things | Account for all used space, show what is unexplained, and offer a fix for each blind spot |
| A utility feels cold | Dinghy, a small boat, shows disk health at a glance through its waterline and mood |

### Build reference

The prototype defines the layout, flows, labels and tone. This spec wins on data, safety and behavior. If they conflict, follow this spec and record the conflict in `docs/decisions.md`.

If a Freeboard codebase already exists, keep any Rust code that meets this spec and rebuild the interface. Do not preserve older screens or flows.

Do not ship these prototype-only parts: example data, **Pretend free space**, **Start over**, the forced skip of `old-experiment`, and the simulated **Allow access** result.

### Product decisions

| Decision | Build this |
| --- | --- |
| Disk map | In the first version |
| Duplicate detection | In the first version |
| Keep me tidy rules | In the first version, off by default, Safe items only |
| Docker disk compaction | In the first version as a Review item, only where a supported method exists |
| Manual folder browsing | Not in the first version |
| Mascot name | Dinghy, as a placeholder |
| Product name | Freeboard, as the working name |

### Goals

- Show current free space and warn before it becomes critical.
- Explain as close to 100% of used space as possible, and label the rest as not explained.
- Show the size, risk, reason, action and recovery for every suggestion.
- Let the user clean quickly, or control each removal down to individual parts.
- Preview every manual cleanup before it runs.
- Stay small and quiet in the background.

### Non-goals for the first version

- Removing Review or Risky items without user confirmation.
- Cloud accounts, sync, telemetry or remote management.
- General PC optimization, such as memory cleaning, registry cleaning or startup management.
- A general file manager or manual folder browser.
- Mac App Store distribution, because its sandbox limits whole-disk scanning.

### Performance targets

| Metric | Target |
| --- | --- |
| Installer size | 15 MB or less |
| Memory while hidden in the tray | 40 MB or less, with the webview destroyed |
| CPU while idle | Below 1% averaged over 5 minutes |
| Quick scan | Under 10 seconds |
| Deep scan of 1 million files on an SSD | Under 60 seconds |

These are design targets, not measured results. Report the real numbers at each milestone.

## Instructions for the implementing agent

Implement this specification milestone by milestone, and treat the safety invariants below as stricter than any feature requirement. If a feature conflicts with an invariant, keep the invariant and record the conflict.

1. Complete one milestone at a time and stop when its exit criteria pass.
2. Before relying on a CLI flag or output format, check it against the installed tool. The commands here are design intent, not a guaranteed interface.
3. Pin dependency versions in the lockfiles. Use current stable releases, and do not add a dependency when the standard library is enough.
4. Run scanning and cleanup code only against temporary fixtures during tests. Never run real cleanup commands against the developer's machine.
5. Keep product decisions in `docs/decisions.md`, with the date and reason.
6. If a requirement is unclear and affects deleted data, stop and ask.

### Safety invariants

Every invariant needs at least one automated test.

1. **Scans are read-only.** Detectors may read files and run read-only commands. They must never change anything.
2. **Nothing runs without a confirmed plan.** Cleanup requires a saved plan, and the user must confirm that exact plan. The only exception is a Keep me tidy rule that the user turned on.
3. **The backend owns paths and commands.** The frontend sends only plan and suggestion IDs. It never sends a path or command to execute.
4. **Every item is checked again immediately before cleanup.** The check confirms the item's identity, path, modification state and Git or Docker status. If anything changed, skip the item and report why.
5. **Protected paths are never touched.** Reject the filesystem root, home directory, system directories, application directories, repository roots, and `.git` directories or files. Canonicalize the path first and test against the canonical form.
6. **Symlinks and junctions are never followed.** Never follow them while scanning or deleting. Removing a link removes only the link.
7. **Dangerous commands are banned.** Do not call `git worktree remove --force`, `git clean`, `docker system prune --all`, or `docker volume prune`.
8. **Commands run without a shell.** Use an executable and an argument array, with no string interpolation.
9. **Git work is never preselected.** Uncommitted or unpushed work is `Risky`. It is shown for awareness and blocked from cleanup in v1.
10. **Docker volumes are never preselected.** Volumes can hold unique data. Each one requires individual confirmation.
11. **Unknown means less safe.** If a status check fails or times out, raise the risk level. Never assume an item is safe.
12. **All data stays local.** Freeboard makes no network calls in v1, except those made by developer tools it runs on request.
13. **Every action leaves a record.** Store the plan, each item result, and the before and after free space in local history.

One exception applies to invariant 7. `docker builder prune` and `docker image prune` stop to ask for confirmation, so the backend may pass `--force` to these two commands after the user has confirmed the plan. It only skips Docker's prompt. No other command may use a force flag.

Keep me tidy rules have strict limits. A rule covers only the Safe detector it names, passes the same pre-cleanup checks, and writes the same local record. A rule can never remove Review or Risky items, or anything the user marked Keep.

The disk report stays on the computer until the user pastes it somewhere. It contains sizes, folder names and risk levels, and never file contents.

### Delete and restore policy

The recovery method must be visible before confirmation, because moving items to the Trash does not free space until the Trash is emptied.

| Risk | Default action | Frees space immediately | Recovery |
| --- | --- | --- | --- |
| Safe | Native cleanup command, or permanent delete of regenerable data | Yes | Regenerate with the owning tool |
| Review | Move to Trash where possible; use a native command for Docker and Git items | Trash: only after emptying; native: yes | Restore from Trash, or recreate if the item used a native command |
| Risky | Blocked or individually confirmed, depending on the detector | Depends on the action | State the exact recovery, or "not recoverable" |

The plan must show three totals: space freed immediately, space held in the Trash, and items that cannot be recovered.

## Technical stack and architecture

Freeboard is a Tauri 2 app with a Rust core that does all scanning and cleanup, and a React interface that only displays results and sends IDs back. The Rust core keeps running in the tray after the webview is destroyed.

&#91;embedded content: Freeboard architecture · 3 layers\]

All filesystem and command access passes through the guard and runner, which is where the safety invariants are enforced.

| Layer | Choice | Purpose |
| --- | --- | --- |
| Shell | Tauri 2 | Windows, tray, notifications, packaging |
| Core | Rust stable, Tokio | Scans, detectors, disk map, monitoring, cleanup |
| Interface | React, TypeScript, Vite | Screens only, with no filesystem or command access |
| Filesystem walking | `jwalk` | Parallel directory traversal |
| File hashing | `blake3` | Exact duplicate detection |
| Free space | `sysinfo` | Volume capacity and free space |
| Local storage | SQLite through `rusqlite` | Settings, samples, decisions, plans and logbook |
| Trash | `trash` crate | Trash on macOS and Linux, Recycle Bin on Windows |
| Logging | `tracing` | Local rotating logs, with no file contents |
| Fonts | Fredoka, Nunito, DM Mono | Bundled with the app, never loaded from a remote host |

### Tauri permissions

- Do not grant the frontend the shell or filesystem plugins.
- Grant only the app's own commands, plus notification and autostart permissions.
- Set a strict content security policy, with no remote scripts or remote content.

### Repository layout

```text
freeboard/
├── src/                    # React interface
│   ├── features/
│   │   ├── home/           # Dinghy, free space, goal, areas, pile
│   │   ├── questions/      # quick questions and Everything I found
│   │   ├── disk-map/       # map, blind spots, growth, places checked, report
│   │   ├── cleanup/        # confirmation and progress
│   │   └── logbook/
│   ├── components/
│   └── lib/                # typed wrappers around Tauri commands
├── src-tauri/
│   ├── capabilities/
│   └── src/
│       ├── commands/       # thin Tauri command handlers
│       ├── monitor/        # free-space sampling, status and forecast
│       ├── scan/           # scan orchestration and size measurement
│       ├── detectors/      # one module for each detector family
│       ├── diskmap/        # areas, coverage, blind spots and growth
│       ├── selection/      # goal, area and pile rules
│       ├── cleanup/        # plans, safety checks, execution
│       ├── rules/          # Keep me tidy rules
│       ├── platform/       # macOS, Linux and Windows differences
│       ├── runner.rs       # the only module that starts external commands
│       ├── guard.rs        # protected-path checks
│       ├── store.rs        # SQLite access and migrations
│       └── tray.rs
├── tests/fixtures/
└── docs/decisions.md
```

## Data model and risk rules

Every detector returns the same `Suggestion` shape, so the interface and cleanup engine never need tool-specific logic. Risk comes from the rules below, not from size or age alone.

```ts
type Risk = "safe" | "review" | "risky";
type Action = "native_command" | "delete" | "trash" | "none";
type Group = "docker" | "projects" | "caches" | "ai_models" | "phones_emulators"
  | "old_tools_apps" | "your_files" | "system";

interface SuggestionPart {
  id: string;
  label: string;           // for example a project or model name
  location: string;
  sizeBytes: number;
  selected: boolean;
}

interface Suggestion {
  id: string;              // stable hash of detector ID and resource identity
  detector: string;        // for example "docker.build_cache"
  group: Group;
  mapArea: string;
  title: string;
  location: string;        // path or tool-level description
  sizeBytes: number;       // actual on-disk size of the selected parts
  fullSizeBytes: number;
  lastUsedAt: string | null;
  lastUsedLabel: string;
  risk: Risk;
  action: Action;
  actionLabel: string;     // the exact command or file action shown to the user
  reason: string;
  recovery: string;
  recoverable: boolean;
  blockedReason: string | null;
  needsConfirmation: boolean;
  parts: SuggestionPart[]; // empty when the item can't be split
  fingerprint: string;     // used by the pre-cleanup check
}

interface PileState {
  goalBytes: number | "all";
  areas: Group[];          // empty means everywhere
  mode: "auto" | "manual";
  addedIds: string[];
  takenOutIds: string[];
  itemIds: string[];       // the current result
  totalBytes: number;
}

interface CleanupPlan {
  id: string;
  createdAt: string;
  expiresAt: string;       // 30 minutes after creation
  items: { suggestionId: string; partIds: string[] }[];
  bytesFreedNow: number;
  bytesHeldInTrash: number;
  unrecoverableIds: string[];
}

interface DiskMap {
  usedBytes: number;
  explainedBytes: number;
  notExplainedBytes: number;
  areas: { id: string; label: string; bytes: number; leaveAloneBytes: number }[];
}

interface BlindSpot {
  id: string;
  kind: "unscanned_folder" | "needs_permission" | "unknown_files" | "tool_unavailable";
  title: string;
  text: string;
  location: string;
  sizeBytes: number | null;
  actionLabel: string;
}

interface PlaceChecked {
  detector: string;
  label: string;
  state: "found" | "clean" | "unavailable";
  bytes: number;
}
```

### Risk levels

| Risk | Meaning | Preselected | Examples |
| --- | --- | --- | --- |
| Safe | Fully regenerable, and the owning tool or a lockfile can rebuild it | Yes | Docker build cache, `target`, pnpm unreferenced packages |
| Review | Probably unneeded, but it depends on the user's context | No | Merged worktrees, unused tagged images, old installers |
| Risky | May contain unique work or data | Never | Dirty worktrees, Docker volumes |

### Ordering

Quick questions and each group in Everything I found are sorted by size, largest first. Quick questions include Review and Risky items, except blocked items, items marked Keep, items in the pile, items set to Ask me later and items that changed during a cleanup.

### Measuring size

- Report actual on-disk size, not apparent size. Use `st_blocks` on macOS and Linux, and `GetCompressedFileSizeW` on Windows.
- Count each hard-linked file once, using device and inode or the Windows volume and file IDs.
- For pnpm-managed `node_modules`, show only space that removal would really free.
- Show `about` before a size when the tool can't report it exactly.

### Local storage

| Table | Contents | Retention |
| --- | --- | --- |
| `settings` | User settings as key and JSON value | Permanent |
| `free_space_samples` | Volume, time, free bytes and total bytes | Hourly for 30 days |
| `suggestions`, `suggestion_parts` | Results of the latest scan | Replaced on each scan |
| `item_decisions` | Keep, Ask me later and part selections by stable resource ID | Until changed |
| `pile_state` | Goal, areas, mode, added and taken-out items | Until the next cleanup or reset |
| `map_snapshots` | Area totals, used to measure growth | Daily for 30 days |
| `blind_spot_decisions` | Files the user marked as theirs | Until changed |
| `tidy_rules`, `tidy_runs` | Enabled rules and their runs | Rules permanent, runs 1 year |
| `cleanup_runs`, `cleanup_items` | Plan, item results and measured free space | 1 year |

## Scans and detectors

Freeboard runs a light quick scan on a schedule and a deeper scan on request, and each detector can fail without stopping the others. If a tool is missing or not running, show that state and continue.

| Scan | What it does | Runs | Target |
| --- | --- | --- | --- |
| Quick | Reads tool reports and known cache locations | Every hour by default | Under 10 seconds |
| Deep | Walks the configured folders to find projects, worktrees and files | On request, or daily if enabled | Under 60 seconds for 1 million files |

All external commands run through `runner.rs`, with a 20-second default timeout. A timeout marks that detector as unavailable.

### Detector summary

| Detector | Finds | Default risk | Action | Platforms |
| --- | --- | --- | --- | --- |
| `docker.build_cache` | Unused build cache | Safe | `docker builder prune` | All |
| `docker.dangling_images` | Untagged, unused images | Safe | `docker image prune` | All |
| `docker.disk_slack` | Empty space inside Docker Desktop's disk | Review | Compact with a supported method, after warning that Docker will stop briefly | macOS, Windows |
| `docker.unused_images` | Tagged images with no container | Review | `docker image rm <image>` | All |
| `docker.stopped_containers` | Containers stopped for 14+ days | Review | `docker container rm <id>` | All |
| `docker.volumes` | Volumes with no container | Risky | `docker volume rm <name>`, with individual confirmation | All |
| `git.worktrees` | Linked worktrees | Review or Risky | `git worktree remove <path>` | All |
| `project.dependencies` | `node_modules`, `.venv` | Safe or Review | Delete or Trash, split by project | All |
| `project.build_output` | `target`, `dist`, `build`, `.next` | Safe | Delete | All |
| `cache.pnpm` | Unreferenced packages | Safe | `pnpm store prune` | All |
| `cache.pip` | Download cache | Safe | `pip cache purge` | All |
| `cache.npm` | npm content cache | Safe | Delete `_cacache` | All |
| `cache.homebrew` | Old downloads | Safe | `brew cleanup` | macOS, Linux |
| `cache.jetbrains` | Caches from removed IDE versions | Safe | Delete | All |
| `cache.electron_apps` | Slack, Discord and Teams caches | Safe | Delete while the apps are closed | All |
| `cache.playwright` | Browser versions no project needs | Safe | Delete | All |
| `cache.cargo` | Registry and Git download caches | Review | Trash | All |
| `cache.gradle` | Gradle caches | Review | Trash, only when no Gradle daemon is running | All |
| `cache.xcode` | DerivedData | Safe | Delete | macOS |
| `ai.ollama` | Models not run for 60+ days | Review | `ollama rm <model>`, split by model | All |
| `ai.huggingface` | Downloaded models and datasets | Review | Trash | All |
| `ai.lm_studio` | Downloaded models | Review | Trash, split by model | All |
| `mobile.ios_backups` | Old device backups | Review | Trash, split by backup | macOS, Windows |
| `mobile.android_avd` | Emulators not started for 90+ days | Review | Trash, split by emulator | All |
| `mobile.simulators` | Unavailable iOS simulators | Review | `xcrun simctl delete unavailable` | macOS |
| `tools.node_versions` | Node versions no project requests | Review | `nvm uninstall <version>`, split by version | All |
| `tools.rust_toolchains` | Toolchains no project uses | Review | `rustup toolchain uninstall <name>`, split by toolchain | All |
| `apps.unused` | Apps not opened for 12+ months | Review | Trash on macOS. On Windows and Linux, link to the system uninstaller | All |
| `files.duplicates` | Exact duplicate files | Review | Move extra copies to Trash, keeping one copy | All |
| `files.large_media` | Video files not opened for 6+ months | Review | Trash | All |
| `files.installers` | Old installers and disk images in Downloads | Review | Trash | All |
| `files.windows_temp` | Old files in `%TEMP%` | Review | Recycle Bin | Windows |
| `system.snapshots` | Local Time Machine snapshots | Review | `tmutil deletelocalsnapshots <date>`, only after a newer backup exists | macOS |
| `system.stale_temp` | Temp files left by apps no longer running | Review | Trash | macOS |

`npm cache clean` requires a force flag, which is not an allowed exception, so the npm detector removes the cache folder through the normal safety checks.

### Docker

- Check `docker version` first. If Docker isn't installed or running, show that and skip the Docker detectors.
- Read data with `docker system df`, `docker image ls`, `docker ps -a` and `docker volume ls`, using JSON output where available.
- Treat an image as used if any container references it, including stopped containers.
- Treat a volume as attached if any container mounts it, including stopped containers.
- Never use `docker system prune`.

### Git worktrees

Discover repositories under the configured folders, then read each one with `git worktree list --porcelain`. Never suggest the main worktree.

| State | Risk | Cleanup |
| --- | --- | --- |
| Clean and merged into the default branch | Review | Allowed |
| Clean, pushed, not merged | Review | Allowed |
| Uncommitted or untracked files | Risky | Blocked |
| Unpushed commits, or no upstream | Risky | Blocked |
| Locked worktree | Risky | Blocked |
| In the middle of a merge or rebase | Risky | Blocked |
| Currently open in a detected process | Risky | Blocked |
| Status can't be determined | Risky | Blocked |

Git cannot reliably detect squash merges, so show those branches as "pushed, not merged".

### Projects

Suggest generated folders only when ownership is clear, so an unrelated folder named `build` is never included.

| Folder | Required evidence | Risk |
| --- | --- | --- |
| `node_modules` | `package.json` and a lockfile beside it | Safe |
| `target` | `Cargo.toml` beside it | Safe |
| `.venv` | `pyproject.toml` or `requirements.txt` beside it | Review |
| `dist`, `build`, `.next` | A project manifest beside it, and the folder ignored by Git | Safe |

A project counts as inactive when none of its source files have changed for 30 days. Skip projects with a running process in that folder, and never scan inside the generated folders themselves.

### Files and system items

- **Installers:** suggest `.dmg`, `.pkg`, `.exe`, `.msi`, `.AppImage`, `.deb`, `.rpm` and `.iso` files in Downloads that have not been opened or modified for 90 days.
- **Media:** suggest video files of 500 MB or more that have not been opened for 6 months. Do not suggest photos or documents.
- **Duplicates:** compare by size first, then hash candidates with `blake3`. Treat files as duplicates only when their contents match exactly. Keep one copy, and never include files in hidden folders, app bundles or repositories.
- **Apps:** use last-opened data. If that data is missing, do not suggest the app.
- **Snapshots:** skip this detector unless the newest backup can be confirmed.

If a tool has no safe, supported way to perform an action, show the size for information and keep the item out of cleanup. This applies especially to `docker.disk_slack`.

### Disk accounting

Every scan must account for used space from the top down, then assign it to one area. This is what makes gaps visible.

1. Start with used bytes for the monitored volume.
2. Assign bytes to Projects, Docker, AI models, Caches, Your files, Phones and emulators, Apps and tools, or macOS, using the matching system name on Windows and Linux.
3. Count every file once. A more specific detector takes priority over its parent folder.
4. Put everything else in **Not explained**.
5. Calculate coverage as explained bytes divided by used bytes.

The map totals must equal used bytes within 1%. If they don't, show that the map is incomplete, and do not hide the difference.

### Blind spots

| Kind | Example | Action |
| --- | --- | --- |
| Folder outside the scan list | `~/archive` | Scan this folder |
| Protected location | `/private/var/folders` | Open the operating system permission flow |
| Unknown files | `.dat` files from an unknown tool | Mark them as yours, or inspect the folder |
| Tool not available | Docker is not running | Show how to start it, then scan again |

A blind spot stays visible until it is resolved or the user marks the files as theirs. Show as much size information as the system allows, and write "unknown" instead of guessing.

### Places checked and growth

- List every detector that ran as found, clean or unavailable. **Found** and **Clean** are different results, and an unavailable detector must never look clean.
- Compare daily area totals, and show the fastest-growing areas over 7 days.
- Show the disk report as copyable JSON with free space, coverage, areas, suggestions and blind spots.

## Cleanup and monitoring

The user can clean from one goal or decide item by item, and every manual path ends in the same confirmed plan. Anything that changed since the scan is skipped.

&#91;embedded content: Cleanup flow · 7 steps, 2 decisions\]

Each item is checked on its own, so one changed item doesn't stop the rest of the plan.

### Building the pile

The pile is the set of crates the main button will remove. The rules below are required behavior.

| User action | Result |
| --- | --- |
| Choose 20 GB, 50 GB or 100 GB | Fill automatically to at least that amount |
| Choose All I can | Add every Safe item in the chosen areas |
| Choose Where from | Limit automatic filling to those groups. Everywhere clears the limit |
| Add an item or part | Keep it in the pile, even if it isn't Safe |
| Take a crate out | Remove it, switch to manual mode, and do not replace it |
| Keep an item | Remove it from the pile and all automatic choices |
| Stop keeping | Make the item available again |
| Choose a size again | Leave manual mode and clear taken-out items |
| Change Where from | Leave manual mode, but keep taken-out items out |

Automatic filling starts with items the user added. It then adds Safe items, largest first, until the goal is met. Review and Risky items are never added automatically.

In manual mode, show: "You chose these by hand. Pick a size again to refill the pile." If the pile is empty, disable the main button and show: "The pile is empty. Pick a size to fill it again." If Safe items fall short of the goal, show the remaining amount and point to the items below.

### Item-level controls

| Control | Where it appears | Behavior |
| --- | --- | --- |
| Checkbox | Everything I found, Disk map | Add to or take out of the pile |
| Remove now | Everything I found, Disk map | Open a confirmation for that item |
| Keep | Everything I found, Disk map | Mark as Keep, and offer Stop keeping |
| Remove all here | Group or map area | Open a confirmation for all eligible items in that scope |
| Add all to pile | Group or map area | Add all eligible items in that scope |
| Choose which | Items with parts | Select individual projects, models, emulators, apps, backups or versions |

**Remove all here** and **Add all to pile** exclude blocked items, kept items, changed items, and items that need individual confirmation.

For split items, size and every total follow the selected parts. After a partial cleanup, unselected parts stay listed. If no parts are selected, disable the item's checkbox and **Remove now**.

### Quick questions

Show one card at a time, biggest first. Each card shows the item, size, reason, location, last use, action, recovery and **Choose which** where available.

| Button | Behavior |
| --- | --- |
| Add to pile | Add the item and move to the next card |
| Keep, don't ask again | Mark as Keep until the user chooses Stop keeping |
| Ask me later | Skip it and offer it again from the finished state or the next scan |

A Risky item with `needsConfirmation` requires a tick before **Add to pile** works.

### Cleanup rules

- The backend builds and stores the plan from selected IDs, then returns its totals and contents.
- A plan expires after 30 minutes, or sooner if a new scan runs. Confirmation of an expired plan is rejected.
- Each `needsConfirmation` item requires its own acknowledgment in the confirmation.
- Show three totals: free right away, held in the Trash, and cannot be recovered.
- Keep the confirm button visible while a long item list scrolls.
- Only one cleanup can run at a time.
- The user can cancel between items. Cancellation never interrupts an item already running.
- If one item fails, record the error and continue with the next item.
- After the run, measure free space again and report the measured change as well as the expected total.
- A skipped item stays listed and explains why it was skipped.

### Keep me tidy rules

| Rule | Default | Limit |
| --- | --- | --- |
| Clear package caches over 30 days old when space is Low | Off | Named Safe cache detectors only |
| Clear unused Docker build cache every week | Off | `docker.build_cache` only |

Rules run at low priority, stop if a pre-cleanup check fails, and appear in the Logbook. Turning a rule on must state exactly what it will remove.

### Pre-cleanup checks

| Resource | Checks immediately before cleanup |
| --- | --- |
| All paths | Canonical path, protected-path rules, no symlink swap, same fingerprint |
| Worktree | Still clean, still pushed or merged, not locked, no new commits |
| Project folder | Manifest still present, no process running in the project |
| Docker image or volume | Still unused by every container |
| Docker container | Still stopped |

### Trash

Items moved to the Trash remain listed as space held in the Trash until the user empties them or removes them outside Freeboard. Emptying affects only items Freeboard moved there and requires a separate confirmation. If a volume or system has no usable Trash, stop that item and offer permanent deletion as a new, separately confirmed plan.

### Disk status

Freeboard monitors the home volume by default, plus any other fixed volumes the user adds. It samples free space every 5 minutes in memory and stores one sample each hour.

| Status | Default rule | Tray icon | Notification |
| --- | --- | --- | --- |
| Healthy | 20 GB free or more | Neutral ring | None |
| Low | Below 20 GB | Amber ring | Once on entering Low |
| Critical | Below 8 GB | Red ring | On entering Critical, then every 6 hours while unchanged |

A status improves only after free space rises 2 GB above its threshold, which stops repeated alerts around the limit. Clicking a notification opens Suggestions.

### Forecast

Calculate the daily change with a linear fit over the last 14 days of samples. Show "About N days left" only when there are at least 3 days of samples and free space is falling. Exclude cleanup jumps from the fit. Otherwise, show no forecast.

### Background behavior

- Closing the window hides it to the tray, and the webview is destroyed after 60 seconds hidden.
- Monitoring, scheduled quick scans and enabled rules continue in Rust.
- Deep scans run at low priority and pause on battery power by default.
- The tray view shows Dinghy's status, free space, the current pile total and a button to review it.
- The tray view closes with its close button, Esc, or a click anywhere outside it.
- The tray menu offers Open Freeboard, Scan Now, Pause Notifications and Quit.

## Interface and app contract

`freeboard-prototype.html` is the visual reference for this section. Its numbers are example data, not requirements. Open it in a browser, and use it to match layout, tone and interaction.

### Screen structure

The app has one home view with three sections. Keep Settings behind the gear button, so the main navigation stays small.

| Area | Must show | Main actions |
| --- | --- | --- |
| Tray | Status, free space, pile total | Review pile, close |
| Home | Dinghy, speech bubble, free space, status, forecast, goal, Where from, pile total | Toss, See what is inside, Empty it |
| Quick questions | One card, protected items, Everything I found, Keep me tidy | Decide items, remove by group or item |
| Disk map | Coverage, map, area details, blind spots, growth, places checked, disk report | Fix blind spots, remove by area or item, copy report |
| Logbook | Date, result, space freed, Trash amount, skipped items | Open run details |
| Settings | Warning levels, scan folders, excluded folders, scan schedule, startup, notifications | Change settings |

### Dinghy

Dinghy is an original mascot. Draw it with code or SVG, and keep status available as text.

| Status | Face | Water |
| --- | --- | --- |
| Healthy | Happy | Sits high |
| Low | Worried | Sits lower |
| Critical | Panicked, with a sweat drop | Almost at the deck |

Deck height moves in a straight line from 0 GB to 60 GB free, then stays still. Show up to 9 crates, largest first, each labeled with its rounded GB. Crates leave the boat during cleanup, and the boat rises as measured free space changes. Respect the operating system's reduced motion setting.

### Visual system

- Match the prototype's color tokens in light and dark modes. Dark mode is a night scene.
- Use Fredoka for headings, Nunito for interface text and DM Mono for paths and commands.
- Use semantic colors for Safe, Review and Risky, and always include the text label.
- Keep corners soft and spacing generous, with one primary action in view.

### Disk map rules

Draw a squarified treemap to scale. Each area is a real button with a text label for assistive tools. Selecting one shows its items and the space to leave alone. Not explained uses a striped, dashed style. Labels must wrap or hide cleanly, and never clip.

### Copy

Dinghy may speak in the first person in the bubble, blind spots and disk map. Controls stay plain and exact. Use these labels: **Toss**, **Add to pile**, **Remove now**, **Keep**, **Stop keeping**, **Keep, don't ask again**, **Ask me later**, **Choose which**, **Remove all here** and **Add all to pile**.

### Accessibility

- Every control works by keyboard and has a visible focus style.
- Sheets trap focus, close with Esc when no cleanup is running, and return focus when closed.
- Do not rely on color or animation alone.
- Re-rendering a list keeps open groups and keyboard focus in place.

### First-run setup

First launch opens a five-step setup in a modal window, never a single long form. The left half is Dinghy's sea scene with a speech bubble; the right half holds a buoy progress row, the step content and a footer with **Back** and the main button. **Skip setup** (and Esc) is available on steps 1 to 4 and keeps the defaults. On narrow windows the two halves stack, sea on top.

| Step | Title | What it shows | Dinghy |
| --- | --- | --- | --- |
| 1 Hello | Know before your disk is full. | Free space and disk size from `get_disk_status`, a used-space bar, three promises: warns early, tosses what comes back, stays on this computer. Button **Let's set sail**. | Waves a flag |
| 2 Waterline | Where's your waterline? | Three presets as radio cards: Relaxed 10/4 GB, Balanced 20/8 GB (tagged Recommended, preselected), Roomy 50/20 GB. A 0–160 GB gauge with red, amber and green zones and a marker at current free space, plus "Right now: Low". | Bubble reacts to the chosen preset |
| 3 Folders | Where do you keep your code? | Folders found by a shallow search for git repositories, as checkbox cards with path, repository count and size; code folders preselected, Downloads unchecked. **+ Add another folder** opens an inline path field and native picker. Below: tools found on PATH as chips; missing ones struck through. | Holds a spyglass |
| 4 Promises | What I read, and what I never do. | Two checklists (reads: free space every 5 minutes, tool reports, sizes and dates in chosen folders; never: remove without a confirmed plan, send data anywhere, touch uncommitted or unpushed work). Switches: menu bar free space (on), open at login (on). Full Disk Access row with **Later**. Button **Start first look**. | Holds a green shield |
| 5 First look | Taking a first look… / Here's what I found. | Progress over the 49 places with the current place named; findings appear one by one with sizes; then the total that could come back and how much of it is Safe. Button **Show me**, disabled until done. | Crates land on deck as findings arrive; smiles when done |

**Show me** saves the thresholds, the chosen folders and the two switches, marks setup complete, and opens the main window on Quick questions. Skipping saves nothing except `setupDone: false`, and Settings shows a **Run setup again** link. The first look is the normal first scan, not a separate code path.

### Menu bar window

Clicking the menu bar item opens a 330 px window that answers "am I OK, and what would one click get me?". It closes on outside click, Esc or its close button. Sections, top to bottom, separated by hairlines:

1. **Header:** small Dinghy, "Freeboard", settings gear, close.
2. **Status:** free space as the large number with the status pill; a gauge from 0 to 60+ GB with red (under Critical), amber (under Low) and green zones at the user's thresholds, a dot at current free space and a striped stretch to where the pile would land; one Dinghy line plus "Full in about N days" from the forecast. The section is tinted amber when Low and pink when Critical.
3. **Ready to toss:** the three biggest crates in the pile with sizes, then "+ N more" with their total. The Toss button opens the normal confirm sheet.
4. **Links:** "N quick questions · X GB ›" opens the main window on Quick questions; "\<area> grew fastest +X GB this week".
5. **Footer:** "Scanned 18 min ago", **Scan now** (quick scan, label becomes "Scanning…"), **Open Freeboard**.

| State | Dinghy line | Toss button |
| --- | --- | --- |
| Healthy | Plenty of room. Nothing urgent. | Quiet outlined style, with "Optional. Everything here rebuilds by itself." |
| Low | Riding low. Toss X GB and I float again. | Filled primary |
| Critical | Nearly under! Toss something now. | Filled primary |
| Nothing Safe to toss | (status line only) | Hidden; "Nothing Safe to toss right now." |

The window never shows Review or Risky items as tossable. On phones-width previews it spans the screen with a 12 px gutter.

### Tauri commands

| Command | Input | Returns |
| --- | --- | --- |
| `get_status` | None | Volumes, status, forecast, Trash total |
| `start_scan` | `kind` | `scanId` |
| `get_suggestions` | Optional filters | `Suggestion[]` |
| `get_disk_map` | Volume | `DiskMap` |
| `get_blind_spots` | Volume | `BlindSpot[]` |
| `resolve_blind_spot` | `id`, `action` | Updated map and blind spots |
| `get_places_checked` | None | `PlaceChecked[]` |
| `get_growth` | `days` | Area changes |
| `get_pile` | None | `PileState` |
| `set_pile_goal` | `goalBytes` | `PileState` |
| `set_pile_areas` | `areas` | `PileState` |
| `add_to_pile` | `itemIds` | `PileState` |
| `take_out_of_pile` | `itemIds` | `PileState` |
| `set_item_decision` | `itemId`, `decision` | Updated item |
| `set_item_parts` | `itemId`, `partIds` | Updated item |
| `create_plan` | `{ source: "pile" }` or `itemIds` | `CleanupPlan` |
| `acknowledge_item` | `planId`, `itemId` | Updated plan |
| `run_plan` | `planId` | `runId` |
| `cancel_run` | `runId` | Accepted or not |
| `empty_freeboard_trash` | Confirmation token | Result |
| `set_tidy_rule` | `ruleId`, `enabled` | Updated rules |
| `create_disk_report` | None | Report JSON |
| `list_logbook` | Page | Run summaries |
| `get_settings` | None | `Settings` |
| `update_settings` | Partial settings | `Settings` |

### Events

| Event | Payload |
| --- | --- |
| `status_changed` | Volume, status, free bytes |
| `scan_progress` | Scan ID, phase, detector, current count |
| `scan_finished` | Scan ID, detector errors, totals |
| `pile_changed` | `PileState` |
| `disk_map_changed` | `DiskMap` |
| `blind_spots_changed` | `BlindSpot[]` |
| `cleanup_item_updated` | Run ID, item ID, state, reason |
| `cleanup_finished` | Run ID, measured bytes freed, skipped count |
| `tidy_rule_ran` | Rule ID, result, bytes freed |

### Default settings

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
| Scan folders | Chosen during first run |

The first run explains what Freeboard reads, asks for scan folders, and requests an operating system permission only when a feature needs it. This flow isn't in the prototype, so use its visual style.

### Required behavior missing from the prototype

- First-run setup and the permission flow.
- A confirmation before emptying Trash items.
- Cancel during cleanup.
- Scan progress, scan failure and detector-unavailable states.
- Empty states for a new install, no suggestions, and an empty Logbook.
- Run details in the Logbook.
- Adding and excluding scan folders in Settings.
- Windows and Linux labels, including Recycle Bin and the correct system area name.

## Delivery plan

Build the read-only product first, because cleanup should arrive only after its suggestions and disk map have proven reliable. Each milestone must pass on macOS, Linux and Windows before the next one starts.

### Platform notes

| Platform | Requirements |
| --- | --- |
| macOS | Request Full Disk Access only when the user chooses to fix that blind spot. Report APFS purgeable space and local snapshots separately, because totals may differ from Finder. Sign and notarize release builds. |
| Linux | Test tray behavior on GNOME and KDE. Test WebKitGTK early, especially animation and the treemap. Respect XDG cache and data locations. Ship AppImage and `.deb` first. |
| Windows | Treat junctions and reparse points as links. Support long paths. Use the Recycle Bin for Trash actions. Sign release builds to reduce SmartScreen warnings. |
| Docker Desktop | Pruned space may stay inside its virtual disk, so show the measured change instead of promising the expected amount. Compact only through a supported method, after warning that Docker will stop briefly. |
| WSL | Don't scan WSL filesystems in the first version. Show virtual disk files in the map when found. |

### Milestones

1. **Foundation:** create the Tauri app, permissions, SQLite storage, tray, settings, bundled fonts, design tokens and CI builds on all three platforms.
   - Exit: installers build, and the frontend cannot reach filesystem or shell APIs.
2. **Monitoring and home view:** add sampling, status, notifications, forecast, Dinghy, tray view and background mode.
   - Exit: thresholds and hysteresis are tested, the tray view closes every required way, and hidden-mode memory is measured.
3. **Disk understanding:** add the scan engine, size measurement, detectors, disk accounting, blind spots, places checked, growth and disk report.
   - Exit: fixture tests pass, no scan changes a fixture, and map totals match used bytes within 1%.
4. **Selection:** add goals, areas, the pile, quick questions, item controls, parts and Keep decisions, all read-only.
   - Exit: every rule in Building the pile has a passing test.
5. **Cleanup:** add plans, pre-cleanup checks, execution, Trash handling, cancellation and the Logbook.
   - Exit: every safety invariant has a passing test.
6. **Keep me tidy and release:** add rules, first-run setup, empty states, performance work, accessibility, signing and documentation.
   - Exit: every acceptance criterion below passes.

### Test plan

- **Unit tests:** risk rules, protected paths, size counting, status thresholds, forecast, plan expiry and every pile rule.
- **Fixture tests:** Git repositories for every worktree state, project folders, duplicates, symlink traps, and fake `docker`, `ollama`, `nvm` and `rustup` executables.
- **Accounting tests:** hard links, nested detectors, unreadable folders, totals and blind spots.
- **Change-during-cleanup tests:** add a commit, modify a file, start a container and swap a path for a symlink between scan and cleanup.
- **Partial-selection tests:** remove selected parts, keep the rest, and confirm that sizes and the map update.
- **Failure tests:** missing tools, command timeouts, permission errors and malformed JSON.
- **Contract tests:** the frontend cannot execute a path or command directly.
- **Rule tests:** rules stay off by default, skip kept items and can't remove Review or Risky items.
- **Interface tests:** keyboard use, focus return, Esc, reduced motion and persistent open groups.
- **Manual checks:** tray, notifications, Trash and permissions on real macOS, Linux and Windows machines.

### Acceptance criteria

- [ ] The app visually matches the prototype in light and dark modes.
- [ ] Dinghy's position and mood match the disk status, and the status also appears in text.
- [ ] The tray view closes with its close button, Esc and an outside click.
- [ ] Choosing a goal fills the pile with Safe items, and one confirmation removes them.
- [ ] Where from limits automatic filling to the chosen groups.
- [ ] Taking a crate out reduces the total and doesn't refill the pile.
- [ ] Users can remove or keep an individual item from Everything I found and the Disk map.
- [ ] Users can act on a whole group or map area.
- [ ] Users can choose individual parts, and unselected parts stay listed after cleanup.
- [ ] Crossing Low or Critical sends one notification under the rules above.
- [ ] Suggestions cover every detector family in this spec.
- [ ] Every suggestion shows its size, risk, reason, action and recovery.
- [ ] The disk map accounts for used space within 1%, and unexplained space is visible.
- [ ] Each blind spot has a working action, and a resolved blind spot updates coverage.
- [ ] Places checked separates found, clean and unavailable detectors.
- [ ] The disk report can be copied and contains no file contents.
- [ ] Dirty, unpushed, locked and unknown-state worktrees can't be selected.
- [ ] Docker volumes are never preselected and need individual acknowledgment.
- [ ] The plan separates space freed now, space held in Trash and unrecoverable items.
- [ ] Items that changed after the scan are skipped and explained.
- [ ] Cleanup results show measured free-space changes.
- [ ] Keep me tidy rules are off by default and can remove only named Safe items.
- [ ] A failed or missing tool does not stop other detectors.
- [ ] Every safety invariant has an automated test.
- [ ] The app makes no network calls of its own.
- [ ] Performance results are reported against the targets in the overview.

### Open questions

- Will Freeboard be open source? If so, which license?
- Which operating system should get the first signed release?
- Are Freeboard and Dinghy the final names after domain, package and trademark checks?
- Should a later version add manual folder browsing?

## Sources

- [Tauri vs. Electron vs. Deno Desktop vs. Electrobun (Better Stack)](https://betterstack.com/community/guides/scaling-nodejs/tauri-vs-electron-vs-deno-vs-electrobun/)
- [Desktop App Frameworks 2026 (Youngju Kim)](https://www.youngju.dev/split/culture/2026-05-14-desktop-app-frameworks-2026-tauri-electron-wails-compose-multiplatform-maui-flutter-comparison-deep-dive-2026)
- [dua-cli package details (Debian)](https://dfsg-new-queue.debian.org/reviews/dua-cli)
- [diskr-cli README](https://docs.rs/crate/diskr-cli/1.0.0/source/README.md)
