// The cleanup sheets (prototype `confirmSheet()`, `run()` and
// `outcomeSheet()`), plus the Trash confirmation the prototype does not have
// (spec "Required behavior missing from the prototype"). One sheet stays open
// from confirmation to result, so focus returns to the control that opened it.
import { t } from "../../i18n";
import { itemName, tb } from "../../i18n/backend";
import { Sheet } from "../../components/Sheet";
import { gb, trashWord } from "../../lib/format";
import type { CleanupPlan } from "../../lib/types";
import { ackText, age, confirmHead, cubeLooks, expectedFreed, freeBar, isCommand, resultSections, runFigures, trashUnavailable } from "./cleanupModel";
import type { Cleanup, Flow } from "./useCleanup";


const SHIELD = (
  <svg viewBox="0 0 16 16" aria-hidden="true">
    <path d="M8 1.5l5.5 2v4.2c0 3.2-2.3 5.6-5.5 6.8C4.8 13.3 2.5 10.9 2.5 7.7V3.5Z" fill="var(--safe)" opacity=".18" stroke="var(--safe)" strokeWidth="1.4" />
    <path d="M5.3 8.2l1.9 1.9 3.6-3.8" fill="none" stroke="var(--safe)" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

function Confirm({ c, flow }: { c: Cleanup; flow: Extract<Flow, { kind: "confirm" }> }) {
  const { plan, permanent } = flow;
  const one = plan.items.length === 1;
  const lost = plan.unrecoverableIds.length;
  const review = plan.items.filter((s) => s.risk !== "safe").length;
  const acked = plan.needsAckIds.every((id) => flow.acks[id]);
  const stale = flow.error?.code === "expired" || flow.error?.code === "invalidated";
  const { go } = confirmHead(plan, permanent);
  return (
    <>
      <div className="st-body">
        <div className="cf-sum">
          <div className="cf-big num">
            {`${gb(plan.bytesFreedNow)} `}
            <small>{t("cs.freenow")}</small>
          </div>
          <div className="cf-flags">
            {plan.bytesHeldInTrash > 0 && <span className="cf-flag num">{t("cs.trashfirst", { x: gb(plan.bytesHeldInTrash), t: trashWord(flow.platform) })}</span>}
            <span className="cf-flag">{review ? (one ? t("cs.approved1") : t("cs.approvedN", { n: review })) : t("cs.allreb")}</span>
            {lost > 0 && <span className="cf-flag bad">{t("cs.lostN", { n: lost })}</span>}
          </div>
        </div>
        {/* Focusable so the list scrolls by keyboard when it is long. */}
        <div className="cf-list plan-list" tabIndex={0} role="region" aria-label={t("cs.items")}>
          {plan.items.map((s) => {
            const parts = s.parts.filter((p) => p.selected);
            return (
              <div className="cf-item" key={s.id}>
                {!one && (
                  <div className="cf-top">
                    <b>{itemName(s)}</b>
                    <span className="num">{gb(s.sizeBytes)}</span>
                  </div>
                )}
                <dl className="cf-lines">
                  <dt>{t("cs.runs")}</dt>
                  <dd className={isCommand(s) ? "mono" : undefined}>{tb(s.actionLabel)}</dd>
                  <dt>{t("f.back")}</dt>
                  <dd>{tb(s.recovery)}</dd>
                  <dt>{t("f.lastused")}</dt>
                  <dd>{age(s.lastUsedLabel)}</dd>
                </dl>
                {parts.length > 0 && (
                  <details className="cf-parts">
                    <summary>
                      {t(parts.length > 3 ? "cs.inclmore" : "cs.incl", {
                        n: parts.length,
                        list: parts
                          .slice(0, 3)
                          .map((p) => tb(p.label))
                          .join(", "),
                        m: parts.length - 3,
                      })}
                    </summary>
                    <ul>
                      {parts.map((p) => (
                        <li key={p.id}>
                          {`${tb(p.label)} `}
                          <span className="muted num">{gb(p.sizeBytes)}</span>
                        </li>
                      ))}
                    </ul>
                  </details>
                )}
                {plan.needsAckIds.includes(s.id) && (
                  <label className="ack">
                    <input type="checkbox" id={`sheet-ack-${s.id}`} data-ack={s.id} checked={!!flow.acks[s.id]} onChange={(e) => c.ack(s.id, e.target.checked)} />
                    {ackText(s)}
                  </label>
                )}
                {!one && (
                  <button className="link small cf-leave" data-leave={s.id} aria-label={t("cs.leave1", { x: itemName(s) })} onClick={() => c.leaveOut(s.id)}>
                    {t("cs.leave")}
                  </button>
                )}
              </div>
            );
          })}
        </div>
        <p className="cf-safe">
          {SHIELD}
          <span>{t("cs.safe")}</span>
        </p>
        {!acked && !one && (
          <p className="note cf-hint">{t("cs.hint")}</p>
        )}
        {flow.error && (
          <p className="note" role="alert">
            {tb(flow.error.message)}
          </p>
        )}
      </div>
      <div className="st-foot">
        <button className="btn" data-act="close" onClick={c.close}>
          {t("cs.cancel")}
        </button>
        {stale ? (
          <button className="btn primary" id="go" data-act="replan" onClick={c.replan}>
            {t("cs.again")}
          </button>
        ) : (
          <button className={permanent ? "btn bad" : "btn primary"} id="go" data-act="run" disabled={!acked || flow.busy} onClick={c.run}>
            {go}
          </button>
        )}
      </div>
    </>
  );
}

/** The little 3D deck (prototype `voyageHTML`): crates lift, tumble overboard
 * with a splash, wobble "no" when they stay, and shake when they fail. */
function Voyage({ plan, freed, looks, splashes }: { plan: CleanupPlan; freed: number; looks: string[]; splashes: number }) {
  const spots: [number, number][] = [
    [0, 56],
    [36, 56],
    [72, 56],
    [108, 56],
    [18, 24],
    [54, 24],
    [90, 24],
    [36, -8],
    [72, -8],
  ];
  const n = Math.min(plan.items.length, 9);
  return (
    <div className="voyage" aria-hidden="true">
      <svg className="vsea" viewBox="0 0 440 64" preserveAspectRatio="none">
        <path d={"M0 18 q20 -10 40 0" + " t40 0".repeat(10) + " V64 H0Z"} fill="var(--sea-a)" />
        <path d={"M0 30 q20 -10 40 0" + " t40 0".repeat(10) + " V64 H0Z"} fill="var(--sea-b)" />
      </svg>
      <svg className="hull" viewBox="0 0 230 46">
        <path d="M0 2H230Q224 40 176 44H40Q6 40 0 2Z" fill="var(--hull)" />
        <path d="M0 2H230L228 10H2Z" fill="var(--hull-dark)" />
      </svg>
      <div className="deck" id="deck">
        {Array.from({ length: n }, (_, i) => n - 1 - i).map((k) => (
          <div key={k} className={looks[k] ? `cube ${looks[k]}` : "cube"} id={`cube-${k}`} style={{ ["--at" as string]: `translate3d(${spots[k][0]}px,${spots[k][1]}px,0)`, transform: "var(--at)" }}>
            <i></i>
            <i></i>
            <i></i>
            <i></i>
            <i></i>
            <i></i>
          </div>
        ))}
      </div>
      <svg key={splashes} className={splashes ? "splash go" : "splash"} id="splash" viewBox="0 0 70 24">
        <ellipse cx="35" cy="12" rx="32" ry="9" />
        <ellipse cx="35" cy="12" rx="20" ry="6" />
      </svg>
      <div className="count num">
        <b id="run-freed">{gb(freed)}</b>
        {t("cs.freedsofar")}
      </div>
    </div>
  );
}

function Running({ c, flow }: { c: Cleanup; flow: Extract<Flow, { kind: "run" }> }) {
  const f = runFigures(flow.plan, flow.run);
  const total = flow.plan.items.length;
  const at = flow.plan.items[f.at - 1];
  const current = at ? itemName(at) : "";
  return (
    <>
      <div className="st-body">
        <Voyage plan={flow.plan} freed={f.freed} looks={cubeLooks(f.outcomes)} splashes={f.tossed} />
        <div className="now-doing">
          <span id="run-now" aria-live="polite">{t("cs.checking", { x: current }) + (f.within ? ` \u00b7 ${t("cs.within", { a: f.within.done, b: f.within.of })}` : "")}</span>
          <span className="num muted" id="run-of">{t("cs.within", { a: f.at, b: total })}</span>
        </div>
        <div className="progress" role="progressbar" aria-label={t("cs.progress")} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(f.fraction * 100)}>
          <i id="run-bar" style={{ width: f.fraction * 100 + "%" }}></i>
        </div>
        <div className="tally" id="run-tally">
          <span className="safe">
            <i></i>
            {t("cs.tossed", { n: f.tossed })}
          </span>
          {f.stayed > 0 && (
            <span className="review">
              <i></i>
              {t("cs.stayed", { n: f.stayed })}
            </span>
          )}
          {f.failed > 0 && (
            <span className="risky">
              <i></i>
              {t("cs.failed", { n: f.failed })}
            </span>
          )}
        </div>
      </div>
      <div className="st-foot">
        {/* An item already running is never cut off halfway. */}
        <button className="btn" id="run-cancel" data-act="cancel" disabled={flow.canceling} onClick={c.cancel}>
          {flow.canceling ? t("cs.stopping") : t("cs.stop")}
        </button>
      </div>
    </>
  );
}

