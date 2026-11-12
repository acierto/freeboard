// What the cleanup sheets say, as pure functions over backend data. Ported
// from the prototype's `confirmSheet()`, `cmdFor()` and `run()`.
import { lang, t } from "../../i18n/index.ts";
import { itemName, tb } from "../../i18n/backend.ts";
import { GB, gb } from "../../lib/format.ts";
import type { CleanupPlan, ItemState, Platform, RunDetail, RunItem, Suggestion } from "../../lib/types.ts";

/** The action line of a plan row (prototype `cmdFor`): the item's action,
 * plus the chosen parts when only some of them go. */
export function cmdFor(s: Suggestion): string {
  const action = tb(s.actionLabel);
  if (!s.parts.length) return action;
  const on = s.parts.filter((p) => p.selected);
  return on.length === s.parts.length ? action : action + ": " + on.map((p) => tb(p.label)).join(", ");
}

/** Only commands use the code font in plan rows (decision 2026-11-05). */
export const isCommand = (s: Pick<Suggestion, "action">) => s.action === "native_command";



/** The acknowledgment next to an item that needs its own confirmation. */
export const ackText = (s: Suggestion) => (s.recoverable ? t("ack.what") : t("ack.lost"));

const FINAL: ItemState[] = ["done", "moved_to_trash", "skipped", "failed", "canceled", "interrupted", "gone"];
export const isFinal = (s: ItemState) => FINAL.includes(s);
const removed = (s: ItemState) => s === "done" || s === "moved_to_trash";
/** It left the list: removed, moved to the Trash, or already gone. */
const left = (s: ItemState) => removed(s) || s === "gone";

/** A run's rows grouped by plan item: a split item has one row per part. */
export function stepsOf(run: RunDetail | null, itemId: string): RunItem[] {
  return run ? run.items.filter((i) => (i.parentId ?? i.itemId) === itemId) : [];
}

export interface Progress {
  /** "Checking <item>": the item being handled, or the last one handled. */
  current: string;
  /** Plan items handled so far, out of all of them. */
  done: number;
  total: number;
  /** Plan items that left the boat (removed or moved to the Trash). */
  gone: Set<string>;
}

/** Progress by plan item, as the prototype counts it (crates, not parts). */
export function progressOf(plan: CleanupPlan, run: RunDetail | null): Progress {
  const total = plan.items.length;
  let done = 0;
  let current = "";
  const gone = new Set<string>();
  for (const s of plan.items) {
    const steps = stepsOf(run, s.id);
    if (steps.length && steps.every((x) => isFinal(x.state))) {
      done++;
      current = itemName(s);
      if (steps.some((x) => left(x.state))) gone.add(s.id);
    }
  }
  const active = plan.items.find((s) => stepsOf(run, s.id).some((x) => x.state === "checking" || x.state === "running"));
  if (active) current = itemName(active);
  if (!current && plan.items[0]) current = itemName(plan.items[0]);
  return { current, done, total, gone };
}

/** What the run expected to free right away, counting only the items that
 * were removed (a skipped or failed item frees nothing, as expected). */
export function expectedFreed(run: RunDetail): number {
  return run.items.filter((i) => i.state === "done").reduce((t, i) => t + i.expectedBytes, 0);
}

export const trashUnavailable =(i: RunItem) => i.state === "failed" && !!i.reason?.startsWith("Trash unavailable");


/** The Logbook's one-line note (prototype SAMPLE_LOG wording). */
export function logNote(r: { trashBytes: number; skipped: number }, platform: Platform, trash: (p: Platform) => string): string {
  const parts: string[] = [];
  if (r.trashBytes > 0) parts.push(t("log.kept", { x: gb(r.trashBytes), t: trash(platform) }));
  if (r.skipped > 0) parts.push(r.skipped === 1 ? t("log.skipped1") : t("log.skippedN", { n: r.skipped }));
  return parts.length ? parts.join(". ") : t("log.none");
}

