// The Quick questions tab (prototype `renderTidy()`): one question card, the
// protected list, and Everything I found. Every choice goes to the backend,
// which owns the pile and the decisions.
import { useState } from "react";
import { GROUPS, RISK_LABEL, gb, plural } from "../../lib/format";
import type { ItemDecision, PileState, Suggestion } from "../../lib/types";
import { questionDeck } from "../home/homeModel";
import { age, cmdFor } from "../cleanup/cleanupModel";
import { Parts } from "./Parts";
import { RiskBox } from "./RiskBox";

export interface Choices {
  add(ids: string[]): void;
  takeOut(id: string): void;
  decide(id: string, d: ItemDecision): void;
  parts(id: string, partIds: string[]): void;
  /** Remove now and Remove all: open a confirmation for these items. */
  remove(ids: string[]): void;
  /** Review and toss: open the confirmation for the pile. */
  toss(): void;
}

function Question({ items, pile, act }: { items: Suggestion[]; pile: PileState; act: Choices }) {
  const deck = questionDeck(pile, items);
  const [acks, setAcks] = useState<Record<string, boolean>>({});
  const q = deck[0];
  if (!q) {
    const laters = items.filter((s) => s.decision === "later");
    return (
      <div className="card stack">
        <h2>All answered</h2>
        <p className="muted">You have no questions left. Your answers are in the crate pile above.</p>
        {laters.length > 0 && (
          <button className="btn" data-act="bring-back" onClick={() => laters.forEach((s) => act.decide(s.id, null))}>
            {`Ask me about ${laters.length} I skipped`}
          </button>
        )}
      </div>
    );
  }
  const acked = !q.needsConfirmation || !!acks[q.id];
  return (
    <div className="card q">
      <div className="row between">
        <span className="note">{`${deck.length} left, biggest first`}</span>
        <span className={"pill " + q.risk}>{RISK_LABEL[q.risk]}</span>
      </div>
      <h2>{q.title}</h2>
      <div className="q-size num">{gb(q.sizeBytes)}</div>
      <p>{q.reason}</p>
      <RiskBox s={q} open={q.risk === "risky"} />
      <dl className="facts">
        <dt>Where</dt>
        <dd className="mono">{q.location}</dd>
        <dt>Last used</dt>
        <dd>{q.lastUsedLabel}</dd>
        <dt>Would run</dt>
        <dd className="mono">{q.actionLabel}</dd>
        <dt>To get it back</dt>
        <dd>{q.recovery}</dd>
      </dl>
      {q.needsConfirmation && (
        <label className="ack">
          <input type="checkbox" id={`ack-${q.id}`} data-ack={q.id} checked={!!acks[q.id]} onChange={(e) => setAcks((a) => ({ ...a, [q.id]: e.target.checked }))} />
          I understand this cannot be recovered.
        </label>
      )}
      <Parts item={q} ctx="q" onChange={(ids) => act.parts(q.id, ids)} />
      <div className="q-actions">
        <button className="btn primary" id="q-toss" data-q="toss" disabled={!acked} onClick={() => act.add([q.id])}>
          Add to pile
        </button>
        <button className="btn" data-q="keep" onClick={() => act.decide(q.id, "keep")}>
          {"Keep, don\u2019t ask again"}
        </button>
        <button className="btn" data-q="later" onClick={() => act.decide(q.id, "later")}>
          Ask me later
        </button>
      </div>
    </div>
  );
}

const tagFor = (s: Suggestion) => (s.blockedReason ? "Protected" : s.decision === "keep" ? "Kept" : s.skipReason ? "Changed, scan again" : RISK_LABEL[s.risk]);

/** Items a whole-group action may include: not blocked, kept, changed, or
 * needing their own confirmation (spec "Item-level controls"). */
export const usable = (s: Suggestion) => !s.blockedReason && s.decision !== "keep" && !s.skipReason && !s.needsConfirmation && s.sizeBytes > 0 && s.action !== "none";

const RCLS = { safe: "safe", review: "review", risky: "risky" } as const;

const LOCK = (
  <svg className="lockic" viewBox="0 0 12 12" aria-hidden="true">
    <rect x="1.5" y="5" width="9" height="6.5" rx="1.5" fill="var(--risky)" />
    <path d="M3.5 5V3.6a2.5 2.5 0 0 1 5 0V5" fill="none" stroke="var(--risky)" strokeWidth="1.5" />
  </svg>
);

