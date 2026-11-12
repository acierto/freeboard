import type { Lang, LanguagePref } from "../i18n";
// The typed contract between the interface and the Rust core (serde camelCase).
// Source: docs/reference/FREEBOARD_SPEC.md, "Data model and risk rules" and
// "Tauri commands". The interface sends only IDs, enums and numbers.

export type Risk = "safe" | "review" | "risky";
export type Action = "native_command" | "delete" | "trash" | "none";
export type Group =
  | "docker"
  | "projects"
  | "caches"
  | "ai_models"
  | "phones_emulators"
  | "old_tools_apps"
  | "your_files"
  | "system";

export interface SuggestionPart {
  id: string;
  label: string;
  location: string;
  sizeBytes: number;
  selected: boolean;
}

/** Keep, Ask me later, or no decision (spec `item_decisions`). */
export type ItemDecision = "keep" | "later" | null;

export interface Suggestion {
  id: string;
  detector: string;
  group: Group;
  mapArea: string;
  title: string;
  location: string;
  /** On-disk size of the selected parts. */
  sizeBytes: number;
  fullSizeBytes: number;
  lastUsedAt: string | null;
  lastUsedLabel: string;
  risk: Risk;
  action: Action;
  actionLabel: string;
  reason: string;
  recovery: string;
  recoverable: boolean;
  blockedReason: string | null;
  needsConfirmation: boolean;
  parts: SuggestionPart[];
  fingerprint: string;
  // Additions (never replace spec fields)
  sizeIsEstimate: boolean;
  /** Why the item was skipped in a cleanup (it changed since the scan). */
  skipReason: string | null;
  decision: ItemDecision;
  /** Name shown in the per-item acknowledgment ("billing_pgdata volume"). */
  ackName: string | null;
  /** Why this item is Review or Risky. Empty for Safe. */
  riskWhy: string[];
  /** Commands the user can run first to make removing it safer (only shown). */
  safer: SaferStep[];
  /** Git worktrees: what the Worktrees view shows. */
  worktree: WorktreeInfo | null;
}

/** Spec "Git worktrees": Ready to remove, Needs a look, Has your work. */
export type WtStatus = "ready" | "look" | "work";
export type WtMerged = "yes" | "squash" | "no";

export interface WorktreeInfo {
  repo: string;
  /** The main checkout, as shown ("~/proj/app"). */
  root: string;
  branch: string | null;
  /** The worktree's folder, relative to the main checkout when inside it. */
  relPath: string;
  status: WtStatus;
  /** "agent", "claude" or "codex" for branches coding agents make. */
  agent: string | null;
  locked: boolean;
  idleDays: number | null;
  dirty: number;
  dirtyFiles: string[];
  merged: WtMerged;
  ahead: number;
  pushed: boolean;
  /** Why it is "work" when not for uncommitted files. */
  hold: "merging" | "in_use" | "no_branch" | "unknown" | null;
  /** The exact commands removal runs. */
  commands: string[];
  reopen: string | null;
}

export interface WorktreeRepo {
  name: string;
  root: string;
  /** The main checkout's own size, when a deep scan measured it. */
  mainBytes: number | null;
}

export interface WorktreesView {
  repos: WorktreeRepo[];
  /** Every linked worktree, as a suggestion with `worktree` set. */
  items: Suggestion[];
  /** Worktrees found by an older version: a deep scan sorts them. */
  stale: number;
}

export interface SaferStep {
  label: string;
  command: string;
}

export type Goal = number | "all";

export interface PileState {
  goalBytes: Goal;
  /** Empty means everywhere, unless `noAreas`. */
  areas: Group[];
  /** Every area unticked: only added items fill the pile. */
  noAreas: boolean;
  mode: "auto" | "manual";
  addedIds: string[];
  takenOutIds: string[];
  /** The current result, largest first. */
  itemIds: string[];
  totalBytes: number;
}

export interface DiskMapArea {
  id: string;
  label: string;
  bytes: number;
  leaveAloneBytes: number;
  // Additions (never replace spec fields)
  /** What the leave-alone space is ("Models you used recently"). */
  leaveAloneLabel: string;
}

export interface DiskMap {
  usedBytes: number;
  explainedBytes: number;
  notExplainedBytes: number;
  /** In prototype TILES order; Not explained is `notExplainedBytes`. */
  areas: DiskMapArea[];
  // Additions (never replace spec fields)
  /** The areas add up to more than used space by over 1%. */
  incomplete: boolean;
  overBytes: number;
  /** When folders were last measured (a deep scan); null before the first. */
  measuredAt: string | null;
  /** macOS: purgeable space and local snapshots, reported separately. */
  purgeableBytes: number | null;
  snapshotCount: number | null;
  /** macOS: the last measurement had Full Disk Access and walked the system
   * places too; what stays untraced is only macOS's own. */
  systemMeasured: boolean;
}

export type BlindSpotKind = "unscanned_folder" | "needs_permission" | "unknown_files" | "tool_unavailable";
export type BlindSpotAction = "scan_folder" | "mark_mine" | "open_settings" | "scan_again";

