// TEST-ONLY fixture backend used by the visual verifier (`vite build --mode visual`).
// It is never part of the production bundle (scripts/visual/verify.mjs checks).
// Marker: FREEBOARD_FIXTURE_BACKEND
//
// The data is the prototype's example data (docs/reference/freeboard-prototype.html:
// ITEMS, TILES, SPOTS), served through the same commands the Rust core offers,
// and the pile follows the prototype's rules (picks, freezePile, addToPile,
// takeOut). Numbers are the prototype's, so captures can match it pixel for pixel.
//
// Query parameters:
//   free=<GB>          current free space (default 12.4, the prototype's BASE_FREE)
//   platform=macos|linux|windows
//   forecast=none      no forecast
//   trash=<GB>         held in Freeboard's Trash

import type { Backend } from "./api";
import type { AppStatus, BlindSpot, DiskMap, EventMap, Goal, Group, PileState, PlaceChecked, Platform, Settings, Suggestion } from "./types";

export const FIXTURE_MARKER = "FREEBOARD_FIXTURE_BACKEND";
const G = 1e9;
const TOTAL_GB = 512;
const BASE_FREE = 12.4;
const RATE_GB = 2.2;

type Row = [
  id: string,
  group: string,
  map: string,
  name: string,
  where: string,
  size: number,
  last: string,
  risk: "Safe" | "Review" | "Risky",
  action: "native" | "delete" | "trash" | "none",
  cmd: string,
  why: string,
  back: string,
  extra?: { ack?: boolean; lost?: boolean; blocked?: string; parts?: [string, number][]; need?: string },
];

