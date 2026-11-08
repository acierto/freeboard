// The Quick questions tab (prototype `renderTidy()`): one question card, the
// protected list, and Everything I found. Every choice goes to the backend,
// which owns the pile and the decisions.
import { isWorktree } from "../worktrees/worktreesModel";
import { areaName, t } from "../../i18n";
import { itemName, tb } from "../../i18n/backend";
import { Fragment, useEffect, useState } from "react";
import { api, errorText } from "../../lib/api";
import { track } from "../../lib/useBusy";
import { GROUPS, RISK_LABEL, gb } from "../../lib/format";
import type { DetectorError, ItemDecision, PileState, Suggestion, TidyRule, TidyRuleId } from "../../lib/types";
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
        <h2>{t("q.all")}</h2>
        <p className="muted">{t("q.none")}</p>
        {laters.length > 0 && (
          <button className="btn" data-act="bring-back" onClick={() => laters.forEach((s) => act.decide(s.id, null))}>
            {t("q.bringback", { n: laters.length })}
          </button>
        )}
      </div>
    );
  }
  const acked = !q.needsConfirmation || !!acks[q.id];
  return (
    <div className="card q">
      <div className="row between">
        <span className="note">{t("q.left", { n: deck.length })}</span>
        <span className={"pill " + q.risk}>{RISK_LABEL[q.risk]}</span>
      </div>
      <h2>{itemName(q)}</h2>
      <div className="q-size num">{gb(q.sizeBytes)}</div>
      <p>{tb(q.reason)}</p>
      <RiskBox s={q} open={q.risk === "risky"} />
      <dl className="facts">
        <dt>{t("f.where")}</dt>
        <dd className="mono">{tb(q.location)}</dd>
        <dt>{t("f.lastused")}</dt>
        <dd>{tb(q.lastUsedLabel)}</dd>
        <dt>{t("f.run")}</dt>
        <dd className="mono">{tb(q.actionLabel)}</dd>
        <dt>{t("f.back")}</dt>
        <dd>{tb(q.recovery)}</dd>
      </dl>
      {q.needsConfirmation && (
        <label className="ack">
          <input type="checkbox" id={`ack-${q.id}`} data-ack={q.id} checked={!!acks[q.id]} onChange={(e) => setAcks((a) => ({ ...a, [q.id]: e.target.checked }))} />
          {t("ack.lost")}
        </label>
      )}
      <Parts item={q} ctx="q" onChange={(ids) => act.parts(q.id, ids)} />
      <div className="q-actions">
        <button className="btn primary" id="q-toss" data-q="toss" disabled={!acked} onClick={() => act.add([q.id])}>
          {t("q.add")}
        </button>
        <button className="btn" data-q="keep" onClick={() => act.decide(q.id, "keep")}>
          {t("q.keep")}
        </button>
        <button className="btn" data-q="later" onClick={() => act.decide(q.id, "later")}>
          {t("q.later")}
        </button>
      </div>
    </div>
  );
}

