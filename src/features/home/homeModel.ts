// What the home view says, as pure functions over backend data. Ported from
// the prototype's `renderHero()`; the pile itself is computed by the backend
// (design D3), this only describes it.
import { GB, GROUPS, gb, plural } from "../../lib/format.ts";
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

export function tossLabel(count: number, pileBytes: number): string {
  return count ? "Toss " + gb(pileBytes) + " overboard" : "Nothing to toss";
}

const kept = (s: Suggestion) => s.decision === "keep";

/** Safe items the pile may fill from: not taken out, not kept, in the chosen areas. */
export function safeAvailableBytes(pile: PileState, all: Suggestion[]): number {
  const out = new Set(pile.takenOutIds);
  return all
    .filter((s) => s.risk === "safe" && !s.blockedReason && !s.skipReason && !out.has(s.id) && !kept(s) && s.sizeBytes > 0)
    .filter((s) => pile.areas.length === 0 || pile.areas.includes(s.group))
    .reduce((t, s) => t + s.sizeBytes, 0);
}

/** The note under the Toss button (spec "Building the pile" messages). */
export function pileNote(pile: PileState, all: Suggestion[]): string {
  const items = pileItems(pile, all);
  const anyArea = pile.areas.length > 0;
  if (!items.length) {
    if (pile.mode === "manual") return "The pile is empty. Pick a size to fill it again.";
    return anyArea ? "This area has no Safe crates. Tick the items you want below." : "Answer a quick question below, or check the disk map.";
  }
  let note = plural(items.length, "crate") + ". ";
  const gap = pile.goalBytes === "all" || pile.mode === "manual" ? 0 : pile.goalBytes - pile.totalBytes;
  if (pile.mode === "manual") note += "You chose these by hand. Pick a size again to refill the pile.";
  else if (gap > 0.05 * GB)
    note += (anyArea ? "Safe crates in this area stop at " : "Safe crates stop at ") + gb(safeAvailableBytes(pile, all)) + ". Add more items below to find " + gb(gap) + " more.";
  else if (items.every((s) => s.risk === "safe")) note += "Everything here can be rebuilt by its tool.";
  else if (items.some((s) => s.risk === "risky")) note += "Includes something that cannot be recovered.";
  else note += "Includes crates you approved.";
  return note;
}

/** Where from chips: Everywhere plus each group that has something in it. */
export function areaChoices(all: Suggestion[]): Group[] {
  return GROUPS.map((g) => g.id).filter((g) => all.some((s) => s.group === g));
}

/** Quick questions still to answer (spec "Quick questions"): Review and Risky
 * items, except blocked, kept, already in the pile, Ask me later, and changed. */
export function questionDeck(pile: PileState, all: Suggestion[]): Suggestion[] {
  const added = new Set(pile.addedIds);
  return all
    .filter((s) => s.risk !== "safe" && !s.blockedReason && s.sizeBytes > 0 && !added.has(s.id) && !s.decision && !s.skipReason)
    .sort((a, b) => b.sizeBytes - a.sizeBytes);
}

/** Percentage of used space the disk map explains, for the tab badge. */
export function coveragePercent(usedBytes: number, notExplainedBytes: number): number {
  return usedBytes ? ((usedBytes - notExplainedBytes) / usedBytes) * 100 : 100;
}
