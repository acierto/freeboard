// The Worktrees tab (prototype Version 19 `renderWT`, spec "Worktrees view"):
// a summary card with the rule, a bar and four status tiles that filter, one
// card per repository, a row per worktree that opens in place, and a dock to
// remove the selected ones. Removal runs as a queue in the dock (prototype
// Version 20, spec "Removal queue"): one worktree at a time, each through its
// own confirmed plan and run (checked again just before, 120 s limit), while
// the list stays usable: ticked worktrees join the end of the queue, a
// waiting one can be taken out, and Pause stops after the current one.
import { Fragment, useEffect, useRef, useState } from "react";
import { api, errorText, on } from "../../lib/api";
import { gb } from "../../lib/format";
import { track } from "../../lib/useBusy";
import type { WorktreesView as View } from "../../lib/types";
import { t } from "../../i18n";
import { tb } from "../../i18n/backend";
import { WT_COLOR, WT_KEYS, acknowledgeable, branchGoes, checksOf, fixesOf, idleShort, reposOf, rowsOf, selectable, shown, statusLabel, tally, whyOf, type WtFilter, type WtRow } from "./worktreesModel";

const BRANCH_ICON = (
  <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
    <circle cx="4" cy="3" r="1.8" fill="none" stroke="currentColor" strokeWidth="1.5" />
    <circle cx="4" cy="13" r="1.8" fill="none" stroke="currentColor" strokeWidth="1.5" />
    <circle cx="12" cy="5" r="1.8" fill="none" stroke="currentColor" strokeWidth="1.5" />
    <path d="M4 5v6M12 7c0 3-8 2-8 4" fill="none" stroke="currentColor" strokeWidth="1.5" />
  </svg>
);

/** The removal queue (spec "Removal queue"). */
interface Queue {
  /** Waiting, in order. */
  queue: string[];
  cur: string | null;
  done: string[];
  failed: { id: string; reason: string; short: string }[];
  freed: number;
  paused: boolean;
  finished: boolean;
  /** Rows as they were when queued, so removed ones stay visible until Close. */
  rows: Record<string, WtRow>;
  /** Worktrees whose files the user agreed to lose, as of queueing. */
  acked: string[];
  /** Row order when the queue started (removed rows keep their place). */
  order: string[];
}

const REMOVED = ["done", "moved_to_trash"];

/** One row's place in the queue, if it has one. */
type QState = { k: "done" } | { k: "cur" } | { k: "queued"; pos: number } | { k: "failed"; reason: string } | null;

function qStateOf(q: Queue | null, id: string): QState {
  if (!q) return null;
  if (q.done.includes(id)) return { k: "done" };
  if (q.cur === id) return { k: "cur" };
  const pos = q.queue.indexOf(id);
  if (pos > -1) return { k: "queued", pos: pos + 1 };
  const f = q.failed.find((x) => x.id === id);
  return f ? { k: "failed", reason: f.reason } : null;
}

const inQueue = (q: Queue | null, id: string) => !!q && (q.cur === id || q.queue.includes(id) || q.done.includes(id));

/** Why a removal did not happen, in the row's words. */
const failShort = (reason: string | null) => (!reason || /^It took longer than/.test(reason) ? t("wt.q.timedoutS") : tb(reason));
const failText = (reason: string | null) => (!reason || /^It took longer than/.test(reason) ? t("wt.q.timedout") : tb(reason));

