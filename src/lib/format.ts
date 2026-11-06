import type { DiskStatus, Group, Platform, Risk, Suggestion } from "./types";

export const GB = 1e9;

/** Decimal gigabytes with one decimal place, as in the prototype ("12.4 GB"). */
export const gbNum = (bytes: number) => (bytes / GB).toFixed(1);
/** Sizes too small to show at one decimal read "<0.1 GB", never a misleading
 * "0.0 GB" (found on the real app, 2026-11-06). */
export const gb = (bytes: number) => (bytes > 0 && bytes < 0.05 * GB ? "<0.1 GB" : gbNum(bytes) + " GB");
export const sizeText = (s: Pick<Suggestion, "sizeBytes" | "sizeIsEstimate">) => (s.sizeIsEstimate ? "about " : "") + gb(s.sizeBytes);

/** Groups in display order, with the prototype's labels. */
export const GROUPS: { id: Group; label: string }[] = [
  { id: "docker", label: "Docker" },
  { id: "projects", label: "Projects" },
  { id: "caches", label: "Caches" },
  { id: "ai_models", label: "AI models" },
  { id: "phones_emulators", label: "Phones and emulators" },
  { id: "old_tools_apps", label: "Old tools and apps" },
  { id: "your_files", label: "Your files" },
  { id: "system", label: "System" },
];
export const groupLabel = (g: Group) => GROUPS.find((x) => x.id === g)?.label ?? g;

export const RISK_LABEL: Record<Risk, string> = { safe: "Safe", review: "Review", risky: "Risky" };

/** Status pill classes and labels (prototype `status()`). */
export const STATUS: Record<DiskStatus, { key: "good" | "warn" | "crit"; label: string }> = {
  healthy: { key: "good", label: "Healthy" },
  low: { key: "warn", label: "Low" },
  critical: { key: "crit", label: "Critical" },
};

export const trashWord = (p: Platform) => (p === "windows" ? "Recycle Bin" : "Trash");

/** What the tray view calls the place it lives in. */
export const trayWord = (p: Platform) => (p === "macos" ? "menu bar" : p === "windows" ? "notification area" : "tray");

export const plural = (n: number, word: string) => n + " " + word + (n === 1 ? "" : "s");