// Copied from the prototype's ITEMS, in the same order.
const ROWS: Row[] = [
  ["d-cache", "Docker", "docker", "Docker build cache", "142 layers", 14.8, "Last used 23 days ago", "Safe", "native", "docker builder prune", "No recent build reused these layers.", "Docker rebuilds them on the next build."],
  ["d-dangling", "Docker", "docker", "Untagged Docker images", "31 images", 6.2, "Over 30 days old", "Safe", "native", "docker image prune", "Old layers with no tag and no container.", "Rebuild from the Dockerfile."],
  ["d-slack", "Docker", "docker", "Empty space in Docker\u2019s disk", "Docker.raw", 19.0, "Stays empty after pruning", "Review", "native", "Compact the Docker disk image", "Docker Desktop keeps this space reserved even when nothing uses it. Docker must stop for a moment.", "Nothing is lost. Docker grows the disk again when needed."],
  ["d-vol", "Docker", "docker", "Volume billing_pgdata", "Not attached to a container", 7.6, "Last attached 40 days ago", "Risky", "native", "docker volume rm billing_pgdata", "Nothing uses it now, but it may hold the only copy of this data.", "It cannot be recovered.", { ack: true, lost: true }],
  ["d-pg", "Docker", "docker", "postgres:14 image", "No container uses it", 3.4, "Last used 71 days ago", "Review", "native", "docker image rm postgres:14", "A tagged image no container uses.", "Run docker pull postgres:14."],
  ["d-stop", "Docker", "docker", "Stopped containers", "6 containers", 1.1, "Stopped over 14 days ago", "Review", "native", "docker container rm", "Their writable layers still take up space.", "Create them again. Files written inside them are lost."],
  ["b-node", "Projects", "projects", "node_modules in quiet projects", "~/projects", 18.7, "Idle for 60+ days", "Safe", "delete", "Delete the folders", "Every project has a lockfile.", "Run pnpm install.", { parts: [["web-app", 4.2], ["admin-portal", 3.1], ["mobile-app", 2.6], ["docs-site", 1.7], ["storybook", 1.4], ["landing", 1.1], ["api-client", 0.9], ["scripts", 0.8], ["cli-tools", 0.7], ["demo", 0.6], ["tests-e2e", 0.5], ["poc-graphql", 0.5], ["design-tokens", 0.3], ["sandbox", 0.3]] }],
  ["b-target", "Projects", "projects", "Rust build output", "~/projects/rust-cli/target", 9.2, "Last built 45 days ago", "Safe", "delete", "Delete the folder", "Cargo can regenerate this.", "Run cargo build."],
  ["wt-pay", "Projects", "projects", "Worktree payments-v2", "~/work/web-app/.worktrees/payments-v2", 6.9, "Edited 9 days ago", "Risky", "none", "None", "3 uncommitted files and 2 unpushed commits.", "Your work exists only here.", { blocked: "Commit and push it, and I can clean this next time." }],
  ["wt-exp", "Projects", "projects", "Worktree old-experiment", "~/work/web-app/.worktrees/old-experiment", 5.1, "Last commit 52 days ago", "Review", "native", "git worktree remove", "No local changes. The branch is pushed but not merged.", "Run git worktree add with the pushed branch."],
  ["wt-auth", "Projects", "projects", "Worktree fix-auth", "~/work/api-server/.worktrees/fix-auth", 4.3, "Merged 34 days ago", "Review", "native", "git worktree remove", "The branch is merged and has no local changes.", "Run git worktree add with the same branch."],
  ["b-venv", "Projects", "projects", ".venv in data-tools", "~/projects/data-tools/.venv", 2.4, "Last used 88 days ago", "Review", "trash", "Move to Trash", "It may hold packages you installed by hand.", "Restore it from the Trash."],
  ["c-derived", "Caches", "caches", "Xcode DerivedData", "~/Library/Developer/Xcode", 11.3, "Mostly 30+ days old", "Safe", "delete", "Delete the folder", "Xcode regenerates this.", "Build the project again."],
  ["c-pnpm", "Caches", "caches", "Unused pnpm packages", "pnpm store", 5.6, "No project links to them", "Safe", "native", "pnpm store prune", "No project uses these packages.", "pnpm downloads them when needed."],
  ["c-jb", "Caches", "caches", "Old JetBrains caches", "~/Library/Caches/JetBrains", 5.1, "From 2 removed versions", "Safe", "delete", "Delete the folders", "They belong to IDE versions you removed.", "Nothing to restore."],
  ["c-apps", "Caches", "caches", "Slack, Discord and Teams caches", "App caches", 3.9, "Rebuilt as you use them", "Safe", "delete", "Delete the caches while the apps are closed", "Messages stay on the server.", "The apps rebuild them."],
  ["c-pw", "Caches", "caches", "Old Playwright browsers", "~/Library/Caches/ms-playwright", 3.6, "Older than your installed version", "Safe", "delete", "Delete old versions", "No project needs these versions.", "Run playwright install."],
  ["c-pip", "Caches", "caches", "pip download cache", "pip cache", 2.6, "Mostly 30+ days old", "Safe", "native", "pip cache purge", "Downloaded wheels pip can fetch again.", "pip downloads them when needed."],
  ["c-brew", "Caches", "caches", "Old Homebrew downloads", "Homebrew cache", 2.1, "Over 30 days old", "Safe", "native", "brew cleanup", "Old versions of installed packages.", "Homebrew downloads them when needed."],
  ["ai-ollama", "AI models", "ai", "Unused Ollama models", "~/.ollama/models", 41.2, "Not run for 60+ days", "Review", "native", "ollama rm", "You have not run these models recently.", "Run ollama pull. It downloads again.", { parts: [["mixtral", 26.0], ["codellama", 8.2], ["phi3", 7.0]] }],
  ["ai-hf", "AI models", "ai", "Hugging Face cache", "~/.cache/huggingface", 12.4, "Last used 47 days ago", "Review", "trash", "Move to Trash", "Downloaded models and datasets.", "Restore from the Trash, or download again."],
  ["m-ios", "Phones and emulators", "mobile", "Old iPhone backups", "~/Library/Application Support/MobileSync", 22.5, "From March 2024", "Review", "trash", "Move to Trash", "Two backups of a phone that has not synced since 2024.", "Restore from the Trash until you empty it.", { parts: [["Backup from March 2024", 13.1], ["Backup from January 2024", 9.4]] }],
  ["m-android", "Phones and emulators", "mobile", "Android emulators", "~/.android/avd", 14.0, "Not started for 90+ days", "Review", "trash", "Move to Trash", "Three emulator images you have not started.", "Restore from the Trash, or create them again.", { parts: [["Pixel_6_API_33", 6.0], ["Pixel_4_API_30", 4.5], ["Tablet_API_31", 3.5]] }],
  ["m-sim", "Phones and emulators", "mobile", "Unavailable iOS simulators", "9 devices", 8.4, "Runtime no longer installed", "Review", "native", "xcrun simctl delete unavailable", "They depend on runtimes that are gone.", "Create them again in Xcode."],
  ["o-apps", "Old tools and apps", "apps", "Apps you never open", "/Applications", 16.3, "Not opened for a year", "Review", "trash", "Move to Trash", "You have not opened these apps for 12+ months.", "Restore from the Trash, or reinstall.", { parts: [["GarageBand", 6.1], ["iMovie", 4.2], ["Unity Hub", 3.0], ["Blender", 3.0]] }],
  ["o-node", "Old tools and apps", "apps", "Old Node versions", "~/.nvm/versions", 4.8, "No project asks for them", "Review", "native", "nvm uninstall", "No project pins these 3 versions.", "Run nvm install.", { parts: [["Node 14", 1.4], ["Node 16", 1.6], ["Node 18", 1.8]] }],
  ["o-rust", "Old tools and apps", "apps", "Old Rust toolchains", "~/.rustup/toolchains", 3.7, "Last used 5 months ago", "Review", "native", "rustup toolchain uninstall", "Two nightly toolchains no project uses.", "Run rustup toolchain install."],
  ["f-dups", "Your files", "files", "Duplicate files", "214 copies", 9.4, "Exact byte-for-byte copies", "Review", "trash", "Move extra copies to Trash", "One copy of each file stays where it is.", "Restore from the Trash."],
  ["f-rec", "Your files", "files", "Old screen recordings", "~/Desktop", 8.8, "Not opened for 6+ months", "Review", "trash", "Move to Trash", "11 large video files on your Desktop.", "Restore from the Trash."],
  ["f-dl", "Your files", "files", "Installers in Downloads", "~/Downloads", 4.6, "Not opened for 90+ days", "Review", "trash", "Move to Trash", "12 installers and disk images.", "Restore from the Trash."],
  ["s-snap", "System", "system", "Time Machine snapshots", "Local snapshots", 15.2, "Your latest backup is safe", "Review", "native", "tmutil deletelocalsnapshots", "macOS keeps these on disk until it needs the room.", "Time Machine makes new ones."],
  ["f-laptop", "Your files", "files", "old-laptop backup", "~/archive/old-laptop", 11.2, "Not opened since 2023", "Review", "trash", "Move to Trash", "A full copy of a previous computer.", "Restore from the Trash.", { need: "archive" }],
  ["s-temp", "System", "system", "Leftover app temp files", "/private/var/folders", 5.4, "Over 30 days old", "Review", "trash", "Move to Trash", "Files left behind by apps that are no longer running.", "Restore from the Trash.", { need: "access" }],
];

