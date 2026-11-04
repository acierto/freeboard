import type { Category, DiskStatus, ItemState, Platform, Risk, Suggestion } from "./types";

export const GB = 1e9;
/** The hull gauge shows the top 120 GB of the disk. */
export const SCALE_GB = 120;

/** Decimal gigabytes with one decimal place, as in the prototype ("12.4 GB"). */
export const gbNum = (bytes: number) => (bytes / GB).toFixed(1);
export const gb = (bytes: number) => gbNum(bytes) + " GB";
export const sizeText = (s: Pick<Suggestion, "sizeBytes" | "sizeIsEstimate">) => (s.sizeIsEstimate ? "about " : "") + gb(s.sizeBytes);

/** Threshold labels: whole numbers without decimals ("Low 20 GB"). */
export const gbLabel = (bytes: number) => {
  const v = bytes / GB;
  return Number.isInteger(v) ? String(v) : v.toFixed(1);
};

export const CATEGORIES: { id: Category; name: string }[] = [
  { id: "docker", name: "Docker" },
  { id: "worktrees", name: "Git worktrees" },
  { id: "builds", name: "Dependencies and build output" },
  { id: "caches", name: "Package and tool caches" },
  { id: "files", name: "Old files" },
];

export const RISK_LABEL: Record<Risk, string> = { safe: "Safe", review: "Review", risky: "Risky" };

export const STATUS: Record<DiskStatus, { key: "good" | "warn" | "crit"; label: string }> = {
  healthy: { key: "good", label: "Healthy" },
  low: { key: "warn", label: "Low" },
  critical: { key: "crit", label: "Critical" },
};

export const trashWord = (p: Platform) => (p === "windows" ? "Recycle Bin" : "Trash");

/** Only eligible Safe items start selected (invariants 9 and 10). */
export const preselected = (s: Suggestion) => s.risk === "safe" && !s.blockedReason && !s.needsConfirmation && s.action !== "none";
export const selectable = (s: Suggestion) => !s.blockedReason && s.action !== "none";

/** Show home-relative paths as ~/... on macOS and Linux, like the prototype. */
export function displayPath(p: string, platform: Platform): string {
  if (platform === "windows") return p;
  const m = /^(\/Users\/[^/]+|\/home\/[^/]+)(\/.*)?$/.exec(p);
  return m ? "~" + (m[2] ?? "") : p;
}

export function relativeTime(iso: string | null, now = Date.now()): string {
  if (!iso) return "never";
  const s = Math.max(0, Math.round((now - Date.parse(iso)) / 1000));
  if (s < 60) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return m === 1 ? "1 minute ago" : m + " minutes ago";
  const h = Math.round(m / 60);
  if (h < 24) return h === 1 ? "1 hour ago" : h + " hours ago";
  const d = Math.round(h / 24);
  return d === 1 ? "1 day ago" : d + " days ago";
}

const DATE = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric", year: "numeric", timeZone: "UTC" });
export const shortDate = (iso: string) => DATE.format(new Date(iso));

export function runLabel(state: ItemState, platform: Platform): string {
  switch (state) {
    case "waiting":
      return "Waiting";
    case "checking":
      return "Checking";
    case "running":
      return "Cleaning";
    case "done":
      return "Cleaned";
    case "moved_to_trash":
      return "Moved to " + trashWord(platform);
    case "skipped":
      return "Skipped";
    case "failed":
      return "Failed";
    case "canceled":
      return "Canceled";
    default:
      return "Interrupted";
  }
}

export function runClass(state: ItemState): string {
  if (state === "skipped") return "result skip";
  if (state === "done" || state === "moved_to_trash") return "result ok";
  if (state === "failed") return "result fail";
  return "result";
}

const DETECTOR_PHASE: Record<string, string> = {
  docker: "Checking Docker",
  git: "Checking Git worktrees",
  project: "Checking project folders",
  cache: "Checking caches",
  files: "Checking old files",
};
export const phaseLabel = (detector: string | null) =>
  detector ? (DETECTOR_PHASE[detector.split(".")[0]] ?? "Checking " + detector) : "Scanning";

export const plural = (n: number, word: string) => n + " " + word + (n === 1 ? "" : "s");
