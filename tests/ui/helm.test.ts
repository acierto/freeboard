// The helm's Make room panel: From checkboxes, the pile meter and the note.
// Run with `npm run test:ui` (Node's built-in test runner).
import { test } from "node:test";
import assert from "node:assert/strict";
import { areaChoices, areaTicked, meterOf, pileNote, reviewOnlyAreas, safeIn, toggledAreas } from "../../src/features/home/homeModel.ts";
import type { PileState, Suggestion } from "../../src/lib/types.ts";

const GB = 1e9;
const item = (id: string, group: Suggestion["group"], gb: number, risk: Suggestion["risk"] = "safe", extra: Partial<Suggestion> = {}) =>
  ({ id, group, sizeBytes: gb * GB, risk, blockedReason: null, skipReason: null, needsConfirmation: false, decision: null, ...extra }) as unknown as Suggestion;
const ALL = [
  item("d", "docker", 21),
  item("p", "projects", 27.9),
  item("c", "caches", 34.2),
  item("ai", "ai_models", 40, "review"),
  item("kept", "phones_emulators", 5, "safe", { decision: "keep" } as Partial<Suggestion>),
  item("locked", "your_files", 9, "review", { blockedReason: "uncommitted changes" } as Partial<Suggestion>),
];
const pile = (p: Partial<PileState> = {}): PileState => ({ goalBytes: 50 * GB, areas: [], noAreas: false, mode: "auto", addedIds: [], takenOutIds: [], itemIds: [], totalBytes: 0, ...p });

test("From lists areas with Safe crates and their Safe size; review-only areas are named apart", () => {
  assert.deepEqual(areaChoices(ALL), ["docker", "projects", "caches"]);
  assert.equal(safeIn("projects", ALL), 27.9 * GB);
  assert.equal(safeIn("phones_emulators", ALL), 0, "a kept item is not Safe space");
  // Blocked and kept items need no OK: they are never offered.
  assert.deepEqual(reviewOnlyAreas(ALL), ["ai_models"]);
});

test("every area starts ticked; unticking one keeps the others; ticking all again means everywhere", () => {
  const choices = areaChoices(ALL);
  const p = pile();
  assert.ok(choices.every((g) => areaTicked(p, g)));
  const one = toggledAreas(p, "caches", choices);
  assert.deepEqual(one, { areas: ["docker", "projects"], none: false });
  assert.deepEqual(toggledAreas(pile({ areas: one.areas }), "caches", choices), { areas: [], none: false });
});

test("unticking the last area means none, and the note says so", () => {
  const choices = areaChoices(ALL);
  assert.deepEqual(toggledAreas(pile({ areas: ["docker"] }), "docker", choices), { areas: [], none: true });
  const none = pile({ noAreas: true });
  assert.ok(!areaTicked(none, "docker"));
  assert.equal(pileNote(none, ALL), "No areas picked. Tick at least one, or use all areas.");
});

test("the meter shows progress to the goal and the gap that needs an OK", () => {
  const p = pile({ areas: ["docker", "projects"], itemIds: ["p", "d"], totalBytes: 48.9 * GB });
  const m = meterOf(p, ALL);
  assert.equal(m.right, "goal 50 GB");
  assert.equal(Math.round(m.fill * 10) / 10, 97.8);
  assert.equal(Math.round(m.short / 1e8) / 10, 1.1);
  assert.equal(pileNote(p, ALL), "", "an all-Safe pile needs no note");
  const all = meterOf(pile({ goalBytes: "all", itemIds: ["c", "p", "d"], totalBytes: 83.1 * GB }), ALL);
  assert.equal(all.right, "every Safe crate");
  assert.equal(all.short, 0);
  assert.equal(meterOf(pile({ mode: "manual", itemIds: ["d"], totalBytes: 21 * GB }), ALL).right, "chosen by hand");
});
