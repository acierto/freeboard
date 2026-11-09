// The Worktrees tab (prototype Version 19 `renderWT`, spec "Worktrees view"):
// a summary card with the rule, a bar and four status tiles that filter, one
// card per repository, a row per worktree that opens in place, and a dock to
// remove the selected ones. Removal is the normal confirmed plan and run, one
// worktree at a time with a 120 s limit each; its progress shows in the
// summary card.
import { useEffect, useRef, useState } from "react";
import { api, errorText, on } from "../../lib/api";
import { gb } from "../../lib/format";
import { track } from "../../lib/useBusy";
import type { RunDetail, WorktreesView as View } from "../../lib/types";
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

/** A removal run as the summary card shows it. */
interface Run {
  runId: string | null;
  ids: string[];
  detail: RunDetail | null;
  error: string | null;
}

const FINAL = ["done", "moved_to_trash", "skipped", "failed", "canceled", "interrupted"];

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

function Row({ r, open, onOpen, checked, onCheck, busy, current, acked, onAck }: { r: WtRow; open: boolean; onOpen: () => void; checked: boolean; onCheck: (v: boolean) => void; busy: boolean; current: boolean; acked: boolean; onAck: (v: boolean) => void }) {
  const agent = r.info?.agent;
  return (
    <div className={`wt-row ${r.key}${open ? " open" : ""}${current ? " busy" : ""}`}>
      <div className="wt-line">
        {r.key === "main" ? (
          <span className="wt-cb" aria-hidden="true"></span>
        ) : (
          <input type="checkbox" className="wt-cb" data-wt-sel={r.id} aria-label={t("wt.select", { x: r.name })} checked={checked && selectable(r, acked)} disabled={!selectable(r, acked) || busy} onChange={(e) => onCheck(e.target.checked)} />
        )}
        <button className="wt-main" data-wt-open={r.id} aria-expanded={open} onClick={onOpen}>
          <span className="wt-b">
            {BRANCH_ICON}
            <span className="mono">{r.name}</span>
            {agent && <span className="wt-ag">{t("wt.ag." + agent)}</span>}
            {r.info?.locked && <span className="wt-ag lock">{t("wt.locked")}</span>}
          </span>
          <span className="wt-why">
            <span className="wt-dot" style={{ background: WT_COLOR[r.key] }}></span>
            {current ? t("wt.removing1") : whyOf(r)}
          </span>
        </button>
        <span className="wt-meta num">
          <b>{gb(r.bytes)}</b>
          <small>{r.info ? idleShort(r.info.idleDays) : ""}</small>
        </span>
      </div>
      {open && <Detail r={r} acked={acked} onAck={onAck} />}
    </div>
  );
}

