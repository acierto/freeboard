// Slice 2: what the cleanup sheets and the Logbook say.
// Run with `npm run test:ui` (Node's built-in test runner).
import { test } from "node:test";
import assert from "node:assert/strict";
import { ackText, age, cmdFor, confirmHead, cubeLooks, expectedFreed, logNote, progressOf, resultSections, resultWord, runFigures } from "../../src/features/cleanup/cleanupModel.ts";
import type { CleanupPlan, RunDetail, RunItem, Suggestion } from "../../src/lib/types.ts";

const GB = 1e9;
const trash = (p: string) => (p === "windows" ? "Recycle Bin" : "Trash");

function item(id: string, over: Partial<Suggestion> = {}): Suggestion {
  return {
    id,
    detector: "x",
    group: "projects",
    mapArea: "projects",
    title: id,
    location: "",
    sizeBytes: GB,
    fullSizeBytes: GB,
    lastUsedAt: null,
    lastUsedLabel: "",
    risk: "safe",
    action: "delete",
    actionLabel: "Delete the folders",
    reason: "",
    recovery: "",
    recoverable: true,
    blockedReason: null,
    needsConfirmation: false,
    parts: [],
    fingerprint: "",
    sizeIsEstimate: false,
    skipReason: null,
    decision: null,
    ackName: null,
    riskWhy: [],
    safer: [],
    ...over,
  };
}

function plan(items: Suggestion[]): CleanupPlan {
  return {
    id: "p",
    createdAt: "",
    expiresAt: "",
    itemIds: items.map((s) => s.id),
    planItems: items.map((s) => ({ suggestionId: s.id, partIds: s.parts.filter((p) => p.selected).map((p) => p.id) })),
    bytesFreedNow: 0,
    bytesHeldInTrash: 0,
    unrecoverableIds: [],
    items,
    needsAckIds: [],
    acknowledgedIds: [],
    unrecoverableBytes: 0,
    freeBytesNow: 0,
    ready: true,
    state: "open",
  };
}

const row = (itemId: string, state: RunItem["state"], parentId: string | null = null, over: Partial<RunItem> = {}): RunItem => ({
  itemId,
  parentId,
  title: itemId,
  action: "delete",
  actionLabel: "Delete",
  expectedBytes: GB,
  state,
  reason: null,
  ...over,
});

function run(items: RunItem[], over: Partial<RunDetail> = {}): RunDetail {
  return {
    id: "r",
    planId: "p",
    startedAt: "",
    finishedAt: null,
    state: "running",
    freeBefore: 0,
    freeAfter: null,
    expectedBytes: 0,
    measuredBytesFreed: null,
    trashBytes: 0,
    skipped: 0,
    failed: 0,
    involvesDocker: false,
    items,
    freeBytesNow: null,
    ...over,
  };
}

const parts = [
  { id: "a", label: "web-app", location: "", sizeBytes: GB, selected: true },
  { id: "b", label: "admin", location: "", sizeBytes: GB, selected: false },
  { id: "c", label: "docs", location: "", sizeBytes: GB, selected: true },
];

test("the action line names the chosen parts only when some are left out (prototype cmdFor)", () => {
  assert.equal(cmdFor(item("x")), "Delete the folders");
  assert.equal(cmdFor(item("x", { parts })), "Delete the folders: web-app, docs");
  assert.equal(cmdFor(item("x", { parts: parts.map((p) => ({ ...p, selected: true })) })), "Delete the folders");
});

test("confirmation copy follows the prototype", () => {
  assert.deepEqual(confirmHead(plan([item("Worktrees", { action: "native_command" })]), false), { title: "Remove Worktrees?", go: "Remove 1.0 GB" });
  assert.deepEqual(confirmHead(plan([item("venv", { action: "trash" })]), false), { title: "Move to Trash venv?", go: "Move to Trash 1.0 GB" });
  assert.deepEqual(confirmHead(plan([item("a"), item("b")]), false), { title: "Toss 2 items?", go: "Toss 2.0 GB" });
  assert.equal(confirmHead(plan([item("a")]), true).title, "Delete 1 item permanently?");
  assert.equal(age("Projects idle for 592+ days"), "Projects idle for 1+ year");
  assert.equal(age("Projects idle for 1391+ days"), "Projects idle for 3+ years");
  assert.equal(age("Projects idle for 90+ days"), "Projects idle for 3+ months");
  assert.equal(age("Projects idle for 36+ days"), "Projects idle for 36+ days");
  assert.equal(ackText(item("v", { recoverable: false })), "I understand this cannot be recovered.");
});

