// Typed wrappers around the Tauri commands (spec "Tauri commands" table).
// The interface sends only IDs, enums and numbers. It never sends a path or a
// command to execute (invariant 3).

import type {
  AppStatus,
  AreaGrowth,
  BlindSpot,
  BlindSpotAction,
  BlindSpotResult,
  DiskMap,
  EventMap,
  FolderList,
  FolderSuggestion,
  Goal,
  Group,
  ItemDecision,
  LogbookPage,
  PileState,
  PlaceChecked,
  ScanKind,
  Settings,
  SettingsPatch,
  Suggestion,
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
  getSuggestions: () => call<Suggestion[]>("get_suggestions", { filters: null }),
  getPile: () => call<PileState>("get_pile"),
  setPileGoal: (goalBytes: Goal) => call<PileState>("set_pile_goal", { goalBytes }),
  setPileAreas: (areas: Group[]) => call<PileState>("set_pile_areas", { areas }),
  addToPile: (itemIds: string[]) => call<PileState>("add_to_pile", { itemIds }),
  takeOutOfPile: (itemIds: string[]) => call<PileState>("take_out_of_pile", { itemIds }),
  setItemDecision: (itemId: string, decision: ItemDecision) => call<Suggestion>("set_item_decision", { itemId, decision }),
  setItemParts: (itemId: string, partIds: string[]) => call<Suggestion>("set_item_parts", { itemId, partIds }),
  // Disk map (spec "Disk accounting", "Blind spots", "Places checked and growth").
  getDiskMap: () => call<DiskMap>("get_disk_map", { volume: "home" }),
  getBlindSpots: () => call<BlindSpot[]>("get_blind_spots", { volume: "home" }),
  /** By ID and the offered action only; any folder path stays in the backend. */
  resolveBlindSpot: (id: string, action: BlindSpotAction) => call<BlindSpotResult>("resolve_blind_spot", { id, action }),
  getPlacesChecked: () => call<PlaceChecked[]>("get_places_checked"),
  getGrowth: (days: number) => call<AreaGrowth[]>("get_growth", { days }),
  /** Sizes, folder names and risk levels; never file contents. */
  createDiskReport: () => call<unknown>("create_disk_report"),
  listLogbook: (page: number) => call<LogbookPage>("list_logbook", { page }),
  /** Opens a native folder picker in the backend; the path never passes through the interface. */
  addFolder: (list: FolderList) => call<Settings>("add_folder", { list }),
  removeFolder: (list: FolderList, index: number) => call<Settings>("remove_folder", { list, index }),
  /** Home folders that hold Git repositories (labels only). */
  folderSuggestions: () => call<FolderSuggestion[]>("folder_suggestions"),
  /** Adds an offered folder by its index; the path never passes through the interface. */
  addSuggestedFolder: (index: number) => call<Settings>("add_suggested_folder", { index }),
  getSettings: () => call<Settings>("get_settings"),
  updateSettings: (patch: SettingsPatch) => call<Settings>("update_settings", { patch }),
  setNotificationsPaused: (paused: boolean) => call<boolean>("set_notifications_paused", { paused }),
  /** Tray view: close it, or open the main window on the pile. */
  closeTrayView: () => call<void>("close_tray_view"),
  reviewPile: () => call<void>("review_pile"),
};

/** Readable text for a failed command: the core sends `{ code, message }`. */
export function errorText(e: unknown): string {
  if (e && typeof e === "object" && "message" in e && typeof (e as { message: unknown }).message === "string") return (e as { message: string }).message;
  return String(e);
}

export async function on<K extends keyof EventMap>(event: K, cb: (payload: EventMap[K]) => void) {
  return (await b()).listen(event, cb);
}