function Detail({ r, acked, onAck }: { r: WtRow; acked: boolean; onAck: (v: boolean) => void }) {
  if (!r.info) {
    return (
      <div className="wt-detail">
        <p className="note">{t("wt.mainnote")}</p>
      </div>
    );
  }
  const w = r.info;
  const dir = r.root + (w.relPath ? "/" + w.relPath : "");
  const fixes = fixesOf(r);
  const files = w.dirtyFiles.slice(0, 3);
  const more = w.dirty - files.length;
  return (
    <div className="wt-detail">
      <div className="wt-cols">
        <div>
          <div className="rx-sub">{t("wt.checked")}</div>
          <ul className="wt-checks">
            {checksOf(w).map(([ok, text]) => (
              <li key={text} className={ok ? "ok" : "no"}>
                {text}
              </li>
            ))}
          </ul>
          {w.dirty > 0 && (
            <div className="wt-files">
              <span className="note">{t("wt.uncommitted")}</span>{" "}
              {[...files, ...(more > 0 ? [t("wt.moreN", { n: more })] : [])].map((f, i) => (
                <span key={f}>
                  {i > 0 && ", "}
                  <span className="mono">{f}</span>
                </span>
              ))}
            </div>
          )}
        </div>
        <div>
          <div className="rx-sub">{t("f.where")}</div>
          <p className="mono wt-path">{dir}</p>
          <div className="rx-sub">{t("wt.after")}</div>
          <p className="wt-after">
            {branchGoes(w) ? (
              t("wt.after.goes")
            ) : !w.branch ? (
              t("wt.after.detached")
            ) : (
              <>
                {t("wt.after.stays1")}
                <span className="mono">{w.branch ?? r.name}</span>
                {w.ahead ? t(w.ahead === 1 ? "wt.after.stays2.1" : "wt.after.stays2.N", { n: w.ahead }) : t("wt.after.stays2")}
                {w.reopen && <span className="mono">{w.reopen}</span>}
                {w.reopen && "."}
              </>
            )}
          </p>
          <div className="rx-sub">{t("f.run")}</div>
          <code className="mono wt-cmd">{w.commands.join("\n")}</code>
          {fixes.length > 0 && (
            <>
              <div className="rx-sub">{t("wt.fix")}</div>
              <ol className="rx-steps">
                {fixes.map(([label, cmd]) => (
                  <li key={cmd}>
                    <span>{label}</span>
                    <code className="mono">{cmd}</code>
                  </li>
                ))}
              </ol>
            </>
          )}
        </div>
      </div>
      {acknowledgeable(r) && (
        <label className="ack">
          <input type="checkbox" data-wt-ack={r.id} checked={acked} onChange={(e) => onAck(e.target.checked)} />
          {" " + (w.dirty === 1 ? t("wt.ack1") : t("wt.ackN", { n: w.dirty }))}
        </label>
      )}
    </div>
  );
}

function Row({ r, open, onOpen, checked, onCheck, qs, onTakeOut, acked, onAck }: { r: WtRow; open: boolean; onOpen: () => void; checked: boolean; onCheck: (v: boolean) => void; qs: QState; onTakeOut: () => void; acked: boolean; onAck: (v: boolean) => void }) {
  const agent = r.info?.agent;
  const done = qs?.k === "done";
  const cls = `wt-row ${r.key}${open && !done ? " open" : ""}${qs?.k === "cur" ? " busy" : ""}${qs?.k === "queued" ? " queued" : ""}${done ? " removed" : ""}`;
  let lead;
  if (r.key === "main") lead = <span className="wt-cb" aria-hidden="true"></span>;
  else if (done)
    lead = (
      <span className="wt-state ok" role="img" aria-label={t("wt.q.removedA")}>
        {"✓"}
      </span>
    );
  else if (qs?.k === "cur")
    lead = (
      <span className="wt-state">
        <span className="spin"></span>
      </span>
    );
  else if (qs?.k === "queued")
    lead = (
      <span className="wt-state q num" role="img" aria-label={t("wt.q.posA", { n: qs.pos })}>
        {qs.pos}
      </span>
    );
  else lead = <input type="checkbox" className="wt-cb" data-wt-sel={r.id} aria-label={t("wt.select", { x: r.name })} checked={checked && selectable(r, acked)} disabled={!selectable(r, acked)} onChange={(e) => onCheck(e.target.checked)} />;
  const why = done ? (r.info && branchGoes(r.info) ? t("wt.q.removedGone") : t("wt.q.removedKept")) : qs?.k === "cur" ? t("wt.q.now1") : qs?.k === "queued" ? t("wt.q.pos", { n: qs.pos }) : qs?.k === "failed" ? qs.reason : whyOf(r);
  const dot = done ? "var(--safe)" : qs?.k === "cur" || qs?.k === "queued" ? "var(--accent)" : qs?.k === "failed" ? "var(--risky)" : WT_COLOR[r.key];
  return (
    <div className={cls}>
      <div className="wt-line">
        {lead}
        <button className="wt-main" data-wt-open={r.id} aria-expanded={open && !done} disabled={done} onClick={onOpen}>
          <span className="wt-b">
            {BRANCH_ICON}
            <span className="mono">{r.name}</span>
            {agent && <span className="wt-ag">{t("wt.ag." + agent)}</span>}
            {r.info?.locked && <span className="wt-ag lock">{t("wt.locked")}</span>}
          </span>
          <span className="wt-why">
            <span className="wt-dot" style={{ background: dot }}></span>
            {why}
          </span>
        </button>
        {qs?.k === "queued" ? (
          <button className="link small wt-unq" data-wt-unq={r.id} onClick={onTakeOut}>
            {t("wt.q.takeout")}
          </button>
        ) : (
          <span className="wt-meta num">
            <b>{gb(r.bytes)}</b>
            <small>{done ? t("wt.q.freed") : r.info ? idleShort(r.info.idleDays) : ""}</small>
          </span>
        )}
      </div>
      {open && !done && <Detail r={r} acked={acked} onAck={onAck} />}
    </div>
  );
}