/** What happened to one item or part, for run details. */
export function resultWord(i: RunItem, platform: Platform, trash: (p: Platform) => string): string {
  const why = i.reason ? `: ${tb(i.reason)}` : "";
  switch (i.state) {
    case "done":
      // A worktree says whether its branch was kept.
      return t("c.removed") + why;
    case "moved_to_trash":
      return t("c.movedto", { t: trash(platform) });
    case "skipped":
      return t("c.skipped") + why;
    case "gone":
      return t("c.gone") + why;
    case "failed":
      return t("c.notremoved") + why;
    case "canceled":
      return t("c.notstarted");
    case "interrupted":
      return t("c.interrupted") + why;
    default:
      return t("c.waiting");
  }
}

/** "592+ days" reads as "1+ year" (prototype `age()`). Other languages
 * show the backend's text translated, with the days as they are. */
export function age(text: string): string {
  if (lang() !== "en") return tb(text);
  return text.replace(/(\d+)\+ days/, (m, d: string) => {
    const n = +d;
    return n >= 730 ? Math.floor(n / 365) + "+ years" : n >= 365 ? "1+ year" : n >= 60 ? Math.floor(n / 30) + "+ months" : m;
  });
}

/** The verb for one item (prototype `verbFor`). */
export const verbFor = (s: Pick<Suggestion, "action">) => (s.action === "trash" ? t("v.trash") : s.action === "delete" ? t("v.delete") : t("v.remove"));

/** The confirmation's title and button (prototype `confirmSheet()`). */
export function confirmHead(plan: CleanupPlan, permanent: boolean): { title: string; go: string } {
  const n = plan.items.length;
  const total = plan.items.reduce((t, s) => t + s.sizeBytes, 0);
  if (permanent) return { title: n === 1 ? t("c.del1") : t("c.delN", { n }), go: t("c.delgo") };
  if (n === 1) return { title: t("c.one", { v: verbFor(plan.items[0]), x: itemName(plan.items[0]) }), go: t("c.onego", { v: verbFor(plan.items[0]), x: gb(total) }) };
  return { title: t("c.tossN", { n }), go: t("c.tossgo", { x: gb(total) }) };
}

/** What happened to one plan item, from its steps (a split item has one
 * step per part): it left the boat if any part did. */
export type Outcome = "waiting" | "active" | "tossed" | "stayed" | "failed" | "not_started";
export function outcomeOf(run: RunDetail | null, itemId: string): Outcome {
  const steps = stepsOf(run, itemId);
  if (!steps.length) return "waiting";
  // Started once any step runs or ends: an item with hundreds of parts is
  // active between two of them, not waiting (found on a real Mac, 2026-11-07).
  if (!steps.every((x) => isFinal(x.state))) return steps.some((x) => x.state === "checking" || x.state === "running" || isFinal(x.state)) ? "active" : "waiting";
  if (steps.some((x) => removed(x.state))) return "tossed";
  if (steps.some((x) => x.state === "failed" || x.state === "interrupted")) return "failed";
  if (steps.some((x) => x.state === "skipped")) return "stayed";
  return "not_started";
}

/** The run sheet's figures (prototype `run()`). */
export function runFigures(plan: CleanupPlan, run: RunDetail | null) {
  const out = plan.items.map((s) => outcomeOf(run, s.id));
  const count = (o: Outcome) => out.filter((x) => x === o).length;
  const active = out.indexOf("active");
  const handled = out.filter((o) => o !== "waiting" && o !== "active").length;
  // Space freed so far: removed steps that did not go to the Trash.
  const freed = (run?.items ?? []).filter((i) => i.state === "done").reduce((t, i) => t + i.expectedBytes, 0);
  // The bar moves one item's width per item, as the prototype's, and inside
  // an item with many parts it moves part by part.
  const share = (id: string) => {
    const st = stepsOf(run, id);
    return st.length ? st.filter((x) => isFinal(x.state)).length / st.length : 0;
  };
  const fraction = plan.items.length ? plan.items.reduce((t, s) => t + share(s.id), 0) / plan.items.length : 0;
  // Inside the item being handled: "37 of 244".
  const cur = active >= 0 ? stepsOf(run, plan.items[active].id) : [];
  const within = cur.length > 1 ? { done: cur.filter((x) => isFinal(x.state)).length, of: cur.length } : null;
  return { outcomes: out, tossed: count("tossed"), stayed: count("stayed"), failed: count("failed"), handled, at: active >= 0 ? active + 1 : Math.min(Math.max(handled, 1), plan.items.length), freed, fraction, within };
}

