// The cleanup sheets (prototype `confirmSheet()`, `run()` and its result),
// plus the Trash confirmation the prototype does not have (spec "Required
// behavior missing from the prototype"). One sheet stays open from
// confirmation to result, so focus returns to the control that opened it.
import { Sheet } from "../../components/Sheet";
import { gb, plural, trashWord } from "../../lib/format";
import { ackText, cmdFor, confirmTitle, expectedFreed, goLabel, isCommand, progressOf, resultTitle, trashUnavailable } from "./cleanupModel";
import type { Cleanup, Flow } from "./useCleanup";

const DOCKER_NOTE = "Docker Desktop may take a while to return its space. I will show the measured change in the Logbook.";

function titleOf(flow: Flow): string {
  switch (flow.kind) {
    case "confirm":
      return confirmTitle(flow.plan, flow.permanent);
    case "run":
      return "Tossing crates";
    case "result":
      return resultTitle(flow.run);
    case "trash":
      return `Empty ${gb(flow.trash.bytes)} from the ${trashWord(flow.platform)}?`;
    case "error":
      return "I can't do that right now";
  }
}

function Confirm({ c, flow }: { c: Cleanup; flow: Extract<Flow, { kind: "confirm" }> }) {
  const { plan, permanent } = flow;
  const lost = plan.unrecoverableIds.length;
  const acked = plan.needsAckIds.every((id) => flow.acks[id]);
  const stale = flow.error?.code === "expired" || flow.error?.code === "invalidated";
  return (
    <>
      <p className="muted">Each one is checked again first. Anything that changed since the scan stays on board.</p>
      <div className="sum3">
        <div>
          <b className="num">{gb(plan.bytesFreedNow)}</b>Free right away
        </div>
        <div>
          <b className="num">{gb(plan.bytesHeldInTrash)}</b>
          {`Held in the ${trashWord(flow.platform)}`}
        </div>
        <div className={lost ? "bad" : ""}>
          <b className="num">{lost}</b>Cannot be recovered
        </div>
      </div>
      {/* Focusable so the list scrolls by keyboard when it is long. */}
      <div className="plan-list" tabIndex={0} role="region" aria-label="Items in this plan">
        {plan.items.map((s) => (
          <div className="pile-row" style={{ gridTemplateColumns: "minmax(0,1fr) auto" }} key={s.id}>
            <span>
              {s.title}
              <span className={isCommand(s) ? "sub mono" : "sub"}>{cmdFor(s)}</span>
            </span>
            <span className="num">{gb(s.sizeBytes)}</span>
            {plan.needsAckIds.includes(s.id) && (
              <label className="ack" style={{ gridColumn: "1/-1" }}>
                <input type="checkbox" id={`sheet-ack-${s.id}`} data-ack={s.id} checked={!!flow.acks[s.id]} onChange={(e) => c.ack(s.id, e.target.checked)} />
                {ackText(s)}
              </label>
            )}
          </div>
        ))}
      </div>
      {flow.error && (
        <p className="note" role="alert">
          {flow.error.message}
        </p>
      )}
      <div className="row">
        {stale ? (
          <button className="btn primary" id="go" data-act="replan" onClick={c.replan}>
            Check again
          </button>
        ) : (
          <button className="btn primary" id="go" data-act="run" disabled={!acked || flow.busy} onClick={c.run}>
            {goLabel(plan, permanent)}
          </button>
        )}
        <button className="btn" data-act="close" onClick={c.close}>
          Not now
        </button>
      </div>
    </>
  );
}

function Running({ c, flow }: { c: Cleanup; flow: Extract<Flow, { kind: "run" }> }) {
  const p = progressOf(flow.plan, flow.run);
  return (
    <>
      <p id="run-now" className="muted" aria-live="polite">
        {`Checking ${p.current}`}
      </p>
      <div className="progress" role="progressbar" aria-label="Cleanup progress" aria-valuemin={0} aria-valuemax={p.total} aria-valuenow={p.done}>
        <i id="run-bar" style={{ width: (p.done / p.total) * 100 + "%" }}></i>
      </div>
      <div className="row">
        <button className="btn" data-act="cancel" disabled={flow.canceling} onClick={c.cancel}>
          {flow.canceling ? "Stopping after this one" : "Cancel"}
        </button>
      </div>
    </>
  );
}