const GROUP_ID: Record<string, Group> = {
  Docker: "docker",
  Projects: "projects",
  Caches: "caches",
  "AI models": "ai_models",
  "Phones and emulators": "phones_emulators",
  "Old tools and apps": "old_tools_apps",
  "Your files": "your_files",
  System: "system",
};
const ACTION = { native: "native_command", delete: "delete", trash: "trash", none: "none" } as const;

// Prototype TILES. Your files absorbs changes in free space (prototype `tileSize`).
const TILES: [id: string, label: string, gb: number, rest: string][] = [
  ["projects", "Projects", 96, "Active projects and code"],
  ["docker", "Docker", 72, "Images and data you are using"],
  ["ai", "AI models", 61.6, "Models you used recently"],
  ["caches", "Caches", 58, "Caches in active use"],
  ["files", "Your files", 58, "Documents, photos and videos"],
  ["mobile", "Phones and emulators", 45, "Current simulators"],
  ["apps", "Apps and tools", 44, "Apps and tools you use"],
  ["system", "macOS", 44.2, "macOS itself"],
];
const NOT_EXPLAINED_GB = 20.8;

// Prototype SPOTS, GROWING and EXTRA_PLACES.
const SPOTS = [
  { id: "archive", size: 11.2, to: "files", kind: "unscanned_folder", action: "scan_folder", title: "A folder I was not asked to scan", text: "~/archive holds 11.2 GB that no detector looked at.", label: "Scan this folder", done: "Scanned. I found an old laptop backup.", where: "~/archive" },
  // The prototype grants access at once. The real app only opens System
  // Settings and waits for a scan that can read the place (design D5); this
  // test-only fixture shows the state after that scan.
  { id: "access", size: 5.4, to: "system", kind: "needs_permission", action: "open_settings", title: "A place I am not allowed to read", text: "5.4 GB in /private/var/folders needs Full Disk Access.", label: "Allow access", done: "Now readable. I found leftover temp files.", where: "/private/var/folders" },
  { id: "mystery", size: 4.2, to: "files", kind: "unknown_files", action: "mark_mine", title: "Files I do not recognize", text: "~/tools/sim-data has 4.2 GB of .dat files from an unknown tool.", label: "They are mine, keep them", done: "Marked as yours. I will not ask again.", where: "~/tools/sim-data" },
] as const;
const GROWING: [string, string, number][] = [
  ["docker", "Docker", 9.3],
  ["ai", "AI models", 7.0],
  ["web-app", "~/projects/web-app", 4.1],
  ["slack", "Slack cache", 1.2],
];
const EXTRA_PLACES = ["npm cache", "Yarn cache", "Bun cache", "Maven repository", "Gradle caches", "Go module cache", "CocoaPods", "Conda environments", "pyenv versions", "Cargo registry", "Turbo and Nx caches", "VS Code old extensions", "Browser caches", "LM Studio models", "Trash", "Log files", "iOS device support", "Mail attachments"];

