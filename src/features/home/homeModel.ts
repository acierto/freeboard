// What the home view says, as pure functions over backend data. Ported from
// the prototype's `renderHero()`; the pile itself is computed by the backend
// (design D3), this only describes it.
import { GB, GROUPS, gb } from "../../lib/format.ts";
import type { DiskStatus, Forecast, Group, PileState, Suggestion } from "../../lib/types.ts";
import type { Mood } from "../../components/dinghyGeometry.ts";

export interface Thresholds {
  lowThresholdBytes: number;
  criticalThresholdBytes: number;
}

/** Status from thresholds alone, without hysteresis (fixtures and tests). */
export function rawStatus(freeBytes: number, t: Thresholds): DiskStatus {
  if (freeBytes < t.criticalThresholdBytes) return "critical";
  if (freeBytes < t.lowThresholdBytes) return "low";
  return "healthy";
}

/** Dinghy's mood for a status: happy, worried or panicked. */
export function moodOf(status: DiskStatus): Mood {
  return status === "critical" ? "crit" : status === "low" ? "warn" : "good";
}

/** The pile's suggestions, in the backend's order (the crate order). */
export function pileItems(pile: PileState, all: Suggestion[]): Suggestion[] {
  const byId = new Map(all.map((s) => [s.id, s]));
  return pile.itemIds.map((id) => byId.get(id)).filter((s): s is Suggestion => !!s);
}

export function bubbleText(mood: Mood, freeBytes: number, pileBytes: number): string {
  if (mood === "crit") return "Help! I am nearly under. Only " + gb(freeBytes) + " left.";
  if (mood === "warn") return "I am riding low. Toss " + gb(pileBytes) + " and I will float again.";
  return pileBytes > 0 ? "Floating nicely! " + gb(freeBytes) + " free." : "All tidy! Nothing to toss right now.";
}

export function forecastText(f: Forecast | null): string {
  if (!f) return "";
  return "At about " + gb(f.bytesPerDay) + " a day, this disk fills in about " + Math.max(1, Math.round(f.daysLeft)) + " days.";
}

/** The small line in the Toss button. */
export const tossNote = (items: Suggestion[]) => (items.every((s) => s.risk === "safe") ? "All rebuildable" : "Includes items you approved");

const kept = (s: Suggestion) => s.decision === "keep";

/** Safe items the pile may fill from: not taken out, not kept, in the chosen areas. */
export function safeAvailableBytes(pile: PileState, all: Suggestion[]): number {
  const out = new Set(pile.takenOutIds);
  return all
    .filter((s) => s.risk === "safe" && !s.blockedReason && !s.skipReason && !s.needsConfirmation && !out.has(s.id) && !kept(s) && s.sizeBytes > 0)
    .filter((s) => !pile.noAreas && (pile.areas.length === 0 || pile.areas.includes(s.group)))
    .reduce((t, s) => t + s.sizeBytes, 0);
}

/** The note under the Toss button (prototype `renderHero()`). The meter
 * above it says how close the pile is to the goal. */
export function pileNote(pile: PileState, all: Suggestion[]): string {
  const items = pileItems(pile, all);
  const anyArea = pile.areas.length > 0;
  if (!items.length) {
    if (pile.noAreas) return "No areas picked. Tick at least one, or use all areas.";
    if (pile.mode === "manual") return "The pile is empty. Pick a size to fill it again.";
    return anyArea ? "This area has no Safe crates. Tick the items you want below." : "Answer a quick question below, or check the disk map.";
  }
  if (pile.mode === "manual") return "You chose these by hand. Pick a size again to refill the pile.";
  if (items.every((s) => s.risk === "safe")) return "";
  return items.some((s) => s.risk === "risky") ? "Includes something that cannot be recovered." : "Includes crates you approved.";
}

/** Safe space automatic filling may use in one area. */
export function safeIn(g: Group, all: Suggestion[]): number {
  return all
    .filter((s) => s.group === g && s.risk === "safe" && !s.blockedReason && !s.skipReason && !s.needsConfirmation && !kept(s) && s.sizeBytes > 0)
    .reduce((t, s) => t + s.sizeBytes, 0);
}