export function WorktreesView({ view, onChanged, scanning }: { view: View | null; onChanged: () => void; scanning: boolean }) {
  const [filter, setFilter] = useState<WtFilter>("all");
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [open, setOpen] = useState<Set<string>>(new Set());
  // Worktrees whose uncommitted files the user agreed to lose (in the row).
  const [acks, setAcks] = useState<Set<string>>(new Set());
  const [run, setRun] = useState<Run | null>(null);
  const runRef = useRef<Run | null>(null);
  runRef.current = run;
  const changed = useRef(onChanged);
  changed.current = onChanged;

  // Follow the run: each item update and the finish reload its record.
  useEffect(() => {
    const refresh = (runId: string) =>
      void track(api.getRun(runId)).then((detail) => {
        const r = runRef.current;
        if (!r || r.runId !== runId) return;
        setRun({ ...r, detail });
        if (detail.state !== "running") changed.current();
      });
    const offs = [
      on("cleanup_item_updated", (u) => runRef.current?.runId === u.runId && refresh(u.runId)),
      on("cleanup_finished", (u) => runRef.current?.runId === u.runId && refresh(u.runId)),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  }, []);

  if (!view) return null;
  const rows = rowsOf(view);
  const { by, count, total, agents } = tally(rows);
  const repos = reposOf(rows);
  const d = run?.detail ?? null;
  const running = !!run && (!d || d.state === "running");
  const chosen = rows.filter((r) => sel.has(r.id) && selectable(r, acks.has(r.id)));
  const chosenBytes = chosen.reduce((n, r) => n + r.bytes, 0);
  const toggle = (set: Set<string>, id: string, on: boolean) => {
    const n = new Set(set);
    if (on) n.add(id);
    else n.delete(id);
    return n;
  };
  const pick = (f: WtFilter) => setFilter(filter === f && f !== "all" ? "all" : f);

  /** The normal confirmed plan and run, for the chosen worktrees, biggest first. */
  const remove = (ids: string[]) => {
    const start: Run = { runId: null, ids, detail: null, error: null };
    setRun(start);
    setSel(new Set());
    void track(api.createPlan(ids))
      // The plan records each acknowledgment the user gave in a row.
      .then(async (plan) => {
        for (const id of plan.needsAckIds) if (acks.has(id)) plan = await track(api.acknowledgeItem(plan.id, id));
        return track(api.runPlan(plan.id));
      })
      .then((runId) => {
        setRun({ ...start, runId });
        return track(api.getRun(runId)).then((detail) => {
          setRun((r) => (r && r.runId === runId ? { ...r, detail } : r));
          if (detail.state !== "running") changed.current();
        });
      })
      .catch((e) => setRun({ ...start, error: tb(errorText(e)) }));
  };

  const items = d?.items ?? [];
  const doneItems = items.filter((i) => i.state === "done");
  const failed = items.filter((i) => i.state === "failed" || i.state === "interrupted" || i.state === "skipped");
  const handled = items.filter((i) => FINAL.includes(i.state)).length;
  const current = items.find((i) => i.state === "checking" || i.state === "running")?.itemId ?? null;
  const freed = doneItems.reduce((n, i) => n + i.expectedBytes, 0);
  const nameOf = (id: string) => rows.find((r) => r.id === id)?.name ?? items.find((i) => i.itemId === id)?.title ?? id;

  const runBox = run && (
    <div className="wt-run" role="status" aria-live="polite">
      <div className="row between">
        <b>{run.error ? t("wt.run.error") : running ? t("wt.run.now", { a: handled, b: run.ids.length }) : failed.length ? t("wt.run.doneF", { a: doneItems.length, b: failed.length }) : t("wt.run.done", { a: doneItems.length })}</b>
        <span className="num">{t("cs.freed", { x: gb(freed) })}</span>
      </div>
      <div className="wt-prog">
        <i style={{ width: ((run.ids.length ? handled / run.ids.length : 0) * 100).toFixed(1) + "%" }}></i>
      </div>
      {running && <p className="note">{t("wt.run.note")}</p>}
      {run.error && <p className="note">{run.error}</p>}
      {!running && failed.length > 0 && (
        <>
          <p className="note">
            {failed.map((i, n) => (
              <span key={i.itemId}>
                {n > 0 && <br />}
                <span className="mono">{nameOf(i.itemId)}</span>
                {`: ${/^It took longer than/.test(i.reason ?? "") ? t("wt.timeout") : tb(i.reason ?? "")}`}
              </span>
            ))}
          </p>
          <div className="row">
            <button className="btn small" data-wt-act="retry" onClick={() => remove(failed.map((i) => i.itemId))}>
              {t("cs.retry", { n: failed.length })}
            </button>
            <button className="link small" data-wt-act="dismiss" onClick={() => setRun(null)}>
              {t("cs.close")}
            </button>
          </div>
        </>
      )}
      {!running && !failed.length && (
        <button className="link small" data-wt-act="dismiss" onClick={() => setRun(null)}>
          {t("cs.close")}
        </button>
      )}
    </div>
  );

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
        {runBox}
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
      {repos.map(({ repo, root, rows: all, bytes }) => {
        const list = all.filter((r) => shown(r, filter));
        if (!list.length) return null;
        // Rows already ticked are not offered again (prototype Version 20).
        const ready = all.filter((r) => r.key === "ready" && !sel.has(r.id));
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
                <button className="btn small" data-wt-selrepo={repo} disabled={running} onClick={() => setSel(new Set([...sel, ...ready.map((r) => r.id)]))}>
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
                busy={running}
                current={current === r.id}
              />
            ))}
          </section>
        );
      })}
      {chosen.length > 0 && !running && (
        <div className="wt-dock" role="region" aria-label={t("wt.dock")}>
          <span>
            <b className="num">{chosen.length}</b>
            {` ${t("wt.selected")} · `}
            <b className="num">{gb(chosenBytes)}</b>
          </span>
          <button className="link small" data-wt-act="clear" onClick={() => setSel(new Set())}>
            {t("wt.clear")}
          </button>
          <button className="btn primary" data-wt-act="remove" onClick={() => remove([...chosen].sort((a, b) => b.bytes - a.bytes).map((r) => r.id))}>
            {chosen.length === 1 ? t("wt.remove1") : t("wt.removeN", { n: chosen.length })} <span className="num dim">{gb(chosenBytes)}</span>
          </button>
        </div>
      )}
    </>
  );
}
