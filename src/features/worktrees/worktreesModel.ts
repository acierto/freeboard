// The Worktrees view as pure functions (prototype Version 19 `wtVerdict`,
// `wtChecks`, `renderWT`; spec "Worktrees view"). Removing a worktree deletes
// its folder, not its branch, so only uncommitted files can be lost; the core
// decides each worktree's status, this only describes it.
import type { Suggestion, WorktreeInfo, WorktreesView } from "../../lib/types.ts";
import { t } from "../../i18n/index.ts";

/** A worktree item, also one stored by a version before the Worktrees view
 * (no `worktree` facts yet): it belongs in the Worktrees tab, not in Quick
 * questions or the Disk map lists (found on a real Mac, 2026-11-07). */
export const isWorktree = (s: Suggestion) => !!s.worktree || s.detector === "git.worktrees";

export type WtKey = "ready" | "look" | "work" | "main";
export const WT_KEYS: WtKey[] = ["ready", "look", "work", "main"];
export type WtFilter = "all" | "agent" | WtKey;

/** Status colours (prototype `WT_STATES`). */
export const WT_COLOR: Record<WtKey, string> = { ready: "var(--safe)", look: "var(--review)", work: "var(--risky)", main: "var(--muted)" };

export interface WtRow {
  id: string;
  key: WtKey;
  repo: string;
  root: string;
  /** The branch, or the folder's name for a worktree on no branch. */
  name: string;
  bytes: number;
  /** null for a main checkout. */
  info: WorktreeInfo | null;
  /** The worktree item (null for a main checkout). */
  item: Suggestion | null;
}

/** Every worktree, plus each measured repository's main checkout. */
export function rowsOf(view: WorktreesView): WtRow[] {
  const rows: WtRow[] = view.items
    .filter((s) => s.worktree)
    .map((s) => {
      const w = s.worktree!;
      return { id: s.id, key: w.status, repo: w.repo, root: w.root, name: w.branch ?? w.relPath.split("/").pop() ?? w.relPath, bytes: s.sizeBytes, info: w, item: s };
    });
  for (const r of view.repos) if (r.mainBytes !== null) rows.push({ id: "main:" + r.root, key: "main", repo: r.name, root: r.root, name: "main", bytes: r.mainBytes, info: null, item: null });
  return rows;
}

export const statusLabel = (k: WtKey) => t("wt.st." + k);

/** "today", "yesterday", "12 days ago" (prototype `wtIdle`). */
export function idleText(d: number | null): string {
  if (d === null) return t("wt.unknownage");
  return d === 0 ? t("wt.today") : d === 1 ? t("wt.yesterday") : t("wt.daysago", { n: d });
}

/** The short age in a row: "today", "12d ago". */
export const idleShort = (d: number | null) => (d === null ? "" : d === 0 ? t("wt.today") : t("wt.dshort", { n: d }));

/** One plain sentence: why this worktree has its status (prototype `wtVerdict`). */
export function whyOf(r: WtRow): string {
  if (!r.info) return t("wt.why.main");
  const w = r.info;
  if (w.status === "work") {
    if (w.dirty) return w.dirty === 1 ? t("wt.why.dirty1") : t("wt.why.dirtyN", { n: w.dirty });
    return t("wt.why.hold." + (w.hold ?? "unknown"));
  }
  if (w.locked) return t("wt.why.locked", { t: idleText(w.idleDays) });
  if (w.status === "look") return t("wt.why.recent", { t: idleText(w.idleDays) });
  const n = w.idleDays ?? 0;
  return w.merged === "squash" ? t("wt.why.squash", { n }) : w.merged === "yes" ? t("wt.why.merged", { n }) : t("wt.why.kept", { n });
}