/** One quiet line per item; the details, choices and actions open on
 * demand (prototype `rowHTML`). `max` scales the size bar. */
export function Row({ s, ctx, chosen, act, max, startOpen = false }: { s: Suggestion; ctx: string; chosen: Set<string>; act: Choices; max: number; startOpen?: boolean }) {
  const [open, setOpen] = useState(startOpen);
  const can = !s.blockedReason && !s.skipReason && s.action !== "none";
  const kept = s.decision === "keep";
  const on = s.parts.filter((p) => p.selected).length;
  const pct = max ? Math.max(3, (s.sizeBytes / max) * 100) : 0;
  const key = `${ctx}-${s.id}`;
  return (
    <div className="irow">
      {can && !kept ? (
        <input
          type="checkbox"
          id={`${ctx}-pile-${s.id}`}
          data-pile={s.id}
          aria-label={`Add ${s.title} to the pile`}
          checked={chosen.has(s.id)}
          disabled={s.sizeBytes <= 0}
          onChange={(e) => (e.target.checked ? act.add([s.id]) : act.takeOut(s.id))}
        />
      ) : s.blockedReason ? (
        LOCK
      ) : (
        <span></span>
      )}
      <button className="irow-main" id={`${ctx}-x-${s.id}`} data-expand={key} aria-expanded={open} onClick={() => setOpen((v) => !v)}>
        <span className="iname">{s.title}</span>
        <span className="imeta">
          <span className={`rdot ${RCLS[s.risk]}`}></span>
          <span>{tagFor(s)}</span>
          <span>·</span>
          <span>{age(s.lastUsedLabel)}</span>
          {s.parts.length > 0 && on < s.parts.length && (
            <>
              <span>·</span>
              <span>{`${on} of ${s.parts.length} chosen`}</span>
            </>
          )}
        </span>
      </button>
      <span className={`isize num ${RCLS[s.risk]}`}>
        {gb(s.sizeBytes)}
        <span className="szbar" aria-hidden="true">
          <i style={{ width: pct.toFixed(1) + "%" }}></i>
        </span>
      </span>
      {open && (
        <div className="idetail">
          <dl className="facts">
            <dt>Why</dt>
            <dd>{s.reason}</dd>
            <dt>Where</dt>
            <dd className="mono">{s.location}</dd>
            <dt>Would run</dt>
            <dd className="mono">{cmdFor(s)}</dd>
            <dt>To get it back</dt>
            <dd>{s.recovery}</dd>
          </dl>
          <RiskBox s={s} open={false} />
          {can && !kept && <Parts item={s} ctx={ctx} onChange={(ids) => act.parts(s.id, ids)} />}
          {s.blockedReason ? (
            <p className="note">{s.blockedReason}</p>
          ) : kept ? (
            <div className="iactions">
              <button className="btn" id={`${ctx}-keep-${s.id}`} data-keep={s.id} onClick={() => act.decide(s.id, null)}>
                Stop keeping
              </button>
            </div>
          ) : s.skipReason ? (
            <p className="note">It changed after the scan. Scan again to remove it.</p>
          ) : (
            can && (
              <div className="iactions">
                <button className="btn primary" id={`${ctx}-now-${s.id}`} data-now={s.id} disabled={s.sizeBytes <= 0} onClick={() => act.remove([s.id])}>
                  {`Remove ${gb(s.sizeBytes)} now`}
                </button>
                <button className="btn" id={`${ctx}-keep-${s.id}`} data-keep={s.id} onClick={() => act.decide(s.id, "keep")}>
                  {"Keep, don\u2019t suggest"}
                </button>
              </div>
            )
          )}
        </div>
      )}
    </div>
  );
}

/** Whole-group actions as two quiet links (prototype `areaButtons`). */
export function GroupActions({ items, slug, act }: { items: Suggestion[]; slug: string; act: Choices }) {
  const l = items.filter(usable);
  if (!l.length) return null;
  return (
    <div className="grp-tools">
      <button className="link small" id={`add-${slug}`} data-add={slug} onClick={() => act.add(l.map((s) => s.id))}>
        {`Add all ${l.length} to the pile (${gb(l.reduce((t, s) => t + s.sizeBytes, 0))})`}
      </button>
      <button className="link small" id={`clean-${slug}`} data-clean={slug} onClick={() => act.remove(l.map((s) => s.id))}>
        {`Remove all ${l.length}…`}
      </button>
    </div>
  );
}

