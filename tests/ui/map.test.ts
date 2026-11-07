// Task 5.12: the disk map's treemap, tile classes and coverage figures.
// Run with `npm run test:ui` (Node's built-in test runner).
import { test } from "node:test";
import assert from "node:assert/strict";
import { causesOf, coverText, coverage, coverageBadge, defaultTile, explained, splitAreas, squarify, tileClass, tilesOf } from "../../src/features/map/mapModel.ts";
import type { BlindSpot, DiskMap } from "../../src/lib/types.ts";

const GB = 1e9;
// The prototype's TILES.
const SIZES: [string, string, number][] = [
  ["projects", "Projects", 96],
  ["docker", "Docker", 72],
  ["ai", "AI models", 61.6],
  ["caches", "Caches", 58],
  ["files", "Your files", 58],
  ["mobile", "Phones and emulators", 45],
  ["apps", "Apps and tools", 44],
  ["system", "macOS", 44.2],
];
function map(notExplained = 20.8, sizes = SIZES): DiskMap {
  const used = sizes.reduce((t, s) => t + s[2], 0) + notExplained;
  return {
    usedBytes: used * GB,
    explainedBytes: (used - notExplained) * GB,
    notExplainedBytes: notExplained * GB,
    areas: sizes.map(([id, label, gb]) => ({ id, label, bytes: gb * GB, leaveAloneBytes: 0, leaveAloneLabel: "" })),
    incomplete: false,
    overBytes: 0,
    measuredAt: null,
    purgeableBytes: null,
    snapshotCount: null,
  };
}

test("the treemap fills the box exactly, to scale, largest first", () => {
  const boxes = squarify(tilesOf(map()), 0, 0, 100, 100);
  assert.equal(boxes.length, 9);
  assert.equal(boxes[0].t.id, "projects");
  const area = boxes.reduce((t, b) => t + b.w * b.h, 0);
  assert.ok(Math.abs(area - 10000) < 1e-6, String(area));
  const total = boxes.reduce((t, b) => t + b.t.v, 0);
  for (const b of boxes) assert.ok(Math.abs((b.w * b.h) / 10000 - b.t.v / total) < 1e-9, b.t.id);
  for (const b of boxes) assert.ok(b.x >= -1e-9 && b.y >= -1e-9 && b.x + b.w <= 100 + 1e-9 && b.y + b.h <= 100 + 1e-9, b.t.id);
});

test("empty areas get no tile, and Not explained is striped", () => {
  const boxes = squarify(tilesOf(map(0, [...SIZES.slice(0, 7), ["system", "macOS", 0]])), 0, 0, 100, 100);
  assert.ok(!boxes.some((b) => b.t.id === "system" || b.t.id === "unknown"));
  const withUnknown = squarify(tilesOf(map()), 0, 0, 100, 100).find((b) => b.t.id === "unknown")!;
  assert.match(tileClass(withUnknown), /\bunknown\b/);
});

test("small tiles hide their labels and short wide tiles lay them flat", () => {
  const t = { id: "x", name: "X", c: "", v: 1 };
  assert.equal(tileClass({ t, x: 0, y: 0, w: 10, h: 20 }), "tile tiny");
  assert.equal(tileClass({ t, x: 0, y: 0, w: 40, h: 10 }), "tile flat");
  assert.equal(tileClass({ t, x: 0, y: 0, w: 30, h: 30 }), "tile");
});

test("coverage reads as the prototype's", () => {
  const tiles = tilesOf(map());
  assert.equal(coverText(map()).pct, "95.8% explained");
  assert.equal(coverText(map()).line, "I can account for 478.8 of the 499.6 GB in use.");
  assert.equal(Math.round(coverage(map()).pct), Math.round(explained(tiles).pct), "same as the prototype on a complete map");
  assert.equal(Math.round(explained(tiles).pct), 96);
  assert.equal(coverageBadge(map()), 96);
  assert.equal(coverageBadge(map(0)), null, "no badge once everything is explained");
  assert.equal(coverageBadge(null), null);
});

test("the area with the most to remove opens first", () => {
  const items = [
    { mapArea: "ai", sizeBytes: 53.6 * GB },
    { mapArea: "docker", sizeBytes: 52.1 * GB },
  ];
  assert.equal(defaultTile(map(), items), "ai");
});

test("an over-counted map never claims more in use than the disk", () => {
  // Areas add up to far more than used (APFS clones and snapshots counted twice).
  const m = { ...map(), usedBytes: 921 * GB, notExplainedBytes: 0, explainedBytes: 921 * GB };
  m.areas = m.areas.map((a) => ({ ...a, bytes: a.bytes * 10 }));
  assert.equal(coverText(m).line, "I can account for 921.0 of the 921.0 GB in use.");
});

test("areas under 3% are listed apart, never Not explained", () => {
  const m = map(5, [...SIZES, ["browser", "Browser data", 7.4], ["fonts", "Fonts and plug-ins", 4.2]]);
  const { tiles, small } = splitAreas(tilesOf(m));
  assert.deepEqual(small.map((t) => t.id), ["browser", "fonts"]);
  assert.ok(tiles.some((t) => t.id === "unknown"), "Not explained is always drawn, however small");
});

const spot = (id: string, kind: BlindSpot["kind"], gb: number | null, state: BlindSpot["state"] = "open"): BlindSpot => ({
  id,
  kind,
  title: "",
  text: "",
  location: "~/" + id,
  sizeBytes: gb === null ? null : gb * GB,
  actionLabel: "",
  action: kind === "needs_permission" ? "open_settings" : kind === "unknown_files" ? "mark_mine" : "scan_folder",
  state,
  doneText: state === "done" ? "scanned" : null,
});

test("Not explained splits into causes; what no blind spot accounts for is shown as not traced", () => {
  const m = map(100);
  const { causes, left } = causesOf(m, [spot("a", "unscanned_folder", 30), spot("b", "unscanned_folder", 50), spot("c", "needs_permission", null), spot("d", "unscanned_folder", 5, "done"), spot("e", "unknown_files", 4)]);
  assert.deepEqual(causes.map((c) => c.id), ["outside", "locked", "untraced", "strange"]);
  const outside = causes[0];
  assert.deepEqual(outside.open.map((s) => s.id), ["b", "a"], "biggest first");
  assert.deepEqual([outside.bytes / GB, outside.fixed.length], [80, 1]);
  assert.equal(causes[1].bytes, 0, "an unknown size adds nothing");
  assert.equal(Math.round(causes[2].bytes / GB), 16, "100 - 80 - 4");
  assert.equal(Math.round(left / GB), 100);
  assert.deepEqual(causesOf(map(0), []).causes, [], "a fully explained map has no causes");
  assert.ok(!causesOf(map(10), [], "linux").causes[0].why.includes("Library"), "Library is a macOS folder");
});