const HAPPY = (
  <svg viewBox="0 0 96 80" aria-hidden="true">
    <path d="M8 46H90Q86 70 66 74H28Q10 70 8 46Z" fill="var(--hull)" />
    <rect x="16" y="14" width="40" height="34" rx="10" fill="var(--cabin)" stroke="var(--boat-ink)" strokeWidth="2.5" />
    <path d="M24 28q3 -4 6 0M38 28q3 -4 6 0" fill="none" stroke="var(--boat-ink)" strokeWidth="2.5" strokeLinecap="round" />
    <path d="M28 36q6 6 12 0" fill="var(--boat-ink)" />
    <circle cx="22" cy="36" r="3" fill="#FFB4A8" />
    <circle cx="50" cy="36" r="3" fill="#FFB4A8" />
    <path d="M0 72q8 -5 16 0t16 0t16 0t16 0t16 0t16 0V80H0Z" fill="var(--sea-b)" />
  </svg>
);

function Section({ cls, title, sub, n, open, children }: { cls: string; title: string; sub: string; n: number; open: boolean; children: React.ReactNode }) {
  if (!n) return null;
  return (
    <details className={cls ? `res ${cls}` : "res"} open={open}>
      <summary>
        <i></i>
        <span>
          {title}
          <small>{sub}</small>
        </span>
        <span className="n num">{n}</span>
      </summary>
      <ul>{children}</ul>
    </details>
  );
}

