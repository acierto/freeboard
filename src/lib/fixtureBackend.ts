// TEST-ONLY fixture backend used by the visual verifier (`vite build --mode visual`).
// It is never part of the production bundle (see scripts/visual/verify.mjs check).
// Data is the prototype's sample data (docs/reference/freeboard-prototype.html)
// mapped onto the backend contract. Marker: FREEBOARD_FIXTURE_BACKEND
//
// Query parameters:
//   free=<GB>            current free space (default 12.4)
//   platform=macos|linux|windows
//   forecast=none        no forecast
//   trash=<GB>           held in Freeboard Trash
//   removed=a,b          suggestion ids already cleaned up
//   skipped=a,b          suggestion ids skipped in the last cleanup
//   history=empty        no cleanup history
//   suggestions=empty    no suggestions
//   scan=never           no scan has finished yet
//   low=<GB>&critical=<GB> thresholds
//   run=hold             runs stay at "first item checking"
//   fail=<id>            that item fails during a run
//   cancelAfter=<n>      cancel after n items
//   plan=expired         plans are created expired
//   onboarding=1         first run
//   unavailable=docker   Docker detectors unavailable

import type { Backend } from "./api";
import type {
  AppStatus,
  CleanupPlan,
  DiskStatus,
  EventMap,
  HistoryPage,
  ItemState,
  Platform,
  RunDetail,
  RunSummary,
  Settings,
  Suggestion,
} from "./types";

export const FIXTURE_MARKER = "FREEBOARD_FIXTURE_BACKEND";
const G = 1e9;
const b = (gb: number) => Math.round(gb * G);

type Item = {
  id: string;
  cat: Suggestion["category"];
  name: string;
  path: string;
  size: number;
  last: string;
  risk: "Safe" | "Review" | "Risky";
  action: "native" | "delete" | "trash";
  cmd: string;
  why: string;
  recover: string;
  ack?: boolean;
  ackName?: string;
  unrecoverable?: boolean;
  blocked?: string;
  os?: string[];
};

