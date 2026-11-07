import { areaName, fx, t, unitGB } from "../i18n/index.ts";
import type { DiskStatus, Group, Platform, Risk, Suggestion } from "./types";

export const GB = 1e9;

/** Decimal gigabytes with one decimal place, as in the prototype ("12.4 GB");
 * a decimal comma outside English ("12,4 GB", "12,4 Go"). */
export const gbNum = (bytes: number) => fx(bytes / GB);
/** Sizes too small to show at one decimal read "<0.1 GB", never a misleading
 * "0.0 GB" (found on the real app, 2026-11-06). */
export const gb = (bytes: number) => (bytes > 0 && bytes < 0.05 * GB ? "<" + fx(0.1) : gbNum(bytes)) + " " + unitGB();
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
export const groupLabel = (g: Group) => areaName(GROUPS.find((x) => x.id === g)?.label ?? g);

export const RISK_LABEL: Record<Risk, string> = {
  get safe() { return t("risk.safe"); },
  get review() { return t("risk.review"); },
  get risky() { return t("risk.risky"); },
};

/** Status pill classes and labels (prototype `status()`). */
export const STATUS: Record<DiskStatus, { key: "good" | "warn" | "crit"; readonly label: string }> = {
  healthy: { key: "good", get label() { return t("st.good"); } },
  low: { key: "warn", get label() { return t("st.warn"); } },
  critical: { key: "crit", get label() { return t("st.crit"); } },
};

export const trashWord = (p: Platform) => (p === "windows" ? t("trash.win") : t("trash.word"));

/** What the tray view calls the place it lives in. */
export const trayWord = (p: Platform) => (p === "macos" ? "menu bar" : p === "windows" ? "notification area" : "tray");

export const plural = (n: number, word: string) => n + " " + word + (n === 1 ? "" : "s");