function Result({ c, flow }: { c: Cleanup; flow: Extract<Flow, { kind: "result" }> }) {
  const { run, platform } = flow;
  const freed = Math.max(0, run.measuredBytesFreed ?? 0);
  const after = run.freeAfter ?? run.freeBefore;
  const expected = expectedFreed(run);
  const differs = expected > 0 && Math.abs(freed - expected) >= 0.05e9;
  const r = resultSections(flow.plan, run);
  const bar = freeBar(run.freeBefore, after);
  const noTrash = run.items.filter(trashUnavailable);
  const permanentIds = [...new Set(noTrash.map((i) => i.parentId ?? i.itemId))];
  const word = trashWord(platform);
  return (
    <>
      <div className="st-body">
        <div className="done-hero">
          {HAPPY}
          <div>
            {freed > 0 ? <div className="done-big num">{t("cs.freed", { x: gb(freed) })}</div> : <div className="done-big">{t("cs.nofreed")}</div>}
            <div className="freebar" aria-hidden="true">
              <i style={{ width: bar.before, background: "var(--accent)" }}></i>
              <i style={{ width: bar.gained, background: "var(--safe)" }}></i>
            </div>
            <div className="freebar-legend num">
              <span>{t("cs.before", { x: gb(run.freeBefore) })}</span>
              <span>
                <b>{t("cs.now", { x: gb(after) })}</b>
              </span>
            </div>
          </div>
        </div>
        {run.trashBytes > 0 && <p className="note num">{t("cs.intrash", { x: gb(run.trashBytes), t: word })}</p>}
        {differs && <p className="note">{t("cs.expected", { x: gb(expected) })}</p>}
        <Section cls="safe" title={t("cs.s.tossed")} sub={r.tossed.length === 1 ? t("cs.s.tossed1", { x: gb(r.tossedBytes) }) : t("cs.s.tossedN", { n: r.tossed.length, x: gb(r.tossedBytes) })} n={r.tossed.length} open={false}>
          {r.tossed.map((l) => (
            <li key={l.id}>
              {`${l.title} `}
              <span className="num">{gb(l.bytes) + (l.inTrash ? t("cs.intrash1", { t: word }) : "")}</span>
            </li>
          ))}
        </Section>
        <Section cls="review" title={t("cs.s.stayed")} sub={t("cs.s.stayed.s")} n={r.stayed.length} open>
          {r.stayed.map((l) => (
            <li key={l.id}>
              {`${l.title} `}
              <span>{tb(l.why)}</span>
            </li>
          ))}
        </Section>
        <Section cls="risky" title={t("cs.s.failed")} sub={r.reasons.map(tb).join(" · ")} n={r.failed.length} open>
          {r.failed.map((l) => (
            <li key={l.id}>
              {`${l.title} `}
              <span className="num">{gb(l.bytes)}</span>
            </li>
          ))}
        </Section>
        <Section cls="" title={t("cs.s.notstarted")} sub={t("cs.s.notstarted.s")} n={r.not_started.length} open={false}>
          {r.not_started.map((l) => (
            <li key={l.id}>{l.title}</li>
          ))}
        </Section>
        {r.tossed.some((l) => flow.plan.items.find((s) => s.id === l.id)?.group === "docker") && <p className="note">{t("cs.docker")}</p>}
      </div>
      <div className="st-foot">
        {permanentIds.length > 0 && (
          <button className="btn bad" data-act="permanent" onClick={() => c.permanent(permanentIds)}>
            {permanentIds.length === 1 ? t("cs.perm1") : t("cs.permN")}
          </button>
        )}
        {r.retryIds.length > 0 && (
          <button className="btn" id="retry" data-act="retry" onClick={() => c.fromItems(r.retryIds)}>
            {t("cs.retry", { n: r.retryIds.length })}
          </button>
        )}
        <button className="btn primary" id="go" data-act="close" onClick={c.close}>
          {t("cs.done")}
        </button>
      </div>
    </>
  );
}

