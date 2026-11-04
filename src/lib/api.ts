// Typed wrappers around the Tauri commands (source "Tauri commands" table).
// The interface sends only IDs, enums and numbers. It never sends a path or a
// command to execute (invariant 3).

import type {
  AppStatus,
  CleanupPlan,
  EmptyTrashResult,
  EventMap,
  FolderList,
  HistoryPage,
  Risk,
  RunDetail,
  ScanKind,
  Settings,
  SettingsPatch,
  Suggestion,
  TrashSummary,
} from "./types";

export interface Backend {
  invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  listen<K extends keyof EventMap>(event: K, cb: (payload: EventMap[K]) => void): Promise<() => void>;
}

let backend: Backend | null = null;

/** Install a backend. Production uses Tauri; visual tests install a fixture backend. */
export function setBackend(b: Backend) {
  backend = b;
}

async function tauriBackend(): Promise<Backend> {
  const core = await import("@tauri-apps/api/core");
  const ev = await import("@tauri-apps/api/event");
  return {
    invoke: (cmd, args) => core.invoke(cmd, args),
    listen: async (event, cb) => ev.listen(event, (e) => cb(e.payload as never)),
  };
}

async function b(): Promise<Backend> {
  if (!backend) backend = await tauriBackend();
  return backend;
}

const call = async <T>(cmd: string, args?: Record<string, unknown>) => (await b()).invoke<T>(cmd, args);

export const api = {
  getStatus: () => call<AppStatus>("get_status"),
  startScan: (kind: ScanKind) => call<string>("start_scan", { kind }),
  getSuggestions: (filters?: { risk?: Risk }) => call<Suggestion[]>("get_suggestions", { filters: filters ?? null }),
  createPlan: (itemIds: string[]) => call<CleanupPlan>("create_plan", { itemIds }),
  createPermanentPlan: (itemIds: string[]) => call<CleanupPlan>("create_permanent_plan", { itemIds }),
  getPlan: (planId: string) => call<CleanupPlan>("get_plan", { planId }),
  acknowledgeItem: (planId: string, itemId: string) => call<CleanupPlan>("acknowledge_item", { planId, itemId }),
  runPlan: (planId: string) => call<string>("run_plan", { planId }),
  cancelRun: (runId: string) => call<boolean>("cancel_run", { runId }),
  getRun: (runId: string) => call<RunDetail>("get_run", { runId }),
  getTrash: () => call<TrashSummary>("get_trash"),
  emptyFreeboardTrash: (token: string) => call<EmptyTrashResult>("empty_freeboard_trash", { token }),
  listHistory: (page: number) => call<HistoryPage>("list_history", { page }),
  getSettings: () => call<Settings>("get_settings"),
  updateSettings: (patch: SettingsPatch) => call<Settings>("update_settings", { patch }),
  /** Opens a native folder picker in the backend; the path never passes through the interface. */
  addFolder: (list: FolderList) => call<Settings>("add_folder", { list }),
  removeFolder: (list: FolderList, index: number) => call<Settings>("remove_folder", { list, index }),
  setNotificationsPaused: (paused: boolean) => call<boolean>("set_notifications_paused", { paused }),
};

export async function on<K extends keyof EventMap>(event: K, cb: (payload: EventMap[K]) => void) {
  return (await b()).listen(event, cb);
}