/** The From checkboxes: areas with Safe crates, in display order. */
export function areaChoices(all: Suggestion[]): Group[] {
  return GROUPS.map((g) => g.id).filter((g) => safeIn(g, all) > 0);
}

/** Areas whose open items all need the user's OK first ("AI models and 4
 * more areas need your OK first."). Kept and blocked items need none. */
export function reviewOnlyAreas(all: Suggestion[]): Group[] {
  const safe = areaChoices(all);
  return GROUPS.map((g) => g.id).filter((g) => !safe.includes(g) && all.some((s) => s.group === g && !s.blockedReason && !kept(s) && s.sizeBytes > 0));
}

/** Is an area ticked? With no choice made, every area is. */
export const areaTicked = (pile: PileState, g: Group) => !pile.noAreas && (pile.areas.length === 0 || pile.areas.includes(g));

/** The areas after clicking one (prototype click handler): ticking every
 * area again means all areas; unticking the last one means none. */
export function toggledAreas(pile: PileState, g: Group, choices: Group[]): { areas: Group[]; none: boolean } {
  const ticked = new Set(choices.filter((x) => areaTicked(pile, x)));
  if (ticked.has(g)) ticked.delete(g);
  else ticked.add(g);
  if (choices.every((x) => ticked.has(x))) return { areas: [], none: false };
  if (ticked.size === 0) return { areas: [], none: true };
  return { areas: GROUPS.map((x) => x.id).filter((x) => ticked.has(x)), none: false };
}

/** The pile meter: how close the pile is to the goal. */
export function meterOf(pile: PileState, all: Suggestion[]) {
  const total = pile.totalBytes;
  const manual = pile.mode === "manual";
  const target = pile.goalBytes === "all" ? Math.max(total, safeAvailableBytes(pile, all)) : pile.goalBytes;
  const fill = target ? Math.min(100, (total / target) * 100) : 0;
  const short = manual || pile.goalBytes === "all" ? 0 : Math.max(0, pile.goalBytes - total);
  const right = manual ? "chosen by hand" : pile.goalBytes === "all" ? "every Safe crate" : `goal ${Math.round(pile.goalBytes / GB)} GB`;
  return { total, target, fill, short: short > 0.05 * GB ? short : 0, right, manual };
}

/** Quick questions still to answer (spec "Quick questions"): Review and Risky
 * items, except blocked, kept, already in the pile, Ask me later, and changed. */
export function questionDeck(pile: PileState, all: Suggestion[]): Suggestion[] {
  const added = new Set(pile.addedIds);
  return all
    .filter((s) => s.risk !== "safe" && !s.blockedReason && s.sizeBytes > 0 && !added.has(s.id) && !s.decision && !s.skipReason)
    .sort((a, b) => b.sizeBytes - a.sizeBytes);
}

/** What a running scan is doing, from its latest `scan_progress` event. */
export function scanLabel(p: { phase: string; detector: string | null; currentCount: number } | null): string {
  if (!p) return "Scanning your disk…";
  const n = p.currentCount.toLocaleString("en-US");
  if (p.phase === "paused") return "Scan paused while on battery.";
  if (p.phase === "discovering") return `Looking through your folders · ${n} files and folders`;
  if (p.phase === "worktrees") return `Checking Git worktrees · ${n} ${p.currentCount === 1 ? "repository" : "repositories"}`;
  if (p.phase === "projects") return `Measuring project folders · ${n} checked`;
  if (p.phase === "measuring") return `Measuring folders for the disk map · ${n} files and folders`;
  const d = p.detector ?? "";
  const family = d.split(".")[0];
  const names: Record<string, string> = {
    docker: "Checking Docker",
    git: "Checking Git worktrees",
    project: "Checking project folders",
    cache: "Checking caches",
    files: "Checking files",
    mobile: "Checking phones and emulators",
    ai: "Checking AI models",
    tools: "Checking tools",
    apps: "Checking apps",
    system: "Checking the system",
  };
  return `${names[family] ?? "Scanning your disk"}…`;
}

/** Percentage of used space the disk map explains, for the tab badge. */
export function coveragePercent(usedBytes: number, notExplainedBytes: number): number {
  return usedBytes ? ((usedBytes - notExplainedBytes) / usedBytes) * 100 : 100;
}