function EmptyTrash({ c, flow }: { c: Cleanup; flow: Extract<Flow, { kind: "trash" }> }) {
  const word = trashWord(flow.platform);
  return (
    <>
      <p className="muted">{t("cs.empty.p", { t: word })}</p>
      <p className="note">{t("cs.empty.undo")}</p>
      {flow.error && (
        <p className="note" role="alert">
          {tb(flow.error)}
        </p>
      )}
      <div className="row">
        <button className="btn primary" id="go" data-act="empty-confirm" disabled={flow.busy || !flow.trash.confirmationToken} onClick={c.emptyTrash}>
          {t("cs.empty.go")}
        </button>
        <button className="btn" data-act="close" onClick={c.close}>
          {t("cs.empty.later")}
        </button>
      </div>
    </>
  );
}

function titleOf(flow: Flow): string {
  switch (flow.kind) {
    case "confirm":
      return confirmHead(flow.plan, flow.permanent).title;
    case "run":
      return t("cs.t.run");
    case "result": {
      const r = resultSections(flow.plan, flow.run);
      return r.failed.length || r.stayed.length ? t("cs.t.mostly") : t("cs.t.better");
    }
    case "trash":
      return t("cs.empty.title", { x: gb(flow.trash.bytes), t: trashWord(flow.platform) });
    case "error":
      return t("cs.t.error");
  }
}

export function CleanupSheets({ c }: { c: Cleanup }) {
  const flow = c.flow;
  if (!flow) return null;
  const framed = flow.kind === "confirm" || flow.kind === "run" || flow.kind === "result";
  const closeLabel = flow.kind === "confirm" ? t("cs.cancel") : flow.kind === "result" ? t("cs.close") : undefined;
  return (
    <Sheet
      title={titleOf(flow)}
      onClose={c.close}
      locked={flow.kind === "run"}
      stage={flow.kind}
      fallbackFocus="#pile-btn"
      className={framed ? "framed" : undefined}
      closeLabel={closeLabel}
      headRow={framed}
    >
      {flow.kind === "confirm" && <Confirm c={c} flow={flow} />}
      {flow.kind === "run" && <Running c={c} flow={flow} />}
      {flow.kind === "result" && <Result c={c} flow={flow} />}
      {flow.kind === "trash" && <EmptyTrash c={c} flow={flow} />}
      {flow.kind === "error" && (
        <>
          <p className="muted">{tb(flow.message)}</p>
          <div className="row">
            <button className="btn primary" id="go" data-act="close" onClick={c.close}>
              {t("cs.done")}
            </button>
          </div>
        </>
      )}
    </Sheet>
  );
}
