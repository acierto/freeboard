// Dinghy's geometry and mood, ported from the prototype's scene code
// (`boatAt`, `faceSVG`, `cratesSVG`, `waves`). Pure functions with no runtime
// imports, so tests/ui/dinghy.test.ts can run them directly under Node.

export type Mood = "good" | "warn" | "crit";

/** Free space at which the deck stops rising (spec "Dinghy"). */
export const FLOAT_LIMIT_GB = 60;
/** Boat position: x is fixed, y moves 44 px from 0 to 60 GB free. */
export const BOAT_X = 292;
export const BOAT_Y_EMPTY = 212;
export const BOAT_RISE = 44;
export const MAX_CRATES = 9;

/** The boat's vertical offset in scene units for a free-space value in GB. */
export function boatY(freeGb: number): number {
  const f = Math.max(0, freeGb);
  return Number((BOAT_Y_EMPTY - BOAT_RISE * Math.min(1, f / FLOAT_LIMIT_GB)).toFixed(1));
}

/** The CSS transform the prototype writes on #boat. */
export function boatTransform(freeGb: number): string {
  return `translate(${BOAT_X}px,${boatY(freeGb).toFixed(1)}px)`;
}

/** Wave path (prototype `waves()`): a run of quadratic curves, filled down. */
export function wavePath(y: number, amp: number): string {
  let d = "M-600 " + y;
  for (let x = -600; x < 1320; x += 60) d += " q15 " + -amp + " 30 0 t30 0";
  return d + " V360 H-600 Z";
}

/** Crate slots on deck, largest crate first (prototype `cratesSVG`). */
export const CRATE_SLOTS: ReadonlyArray<readonly [number, number]> = [
  [6, -32],
  [40, -32],
  [74, -32],
  [108, -32],
  [23, -60],
  [57, -60],
  [91, -60],
  [40, -88],
  [74, -88],
];

/** Crate labels: up to nine, each the rounded size in GB. Real items can be
 * smaller than half a gigabyte; those read "<1" rather than a misleading "0"
 * (found on a real disk, 2026-11-06; the prototype's data has none). */
export function crateLabels(sizesGb: number[]): string[] {
  return sizesGb.slice(0, MAX_CRATES).map((s) => (s > 0 && Math.round(s) === 0 ? "<1" : String(Math.round(s))));
}