/** The 3D deck shows up to nine crates; crate k stands for items k, k+9, ...
 * (prototype `cubeHTML`). Its look follows the item it stands for now. */
export type CubeLook = "" | "lift" | "gone" | "stay" | "stuck";
export function cubeLooks(outcomes: Outcome[]): CubeLook[] {
  const n = Math.min(outcomes.length, 9);
  const looks: CubeLook[] = [];
  for (let k = 0; k < n; k++) {
    let look: CubeLook = "";
    for (let i = k; i < outcomes.length; i += 9) {
      const o = outcomes[i];
      if (o === "waiting") break;
      if (o === "active") {
        look = "lift";
        break;
      }
      // A later item takes this crate's place once this one is handled.
      if (i + 9 < outcomes.length) {
        look = "";
        continue;
      }
      look = o === "tossed" ? "gone" : o === "stayed" ? "stay" : o === "failed" ? "stuck" : "";
    }
    looks.push(look);
  }
  return looks;
}

/** One line per plan item in the result. */
export interface ResultLine {
  id: string;
  title: string;
  bytes: number;
  inTrash: boolean;
  why: string;
}

/** The result's sections (prototype `outcomeSheet()`): what was tossed, what
 * stayed on board, what could not go (with its reasons, each once) and what
 * did not start. A split item is listed by part where its parts ended
 * differently: two of three node_modules folders that stayed must not hide
 * behind "Tossed" (Linux VM, 2026-11-07). */
export function resultSections(plan: CleanupPlan, run: RunDetail) {
  const sec: Record<"tossed" | "stayed" | "failed" | "not_started", ResultLine[]> = { tossed: [], stayed: [], failed: [], not_started: [] };
  const retry = new Set<string>();
  for (const s of plan.items) {
    const steps = stepsOf(run, s.id);
    const o = outcomeOf(run, s.id);
    if (o === "waiting" || o === "not_started" || !steps.length) {
      sec.not_started.push({ id: s.id, title: itemName(s), bytes: s.sizeBytes, inTrash: false, why: "" });
      retry.add(s.id);
      continue;
    }
    const gone = steps.filter((x) => removed(x.state));
    if (gone.length) sec.tossed.push({ id: s.id, title: itemName(s), bytes: gone.reduce((t, x) => t + x.expectedBytes, 0), inTrash: gone.every((x) => x.state === "moved_to_trash"), why: "" });
    // Steps that did not go: by part when only some did, else the item.
    const whole = !gone.length && steps.length === 1;
    for (const x of steps) {
      if (removed(x.state)) continue;
      const line = (why: string): ResultLine => ({ id: whole ? s.id : x.itemId, title: whole || !x.parentId ? itemName(s) : tb(x.title), bytes: whole ? s.sizeBytes : x.expectedBytes, inTrash: false, why });
      if (x.state === "skipped") sec.stayed.push(line(x.reason ?? t("c.changed")));
      else if (x.state === "failed" || x.state === "interrupted") {
        sec.failed.push(line(x.reason ?? ""));
        retry.add(s.id);
      } else if (x.state === "canceled") {
        sec.not_started.push(line(""));
        retry.add(s.id);
      }
    }
  }
  const reasons = [...new Set(sec.failed.map((l) => l.why).filter(Boolean))];
  const tossedBytes = sec.tossed.reduce((t, l) => t + l.bytes, 0);
  return { ...sec, reasons, tossedBytes, retryIds: [...retry] };
}

/** The free-space bar under the result's headline. */
export function freeBar(before: number, after: number) {
  const max = Math.max(after / GB + 10, 60);
  const w = (v: number) => ((Math.max(0, v) / GB / max) * 100).toFixed(1) + "%";
  return { before: w(before), gained: w(after - before) };
}
