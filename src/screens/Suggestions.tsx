import type { AppModel } from "../App";
import { CATEGORIES, RISK_LABEL, displayPath, gb, selectable, sizeText } from "../lib/format";
import type { Suggestion } from "../lib/types";

export type Filter = "All" | "Safe" | "Review" | "Risky";
const FILTERS: Filter[] = ["All", "Safe", "Review", "Risky"];

function Row({ s, model }: { s: Suggestion; model: AppModel }) {
  const blocked = !selectable(s);
  const p = model.status.platform;
  return (
    <div className="item">
      <input
        type="checkbox"
        id={"sel-" + s.id}
        aria-label={"Select " + s.title}
        checked={model.sel.has(s.id) && !blocked}
        disabled={blocked}
        data-sel={s.id}
        onChange={(e) => model.toggle(s.id, e.target.checked)}
      />
      <details>
        <summary>
          <span className="name">{s.title}</span>
          <span className="sub mono">{displayPath(s.location, p)}</span>
          <span className="sub">{s.lastUsedLabel}</span>
          {s.blockedReason && <span className="blocked">{s.blockedReason}</span>}
          {s.skipReason && <span className="blocked">{s.skipReason}</span>}
        </summary>
        <dl>
          <dt>Why it is listed</dt>
          <dd>{s.reason}</dd>
          <dt>What would run</dt>
          <dd className="mono">{s.actionLabel}</dd>
          <dt>How to get it back</dt>
          <dd>{s.recovery}</dd>
        </dl>
      </details>
      <div className="item-right">
        <span className="size">{sizeText(s)}</span>
        <span className={"pill r-" + s.risk}>{RISK_LABEL[s.risk]}</span>
      </div>
    </div>
  );
}

export function Suggestions({ model, filter, setFilter }: { model: AppModel; filter: Filter; setFilter: (f: Filter) => void }) {
  const vis = model.suggestions.filter((s) => filter === "All" || RISK_LABEL[s.risk] === filter);
  const errors = model.status.scan.detectorErrors;
  const groups = CATEGORIES.map((c) => {
    const list = vis.filter((s) => s.category === c.id).sort((a, b) => b.sizeBytes - a.sizeBytes);
    return { c, list };
  }).filter((g) => g.list.length);
  const sel = model.selected;
  const selBytes = sel.reduce((t, s) => t + s.sizeBytes, 0);

  return (
    <section className="view" id="v-suggestions">
      <div className="view-head">
        <div>
          <h2>Suggestions</h2>
          <p className="muted">Open a row to see why it is listed, what will run and how to get it back.</p>
        </div>
        <div className="chips" id="chips" role="group" aria-label="Filter by risk">
          {FILTERS.map((f) => (
            <button key={f} className="chip" data-filter={f} aria-pressed={filter === f} onClick={() => setFilter(f)}>
              {f}
            </button>
          ))}
        </div>
      </div>
      <div id="groups" style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
        {errors.length > 0 && (
          <div className="notes">
            {errors.map((e) => (
              <p className="group-note" key={e.detector}>
                {e.message}
              </p>
            ))}
          </div>
        )}
        {groups.length ? (
          groups.map(({ c, list }) => (
            <div className="group" key={c.id}>
              <div className="group-head">
                <h3>{c.name}</h3>
                <span className="num muted">{gb(list.reduce((t, s) => t + s.sizeBytes, 0))}</span>
              </div>
              {c.id === "docker" && model.status.platform !== "linux" && (
                <p className="group-note">Docker Desktop keeps this data in a virtual disk, so space may not return to your main drive right away.</p>
              )}
              {list.map((s) => (
                <Row key={s.id} s={s} model={model} />
              ))}
            </div>
          ))
        ) : model.suggestions.length === 0 ? (
          <>
            <p className="muted">
              {model.scanning
                ? "Scanning. Suggestions appear here when the scan finishes."
                : "No suggestions yet. Run a scan to look for space you can reclaim."}
            </p>
            {model.settings.scanFolders.length === 0 && (
              <p className="muted" id="no-folders-note">
                No folders are chosen yet, so projects, worktrees and old installers are not scanned. Choose them in Settings.
              </p>
            )}
            <div className="actions">
              <button className="btn primary" data-action="scan-empty" disabled={model.scanning} onClick={() => model.scan("quick")}>
                Scan now
              </button>
              {model.settings.scanFolders.length === 0 ? (
                <button className="btn" data-action="choose-folders" onClick={() => model.show("settings")}>
                  Choose folders
                </button>
              ) : (
                <button className="btn" data-action="scan-folders-empty" disabled={model.scanning} onClick={() => model.scan("deep")}>
                  Scan folders now
                </button>
              )}
            </div>
          </>
        ) : (
          <p className="muted">No suggestions at this risk level.</p>
        )}
      </div>
      <div className="selbar">
        <span id="sel-total" className="num">
          {sel.length + " selected, " + gb(selBytes)}
        </span>
        <button className="btn primary" data-action="go-plan" id="sel-go" disabled={!sel.length} onClick={model.goPlan}>
          Review cleanup plan
        </button>
      </div>
    </section>
  );
}