test("progress counts crates, not parts, and a crate leaves when its parts are done", () => {
  const p = plan([item("nm", { parts }), item("cache")]);
  const r = run([row("a", "done", "nm"), row("c", "running", "nm"), row("cache", "waiting")]);
  let pr = progressOf(p, r);
  assert.deepEqual([pr.done, pr.total, pr.current, [...pr.gone]], [0, 2, "nm", []]);
  r.items[1].state = "skipped";
  pr = progressOf(p, r);
  assert.deepEqual([pr.done, pr.current, [...pr.gone]], [1, "nm", ["nm"]], "one part removed is enough for the crate to leave");
  r.items[2].state = "skipped";
  pr = progressOf(p, r);
  assert.deepEqual([pr.done, [...pr.gone]], [2, ["nm"]], "a skipped crate stays on board");
});

test("expected space counts only what was removed", () => {
  const r = run([row("a", "done"), row("b", "skipped"), row("c", "moved_to_trash"), row("d", "failed")]);
  assert.equal(expectedFreed(r), GB);
});

test("the run scene follows each crate: lifted, gone, stayed, stuck", () => {
  const p = plan([item("a"), item("b"), item("c"), item("d")]);
  const r = run([row("a", "done"), row("b", "skipped"), row("c", "running"), row("d", "waiting")]);
  const f = runFigures(p, r);
  assert.deepEqual([f.tossed, f.stayed, f.failed, f.handled, f.at], [1, 1, 0, 2, 3]);
  assert.equal(f.freed, GB);
  assert.deepEqual(cubeLooks(f.outcomes), ["gone", "stay", "lift", ""]);
  // An item with many parts: active between two parts, and the bar moves.
  const big = plan([item("imgs"), item("z")]);
  const parts = run([row("p1", "done", "imgs"), row("p2", "done", "imgs"), row("p3", "waiting", "imgs"), row("p4", "waiting", "imgs"), row("z", "waiting")]);
  const g = runFigures(big, parts);
  assert.deepEqual([g.outcomes[0], g.at, g.fraction, g.within], ["active", 1, 0.25, { done: 2, of: 4 }]);
  assert.deepEqual(cubeLooks(g.outcomes), ["lift", ""]);
  // Crate k stands for items k, k+9, ...: once item k is handled and a later
  // one waits, the crate is back on deck for it.
  const many = plan(Array.from({ length: 11 }, (_, i) => item("i" + i)));
  const rows = Array.from({ length: 11 }, (_, i) => row("i" + i, i < 10 ? "done" : "failed"));
  assert.deepEqual(cubeLooks(runFigures(many, run(rows)).outcomes).slice(0, 3), ["gone", "stuck", "gone"]);
});

test("the result groups items and names each reason once", () => {
  const p = plan([item("a"), item("b"), item("c"), item("d"), item("e")]);
  const timeout = "It took longer than 30 minutes, so I stopped it.";
  const r = resultSections(
    p,
    run([row("a", "moved_to_trash"), row("b", "skipped", null, { reason: "A new commit appeared after the scan." }), row("c", "failed", null, { reason: timeout }), row("d", "failed", null, { reason: timeout }), row("e", "canceled")], { state: "canceled" }),
  );
  assert.deepEqual([r.tossed.length, r.stayed.length, r.failed.length, r.not_started.length], [1, 1, 2, 1]);
  assert.ok(r.tossed[0].inTrash);
  assert.deepEqual(r.reasons, [timeout], "identical failures read as one reason");
  assert.deepEqual(r.retryIds, ["c", "d", "e"], "Try again reruns what failed or never started");
  // A split item whose parts ended differently lists the parts that stayed.
  const split = plan([item("nm")]);
  const parts = resultSections(
    split,
    run([row("nm-a", "done", "nm", { title: "node_modules in web-app" }), row("nm-b", "skipped", "nm", { title: "node_modules in admin-portal", reason: "Files inside it changed after the scan." })], { state: "finished" }),
  );
  assert.deepEqual([parts.tossed.length, parts.stayed.map((l) => l.title)], [1, ["node_modules in admin-portal"]]);
});

test("Logbook notes use the prototype's wording, with Recycle Bin on Windows", () => {
  assert.equal(logNote({ trashBytes: 1.2 * GB, skipped: 0 }, "macos", trash), "Kept 1.2 GB in the Trash");
  assert.equal(logNote({ trashBytes: 0, skipped: 1 }, "macos", trash), "1 skipped because it changed");
  assert.equal(logNote({ trashBytes: 1.2 * GB, skipped: 2 }, "windows", trash), "Kept 1.2 GB in the Recycle Bin. 2 skipped because they changed");
  assert.equal(logNote({ trashBytes: 0, skipped: 0 }, "linux", trash), "Nothing skipped");
});

test("run details give each item's result and reason", () => {
  assert.equal(resultWord(row("a", "done"), "macos", trash), "Removed");
  assert.equal(resultWord(row("a", "moved_to_trash"), "windows", trash), "Moved to the Recycle Bin");
  assert.equal(resultWord(row("a", "skipped", null, { reason: "A new commit appeared after the scan." }), "macos", trash), "Skipped: A new commit appeared after the scan.");
});
