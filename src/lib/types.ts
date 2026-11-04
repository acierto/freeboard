// Mirrors src-tauri/src/model.rs and settings.rs (serde camelCase).
// Source contract: docs/reference/FREEBOARD_SPEC.md, "Data model and risk rules".

export type Risk = "safe" | "review" | "risky";
export type Action = "native_command" | "delete" | "trash" | "none";
export type Category = "docker" | "worktrees" | "builds" | "caches" | "files";

export interface Suggestion {
  id: string;
  detector: string;
  category: Category;
  title: string;
  location: string;
  sizeBytes: number;
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
  fingerprint: string;
  // Additions (never replace source fields)
  sizeIsEstimate: boolean;
  skipReason: string | null;
  ackName: string | null;
}

export type PlanState = "open" | "running" | "finished" | "expired" | "invalidated";

export interface CleanupPlan {
  id: string;
  createdAt: string;
  expiresAt: string;
  itemIds: string[];
  bytesFreedNow: number;
  bytesHeldInTrash: number;
  unrecoverableIds: string[];
  // Additions
  items: Suggestion[];
  needsAckIds: string[];
  acknowledgedIds: string[];
  unrecoverableBytes: number;
  freeBytesNow: number;
  ready: boolean;
  state: PlanState;
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

export interface HistoryPage {
  runs: RunSummary[];
  page: number;
  hasMore: boolean;
}

export interface TrashSummary {
  bytes: number;
  items: number;
  confirmationToken: string | null;
}

export interface EmptyTrashResult {
  emptied: number;
  failed: number;
  bytes: number;
  freeBytesAfter: number;
}

export type QuickScanSchedule = "hourly" | "every6_hours" | "daily" | "on_request";
export type DeepScanSchedule = "on_request" | "daily";

export interface Settings {
  lowThresholdBytes: number;
  criticalThresholdBytes: number;
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

/** Settings the interface may change directly. Folder lists are changed only
 * through backend-owned pickers (the interface never sends paths). */
export type SettingsPatch = Partial<
  Pick<
    Settings,
    | "lowThresholdBytes"
    | "criticalThresholdBytes"
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

// Events (source "Events" table) plus `navigate`.
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
export type View = "overview" | "suggestions" | "plan" | "history" | "settings";
export interface Navigate {
  view: View;
}

export interface EventMap {
  status_changed: StatusChanged;
  scan_progress: ScanProgress;
  scan_finished: ScanFinished;
  cleanup_item_updated: CleanupItemUpdated;
  cleanup_finished: CleanupFinished;
  navigate: Navigate;
}