export function WorktreesView({ view, onChanged, scanning }: { view: View | null; onChanged: () => void; scanning: boolean }) {
  const [filter, setFilter] = useState<WtFilter>("all");
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [open, setOpen] = useState<Set<string>>(new Set());
  // Worktrees whose uncommitted files the user agreed to lose (in the row).
  const [acks, setAcks] = useState<Set<string>>(new Set());
  const [q, setQ] = useState<Queue | null>(null);
  const [qOpen, setQOpen] = useState(false);
  const qRef = useRef<Queue | null>(null);
  qRef.current = q;
  const changed = useRef(onChanged);
  changed.current = onChanged;
  /** Runs waiting for their finish event. */
  const waiters = useRef(new Map<string, () => void>());

  useEffect(() => {
    const offs = [on("cleanup_finished", (u) => waiters.current.get(u.runId)?.())];
    return () => offs.forEach((p) => p.then((off) => off()));
  }, []);

  /** Apply a change to the queue at once, so the next step sees it. */
  const update = (f: (q: Queue) => Queue) => {
    if (!qRef.current) return;
    qRef.current = f(qRef.current);
    setQ(qRef.current);
  };

  /** Remove one worktree: its own confirmed plan and run, then the next. */
  const step = async () => {
    const cur = qRef.current;
    if (!cur || cur.cur || cur.paused) return;
    if (!cur.queue.length) {
      if (!cur.finished) update((x) => ({ ...x, finished: true }));
      return;
    }
    const id = cur.queue[0];
    update((x) => ({ ...x, cur: id, queue: x.queue.slice(1) }));
    let removed = false;
    let reason: string | null = null;
    try {
      let plan = await track(api.createPlan([id]));
      for (const a of plan.needsAckIds) if (qRef.current?.acked.includes(a)) plan = await track(api.acknowledgeItem(plan.id, a));
      const runId = await track(api.runPlan(plan.id));
      const finished = new Promise<void>((res) => waiters.current.set(runId, res));
      let detail = await track(api.getRun(runId));
      if (detail.state === "running") {
        await finished;
        detail = await track(api.getRun(runId));
      }
      waiters.current.delete(runId);
      const items = detail.items.filter((i) => (i.parentId ?? i.itemId) === id);
      removed = items.length > 0 && items.every((i) => REMOVED.includes(i.state));
      reason = items.find((i) => !REMOVED.includes(i.state))?.reason ?? null;
    } catch (e) {
      reason = errorText(e);
    }
    update((x) => {
      const bytes = x.rows[id]?.bytes ?? 0;
      return removed
        ? { ...x, cur: null, done: [...x.done, id], failed: x.failed.filter((f) => f.id !== id), freed: x.freed + bytes }
        : { ...x, cur: null, failed: [...x.failed.filter((f) => f.id !== id), { id, reason: failText(reason), short: failShort(reason) }] };
    });
    changed.current();
    void step();
  };

  if (!view) return null;
  const live = rowsOf(view);
  const { by, count, total, agents } = tally(live);
  // Removed rows stay visible, struck through, until the dock is closed.
  const kept = q ? q.done.filter((id) => !live.some((r) => r.id === id)).map((id) => q.rows[id]) : [];
  const place = (id: string) => {
    const i = q ? q.order.indexOf(id) : -1;
    return i < 0 ? Number.MAX_SAFE_INTEGER : i;
  };
  const rows = kept.length ? [...live, ...kept].sort((a, b) => place(a.id) - place(b.id)) : live;
  const repos = reposOf(rows);
  const running = !!q && !q.finished;
  const chosen = live.filter((r) => sel.has(r.id) && selectable(r, acks.has(r.id)) && !inQueue(q, r.id));
  const chosenBytes = chosen.reduce((n, r) => n + r.bytes, 0);
  const toggle = (set: Set<string>, id: string, on: boolean) => {
    const n = new Set(set);
    if (on) n.add(id);
    else n.delete(id);
    return n;
  };
  const pick = (f: WtFilter) => setFilter(filter === f && f !== "all" ? "all" : f);
  const rowOf = (id: string) => rows.find((r) => r.id === id) ?? q?.rows[id];

  /** Put worktrees at the end of the queue (biggest first), starting one if needed. */
  const enqueue = (list: WtRow[]) => {
    const ids = [...list].sort((a, b) => b.bytes - a.bytes).map((r) => r.id);
    const snap = Object.fromEntries(list.map((r) => [r.id, r]));
    const base = qRef.current;
    const fresh = !base || base.finished;
    const next: Queue = fresh
      ? { queue: ids, cur: null, done: [], failed: [], freed: 0, paused: false, finished: false, rows: snap, acked: [...acks], order: rowsOf(view).map((r) => r.id) }
      : { ...base, queue: [...base.queue, ...ids.filter((id) => !inQueue(base, id))], failed: base.failed.filter((f) => !ids.includes(f.id)), rows: { ...base.rows, ...snap }, acked: [...new Set([...base.acked, ...acks])] };
    qRef.current = next;
    setQ(next);
    setSel((s) => new Set([...s].filter((id) => !ids.includes(id))));
    void step();
  };

  const retry = () => {
    const ids = q?.failed.map((f) => f.id) ?? [];
    update((x) => ({ ...x, queue: [...x.queue, ...ids], failed: [], finished: false }));
    void step();
  };

  const n = chosen.length;
  const addBtn = n > 0 && (
    <button className="btn primary" data-wt-act={running ? "enqueue" : "remove"} onClick={() => enqueue(chosen)}>
      {running ? t("wt.q.add", { n }) : n === 1 ? t("wt.remove1") : t("wt.removeN", { n })} <span className="num dim">{gb(chosenBytes)}</span>
    </button>
  );

  let dock = null;
  if (!q) {
    dock = n > 0 && (
      <div className="wt-dock" role="region" aria-label={t("wt.dock")}>
        <span>
          <b className="num">{n}</b>
          {` ${t("wt.selected")} · `}
          <b className="num">{gb(chosenBytes)}</b>
        </span>
        <button className="link small" data-wt-act="clear" onClick={() => setSel(new Set())}>
          {t("wt.clear")}
        </button>
        {addBtn}
      </div>
    );
  } else {
    const handled = q.done.length + q.failed.length;
    const all = handled + q.queue.length + (q.cur ? 1 : 0);
    const wait = q.queue.length;
    const cur = q.cur ? rowOf(q.cur) : null;
    let head;
    let sub;
    if (q.finished) {
      head = (
        <>
          <b>{t("wt.q.doneh")}</b>
          {" " + (q.failed.length ? t("wt.q.doneF", { a: q.done.length, b: q.failed.length }) : t("wt.q.done", { a: q.done.length }))}
        </>
      );
      sub = q.failed.length
        ? q.failed.map((f, i) => (
            <Fragment key={f.id}>
              {i > 0 && " · "}
              <span className="mono">{rowOf(f.id)?.name ?? f.id}</span>
              {" " + f.short}
            </Fragment>
          ))
        : t("wt.q.logbook");
    } else if (q.paused && !q.cur) {
      head = (
        <>
          <b>{t("wt.q.pausedh")}</b>
          {" " + t("wt.q.paused", { a: handled, b: all })}
        </>
      );
      sub = t("wt.q.nothalf", { n: wait });
    } else {
      head = (
        <>
          <span className="spin" aria-hidden="true"></span>
          {t("wt.q.removing") + " "}
          <span className="mono">{cur?.name ?? ""}</span> <span className="muted">{`(${cur?.repo ?? ""})`}</span>
        </>
      );
      sub = t("wt.q.of", { a: handled + 1, b: all }) + " · " + (wait ? t("wt.q.waiting", { n: wait }) : t("wt.q.last")) + (q.paused ? " · " + t("wt.q.pausing") : "");
    }
    dock = (
      <div className={`wt-dock run${q.finished ? " fin" : ""}`} role="region" aria-label={t("wt.q.region")}>
        <div className="wt-dock-top">
          <div className="wt-now" role="status" aria-live="polite">
            <div>{head}</div>
            <small className="num">{sub}</small>
          </div>
          <span className="num wt-freed">
            <b>{gb(q.freed)}</b> {t("wt.q.freed")}
          </span>
        </div>
        <div className="wt-prog2" aria-hidden="true">
          <i className="ok" style={{ width: (all ? (q.done.length / all) * 100 : 0).toFixed(1) + "%" }}></i>
          <i className="bad" style={{ width: (all ? (q.failed.length / all) * 100 : 0).toFixed(1) + "%" }}></i>
        </div>
        {qOpen && wait > 0 && (
          <ol className="wt-qlist">
            {q.queue.map((id, k) => {
              const r = rowOf(id);
              return (
                <li key={id}>
                  <span className="num qn">{k + 1}</span>
                  <span className="mono">{r?.name ?? id}</span>
                  <span className="muted">{r?.repo ?? ""}</span>
                  <span className="num">{gb(r?.bytes ?? 0)}</span>
                  <button className="icon-x small" data-wt-unq={id} aria-label={t("wt.q.takeoutA", { x: r?.name ?? id })} onClick={() => update((x) => ({ ...x, queue: x.queue.filter((y) => y !== id) }))}>
                    {"×"}
                  </button>
                </li>
              );
            })}
          </ol>
        )}
        <div className="wt-dock-acts">
          {n > 0 && !q.finished && <span className="note num">{t("wt.q.selected", { n })}</span>}
          {q.finished ? (
            <>
              {q.failed.length > 0 && (
                <button className="btn small" data-wt-act="retry" onClick={retry}>
                  {t("cs.retry", { n: q.failed.length })}
                </button>
              )}
              {addBtn}
              <button
                className="link small"
                data-wt-act="dismiss"
                onClick={() => {
                  setQ(null);
                  qRef.current = null;
                  setQOpen(false);
                }}
              >
                {t("cs.close")}
              </button>
            </>
          ) : (
            <>
              {wait > 0 && (
                <button className="link small" data-wt-act="qtoggle" aria-expanded={qOpen} onClick={() => setQOpen(!qOpen)}>
                  {qOpen ? t("wt.q.hide") : t("wt.q.show")}
                </button>
              )}
              <button
                className="btn small"
                data-wt-act={q.paused ? "resume" : "pause"}
                onClick={() => {
                  update((x) => ({ ...x, paused: !x.paused }));
                  void step();
                }}
              >
                {q.paused ? t("wt.q.resume") : t("wt.q.pause")}
              </button>
              {addBtn}
            </>
          )}
        </div>
      </div>
    );
  }

  return (
    <>
      <div className="card wt-top">
        <div className="wt-head">
          <div>
            <h2>{t("tab.wt")}</h2>
            <p className="muted num">{t("wt.sum", { n: rows.length, r: repos.length, x: gb(total) })}</p>
          </div>
          <p className="wt-rule">{t("wt.rule")}</p>
        </div>
        <div className="wt-bar" aria-hidden="true">
          {WT_KEYS.filter((k) => by[k] > 0).map((k) => (
            <i key={k} style={{ flex: (by[k] / 1e9).toFixed(2), background: WT_COLOR[k] }} title={`${statusLabel(k)} ${gb(by[k])}`}></i>
          ))}
        </div>
        <div className="wt-legend">
          {WT_KEYS.map((k) => (
            <button key={k} className={filter === k ? "wt-leg on" : "wt-leg"} data-wt-filter={k} aria-pressed={filter === k} onClick={() => pick(k)}>
              <span className="sw" style={{ background: WT_COLOR[k] }}></span>
              <b>{statusLabel(k)}</b>
              <span className="num">{`${count[k]} · ${gb(by[k])}`}</span>
            </button>
          ))}
        </div>
        <div className="wt-filters">
          <button className={filter === "all" ? "chipbtn on" : "chipbtn"} data-wt-filter="all" aria-pressed={filter === "all"} onClick={() => pick("all")}>
            {t("wt.all")}
          </button>
          <button className={filter === "agent" ? "chipbtn on" : "chipbtn"} data-wt-filter="agent" aria-pressed={filter === "agent"} onClick={() => pick("agent")}>
            {t("wt.agents", { n: agents })}
          </button>
        </div>
        {running && (
          <p className="wt-hint">
            <span className="spin" aria-hidden="true"></span>
            {t("wt.q.bg")}
          </p>
        )}
      </div>
      {view.stale > 0 && (
        <div className="card stack" id="wt-stale">
          <p>{view.stale === 1 ? t("wt.stale1") : t("wt.staleN", { n: view.stale })}</p>
          <div className="row">
            <button className="btn primary" data-act="wt-sort" disabled={scanning} onClick={() => void track(api.startScan("deep")).catch(() => undefined)}>
              {scanning ? t("wt.sorting") : t("wt.sort")}
            </button>
          </div>
        </div>
      )}
      {!rows.length && !view.stale && <p className="muted">{t("wt.none")}</p>}
      {repos.map(({ repo, root, rows: withGone }) => {
        const all = withGone.filter((r) => live.includes(r));
        const bytes = all.reduce((n, r) => n + r.bytes, 0);
        const list = withGone.filter((r) => shown(r, filter));
        if (!list.length) return null;
        // Rows already ticked are not offered again (prototype Version 20).
        const ready = all.filter((r) => r.key === "ready" && !sel.has(r.id) && !inQueue(q, r.id));
        return (
          <section className="wt-repo" key={repo}>
            <header className="wt-rh">
              <div>
                <h3>
                  <span className="mono">{repo}</span>
                </h3>
                <span className="note num">
                  {`${all.length === 1 ? t("wt.count1") : t("wt.countN", { n: all.length })} · ${gb(bytes)} · `}
                  <span className="mono">{root}</span>
                </span>
                <div className="wt-mini" aria-hidden="true">
                  {WT_KEYS.map((k) => {
                    const v = all.filter((r) => r.key === k).reduce((n, r) => n + r.bytes, 0);
                    return v ? <i key={k} style={{ flex: (v / 1e9).toFixed(2), background: WT_COLOR[k] }}></i> : null;
                  })}
                </div>
              </div>
              {ready.length > 0 && (
                <button className="btn small" data-wt-selrepo={repo} onClick={() => setSel(new Set([...sel, ...ready.map((r) => r.id)]))}>
                  {t("wt.selready", { n: ready.length })}
                </button>
              )}
            </header>
            {list.map((r) => (
              <Row
                key={r.id}
                r={r}
                open={open.has(r.id)}
                onOpen={() => setOpen(toggle(open, r.id, !open.has(r.id)))}
                checked={sel.has(r.id)}
                onCheck={(v) => setSel(toggle(sel, r.id, v))}
                acked={acks.has(r.id)}
                onAck={(v) => {
                  setAcks(toggle(acks, r.id, v));
                  if (!v) setSel(toggle(sel, r.id, false));
                }}
                qs={qStateOf(q, r.id)}
                onTakeOut={() => update((x) => ({ ...x, queue: x.queue.filter((y) => y !== r.id) }))}
              />
            ))}
          </section>
        );
      })}
      {dock}
    </>
  );
}
