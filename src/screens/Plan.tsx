import type { AppModel } from "../App";
import { TrashBox } from "../components/TrashBox";
import { gb, plural, runClass, runLabel, trashWord } from "../lib/format";
import type { CleanupPlan, RunDetail } from "../lib/types";

interface Props {
  model: AppModel;
  plan: CleanupPlan | null;
  planError: string | null;
  run: RunDetail | null;
  onAck(id: string, checked: boolean): void;
  onRefresh(): void;
  onConfirm(): void;
  onCancel(): void;
  onOverview(): void;
  onPlanMore(): void;
}

export function PlanScreen(props: Props) {
  const { model, plan, run } = props;
  if (run) return <RunView {...props} run={run} />;
  if (!plan || !plan.items.length)
    return (
      <section className="view" id="v-plan">
        <h2>Cleanup plan</h2>
        <p className="muted">No items selected yet.</p>
        {props.planError && <p className="blocked">{props.planError}</p>}
        <div className="actions">
          <button className="btn primary" data-action="go-suggestions" onClick={model.goSuggestions}>
            Choose suggestions
          </button>
        </div>
        <div id="plan-trash">
          <TrashBox model={model} />
        </div>
      </section>
    );

  const p = model.status.platform;
  const lost = plan.unrecoverableIds.length;
  const stale = plan.state === "expired" || plan.state === "invalidated";
  const n = plan.items.length;
  return (
    <section className="view" id="v-plan">
      <div>
        <h2>Cleanup plan</h2>
        <p className="muted">Nothing runs until you confirm. Each item is checked again just before cleanup.</p>
      </div>
      <div className="totals">
        <div className="total">
          <b>{gb(plan.bytesFreedNow)}</b>Freed right away
        </div>
        <div className="total">
          <b>{gb(plan.bytesHeldInTrash)}</b>
          {"Held in the " + trashWord(p) + " until you empty it"}
        </div>
        <div className={"total" + (lost ? " bad" : "")}>
          <b>{plural(lost, "item")}</b>
          {"Cannot be recovered" + (lost ? " (" + gb(plan.unrecoverableBytes) + ")" : "")}
        </div>
      </div>
      <AfterCleanup plan={plan} platform={p} />
      <div>
        {plan.items.map((i) => (
          <div className="plan-row" key={i.id}>
            <div>
              <b>{i.title}</b>
              {/* Only commands use the code font; "Delete the folder" is a sentence (user decision 2026-11-05). */}
              <div className={i.action === "native_command" ? "sub mono" : "sub"}>{i.actionLabel}</div>
              <div className="sub">{i.recovery}</div>
            </div>
            <span className="size">{gb(i.sizeBytes)}</span>
            {plan.needsAckIds.includes(i.id) && (
              <label className="ack">
                <input
                  type="checkbox"
                  id={"ack-" + i.id}
                  data-ack={i.id}
                  checked={plan.acknowledgedIds.includes(i.id)}
                  disabled={stale}
                  onChange={(e) => props.onAck(i.id, e.target.checked)}
                />
                {"I understand the " + (i.ackName ?? i.title) + " will be deleted and cannot be recovered."}
              </label>
            )}
          </div>
        ))}
      </div>
      {stale && (
        <p className="blocked">
          {plan.state === "expired"
            ? "This plan expired after 30 minutes. Make a new plan to check the items again."
            : "A newer scan replaced these suggestions. Make a new plan to check the items again."}
        </p>
      )}
      {props.planError && <p className="blocked">{props.planError}</p>}
      <div className="actions">
        {stale ? (
          <button className="btn primary" onClick={props.onRefresh}>
            Make a new plan
          </button>
        ) : (
          <button className="btn primary" id="confirm-plan" data-action="confirm" disabled={!plan.ready} onClick={props.onConfirm}>
            {"Confirm and clean up " + plural(n, "item")}
          </button>
        )}
        <button className="btn" data-action="go-suggestions" onClick={model.goSuggestions}>
          Change selection
        </button>
      </div>
    </section>
  );
}