export interface BlindSpot {
  id: string;
  kind: BlindSpotKind;
  title: string;
  text: string;
  location: string;
  /** null when the system does not let Freeboard know ("unknown"). */
  sizeBytes: number | null;
  actionLabel: string;
  // Additions (never replace spec fields)
  /** Sent back to `resolve_blind_spot`; the backend checks it. */
  action: BlindSpotAction;
  state: "open" | "working" | "done";
  doneText: string | null;
}

export interface BlindSpotResult {
  map: DiskMap;
  blindSpots: BlindSpot[];
}

export interface PlaceChecked {
  detector: string;
  label: string;
  /** Found and clean differ; unavailable never looks clean. */
  state: "found" | "clean" | "unavailable";
  bytes: number;
}

export interface AreaGrowth {
  id: string;
  label: string;
  bytes: number;
}

export type DiskStatus = "healthy" | "low" | "critical";

export interface VolumeStatus {
  id: string;
  name: string;
  mountPoint: string;
  freeBytes: number;
  totalBytes: number;
  status: DiskStatus;
  purgeableBytes: number | null;
  snapshotCount: number | null;
}

export interface Forecast {
  bytesPerDay: number;
  daysLeft: number;
}

/** This week's pace (menu bar window): free space used over the last 7
 *  days with cleanups left out, and that ÷ 7. */
export interface Pace {
  weekBytes: number;
  bytesPerDay: number;
}

/** Updates (menu bar window's version chip; docs/decisions.md "Updates only when asked"). */
export type UpdateStage = "idle" | "checking" | "latest" | "available" | "downloading" | "ready" | "checkFailed" | "installFailed";
export interface UpdateStatus {
  stage: UpdateStage;
  current: string;
  version: string | null;
  notes: string | null;
}

export interface TrendPoint {
  at: string;
  freeBytes: number;
}

export interface DetectorError {
  detector: string;
  kind: "unavailable" | "timeout" | "permission" | "malformed" | "error" | string;
  message: string;
}

export interface ScanState {
  running: string | null;
  phase: string | null;
  lastFinishedAt: string | null;
  detectorErrors: DetectorError[];
}

export type Platform = "macos" | "linux" | "windows";

export interface AppStatus {
  volumes: VolumeStatus[];
  status: DiskStatus;
  forecast: Forecast | null;
  pace: Pace | null;
  trashBytes: number;
  trend: TrendPoint[];
  scan: ScanState;
  notificationsPaused: boolean;
  platform: Platform;
  firstRun: boolean;
  /** The first preferred system language Freeboard supports. */
  systemLanguage: Lang;
}

export type ScanKind = "quick" | "deep";

// ----- cleanup and Logbook (slice 2) -----------------------------------------

/** Where a plan's items come from (spec `create_plan`). */
export type PlanSource = "pile" | "items";

/** One plan item and the parts it covers (empty for an item without parts). */
export interface PlanItem {
  suggestionId: string;
  partIds: string[];
}

export type PlanState = "open" | "running" | "finished" | "expired" | "invalidated";

export interface CleanupPlan {
  id: string;
  createdAt: string;
  expiresAt: string;
  itemIds: string[];
  planItems: PlanItem[];
  bytesFreedNow: number;
  bytesHeldInTrash: number;
  unrecoverableIds: string[];
  /** The items as confirmed; sizes and parts follow the selected parts. */
  items: Suggestion[];
  needsAckIds: string[];
  acknowledgedIds: string[];
  unrecoverableBytes: number;
  freeBytesNow: number;
  ready: boolean;
  state: PlanState;
}

export interface TrashSummary {
  bytes: number;
  items: number;
  /** Echoed back to `empty_freeboard_trash`; binds the confirmation to this set. */
  confirmationToken: string | null;
}

export interface EmptyTrashResult {
  emptied: number;
  failed: number;
  bytes: number;
  freeBytesAfter: number;
}

export type ItemState =
  | "waiting"
  | "checking"
  | "running"
  | "done"
  | "moved_to_trash"
  | "skipped"
  | "failed"
  | "canceled"
  | "interrupted"
  /** Already gone before its turn (removed outside Freeboard): nothing done, nothing freed, and it left the list. */
  | "gone";

export interface RunItem {
  /** The item, or for a split item the part, this row records. */
  itemId: string;
  /** For a part: the item it belongs to. */
  parentId: string | null;
  title: string;
  /** The action kind as recorded ("native_command", "delete", "trash"). */
  action: string;
  actionLabel: string;
  expectedBytes: number;
  state: ItemState;
  reason: string | null;
}

export interface RunDetail {
  id: string;
  planId: string;
  startedAt: string;
  finishedAt: string | null;
  state: "running" | "finished" | "canceled" | "interrupted";
  freeBefore: number;
  freeAfter: number | null;
  expectedBytes: number;
  measuredBytesFreed: number | null;
  trashBytes: number;
  skipped: number;
  failed: number;
  involvesDocker: boolean;
  items: RunItem[];
  /** Free space measured when this was read. */
  freeBytesNow: number | null;
}