// Copied from the prototype's ITEMS.
const ITEMS: Item[] = [
  { id: 'd-cache', cat: 'docker', name: 'Docker build cache', path: '142 cache layers', size: 14.8, last: 'Last used 23 days ago', risk: 'Safe', action: 'native', cmd: 'docker builder prune', why: 'Build layers that no recent build has reused.', recover: 'Docker rebuilds the layers on the next build.' },
  { id: 'd-dangling', cat: 'docker', name: 'Untagged Docker images', path: '31 dangling images', size: 6.2, last: 'Created over 30 days ago', risk: 'Safe', action: 'native', cmd: 'docker image prune', why: 'Old image layers with no tag and no container using them.', recover: 'Rebuild the image from its Dockerfile.' },
  { id: 'd-pg', cat: 'docker', name: 'postgres:14 image', path: 'Not used by any container', size: 3.4, last: 'Last used 71 days ago', risk: 'Review', action: 'native', cmd: 'docker image rm postgres:14', why: 'A tagged image that no container uses.', recover: 'Run docker pull postgres:14.' },
  { id: 'd-stopped', cat: 'docker', name: 'Stopped containers', path: '6 containers', size: 1.1, last: 'Stopped over 14 days ago', risk: 'Review', action: 'native', cmd: 'docker container rm <ids>', why: 'Their writable layers still take up space.', recover: 'Create them again. Files written inside them are lost.' },
  { id: 'd-vol', cat: 'docker', name: 'Volume billing_pgdata', path: 'Not attached to a container', size: 7.6, last: 'Last attached 40 days ago', risk: 'Risky', action: 'native', cmd: 'docker volume rm billing_pgdata', why: 'Not attached to a container, but it may hold the only copy of this data.', recover: 'Not recoverable.', ack: true, ackName: 'billing_pgdata volume', unrecoverable: true },
  { id: 'wt-auth', cat: 'worktrees', name: 'api-server: fix-auth', path: '/Users/you/work/api-server/.worktrees/fix-auth', size: 4.3, last: 'Merged 34 days ago', risk: 'Review', action: 'native', cmd: 'git worktree remove .worktrees/fix-auth', why: 'The branch is merged and the worktree has no local changes.', recover: 'Run git worktree add with the same branch.' },
  { id: 'wt-exp', cat: 'worktrees', name: 'web-app: old-experiment', path: '/Users/you/work/web-app/.worktrees/old-experiment', size: 5.1, last: 'Last commit 52 days ago', risk: 'Review', action: 'native', cmd: 'git worktree remove .worktrees/old-experiment', why: 'No local changes, and the branch is pushed but not merged.', recover: 'Run git worktree add with the pushed branch.' },
  { id: 'wt-pay', cat: 'worktrees', name: 'web-app: payments-v2', path: '/Users/you/work/web-app/.worktrees/payments-v2', size: 6.9, last: 'Edited 9 days ago', risk: 'Risky', action: 'native', cmd: 'None', why: '3 uncommitted files and 2 unpushed commits.', recover: 'Your work exists only here.', blocked: 'Blocked in v1. Commit and push this work first.' },
  { id: 'b-node', cat: 'builds', name: 'node_modules in 14 inactive projects', path: '/Users/you/projects/*/node_modules', size: 18.7, last: 'Projects idle for 60+ days', risk: 'Safe', action: 'delete', cmd: 'Delete the 14 folders', why: 'Each project has a lockfile, so these can be installed again.', recover: 'Run pnpm install in the project.' },
  { id: 'b-target', cat: 'builds', name: 'Rust target folder: rust-cli', path: '/Users/you/projects/rust-cli/target', size: 9.2, last: 'Last built 45 days ago', risk: 'Safe', action: 'delete', cmd: 'Delete the folder', why: 'Build output that Cargo can regenerate.', recover: 'Run cargo build.' },
  { id: 'b-venv', cat: 'builds', name: '.venv in data-tools', path: '/Users/you/projects/data-tools/.venv', size: 2.4, last: 'Last used 88 days ago', risk: 'Review', action: 'trash', cmd: 'Move to Trash', why: 'Usually regenerable, but it may hold packages you installed by hand.', recover: 'Restore it from the Trash, or recreate it from requirements.txt.' },
  { id: 'c-derived', cat: 'caches', name: 'Xcode DerivedData', path: '/Users/you/Library/Developer/Xcode/DerivedData', size: 11.3, last: 'Mostly older than 30 days', risk: 'Safe', action: 'delete', cmd: 'Delete the folder', why: 'Xcode regenerates this on the next build.', recover: 'Build the project again.', os: ['macos'] },
  { id: 'c-sim', cat: 'caches', name: 'Unavailable iOS simulators', path: '9 simulator devices', size: 8.4, last: 'Runtime no longer installed', risk: 'Review', action: 'native', cmd: 'xcrun simctl delete unavailable', why: 'These simulators depend on runtimes that are gone.', recover: 'Create the simulator again in Xcode.', os: ['macos'] },
  { id: 'c-pnpm', cat: 'caches', name: 'Unreferenced pnpm packages', path: 'pnpm content store', size: 5.6, last: 'Not linked to any project', risk: 'Safe', action: 'native', cmd: 'pnpm store prune', why: 'Packages in the store that no project links to.', recover: 'pnpm downloads them when needed.' },
  { id: 'c-pip', cat: 'caches', name: 'pip download cache', path: 'pip cache', size: 2.6, last: 'Mostly older than 30 days', risk: 'Safe', action: 'native', cmd: 'pip cache purge', why: 'Downloaded wheels that pip can fetch again.', recover: 'pip downloads them when needed.' },
  { id: 'c-brew', cat: 'caches', name: 'Old Homebrew downloads', path: 'Homebrew cache', size: 2.1, last: 'Older than 30 days', risk: 'Safe', action: 'native', cmd: 'brew cleanup', why: 'Old versions and downloads of installed packages.', recover: 'Homebrew downloads them when needed.', os: ['macos', 'linux'] },
  { id: 'f-temp', cat: 'files', name: 'Old temporary files', path: '%TEMP%', size: 3.2, last: 'Older than 14 days', risk: 'Review', action: 'trash', cmd: 'Move to Recycle Bin', why: 'Temporary files that no running app has open.', recover: 'Restore them from the Recycle Bin.', os: ['windows'] },
  { id: 'f-dl', cat: 'files', name: 'Installers in Downloads', path: '/Users/you/Downloads', size: 4.6, last: 'Not opened in 90+ days', risk: 'Review', action: 'trash', cmd: 'Move to Trash', why: '12 installers and disk images that you have not opened recently.', recover: 'Restore them from the Trash.' },
];

const TREND = [41, 38.5, 37, 33, 31.2, 30, 26.4, 24, 23.1, 20.5, 18, 16.6, 14.1];
const RATE = 2.2;
const TOTAL = 512;
const SKIP_NOTE = "Changed during the last cleanup. Scan again before removing it.";

