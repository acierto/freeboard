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
//   hold=<n>           a cleanup handles its first n items, then waits (the
//                      progress sheet; Cancel stops it there)
//   skip=<id,...>      these items changed since the scan and are skipped
//                      (the prototype always skips old-experiment)
//   notrash=1          moving to the Trash fails, as on a volume without one
//
// Cleanup follows the prototype's `run()`: items run in plan order, Trash
// items are held in the Trash, the rest free space at once, and a split item
// cleaned in part keeps its other parts listed.

import type { Backend } from "./api";
import type { AppStatus, BlindSpot, CleanupPlan, DiskMap, EventMap, Goal, Group, PileState, PlaceChecked, Platform, RunDetail, RunItem, RunSummary, Settings, Suggestion } from "./types";

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
  let freeGb = params.has("free") ? parseFloat(params.get("free")!) : BASE_FREE;
  // Freeboard's own Trash records: what a cleanup moved there.
  const held: { item: string; bytes: number }[] = params.has("trash") ? [{ item: "earlier", bytes: bytes(parseFloat(params.get("trash")!)) }] : [];
  const trashBytes = () => held.reduce((t, h) => t + h.bytes, 0);
  const holdAt = params.has("hold") ? parseInt(params.get("hold")!, 10) : null;
  const changedIds = new Set((params.get("skip") ?? "").split(",").filter(Boolean));
  const noTrash = params.get("notrash") === "1";
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
  const gone = new Set<string>();
  const active = () => items.filter((i) => !gone.has(i.id) && (!i.need || resolvedSpots.has(i.need)));

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

  const folderInfo = new Map<string, { repos: number; bytes: number | null }>([
    ["/Users/you/projects", { repos: 14, bytes: 96 * G }],
    ["/Users/you/work", { repos: 2, bytes: 41 * G }],
    ["/Users/you/Downloads", { repos: 0, bytes: 23 * G }],
    ["/Users/you/Desktop", { repos: 0, bytes: 12 * G }],
  ]);
  const offers = [
    { label: "~/ComfyUI-Installs", repos: 1, bytes: 18 * G },
    { label: "~/material-ui", repos: 1, bytes: 2 * G },
  ];
  let removedFolder: { list: "scanFolders" | "exclusions"; at: number; path: string } | null = null;

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
      trashBytes: trashBytes(),
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

  // ----- cleanup (prototype confirmSheet / run, through the real contract) -----
  const fail = (code: string, message: string) => {
    throw { code, message };
  };
  type Step = { id: string; parent: string | null; title: string; action: Suggestion["action"]; actionLabel: string; size: number };
  type Plan = CleanupPlan & { steps: Step[]; permanent: boolean };
  const plans = new Map<string, Plan>();
  const trashFailed = new Set<string>();
  let planN = 0;
  let runN = 0;
  const strip = ({ need: _need, ...s }: Item): Suggestion => ({ ...s, parts: s.parts.map((p) => ({ ...p })) });

  function makePlan(ids: string[], permanent: boolean): Plan {
    if (!ids.length) fail("empty", "the plan is empty");
    const shown: Suggestion[] = [];
    const steps: Step[] = [];
    const planItems: CleanupPlan["planItems"] = [];
    for (const id of [...new Set(ids)]) {
      const it = active().find((i) => i.id === id);
      if (!it) fail("unknown_id", "unknown item: " + id);
      if (it!.blockedReason) fail("blocked", "item cannot be selected: " + it!.blockedReason);
      if (it!.action === "none") fail("blocked", "item cannot be selected: No cleanup action is available.");
      const s = strip(it!);
      const parts = s.parts.filter((p) => (permanent ? trashFailed.has(p.id) : p.selected));
      if (s.parts.length && !parts.length) fail(permanent ? "internal" : "blocked", permanent ? "permanent deletion is only offered for items whose move to the Trash failed" : "item cannot be selected: Choose at least one part first.");
      if (permanent && !s.parts.length && !trashFailed.has(s.id)) fail("internal", "permanent deletion is only offered for items whose move to the Trash failed");
      if (s.parts.length) {
        s.parts.forEach((p) => (p.selected = parts.includes(p)));
        s.sizeBytes = parts.reduce((t, p) => t + p.sizeBytes, 0);
      }
      if (permanent) Object.assign(s, { action: "delete", actionLabel: "Delete permanently", recovery: "Not recoverable. No Trash is available for it.", recoverable: false, needsConfirmation: true, ackName: s.title });
      shown.push(s);
      planItems.push({ suggestionId: s.id, partIds: parts.map((p) => p.id) });
      if (s.parts.length) for (const p of parts) steps.push({ id: p.id, parent: s.id, title: `${s.title} (${p.label})`, action: s.action, actionLabel: s.actionLabel, size: p.sizeBytes });
      else steps.push({ id: s.id, parent: null, title: s.title, action: s.action, actionLabel: s.actionLabel, size: s.sizeBytes });
    }
    const plan: Plan = {
      id: `plan-${++planN}`,
      createdAt: new Date().toISOString(),
      expiresAt: new Date(Date.now() + 30 * 60 * 1000).toISOString(),
      itemIds: shown.map((s) => s.id),
      planItems,
      bytesFreedNow: shown.filter((s) => s.action !== "trash").reduce((t, s) => t + s.sizeBytes, 0),
      bytesHeldInTrash: shown.filter((s) => s.action === "trash").reduce((t, s) => t + s.sizeBytes, 0),
      unrecoverableIds: shown.filter((s) => !s.recoverable).map((s) => s.id),
      items: shown,
      needsAckIds: shown.filter((s) => s.needsConfirmation).map((s) => s.id),
      acknowledgedIds: [],
      unrecoverableBytes: shown.filter((s) => !s.recoverable).reduce((t, s) => t + s.sizeBytes, 0),
      freeBytesNow: bytes(freeGb),
      ready: false,
      state: "open",
      steps,
      permanent,
    };
    plan.ready = plan.needsAckIds.length === 0;
    plans.set(plan.id, plan);
    return plan;
  }
  const publicPlan = ({ steps: _s, permanent: _p, ...p }: Plan): CleanupPlan => p;

  type Run = { detail: RunDetail; plan: Plan; next: number };
  const runs = new Map<string, Run>();
  const runEntries = (r: Run) => r.plan.planItems.map((pi) => r.detail.items.filter((i) => (i.parentId ?? i.itemId) === pi.suggestionId));
  function setItem(r: Run, i: RunItem, state: RunItem["state"], reason: string | null = null) {
    i.state = state;
    i.reason = reason;
    setTimeout(() => emit("cleanup_item_updated", { runId: r.detail.id, itemId: i.itemId, state, reason }), 0);
  }
  /** Prototype `run()` for one plan item: each part or the item, in order. */
  function runEntry(r: Run, n: number) {
    const pi = r.plan.planItems[n];
    const it = items.find((i) => i.id === pi.suggestionId)!;
    const rows = runEntries(r)[n];
    for (const row of rows) {
      const step = r.plan.steps.find((s) => s.id === row.itemId)!;
      if (changedIds.has(pi.suggestionId) || changedIds.has(step.id)) {
        setItem(r, row, "skipped", "A new commit appeared after the scan.");
        it.skipReason = "A new commit appeared after the scan.";
        continue;
      }
      if (step.action === "trash" && !r.plan.permanent) {
        if (noTrash) {
          trashFailed.add(step.id);
          setItem(r, row, "failed", "Trash unavailable: this volume has no Trash. Nothing else was changed. You can plan a permanent deletion separately.");
          continue;
        }
        held.push({ item: step.id, bytes: step.size });
        setItem(r, row, "moved_to_trash");
      } else {
        freeGb += step.size / G;
        setItem(r, row, "done");
      }
      trashFailed.delete(step.id);
      if (step.parent) {
        it.parts = it.parts.filter((p) => p.id !== step.id);
        it.sizeBytes = it.parts.filter((p) => p.selected).reduce((t, p) => t + p.sizeBytes, 0);
        it.fullSizeBytes = it.parts.reduce((t, p) => t + p.sizeBytes, 0);
        if (!it.parts.length) gone.add(it.id);
      } else gone.add(it.id);
    }
  }
  function finish(r: Run, state: RunDetail["state"]) {
    const d = r.detail;
    d.state = state;
    d.finishedAt = new Date().toISOString();
    d.freeAfter = bytes(freeGb);
    d.measuredBytesFreed = d.freeAfter - d.freeBefore;
    d.trashBytes = held.filter((h) => r.plan.steps.some((s) => s.id === h.item)).reduce((t, h) => t + h.bytes, 0);
    d.skipped = d.items.filter((i) => i.state === "skipped").length;
    d.failed = d.items.filter((i) => i.state === "failed").length;
    r.plan.state = "finished";
    // The pile lasts until the next cleanup: keep the goal and areas, forget
    // hand-picked crates; an item cleaned in part stays out, its remaining
    // parts selected again.
    const partial = r.plan.planItems.filter((pi) => !gone.has(pi.suggestionId) && runEntries(r)[r.plan.planItems.indexOf(pi)].some((i) => i.state === "done" || i.state === "moved_to_trash")).map((pi) => pi.suggestionId);
    manual = null;
    queued.clear();
    excl.clear();
    for (const id of partial) {
      excl.add(id);
      const it = items.find((i) => i.id === id)!;
      it.parts.forEach((p) => (p.selected = true));
      it.sizeBytes = it.parts.reduce((t, p) => t + p.sizeBytes, 0);
    }
    changed();
    setTimeout(() => emit("cleanup_finished", { runId: d.id, measuredBytesFreed: d.measuredBytesFreed!, skippedCount: d.skipped }), 0);
  }
  const sessionRuns: RunSummary[] = [];
  const summary = (d: RunDetail): RunSummary => ({
    id: d.id,
    startedAt: d.startedAt,
    items: d.items.length,
    measuredBytesFreed: d.measuredBytesFreed,
    trashBytes: d.trashBytes,
    skipped: d.skipped,
    removed: new Set(d.items.filter((i) => i.state === "done" || i.state === "moved_to_trash").map((i) => i.parentId ?? i.itemId)).size,
    state: d.state,
  });

  // Prototype SAMPLE_LOG: Oct 28 (7 crates, 21.4 GB, 1.2 GB in the Trash) and
  // Oct 12 (11 crates, 34.9 GB, 1 skipped), with made-up run details.
  const sampleItem = (itemId: string, title: string, action: string, actionLabel: string, gbs: number, state: RunItem["state"] = "done", reason: string | null = null): RunItem => ({
    itemId,
    parentId: null,
    title,
    action,
    actionLabel,
    expectedBytes: bytes(gbs),
    state,
    reason,
  });
  const SAMPLE_RUNS: RunDetail[] = [
    {
      id: "run-2",
      planId: "plan-a",
      startedAt: "2026-10-28T10:00:00Z",
      finishedAt: "2026-10-28T10:01:00Z",
      state: "finished",
      freeBefore: bytes(30),
      freeAfter: bytes(51.4),
      expectedBytes: bytes(21.4),
      measuredBytesFreed: bytes(21.4),
      trashBytes: bytes(1.2),
      skipped: 0,
      failed: 0,
      involvesDocker: true,
      freeBytesNow: null,
      items: [
        sampleItem("s2-1", "Docker build cache", "native_command", "docker builder prune", 9.6),
        sampleItem("s2-2", "Xcode DerivedData", "delete", "Delete the folder", 5.1),
        sampleItem("s2-3", "Old Playwright browsers", "delete", "Delete old versions", 2.4),
        sampleItem("s2-4", "pip download cache", "native_command", "pip cache purge", 1.8),
        sampleItem("s2-5", "Old Homebrew downloads", "native_command", "brew cleanup", 1.5),
        sampleItem("s2-6", ".venv in data-tools", "trash", "Move to Trash", 1.2, "moved_to_trash"),
        sampleItem("s2-7", "Untagged Docker images", "native_command", "docker image prune", 1.0),
      ],
    },
    {
      id: "run-1",
      planId: "plan-b",
      startedAt: "2026-10-12T10:00:00Z",
      finishedAt: "2026-10-12T10:02:00Z",
      state: "finished",
      freeBefore: bytes(20),
      freeAfter: bytes(54.9),
      expectedBytes: bytes(34.9),
      measuredBytesFreed: bytes(34.9),
      trashBytes: 0,
      skipped: 1,
      failed: 0,
      involvesDocker: false,
      freeBytesNow: null,
      items: [
        sampleItem("s1-1", "node_modules in quiet projects", "delete", "Delete the folders", 16.2),
        sampleItem("s1-2", "Rust build output", "delete", "Delete the folder", 7.4),
        sampleItem("s1-3", "Unused pnpm packages", "native_command", "pnpm store prune", 3.1),
        sampleItem("s1-4", "Old JetBrains caches", "delete", "Delete the folders", 2.6),
        sampleItem("s1-5", "Slack, Discord and Teams caches", "delete", "Delete the caches while the apps are closed", 1.9),
        sampleItem("s1-6", "pip download cache", "native_command", "pip cache purge", 1.2),
        sampleItem("s1-7", "Old Homebrew downloads", "native_command", "brew cleanup", 0.9),
        sampleItem("s1-8", "Old Playwright browsers", "delete", "Delete old versions", 0.7),
        sampleItem("s1-9", "Xcode DerivedData", "delete", "Delete the folder", 0.4),
        sampleItem("s1-10", "Untagged Docker images", "native_command", "docker image prune", 0.3),
        sampleItem("s1-11", "Old Rust toolchains", "native_command", "rustup toolchain uninstall", 0.2),
        sampleItem("s1-12", "Worktree old-experiment", "native_command", "git worktree remove", 5.1, "skipped", "A new commit appeared after the scan."),
      ],
    },
  ];

  const handlers: Record<string, (a: Record<string, unknown>) => unknown> = {
    create_plan: (a) => publicPlan(makePlan(a.source === "pile" ? pile().itemIds : (a.itemIds as string[]), false)),
    create_permanent_plan: (a) => publicPlan(makePlan(a.itemIds as string[], true)),
    acknowledge_item: (a) => {
      const p = plans.get(a.planId as string) ?? fail("unknown_id", "unknown plan");
      const plan = p as Plan;
      if (!plan.needsAckIds.includes(a.itemId as string)) fail("unknown_id", "unknown item: " + a.itemId);
      if (!plan.acknowledgedIds.includes(a.itemId as string)) plan.acknowledgedIds.push(a.itemId as string);
      plan.ready = plan.needsAckIds.every((id) => plan.acknowledgedIds.includes(id));
      return publicPlan(plan);
    },
    run_plan: (a) => {
      const plan = (plans.get(a.planId as string) ?? fail("unknown_id", "unknown plan")) as Plan;
      if (plan.state !== "open") fail("already_used", "this plan was already used; create a new one");
      if (!plan.ready) fail("needs_acknowledgment", "acknowledge every item that needs it first");
      if ([...runs.values()].some((r) => r.detail.state === "running")) fail("busy", "another cleanup is running");
      plan.state = "running";
      const detail: RunDetail = {
        id: `run-s${++runN}`,
        planId: plan.id,
        startedAt: new Date().toISOString(),
        finishedAt: null,
        state: "running",
        freeBefore: bytes(freeGb),
        freeAfter: null,
        expectedBytes: plan.steps.filter((s) => s.action !== "trash" || plan.permanent).reduce((t, s) => t + s.size, 0),
        measuredBytesFreed: null,
        trashBytes: 0,
        skipped: 0,
        failed: 0,
        involvesDocker: plan.items.some((s) => s.group === "docker"),
        freeBytesNow: null,
        items: plan.steps.map((s) => ({ itemId: s.id, parentId: s.parent, title: s.title, action: plan.permanent ? "delete" : s.action, actionLabel: s.actionLabel, expectedBytes: s.size, state: "waiting", reason: null })),
      };
      const r: Run = { detail, plan, next: 0 };
      runs.set(detail.id, r);
      sessionRuns.unshift(summary(detail));
      // Synchronous, so a capture never sees a half-finished run; `hold`
      // stops after that many items to show the progress sheet.
      const stop = holdAt ?? plan.planItems.length;
      while (r.next < Math.min(stop, plan.planItems.length)) runEntry(r, r.next++);
      if (r.next >= plan.planItems.length) finish(r, "finished");
      return detail.id;
    },
    cancel_run: (a) => {
      const r = runs.get(a.runId as string);
      if (!r || r.detail.state !== "running") return false;
      for (const i of r.detail.items) if (i.state === "waiting") setItem(r, i, "canceled", "Canceled before it started.");
      finish(r, "canceled");
      return true;
    },
    get_run: (a) => {
      const r = runs.get(a.runId as string);
      const d = r?.detail ?? SAMPLE_RUNS.find((x) => x.id === a.runId) ?? fail("unknown_id", "unknown run");
      if (r) {
        const s = sessionRuns.findIndex((x) => x.id === r.detail.id);
        if (s >= 0) sessionRuns[s] = summary(r.detail);
      }
      return { ...(d as RunDetail), freeBytesNow: bytes(freeGb) };
    },
    get_trash: () => ({ bytes: trashBytes(), items: held.length, confirmationToken: held.length ? `trash-${held.length}-${trashBytes()}` : null }),
    empty_freeboard_trash: (a) => {
      const token = held.length ? `trash-${held.length}-${trashBytes()}` : null;
      if (!token || a.token !== token) fail("bad_token", "invalid confirmation");
      const b = trashBytes();
      const n = held.length;
      freeGb += b / G;
      held.length = 0;
      setTimeout(() => emit("status_changed", { volume: "home", status: status().status, freeBytes: bytes(freeGb) }), 0);
      return { emptied: n, failed: 0, bytes: b, freeBytesAfter: bytes(freeGb) };
    },
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
    folder_suggestions: () => offers.map((o) => ({ label: o.label, repos: o.repos })),
    // The prototype's Settings sheet: four scanned folders, two offered.
    get_folder_details: () => ({
      scan: settings.scanFolders.map((p) => folderInfo.get(p) ?? { repos: 0, bytes: null }),
      skip: settings.exclusions.map(() => ({ repos: 0, bytes: null })),
      suggestions: offers.map((o) => ({ label: o.label, repos: o.repos })),
      suggestionBytes: offers.map((o) => o.bytes),
      lastScanAt: new Date(Date.now() - 18 * 60000).toISOString(),
      removed: removedFolder ? { list: removedFolder.list, label: removedFolder.path.replace(/^\/Users\/you/, "~") } : null,
    }),
    add_folder: () => ({ ...settings }),
    remove_folder: (a) => {
      const which = a.list === "exclusions" ? "exclusions" : "scanFolders";
      const at = a.index as number;
      removedFolder = { list: which, at, path: settings[which].splice(at, 1)[0] };
      return { ...settings };
    },
    undo_remove_folder: () => {
      if (removedFolder) settings[removedFolder.list].splice(removedFolder.at, 0, removedFolder.path);
      removedFolder = null;
      return { ...settings };
    },
    add_suggested_folder: (a) => {
      const o = offers.splice(a.index as number, 1)[0];
      if (o) {
        const p = o.label.replace(/^~/, "/Users/you");
        settings.scanFolders.push(p);
        folderInfo.set(p, { repos: o.repos, bytes: o.bytes });
      }
      removedFolder = null;
      return { ...settings };
    },
    start_scan: () => "scan-1",
    list_logbook: () => ({
      runs: [...sessionRuns.map((s) => summary(runs.get(s.id)!.detail)), ...SAMPLE_RUNS.map(summary)],
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