export interface RunSummary {
  id: string;
  startedAt: string;
  items: number;
  measuredBytesFreed: number | null;
  trashBytes: number;
  skipped: number;
  /** Crates removed or moved to the Trash, at least in part. */
  removed: number;
  state: string;
  /** The Keep me tidy rule that ran it; null for a cleanup you confirmed. */
  rule?: TidyRuleId | null;
}

export type TidyRuleId = "package_caches" | "docker_build_cache";

/** A disk Settings can watch besides the home volume; switched by ID. */
export interface VolumeChoice {
  id: string;
  name: string;
  mountPoint: string;
  totalBytes: number;
  freeBytes: number;
  watched: boolean;
}

/** A Keep me tidy rule (spec "Keep me tidy rules"). */
export interface TidyRule {
  id: TidyRuleId;
  enabled: boolean;
  /** What it removes, as places-checked names ("pnpm store"). */
  removes: string[];
  lastRunAt: string | null;
  /** "done", "nothing" or "failed: <reason>". */
  lastResult: string | null;
  lastBytesFreed: number | null;
}

export interface LogbookPage {
  runs: RunSummary[];
  page: number;
  hasMore: boolean;
}

// ----- settings ---------------------------------------------------------------

export type QuickScanSchedule = "hourly" | "every6_hours" | "daily" | "on_request";
export type DeepScanSchedule = "on_request" | "daily";

export interface Settings {
  lowThresholdBytes: number;
  criticalThresholdBytes: number;
  goal: Goal;
  whereFrom: Group[];
  quickScan: QuickScanSchedule;
  deepScan: DeepScanSchedule;
  inactiveProjectDays: number;
  oldInstallerDays: number;
  pauseDeepScansOnBattery: boolean;
  startAtLogin: boolean;
  notifications: boolean;
  scanFolders: string[];
  exclusions: string[];
  extraVolumes: string[];
  onboardingComplete: boolean;
  /** Free space next to the menu bar or tray icon. */
  menuBarFreeSpace: boolean;
  /** Settings → Language: "system" follows the operating system. */
  language: LanguagePref;
}

/** Settings the interface may change directly. Folder lists change only
 * through backend-owned pickers (the interface never sends paths). */
export type SettingsPatch = Partial<
  Pick<
    Settings,
    | "lowThresholdBytes"
    | "criticalThresholdBytes"
    | "goal"
    | "whereFrom"
    | "quickScan"
    | "deepScan"
    | "inactiveProjectDays"
    | "oldInstallerDays"
    | "pauseDeepScansOnBattery"
    | "startAtLogin"
    | "notifications"
    | "onboardingComplete"
    | "menuBarFreeSpace"
    | "language"
  >
>;

export type FolderList = "scanFolders" | "exclusions";

/** A home folder that holds Git repositories, offered by index (never by path). */
export interface FolderSuggestion {
  label: string;
  repos: number;
  /** Size at the last deep scan; null on a first run. */
  bytes: number | null;
}

/** A developer tool Freeboard can ask, and whether it is installed. */
export interface ToolFound {
  name: string;
  found: boolean;
}

/** Settings' folder rows, matched to the settings lists by position. */
export interface FolderInfo {
  repos: number;
  /** Size at the last deep scan; null until one measured it. */
  bytes: number | null;
}

export interface FolderDetails {
  scan: FolderInfo[];
  skip: FolderInfo[];
  suggestions: FolderSuggestion[];
  suggestionBytes: (number | null)[];
  lastScanAt: string | null;
  /** The folder removed last, while Undo can put it back. */
  removed: { list: FolderList; label: string } | null;
}

// ----- events (spec "Events" table) plus `navigate` -----------------------------

export interface StatusChanged {
  volume: string;
  status: DiskStatus;
  freeBytes: number;
}
export interface ScanProgress {
  scanId: string;
  phase: string;
  detector: string | null;
  currentCount: number;
}
export interface ScanFinished {
  scanId: string;
  detectorErrors: DetectorError[];
  totals: { count: number; bytes: number; safeBytes: number; reviewBytes: number; riskyBytes: number };
}
export interface CleanupItemUpdated {
  runId: string;
  itemId: string;
  state: ItemState;
  reason: string | null;
}
export interface CleanupFinished {
  runId: string;
  measuredBytesFreed: number;
  skippedCount: number;
}
/** Where a notification click or the tray view sends the window. */
export type View = "home" | "pile" | "questions" | "map" | "settings";
export interface Navigate {
  view: View;
}

export interface EventMap {
  status_changed: StatusChanged;
  scan_progress: ScanProgress;
  scan_finished: ScanFinished;
  pile_changed: PileState;
  disk_map_changed: DiskMap;
  blind_spots_changed: BlindSpot[];
  cleanup_item_updated: CleanupItemUpdated;
  cleanup_finished: CleanupFinished;
  navigate: Navigate;
  update_changed: UpdateStatus;
}