export const HISTORY_RUNS: RunDetail[] = [
  {
    id: "run-2", planId: "plan-2", startedAt: "2026-10-28T09:12:00Z", finishedAt: "2026-10-28T09:13:10Z", state: "finished",
    freeBefore: b(10.2), freeAfter: b(31.6), expectedBytes: b(21.4), measuredBytesFreed: b(21.4), trashBytes: b(1.2), skipped: 0, failed: 0, involvesDocker: true,
    items: [
      { itemId: "h-cache", title: "Docker build cache", actionLabel: "docker builder prune", expectedBytes: b(12.1), state: "done", reason: null },
      { itemId: "h-node", title: "node_modules in 9 inactive projects", actionLabel: "Delete the 9 folders", expectedBytes: b(8.1), state: "done", reason: null },
      { itemId: "h-venv", title: ".venv in scratch", actionLabel: "Move to Trash", expectedBytes: b(1.2), state: "moved_to_trash", reason: null },
    ],
  },
  {
    id: "run-1", planId: "plan-1", startedAt: "2026-10-12T16:40:00Z", finishedAt: "2026-10-12T16:42:00Z", state: "finished",
    freeBefore: b(8.0), freeAfter: b(42.9), expectedBytes: b(36.1), measuredBytesFreed: b(34.9), trashBytes: b(3.0), skipped: 1, failed: 0, involvesDocker: false,
    items: [],
  },
];
const HISTORY_ITEMS: Record<string, number> = { "run-2": 7, "run-1": 11 };

function vis(it: Item, p: Platform) {
  return !it.os || it.os.includes(p);
}

function textFor(s: string, p: Platform) {
  return p === "windows" ? s.replace("Move to Trash", "Move to Recycle Bin").replace(/the Trash/g, "the Recycle Bin") : s;
}