const tagFor = (s: Suggestion) => (s.blockedReason ? t("tag.protected") : s.decision === "keep" ? t("tag.kept") : s.skipReason ? t("tag.changed") : RISK_LABEL[s.risk]);

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
          aria-label={t("r.pileadd", { x: itemName(s) })}
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
        <span className="iname">{itemName(s)}</span>
        <span className="imeta">
          <span className={`rdot ${RCLS[s.risk]}`}></span>
          <span>{tagFor(s)}</span>
          <span>·</span>
          <span>{age(s.lastUsedLabel)}</span>
          {s.parts.length > 0 && on < s.parts.length && (
            <>
              <span>·</span>
              <span>{t("r.chosen", { on, n: s.parts.length })}</span>
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
            <dt>{t("f.why")}</dt>
            <dd>{tb(s.reason)}</dd>
            <dt>{t("f.where")}</dt>
            <dd className="mono">{tb(s.location)}</dd>
            <dt>{t("f.run")}</dt>
            <dd className="mono">{cmdFor(s)}</dd>
            <dt>{t("f.back")}</dt>
            <dd>{tb(s.recovery)}</dd>
          </dl>
          <RiskBox s={s} open={false} />
          {can && !kept && <Parts item={s} ctx={ctx} onChange={(ids) => act.parts(s.id, ids)} />}
          {s.blockedReason ? (
            <p className="note">{tb(s.blockedReason)}</p>
          ) : kept ? (
            <div className="iactions">
              <button className="btn" id={`${ctx}-keep-${s.id}`} data-keep={s.id} onClick={() => act.decide(s.id, null)}>
                {t("r.stopkeep")}
              </button>
            </div>
          ) : s.skipReason ? (
            <p className="note">{t("r.changed")}</p>
          ) : (
            can && (
              <div className="iactions">
                <button className="btn primary" id={`${ctx}-now-${s.id}`} data-now={s.id} disabled={s.sizeBytes <= 0} onClick={() => act.remove([s.id])}>
                  {t("r.now", { x: gb(s.sizeBytes) })}
                </button>
                <button className="btn" id={`${ctx}-keep-${s.id}`} data-keep={s.id} onClick={() => act.decide(s.id, "keep")}>
                  {t("r.keep")}
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
        {t("g.addall", { n: l.length, x: gb(l.reduce((n, s) => n + s.sizeBytes, 0)) })}
      </button>
      <button className="link small" id={`clean-${slug}`} data-clean={slug} onClick={() => act.remove(l.map((s) => s.id))}>
        {t("g.removeall", { n: l.length })}
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
        <b>{t("pb.inpile", { n: pile.itemIds.length })}</b>
        {` · ${gb(pile.totalBytes)}`}
      </span>
      <button className="btn primary" id="pilebar-go" data-act="confirm" onClick={act.toss}>
        {t("pb.go")}
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
          {itemName(s) + " "}
          <span className="num">{gb(s.sizeBytes)}</span>
        </b>
        <span>{`${tb(s.reason)} ${t("p.notouch")}`}</span>
        <span className="muted">{tb(s.blockedReason)}</span>
      </div>
    );
  }
  const total = items.reduce((t, s) => t + s.sizeBytes, 0);
  const sorted = [...items].sort((a, b) => b.sizeBytes - a.sizeBytes);
  return (
    <div className="protect" id="protected">
      <b>
        {t("p.notouchN", { n: items.length }) + " "}
        <span className="num">{gb(total)}</span>
      </b>
      <span className="muted">{t("p.why")}</span>
      <details className="parts" data-open="protected">
        <summary>{t("p.show")}</summary>
        {sorted.map((s) => (
          <div className="detail-row" key={s.id}>
            <span>
              {itemName(s)}
              <span className="sub muted" style={{ display: "block", fontSize: 12.5 }}>
                {tb(s.reason)}
              </span>
            </span>
            <span className="num">{gb(s.sizeBytes)}</span>
          </div>
        ))}
      </details>
    </div>
  );
}

/** Places the last scan could not check, one line per cause (five Docker
 * detectors share "Docker is not running"). Spec "Scans and detectors": a
 * missing or stopped tool is shown, and the rest continues. */
function Unchecked({ errors }: { errors: DetectorError[] }) {
  const causes = [...new Set(errors.map((e) => e.message))];
  if (!causes.length) return null;
  return (
    <div className="unchecked" data-supplemental="unchecked">
      <b>{t("u.title")}</b>
      {causes.map((m) => (
        <span className="note" key={m}>
          {tb(m)}
        </span>
      ))}
    </div>
  );
}

function Everything({ items, pile, chosen, act, errors, scanned }: { items: Suggestion[]; pile: PileState; chosen: Set<string>; act: Choices; errors: DetectorError[]; scanned: boolean }) {
  const total = items.reduce((t, s) => t + s.sizeBytes, 0);
  const blocked = items.filter((s) => s.blockedReason);
  return (
    <div className="stack">
      <Protected items={blocked} />
      <div className="card">
        <h3>{t("e.title")}</h3>
        <p className="note" style={{ marginBottom: 6 }}>
          {items.length ? (items.length === 1 ? t("e.sum1", { x: gb(total) }) : t("e.sumN", { n: items.length, x: gb(total) })) : scanned ? t("e.nothing") : t("e.none")}
        </p>
        <Unchecked errors={errors} />
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
                  {areaName(label) + " "}
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
      <KeepMeTidy />
    </div>
  );
}

const RULE_LABEL: Record<TidyRuleId, string> = { package_caches: "k.caches", docker_build_cache: "k.docker" };

/** Keep me tidy (prototype, end of `everything()`): two rules, off until
 * turned on. Turning one on first says exactly what it will remove
 * (spec "Keep me tidy rules"); only then is it saved. */
function KeepMeTidy() {
  const [rules, setRules] = useState<TidyRule[] | null>(null);
  const [asking, setAsking] = useState<TidyRuleId | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    track(api.getTidyRules()).then(setRules, (e) => setError(errorText(e)));
  }, []);
  const save = (id: TidyRuleId, on: boolean) => {
    setAsking(null);
    track(api.setTidyRule(id, on)).then(setRules, (e) => setError(errorText(e)));
  };
  return (
    <div className="card stack" id="keep-tidy">
      <h3>{t("k.title")}</h3>
      <p className="note">{t("k.note")}</p>
      {(rules ?? []).map((r) => (
        // Siblings, as in the prototype: each switch is a direct child of
        // the card's stack.
        <Fragment key={r.id}>
          <label className="toggle">
            <input
              type="checkbox"
              id={r.id === "package_caches" ? "auto-caches" : "auto-docker"}
              data-auto={r.id}
              checked={r.enabled || asking === r.id}
              onChange={(e) => (e.target.checked ? setAsking(r.id) : asking === r.id ? setAsking(null) : save(r.id, false))}
            />
            <span>{t(RULE_LABEL[r.id])}</span>
          </label>
          {asking === r.id && (
            <div className="tidy-ask" role="group" aria-label={t(RULE_LABEL[r.id])} data-supplemental="tidy-confirm">
              <p className="note">{t("k.removes", { x: r.removes.map((d) => tb(d)).join(", ") })}</p>
              <p className="note">{t("k.only")}</p>
              <div className="row" style={{ gap: 8 }}>
                <button className="btn primary" data-act="tidy-on" onClick={() => save(r.id, true)}>
                  {t("k.turnon")}
                </button>
                <button className="btn" data-act="tidy-cancel" onClick={() => setAsking(null)}>
                  {t("k.cancel")}
                </button>
              </div>
            </div>
          )}
          {r.enabled && r.lastRunAt && (
            <p className="note" data-supplemental="tidy-last">
              {r.lastResult === "done" ? t("k.lastdone", { x: gb(Math.max(0, r.lastBytesFreed ?? 0)) }) : r.lastResult === "nothing" ? t("k.lastnothing") : t("k.lastfailed")}
            </p>
          )}
        </Fragment>
      ))}
      {error && <p className="muted">{tb(error)}</p>}
    </div>
  );
}

export function QuickQuestions({ items: all, pile, act, errors = [], scanned = true }: { items: Suggestion[]; pile: PileState; act: Choices; errors?: DetectorError[]; scanned?: boolean }) {
  const chosen = new Set(pile.itemIds);
  // Worktrees have their own tab (spec "Worktrees view"): there can be
  // hundreds, each with its own reason.
  const items = all.filter((s) => !isWorktree(s));
  return (
    <>
      <Question items={items} pile={pile} act={act} />
      <Everything items={items} pile={pile} chosen={chosen} act={act} errors={errors} scanned={scanned} />
    </>
  );
}