/** What was checked, each with a tick or a cross (prototype `wtChecks`). */
export function checksOf(w: WorktreeInfo): [ok: boolean, text: string][] {
  const idle = w.idleDays ?? 0;
  const merged = w.merged !== "no";
  const branch = merged
    ? w.merged === "squash"
      ? t("wt.ck.squash")
      : t("wt.ck.merged")
    : t(w.ahead ? (w.ahead === 1 ? "wt.ck.ahead1" : "wt.ck.aheadN") : "wt.ck.notmerged", { n: w.ahead }) + (w.pushed ? t("wt.ck.pushed") : "") + t("wt.ck.keep");
  return [
    [!w.dirty, w.dirty ? (w.dirty === 1 ? t("wt.ck.dirty1") : t("wt.ck.dirtyN", { n: w.dirty })) : t("wt.ck.clean")],
    [idle >= 7, idle >= 7 ? t("wt.ck.idle", { n: idle }) : t("wt.ck.used", { t: idleText(w.idleDays) })],
    [!w.locked, w.locked ? t("wt.ck.locked") : t("wt.ck.unlocked")],
    [true, branch],
  ];
}

/** Does removal delete the branch too? */
export const branchGoes = (w: WorktreeInfo) => w.commands.some((c) => / branch -[dD] /.test(c));

/** Has your work (uncommitted files, or a hold): removable after the user's
 * acknowledgment in the row. */
export const acknowledgeable = (r: WtRow) => r.key === "work" && !!r.item?.needsConfirmation;

/** Any other worktree can be forced from the row: the acknowledgment then
 * removes it even if a program has it open by its turn. */
export const forceable = (r: WtRow) => !!r.info && r.key !== "work";

/** Can it be selected? Ready and Needs a look can; Has your work only once
 * acknowledged; held work and main checkouts never. */
export const selectable = (r: WtRow, acked = false) => r.key === "ready" || r.key === "look" || (acked && acknowledgeable(r));

/** The worktree's folder as shown: inside the main checkout it is relative
 * to it; elsewhere the core gives the full path (found on a real Mac,
 * 2026-11-12: "~/proj/app/~/.codex/…" when the two were joined). */
export const dirOf = (r: WtRow) => (!r.info || !r.info.relPath ? r.root : /^[~/]/.test(r.info.relPath) ? r.info.relPath : `${r.root}/${r.info.relPath}`);

/** The in-row acknowledgment that makes it selectable (or forces it): what
 * is lost, in the user's words (decision 2026-11-12). */
export function ackLabel(r: WtRow): string {
  const w = r.info;
  if (!w) return "";
  if (w.status !== "work") return t("wt.ack.force");
  if (w.hold) return t("wt.ack." + w.hold);
  return w.dirty === 1 ? t("wt.ack1") : t("wt.ackN", { n: w.dirty });
}

/** Steps that make a worktree with uncommitted files removable (prototype `wtFix`). */
export function fixesOf(r: WtRow): [label: string, cmd: string][] {
  const w = r.info;
  if (!w || !w.dirty) return [];
  const dir = dirOf(r);
  return [
    [t("wt.fix.see"), `git -C ${dir} status --short`],
    [t("wt.fix.commit"), `git -C ${dir} commit -am "wip"`],
  ];
}

/** Totals by status, and the filter a status tile or chip applies. */
export function tally(rows: WtRow[]) {
  const by: Record<WtKey, number> = { ready: 0, look: 0, work: 0, main: 0 };
  const count: Record<WtKey, number> = { ready: 0, look: 0, work: 0, main: 0 };
  for (const r of rows) {
    by[r.key] += r.bytes;
    count[r.key]++;
  }
  return { by, count, total: rows.reduce((n, r) => n + r.bytes, 0), agents: rows.filter((r) => r.info?.agent).length };
}

export const shown = (r: WtRow, f: WtFilter) => f === "all" || (f === "agent" ? !!r.info?.agent : r.key === f);

/** Repositories, biggest first; rows in each biggest first. */
export function reposOf(rows: WtRow[]) {
  const m = new Map<string, WtRow[]>();
  for (const r of rows) m.set(r.repo, [...(m.get(r.repo) ?? []), r]);
  const size = (l: WtRow[]) => l.reduce((n, r) => n + r.bytes, 0);
  return [...m.entries()].map(([repo, l]) => ({ repo, root: l[0].root, rows: [...l].sort((a, b) => b.bytes - a.bytes), bytes: size(l) })).sort((a, b) => b.bytes - a.bytes);
}

/** The badge on the tab: how many are Ready. */
export const readyCount = (view: WorktreesView | null) => (view ? view.items.filter((s) => s.worktree?.status === "ready").length : 0);