/** The pile's total under the list, with the way to toss it (prototype `pileBar`). */
function PileBar({ pile, act }: { pile: PileState; act: Choices }) {
  if (!pile.itemIds.length) return null;
  return (
    <div className="pilebar">
      <span className="num">
        <b>{`${pile.itemIds.length} in the pile`}</b>
        {` · ${gb(pile.totalBytes)}`}
      </span>
      <button className="btn primary" id="pilebar-go" data-act="confirm" onClick={act.toss}>
        Review and toss
      </button>
    </div>
  );
}

/** Items Freeboard will not touch. The prototype shows one card per item;
 * a real disk can have dozens, so with more than one they share a single
 * card and the list is folded (found by the user, 2026-11-06). */
function Protected({ items }: { items: Suggestion[] }) {
  if (!items.length) return null;
  if (items.length === 1) {
    const s = items[0];
    return (
      <div className="protect">
        <b>
          {s.title} <span className="num">{gb(s.sizeBytes)}</span>
        </b>
        <span>{`${s.reason} I will not touch it.`}</span>
        <span className="muted">{s.blockedReason}</span>
      </div>
    );
  }
  const total = items.reduce((t, s) => t + s.sizeBytes, 0);
  const sorted = [...items].sort((a, b) => b.sizeBytes - a.sizeBytes);
  return (
    <div className="protect" id="protected">
      <b>
        {`${items.length} things I will not touch`} <span className="num">{gb(total)}</span>
      </b>
      <span className="muted">They may hold work that exists only here, such as uncommitted or unpushed changes. Commit and push them, and I can clean them next time.</span>
      <details className="parts" data-open="protected">
        <summary>Show them</summary>
        {sorted.map((s) => (
          <div className="detail-row" key={s.id}>
            <span>
              {s.title}
              <span className="sub muted" style={{ display: "block", fontSize: 12.5 }}>
                {s.reason}
              </span>
            </span>
            <span className="num">{gb(s.sizeBytes)}</span>
          </div>
        ))}
      </details>
    </div>
  );
}

function Everything({ items, pile, chosen, act }: { items: Suggestion[]; pile: PileState; chosen: Set<string>; act: Choices }) {
  const total = items.reduce((t, s) => t + s.sizeBytes, 0);
  const blocked = items.filter((s) => s.blockedReason);
  return (
    <div className="stack">
      <Protected items={blocked} />
      <div className="card">
        <h3>Everything I found</h3>
        <p className="note" style={{ marginBottom: 6 }}>
          {items.length ? `${plural(items.length, "item")}, ${gb(total)} in total. Tick to add to the pile. Open a row for details.` : "Nothing yet. Run a scan from Settings, or wait for the next one."}
        </p>
        {GROUPS.map(({ id, label }) => {
          const l = items.filter((s) => s.group === id).sort((a, b) => b.sizeBytes - a.sizeBytes);
          return { id, label, l, bytes: l.reduce((t, s) => t + s.sizeBytes, 0) };
        })
          // Biggest group first.
          .sort((a, b) => b.bytes - a.bytes)
          .map(({ id, label, l, bytes }) => {
          if (!l.length) return null;
          const key = "grp-" + label.replace(/ /g, "-");
          return (
            <details className="grp" data-open={key} key={id}>
              <summary>
                <span>
                  {label}{" "}
                  <span className="muted" style={{ fontWeight: 600 }}>
                    {l.length}
                  </span>
                </span>
                <span className="num muted">{gb(bytes)}</span>
              </summary>
              <GroupActions items={l} slug={key} act={act} />
              {l.map((s) => (
                <Row key={s.id} s={s} ctx="l" chosen={chosen} act={act} max={l[0].sizeBytes} />
              ))}
            </details>
          );
        })}
        <PileBar pile={pile} act={act} />
      </div>
    </div>
  );
}

export function QuickQuestions({ items, pile, act }: { items: Suggestion[]; pile: PileState; act: Choices }) {
  const chosen = new Set(pile.itemIds);
  return (
    <>
      <Question items={items} pile={pile} act={act} />
      <Everything items={items} pile={pile} chosen={chosen} act={act} />
    </>
  );
}