const bytes = (gb: number) => Math.round(gb * G);
const round1 = (n: number) => Math.round(n * 10) / 10;

export function createFixtureBackend(params: URLSearchParams): Backend {
  const platform = (params.get("platform") ?? "macos") as Platform;
  const freeGb = params.has("free") ? parseFloat(params.get("free")!) : BASE_FREE;
  const trashGb = params.has("trash") ? parseFloat(params.get("trash")!) : 0;
  const resolvedSpots = new Set<string>();

  type Item = Suggestion & { need?: string };
  const items: Item[] = ROWS.map(([id, group, map, name, where, size, last, risk, action, cmd, why, back, extra]) => {
    const parts = (extra?.parts ?? []).map(([label, gb], n) => ({ id: `${id}-p${n}`, label, location: where, sizeBytes: bytes(gb), selected: true }));
    return {
      id,
      detector: "fixture." + map,
      group: GROUP_ID[group],
      mapArea: map,
      title: name,
      location: where,
      sizeBytes: bytes(size),
      fullSizeBytes: bytes(size),
      lastUsedAt: null,
      lastUsedLabel: last,
      risk: risk.toLowerCase() as Suggestion["risk"],
      action: ACTION[action],
      actionLabel: cmd,
      reason: why,
      recovery: back,
      recoverable: !extra?.lost,
      blockedReason: extra?.blocked ?? null,
      needsConfirmation: !!extra?.ack,
      parts,
      fingerprint: "fixture",
      sizeIsEstimate: false,
      skipReason: null,
      decision: null,
      ackName: extra?.ack ? name.replace(/^Volume /, "") + " volume" : null,
      need: extra?.need,
    };
  });
  const active = () => items.filter((i) => !i.need || resolvedSpots.has(i.need));

  // ----- the pile (prototype picks / freezePile / addToPile / takeOut) -----
  let goal: Goal = 50 * G;
  let areas: Group[] = [];
  let manual: Set<string> | null = null;
  const queued = new Set<string>();
  const excl = new Set<string>();
  const kept = (i: Item) => i.decision === "keep";
  const bySize = (a: Item, b: Item) => b.sizeBytes - a.sizeBytes;
  const safeItems = () =>
    active()
      .filter((i) => i.risk === "safe" && !excl.has(i.id) && !kept(i) && i.sizeBytes > 0 && (!areas.length || areas.includes(i.group)))
      .sort(bySize);
  function picks(): Item[] {
    if (manual) return active().filter((i) => manual!.has(i.id) && !kept(i) && i.sizeBytes > 0).sort(bySize);
    const out = active().filter((i) => queued.has(i.id) && i.sizeBytes > 0);
    const inPile = new Set(out.map((i) => i.id));
    let have = out.reduce((t, i) => t + i.sizeBytes, 0);
    for (const i of safeItems()) {
      if (inPile.has(i.id)) continue; // the prototype could list an added Safe item twice
      if (goal === "all" || have < goal) {
        out.push(i);
        have += i.sizeBytes;
      }
    }
    return out;
  }
  function pile(): PileState {
    const p = picks();
    return {
      goalBytes: goal,
      areas: [...areas],
      mode: manual ? "manual" : "auto",
      addedIds: [...queued],
      takenOutIds: [...excl],
      itemIds: p.map((i) => i.id),
      totalBytes: p.reduce((t, i) => t + i.sizeBytes, 0),
    };
  }
  const freeze = () => {
    if (!manual) manual = new Set(picks().map((i) => i.id));
  };

  const settings: Settings = {
    lowThresholdBytes: 20 * G,
    criticalThresholdBytes: 8 * G,
    goal: 50 * G,
    whereFrom: [],
    quickScan: "hourly",
    deepScan: "on_request",
    inactiveProjectDays: 30,
    oldInstallerDays: 90,
    pauseDeepScansOnBattery: true,
    startAtLogin: true,
    notifications: true,
    scanFolders: platform === "windows" ? ["C:\\Users\\you\\projects", "C:\\Users\\you\\work", "C:\\Users\\you\\Downloads", "C:\\Users\\you\\Desktop"] : ["/Users/you/projects", "/Users/you/work", "/Users/you/Downloads", "/Users/you/Desktop"],
    exclusions: [],
    extraVolumes: [],
    onboardingComplete: true,
  };

  const status = (): AppStatus => {
    const free = bytes(freeGb);
    const s = free < settings.criticalThresholdBytes ? "critical" : free < settings.lowThresholdBytes ? "low" : "healthy";
    return {
      volumes: [{ id: "home", name: "Macintosh HD", mountPoint: "/", freeBytes: free, totalBytes: TOTAL_GB * G, status: s, purgeableBytes: null, snapshotCount: null }],
      status: s,
      forecast: params.get("forecast") === "none" ? null : { bytesPerDay: RATE_GB * G, daysLeft: freeGb / RATE_GB },
      trashBytes: bytes(trashGb),
      trend: [],
      scan: { running: null, phase: null, lastFinishedAt: "2026-11-04T11:42:00Z", detectorErrors: [] },
      notificationsPaused: false,
      platform,
      firstRun: false,
    };
  };

  const diskMap = (): DiskMap => {
    // Prototype tileSize(): Your files grows or shrinks with free space, and
    // a resolved blind spot moves its size out of Not explained.
    const moved = (id: string) => SPOTS.filter((s) => resolvedSpots.has(s.id) && s.to === id).reduce((t, s) => t + s.size, 0);
    const sizes = TILES.map(([id, label, gb, rest]) => ({ id, label, rest, gb: Math.max(0, round1(gb + moved(id) + (id === "files" ? BASE_FREE - freeGb : 0))) }));
    const un = Math.max(0, round1(NOT_EXPLAINED_GB - SPOTS.filter((s) => resolvedSpots.has(s.id)).reduce((t, s) => t + s.size, 0)));
    const used = round1(sizes.reduce((t, a) => t + a.gb, 0) + un);
    const inArea = (id: string) => active().filter((i) => i.mapArea === id).reduce((t, i) => t + i.fullSizeBytes, 0);
    return {
      usedBytes: bytes(used),
      explainedBytes: bytes(used - un),
      notExplainedBytes: bytes(un),
      areas: sizes.map((a) => ({ id: a.id, label: a.label, bytes: bytes(a.gb), leaveAloneBytes: Math.max(0, bytes(a.gb) - inArea(a.id)), leaveAloneLabel: a.rest })),
      incomplete: false,
      overBytes: 0,
      measuredAt: "2026-11-04T11:42:00Z",
      purgeableBytes: null,
      snapshotCount: null,
    };
  };
  const blindSpots = (): BlindSpot[] =>
    SPOTS.map((s) => ({
      id: s.id,
      kind: s.kind,
      title: s.title,
      text: s.text,
      location: s.where,
      sizeBytes: bytes(s.size),
      actionLabel: s.label,
      action: s.action,
      state: resolvedSpots.has(s.id) ? "done" : "open",
      doneText: resolvedSpots.has(s.id) ? s.done : null,
    }));
  // Prototype placesList(): every item's name (found), then the extra places (clean).
  const places = (): PlaceChecked[] => [
    ...active().map((i) => ({ detector: i.id, label: i.title, state: "found" as const, bytes: i.sizeBytes })),
    ...EXTRA_PLACES.map((n) => ({ detector: "extra." + n, label: n, state: "clean" as const, bytes: 0 })),
  ];
  // Prototype reportText().
  const report = () => {
    const m = diskMap();
    const used = m.usedBytes / G;
    const un = m.notExplainedBytes / G;
    const risk = { safe: "Safe", review: "Review", risky: "Risky" } as const;
    return {
      disk_gb: TOTAL_GB,
      free_gb: +freeGb.toFixed(1),
      explained_pct: +(((used - un) / used) * 100).toFixed(1),
      areas: [...m.areas.map((a) => ({ name: a.label, gb: a.bytes / G })), { name: "Not explained", gb: un }],
      suggestions: active().map((i) => ({ name: i.title, gb: i.sizeBytes / G, risk: risk[i.risk], where: i.location })),
      not_explained: SPOTS.filter((s) => !resolvedSpots.has(s.id)).map((s) => ({ what: s.text, gb: s.size })),
    };
  };

  const listeners: { [K in keyof EventMap]?: ((p: EventMap[K]) => void)[] } = {};
  const emit = <K extends keyof EventMap>(event: K, payload: EventMap[K]) => listeners[event]?.forEach((cb) => cb(payload));
  const changed = () => {
    const p = pile();
    setTimeout(() => emit("pile_changed", p), 0);
    return p;
  };

  const handlers: Record<string, (a: Record<string, unknown>) => unknown> = {
    get_status: () => status(),
    get_settings: () => ({ ...settings }),
    update_settings: (a) => Object.assign(settings, a.patch as Partial<Settings>) && { ...settings },
    get_suggestions: () => active().map(({ need: _need, ...s }) => ({ ...s, parts: s.parts.map((p) => ({ ...p })) })),
    get_disk_map: () => diskMap(),
    get_blind_spots: () => blindSpots(),
    resolve_blind_spot: (a) => {
      resolvedSpots.add(a.id as string);
      // New items appear (prototype `need`), so the suggestions reload too.
      changed();
      setTimeout(() => {
        emit("disk_map_changed", diskMap());
        emit("blind_spots_changed", blindSpots());
      }, 0);
      return { map: diskMap(), blindSpots: blindSpots() };
    },
    get_places_checked: () => places(),
    get_growth: () => GROWING.map(([id, label, gb]) => ({ id, label, bytes: bytes(gb) })),
    create_disk_report: () => report(),
    get_pile: () => pile(),
    set_pile_goal: (a) => {
      goal = a.goalBytes as Goal;
      manual = null;
      excl.clear();
      return changed();
    },
    set_pile_areas: (a) => {
      areas = [...(a.areas as Group[])];
      manual = null;
      return changed();
    },
    add_to_pile: (a) => {
      for (const id of a.itemIds as string[]) {
        queued.add(id);
        excl.delete(id);
        manual?.add(id);
      }
      return changed();
    },
    take_out_of_pile: (a) => {
      freeze();
      for (const id of a.itemIds as string[]) {
        manual!.delete(id);
        queued.delete(id);
        excl.add(id);
      }
      return changed();
    },
    set_item_decision: (a) => {
      const it = items.find((i) => i.id === a.itemId)!;
      it.decision = (a.decision as Suggestion["decision"]) ?? null;
      if (it.decision === "keep") {
        queued.delete(it.id);
        manual?.delete(it.id);
      }
      changed();
      return it;
    },
    set_item_parts: (a) => {
      // Prototype `partSum`: the item's size follows its selected parts.
      const it = items.find((i) => i.id === a.itemId)!;
      const on = new Set(a.partIds as string[]);
      it.parts.forEach((p) => (p.selected = on.has(p.id)));
      it.sizeBytes = it.parts.filter((p) => p.selected).reduce((t, p) => t + p.sizeBytes, 0);
      changed();
      return it;
    },
    folder_suggestions: () => [],
    add_folder: () => ({ ...settings }),
    remove_folder: (a) => {
      const list = a.list === "exclusions" ? settings.exclusions : settings.scanFolders;
      list.splice(a.index as number, 1);
      return { ...settings };
    },
    add_suggested_folder: () => ({ ...settings }),
    start_scan: () => "scan-1",
    // Prototype SAMPLE_LOG: Oct 28 (7 crates, 21.4 GB, 1.2 GB in the Trash) and
    // Oct 12 (11 crates, 34.9 GB, 1 skipped).
    list_logbook: () => ({
      runs: [
        { id: "run-2", startedAt: "2026-10-28T10:00:00Z", items: 7, measuredBytesFreed: 21.4 * G, trashBytes: 1.2 * G, skipped: 0, state: "finished" },
        { id: "run-1", startedAt: "2026-10-12T10:00:00Z", items: 12, measuredBytesFreed: 34.9 * G, trashBytes: 0, skipped: 1, state: "finished" },
      ],
      page: 0,
      hasMore: false,
    }),
    set_notifications_paused: (a) => a.paused,
    close_tray_view: () => undefined,
    review_pile: () => undefined,
  };

  return {
    async invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
      // Behaviour checks (scripts/ui/check.mjs) read the commands the interface sent.
      ((window as unknown as { __FIXTURE_CALLS__?: unknown[] }).__FIXTURE_CALLS__ ??= []).push({ cmd, args: args ?? {} });
      const h = handlers[cmd];
      if (!h) throw new Error("fixture backend: no command " + cmd);
      return structuredClone(h(args ?? {})) as T;
    },
    async listen(event, cb) {
      (listeners[event] ??= [] as never[]).push(cb as never);
      return () => {
        listeners[event] = (listeners[event] as unknown[] | undefined)?.filter((x) => x !== cb) as never;
      };
    },
  };
}
