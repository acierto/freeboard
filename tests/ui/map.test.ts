// Task 5.12: the disk map's treemap, tile classes and coverage figures.
// Run with `npm run test:ui` (Node's built-in test runner).
import { test } from "node:test";
import assert from "node:assert/strict";
import { coverText, coverageBadge, defaultTile, explained, squarify, tileClass, tilesOf } from "../../src/features/map/mapModel.ts";
import type { DiskMap } from "../../src/lib/types.ts";

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
  assert.equal(coverText(tiles).pct, "95.8% explained");
  assert.equal(coverText(tiles).line, "I can account for 478.8 of the 499.6 GB in use.");
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
