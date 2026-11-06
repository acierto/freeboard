// What the cleanup sheets say, as pure functions over backend data. Ported
// from the prototype's `confirmSheet()`, `cmdFor()` and `run()`.
import { gb, plural } from "../../lib/format.ts";
import type { CleanupPlan, ItemState, Platform, RunDetail, RunItem, Suggestion } from "../../lib/types.ts";

/** The action line of a plan row (prototype `cmdFor`): the item's action,
 * plus the chosen parts when only some of them go. */
export function cmdFor(s: Suggestion): string {
  if (!s.parts.length) return s.actionLabel;
  const on = s.parts.filter((p) => p.selected);
  return on.length === s.parts.length ? s.actionLabel : s.actionLabel + ": " + on.map((p) => p.label).join(", ");
}

/** Only commands use the code font in plan rows (decision 2026-11-05). */
export const isCommand = (s: Pick<Suggestion, "action">) => s.action === "native_command";

export function confirmTitle(plan: CleanupPlan, permanent: boolean): string {
  const n = plan.items.length;
  return permanent ? `Delete ${plural(n, "item")} permanently?` : `Toss ${plural(n, "crate")}?`;
}

export function goLabel(plan: CleanupPlan, permanent: boolean): string {
  if (permanent) return "Delete permanently";
  return plan.items.length === 1 ? "Remove it" : "Toss them";
}

/** The acknowledgment next to an item that needs its own confirmation. */
export const ackText = (s: Suggestion) => (s.recoverable ? "I understand what this removes." : "I understand this cannot be recovered.");

const FINAL: ItemState[] = ["done", "moved_to_trash", "skipped", "failed", "canceled", "interrupted"];
export const isFinal = (s: ItemState) => FINAL.includes(s);
const removed = (s: ItemState) => s === "done" || s === "moved_to_trash";

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
      current = s.title;
      if (steps.some((x) => removed(x.state))) gone.add(s.id);
    }
  }
  const active = plan.items.find((s) => stepsOf(run, s.id).some((x) => x.state === "checking" || x.state === "running"));
  if (active) current = active.title;
  if (!current && plan.items[0]) current = plan.items[0].title;
  return { current, done, total, gone };
}

/** What the run expected to free right away, counting only the items that
 * were removed (a skipped or failed item frees nothing, as expected). */
export function expectedFreed(run: RunDetail): number {
  return run.items.filter((i) => i.state === "done").reduce((t, i) => t + i.expectedBytes, 0);
}

export const trashUnavailable =(i: RunItem) => i.state === "failed" && !!i.reason?.startsWith("Trash unavailable");

export function resultTitle(run: RunDetail): string {
  if (run.state === "canceled") return "Stopped";
  if (run.state === "interrupted") return "Interrupted";
  return run.items.some((i) => removed(i.state)) ? "Much better!" : "Nothing tossed";
}

/** The Logbook's one-line note (prototype SAMPLE_LOG wording). */
export function logNote(r: { trashBytes: number; skipped: number }, platform: Platform, trash: (p: Platform) => string): string {
  const parts: string[] = [];
  if (r.trashBytes > 0) parts.push(`Kept ${gb(r.trashBytes)} in the ${trash(platform)}`);
  if (r.skipped > 0) parts.push(`${r.skipped} skipped because ${r.skipped === 1 ? "it" : "they"} changed`);
  return parts.length ? parts.join(". ") : "Nothing skipped";
}

/** What happened to one item or part, for run details. */
export function resultWord(i: RunItem, platform: Platform, trash: (p: Platform) => string): string {
  const why = i.reason ? `: ${i.reason}` : "";
  switch (i.state) {
    case "done":
      return "Removed";
    case "moved_to_trash":
      return `Moved to the ${trash(platform)}`;
    case "skipped":
      return "Skipped" + why;
    case "failed":
      return "Not removed" + why;
    case "canceled":
      return "Not started, the cleanup was stopped";
    case "interrupted":
      return "Interrupted" + why;
    default:
      return "Waiting";
  }
}
