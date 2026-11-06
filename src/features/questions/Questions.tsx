// The Quick questions tab (prototype `renderTidy()`): one question card, the
// protected list, and Everything I found. Every choice goes to the backend,
// which owns the pile and the decisions.
import { useState } from "react";
import { GROUPS, RISK_LABEL, gb, plural } from "../../lib/format";
import type { ItemDecision, PileState, Suggestion } from "../../lib/types";
import { questionDeck } from "../home/homeModel";
import { Parts } from "./Parts";

export interface Choices {
  add(ids: string[]): void;
  takeOut(id: string): void;
  decide(id: string, d: ItemDecision): void;
  parts(id: string, partIds: string[]): void;
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
          Keep, don{"’"}t ask again
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

export function Row({ s, ctx, chosen, act }: { s: Suggestion; ctx: string; chosen: Set<string>; act: Choices }) {
  const can = !s.blockedReason && !s.skipReason && s.action !== "none";
  const kept = s.decision === "keep";
  return (
    <div className="row-item">
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
      ) : (
        <span></span>
      )}
      <span className="main">
        {s.title}
        <span className="sub">{`${tagFor(s)} · ${s.lastUsedLabel}`}</span>
      </span>
      <span className="num size">{gb(s.sizeBytes)}</span>
      {can && !kept && (
        <div className="parts">
          <Parts item={s} ctx={ctx} onChange={(ids) => act.parts(s.id, ids)} />
        </div>
      )}
      {/* Remove now opens the confirmation sheet, which arrives with slice 2 (task 4.3). */}
      {can && !kept && (
        <div className="ctl">
          <button className="mini" id={`${ctx}-keep-${s.id}`} data-keep={s.id} onClick={() => act.decide(s.id, "keep")}>
            Keep
          </button>
        </div>
      )}
      {kept && (
        <div className="ctl">
          <button className="mini" id={`${ctx}-keep-${s.id}`} data-keep={s.id} onClick={() => act.decide(s.id, null)}>
            Stop keeping
          </button>
        </div>
      )}
    </div>
  );
}

export function GroupActions({ items, slug, act }: { items: Suggestion[]; slug: string; act: Choices }) {
  const l = items.filter(usable);
  if (!l.length) return null;
  return (
    <div className="grp-actions">
      <button className="mini" id={`add-${slug}`} data-add={slug} onClick={() => act.add(l.map((s) => s.id))}>
        Add all to pile
      </button>
    </div>
  );
}

function Everything({ items, chosen, act }: { items: Suggestion[]; chosen: Set<string>; act: Choices }) {
  const total = items.reduce((t, s) => t + s.sizeBytes, 0);
  const blocked = items.filter((s) => s.blockedReason);
  return (
    <div className="stack">
      {blocked.map((s) => (
        <div className="protect" key={s.id}>
          <b>
            {s.title} <span className="num">{gb(s.sizeBytes)}</span>
          </b>
          <span>{`${s.reason} I will not touch it.`}</span>
          <span className="muted">{s.blockedReason}</span>
        </div>
      ))}
      <div className="card">
        <h3>Everything I found</h3>
        <p className="note" style={{ marginBottom: 6 }}>
          {items.length ? `${plural(items.length, "item")}, ${gb(total)} in total. Open a group to pick exactly what goes.` : "Nothing yet. Run a scan from Settings, or wait for the next one."}
        </p>
        {GROUPS.map(({ id, label }) => {
          const l = items.filter((s) => s.group === id).sort((a, b) => b.sizeBytes - a.sizeBytes);
          if (!l.length) return null;
          const key = "grp-" + label.replace(/ /g, "-");
          return (
            <details className="grp" data-open={key} key={id}>
              <summary>
                <span>{label}</span>
                <span className="num muted">{gb(l.reduce((t, s) => t + s.sizeBytes, 0))}</span>
              </summary>
              <GroupActions items={l} slug={key} act={act} />
              {l.map((s) => (
                <Row key={s.id} s={s} ctx="l" chosen={chosen} act={act} />
              ))}
            </details>
          );
        })}
      </div>
    </div>
  );
}

export function QuickQuestions({ items, pile, act }: { items: Suggestion[]; pile: PileState; act: Choices }) {
  const chosen = new Set(pile.itemIds);
  return (
    <>
      <Question items={items} pile={pile} act={act} />
      <Everything items={items} chosen={chosen} act={act} />
    </>
  );
}