function RunView({ model, run, onCancel, onOverview, onPlanMore }: Props & { run: RunDetail }) {
  const p = model.status.platform;
  const running = run.state === "running";
  const measured = run.measuredBytesFreed ?? 0;
  const freeAfter = run.freeAfter ?? model.status.volumes[0]?.freeBytes ?? 0;
  const expectedDone = run.items.filter((i) => i.state === "done").reduce((t, i) => t + i.expectedBytes, 0);
  const notStarted = run.items.filter((i) => i.state === "canceled").length;
  const skipText = run.skipped ? " " + (run.skipped === 1 ? "1 item was skipped." : run.skipped + " items were skipped.") : "";
  const failText = run.failed ? " " + (run.failed === 1 ? "1 item failed." : run.failed + " items failed.") : "";
  return (
    <section className="view" id="v-plan">
      <div>
        {running ? (
          <>
            <h2>Cleaning up</h2>
            <p className="muted">Each item is checked again first. Anything that changed is skipped.</p>
          </>
        ) : (
          <>
            <h2>{run.state === "canceled" ? "Cleanup canceled" : run.state === "interrupted" ? "Cleanup interrupted" : "Cleanup finished"}</h2>
            <p className="num">
              Freed <b>{gb(measured)}</b>. Free space is now <b>{gb(freeAfter)}</b>
              {"." + skipText + failText + (notStarted ? " " + plural(notStarted, "item") + " did not start." : "")}
            </p>
            {Math.abs(measured - expectedDone) >= 5e7 && (
              <p className="muted">{"The items were expected to free " + gb(expectedDone) + ". Freeboard shows the change it measured on the disk."}</p>
            )}
            {run.involvesDocker && p !== "linux" && <p className="muted">Docker Desktop may take time to return space to your main drive.</p>}
          </>
        )}
      </div>
      <div>
        {run.items.map((r) => (
          <div className="plan-row" key={r.itemId}>
            <div>
              <b>{r.title}</b>
              {r.reason && <div className="sub">{r.reason}</div>}
            </div>
            <span className={runClass(r.state)}>{runLabel(r.state, p)}</span>
          </div>
        ))}
      </div>
      {running ? (
        <div className="actions">
          <button className="btn" data-action="cancel" onClick={onCancel}>
            Cancel
          </button>
        </div>
      ) : (
        <>
          <div id="plan-trash">
            <TrashBox model={model} />
          </div>
          <div className="actions">
            <button className="btn primary" data-action="go-overview" onClick={onOverview}>
              Back to overview
            </button>
            <button className="btn" data-action="plan-more" onClick={onPlanMore}>
              Plan another cleanup
            </button>
          </div>
        </>
      )}
    </section>
  );
}

// Free space after cleanup. The prototype showed "now to now + freed", which
// read as a puzzle (user, 2026-11-05). The lower figure now counts everything
// except Docker Desktop data, whose virtual disk may not return space right
// away; with no such data it is a single figure.
function AfterCleanup({ plan, platform }: { plan: CleanupPlan; platform: string }) {
  const desktop = platform !== "linux";
  const slow = desktop
    ? plan.items.filter((i) => i.category === "docker" && i.action !== "trash").reduce((t, i) => t + i.sizeBytes, 0)
    : 0;
  const high = plan.freeBytesNow + plan.bytesFreedNow;
  const low = high - slow;
  if (!slow || gb(low) === gb(high))
    return (
      <p className="num">
        Free space after cleanup: <b>{"about " + gb(high)}</b>
      </p>
    );
  return (
    <p className="num">
      Free space after cleanup: <b>{gb(low) + " to " + gb(high)}</b>. The higher figure needs Docker Desktop to return space from its virtual disk.
    </p>
  );
}