function pathFor(path: string, p: Platform) {
  if (p === "windows" && path.startsWith("/Users/you/")) return "C:\\Users\\you\\" + path.slice(11).replace(/\//g, "\\");
  return path;
}

function toSuggestion(it: Item, p: Platform, skipped: Set<string>): Suggestion {
  return {
    id: it.id,
    detector: it.cat + "." + it.id,
    category: it.cat,
    title: it.name,
    location: pathFor(it.path, p),
    sizeBytes: b(it.size),
    lastUsedAt: null,
    lastUsedLabel: it.last,
    risk: it.risk.toLowerCase() as Suggestion["risk"],
    action: it.blocked ? "none" : it.action === "native" ? "native_command" : it.action,
    actionLabel: textFor(it.cmd, p),
    reason: it.why,
    recovery: textFor(it.recover, p),
    recoverable: !it.unrecoverable,
    blockedReason: it.blocked ?? null,
    needsConfirmation: !!it.ack,
    fingerprint: "fx-" + it.id,
    sizeIsEstimate: false,
    skipReason: skipped.has(it.id) ? SKIP_NOTE : null,
    ackName: it.ackName ?? null,
  };
}

export function createFixtureBackend(params: URLSearchParams): Backend {
  const p = (params.get("platform") ?? "macos") as Platform;
  const num = (k: string, d: number) => (params.has(k) ? parseFloat(params.get(k)!) : d);
  const list = (k: string) => new Set((params.get(k) ?? "").split(",").filter(Boolean));
  let free = b(num("free", 12.4));
  let trash = b(num("trash", 0));
  const removed = list("removed");
  const skipped = list("skipped");
  const listeners = new Map<string, Set<(x: unknown) => void>>();
  const emit = (ev: string, payload: unknown) => listeners.get(ev)?.forEach((cb) => cb(payload));
  let settings: Settings = {
    lowThresholdBytes: b(num("low", 20)),
    criticalThresholdBytes: b(num("critical", 8)),
    quickScan: "hourly",
    deepScan: "on_request",
    inactiveProjectDays: 30,
    oldInstallerDays: 90,
    pauseDeepScansOnBattery: true,
    startAtLogin: true,
    notifications: true,
    scanFolders:
      p === "windows"
        ? ["C:\\Users\\you\\projects", "C:\\Users\\you\\work", "C:\\Users\\you\\Downloads"]
        : ["/Users/you/projects", "/Users/you/work", "/Users/you/Downloads"],
    exclusions: [],
    extraVolumes: [],
    onboardingComplete: params.get("onboarding") !== "1",
  };
  if (params.get("onboarding") === "1") settings.scanFolders = [];
  const history: RunDetail[] = params.get("history") === "empty" ? [] : HISTORY_RUNS.map((r) => ({ ...r }));
  const plans = new Map<string, CleanupPlan>();
  const runs = new Map<string, RunDetail>();
  let planSeq = 0;
  const now = () => new Date(Date.now()).toISOString();

  const suggestions = () =>
    params.get("suggestions") === "empty"
      ? []
      : ITEMS.filter((i) => vis(i, p) && !removed.has(i.id) && !(params.get("unavailable") === "docker" && i.cat === "docker")).map((i) =>
          toSuggestion(i, p, skipped),
        );

  const statusOf = (): DiskStatus =>
    free < settings.criticalThresholdBytes ? "critical" : free < settings.lowThresholdBytes ? "low" : "healthy";

  const status = (): AppStatus => ({
    volumes: [{ id: "home", name: "Macintosh HD", mountPoint: "/", freeBytes: free, totalBytes: b(TOTAL), status: statusOf(), purgeableBytes: null, snapshotCount: null }],
    status: statusOf(),
    forecast: params.get("forecast") === "none" ? null : { bytesPerDay: b(RATE), daysLeft: free / G / RATE },
    trashBytes: trash,
    trend: TREND.concat([free / G]).map((v, i) => ({ at: new Date(Date.UTC(2026, 9, 22 + i)).toISOString(), freeBytes: b(v) })),
    scan: {
      running: null,
      phase: null,
      lastFinishedAt: params.get("scan") === "never" ? null : new Date(Date.now() - 18 * 60000).toISOString(),
      detectorErrors:
        params.get("unavailable") === "docker"
          ? // Same shape as the real backend: one error per Docker detector.
            ["build_cache", "dangling_images", "unused_images", "stopped_containers", "volumes"].map((d) => ({
              detector: "docker." + d,
              kind: "unavailable",
              message: "Docker is installed but not running.",
            }))
          : [],
    },
    notificationsPaused: false,
    platform: p,
    firstRun: params.get("onboarding") === "1",
  });

  const buildPlan = (ids: string[], acks: string[] = []): CleanupPlan => {
    const all = suggestions();
    const items = ids.map((id) => all.find((s) => s.id === id)).filter((s): s is Suggestion => !!s && !s.blockedReason);
    const sum = (l: Suggestion[]) => l.reduce((t, s) => t + s.sizeBytes, 0);
    const lost = items.filter((s) => !s.recoverable);
    const needAck = items.filter((s) => s.needsConfirmation).map((s) => s.id);
    const expired = params.get("plan") === "expired";
    const created = Date.now() - (expired ? 31 * 60000 : 0);
    return {
      id: "plan-" + ++planSeq,
      createdAt: new Date(created).toISOString(),
      expiresAt: new Date(created + 30 * 60000).toISOString(),
      itemIds: items.map((s) => s.id),
      bytesFreedNow: sum(items.filter((s) => s.action !== "trash")),
      bytesHeldInTrash: sum(items.filter((s) => s.action === "trash")),
      unrecoverableIds: lost.map((s) => s.id),
      items,
      needsAckIds: needAck,
      acknowledgedIds: acks,
      unrecoverableBytes: sum(lost),
      freeBytesNow: free,
      ready: !expired && needAck.every((id) => acks.includes(id)),
      state: expired ? "expired" : "open",
    };
  };

  async function runAll(run: RunDetail, plan: CleanupPlan) {
    const fail = params.get("fail");
    const cancelAfter = params.has("cancelAfter") ? parseInt(params.get("cancelAfter")!, 10) : -1;
    const set = (i: number, state: ItemState, reason: string | null) => {
      run.items[i] = { ...run.items[i], state, reason };
      emit("cleanup_item_updated", { runId: run.id, itemId: run.items[i].itemId, state, reason });
    };
    if (params.get("run") === "hold") {
      set(0, "checking", null);
      return;
    }
    let freed = 0;
    for (let i = 0; i < run.items.length; i++) {
      await Promise.resolve();
      if (cancelAfter >= 0 && i >= cancelAfter) {
        for (let j = i; j < run.items.length; j++) set(j, "canceled", null);
        run.state = "canceled";
        break;
      }
      const s = plan.items[i];
      if (s.id === "wt-exp") {
        set(i, "skipped", "A new commit appeared after the scan.");
        run.skipped++;
        skipped.add(s.id);
      } else if (s.id === fail) {
        set(i, "failed", "The command exited with an error. Nothing was removed.");
        run.failed++;
      } else if (s.action === "trash") {
        set(i, "moved_to_trash", null);
        run.trashBytes += s.sizeBytes;
        trash += s.sizeBytes;
        removed.add(s.id);
      } else {
        set(i, "done", null);
        freed += s.sizeBytes;
        removed.add(s.id);
      }
    }
    free += freed;
    if (run.state === "running") run.state = "finished";
    run.finishedAt = now();
    run.freeAfter = free;
    run.measuredBytesFreed = freed;
    history.unshift(run);
    emit("cleanup_finished", { runId: run.id, measuredBytesFreed: freed, skippedCount: run.skipped });
  }

  const handlers: Record<string, (a: Record<string, unknown>) => unknown> = {
    get_status: () => status(),
    start_scan: () => "scan-1",
    get_suggestions: () => suggestions(),
    create_plan: (a) => {
      const plan = buildPlan(a.itemIds as string[]);
      plans.set(plan.id, plan);
      return plan;
    },
    get_plan: (a) => plans.get(a.planId as string),
    acknowledge_item: (a) => {
      const old = plans.get(a.planId as string)!;
      const acks = Array.from(new Set([...old.acknowledgedIds, a.itemId as string]));
      const plan = { ...buildPlan(old.itemIds, acks), id: old.id };
      plans.set(plan.id, plan);
      return plan;
    },
    run_plan: (a) => {
      const plan = plans.get(a.planId as string)!;
      const run: RunDetail = {
        id: "run-" + (history.length + 10),
        planId: plan.id,
        startedAt: now(),
        finishedAt: null,
        state: "running",
        freeBefore: free,
        freeAfter: null,
        expectedBytes: plan.bytesFreedNow,
        measuredBytesFreed: null,
        trashBytes: 0,
        skipped: 0,
        failed: 0,
        involvesDocker: plan.items.some((s) => s.category === "docker"),
        items: plan.items.map((s) => ({ itemId: s.id, title: s.title, actionLabel: s.actionLabel, expectedBytes: s.sizeBytes, state: "waiting" as ItemState, reason: null })),
      };
      runs.set(run.id, run);
      void runAll(run, plan);
      return run.id;
    },
    cancel_run: () => true,
    get_run: (a) => runs.get(a.runId as string) ?? history.find((r) => r.id === a.runId),
    get_trash: () => ({ bytes: trash, items: trash ? 1 : 0, confirmationToken: trash ? "fixture-token" : null }),
    empty_freeboard_trash: () => {
      const bytes = trash;
      free += trash;
      trash = 0;
      return { emptied: 1, failed: 0, bytes, freeBytesAfter: free };
    },
    list_history: (a): HistoryPage => {
      const runsOut: RunSummary[] = history.map((r) => ({
        id: r.id,
        startedAt: r.startedAt,
        items: HISTORY_ITEMS[r.id] ?? r.items.length,
        measuredBytesFreed: r.measuredBytesFreed,
        trashBytes: r.trashBytes,
        skipped: r.skipped,
        state: r.state,
      }));
      return { runs: runsOut, page: (a.page as number) ?? 0, hasMore: false };
    },
    get_settings: () => settings,
    update_settings: (a) => {
      settings = { ...settings, ...(a.patch as Partial<Settings>) };
      return settings;
    },
    add_folder: (a) => {
      const k = a.list as "scanFolders" | "exclusions";
      settings = { ...settings, [k]: [...settings[k], p === "windows" ? "C:\\Users\\you\\code" : "/Users/you/code"] };
      return settings;
    },
    remove_folder: (a) => {
      const k = a.list as "scanFolders" | "exclusions";
      settings = { ...settings, [k]: settings[k].filter((_, i) => i !== a.index) };
      return settings;
    },
    set_notifications_paused: (a) => a.paused,
  };

  return {
    async invoke<T>(cmd: string, args?: Record<string, unknown>) {
      const h = handlers[cmd];
      if (!h) throw new Error("unknown command " + cmd);
      return structuredClone(h(args ?? {})) as T;
    },
    async listen<K extends keyof EventMap>(event: K, cb: (payload: EventMap[K]) => void) {
      const set = listeners.get(event) ?? new Set();
      set.add(cb as (x: unknown) => void);
      listeners.set(event, set);
      return () => set.delete(cb as (x: unknown) => void);
    },
  };
}