function Result({ c, flow }: { c: Cleanup; flow: Extract<Flow, { kind: "result" }> }) {
  const { run, platform } = flow;
  const freed = Math.max(0, run.measuredBytesFreed ?? 0);
  const expected = expectedFreed(run);
  const differs = expected > 0 && Math.abs(freed - expected) >= 0.05e9;
  const skipped = run.items.filter((i) => i.state === "skipped");
  const failed = run.items.filter((i) => i.state === "failed" || i.state === "interrupted");
  const canceled = run.items.filter((i) => i.state === "canceled").length;
  const noTrash = failed.filter(trashUnavailable);
  const permanentIds = [...new Set(noTrash.map((i) => i.parentId ?? i.itemId))];
  return (
    <>
      <p className="num">
        I freed <b>{gb(freed)}</b>. You now have <b>{gb(run.freeAfter ?? run.freeBefore)}</b> free.
      </p>
      {differs && <p className="note">{`I expected to free ${gb(expected)}. This is the change your disk reports.`}</p>}
      {run.trashBytes > 0 && <p className="num">{`${gb(run.trashBytes)} is in the ${trashWord(platform)} and frees up when you empty it.`}</p>}
      {skipped.map((i) => (
        <p style={{ color: "var(--review)" }} key={i.itemId}>
          {`${i.title} stayed on board. ${i.reason ?? "It changed after the scan."}`}
        </p>
      ))}
      {failed.map((i) => (
        <p style={{ color: "var(--review)" }} key={i.itemId}>
          {`${i.title} was not removed. ${i.reason ?? ""}`.trim()}
        </p>
      ))}
      {canceled > 0 && <p className="note">{`${plural(canceled, "item")} did not start because you stopped the cleanup.`}</p>}
      {run.involvesDocker && <p className="note">{DOCKER_NOTE}</p>}
      <div className="row">
        <button className="btn primary" id="go" data-act="close" onClick={c.close}>
          Done
        </button>
        {permanentIds.length > 0 && (
          <button className="btn bad" data-act="permanent" onClick={() => c.permanent(permanentIds)}>
            {`Delete ${permanentIds.length === 1 ? "it" : "them"} permanently instead`}
          </button>
        )}
      </div>
    </>
  );
}

function EmptyTrash({ c, flow }: { c: Cleanup; flow: Extract<Flow, { kind: "trash" }> }) {
  const word = trashWord(flow.platform);
  return (
    <>
      <p className="muted">{`Only the items I moved there are deleted. Everything else in your ${word} stays where it is.`}</p>
      <p className="note">This cannot be undone.</p>
      {flow.error && (
        <p className="note" role="alert">
          {flow.error}
        </p>
      )}
      <div className="row">
        <button className="btn primary" id="go" data-act="empty-confirm" disabled={flow.busy || !flow.trash.confirmationToken} onClick={c.emptyTrash}>
          Empty it
        </button>
        <button className="btn" data-act="close" onClick={c.close}>
          Not now
        </button>
      </div>
    </>
  );
}

export function CleanupSheets({ c }: { c: Cleanup }) {
  const flow = c.flow;
  if (!flow) return null;
  return (
    <Sheet title={titleOf(flow)} onClose={c.close} locked={flow.kind === "run"} stage={flow.kind} fallbackFocus="#pile-btn">
      {flow.kind === "confirm" && <Confirm c={c} flow={flow} />}
      {flow.kind === "run" && <Running c={c} flow={flow} />}
      {flow.kind === "result" && <Result c={c} flow={flow} />}
      {flow.kind === "trash" && <EmptyTrash c={c} flow={flow} />}
      {flow.kind === "error" && (
        <>
          <p className="muted">{flow.message}</p>
          <div className="row">
            <button className="btn primary" id="go" data-act="close" onClick={c.close}>
              Done
            </button>
          </div>
        </>
      )}
    </Sheet>
  );
}
