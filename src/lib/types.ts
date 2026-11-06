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
}

export type Goal = number | "all";

export interface PileState {
  goalBytes: Goal;
  /** Empty means everywhere. */
  areas: Group[];
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
  trashBytes: number;
  trend: TrendPoint[];
  scan: ScanState;
  notificationsPaused: boolean;
  platform: Platform;
  firstRun: boolean;
}

export type ScanKind = "quick" | "deep";

// ----- cleanup and Logbook (adapted in milestone 5) -------------------------

export type ItemState =
  | "waiting"
  | "checking"
  | "running"
  | "done"
  | "moved_to_trash"
  | "skipped"
  | "failed"
  | "canceled"
  | "interrupted";

export interface RunItem {
  itemId: string;
  title: string;
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
}

export interface RunSummary {
  id: string;
  startedAt: string;
  items: number;
  measuredBytesFreed: number | null;
  trashBytes: number;
  skipped: number;
  state: string;
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
  >
>;

export type FolderList = "scanFolders" | "exclusions";

/** A home folder that holds Git repositories, offered by index (never by path). */
export interface FolderSuggestion {
  label: string;
  repos: number;
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
export type View = "home" | "pile";
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
}
