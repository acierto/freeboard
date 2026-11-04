import type { AppModel } from "../App";
import { Gauge } from "../components/Gauge";
import { Trend } from "../components/Trend";
import { TrashBox } from "../components/TrashBox";
import { CATEGORIES, GB, STATUS, gb, phaseLabel, relativeTime } from "../lib/format";
import type { Risk } from "../lib/types";

const RISK_COLOR: Record<Risk, string> = { safe: "var(--good)", review: "var(--warn)", risky: "var(--crit)" };
const RISK_NAME: Record<Risk, string> = { safe: "Safe", review: "Review", risky: "Risky" };

export function Overview({ model }: { model: AppModel }) {
  const { status, settings, suggestions } = model;
  const vol = status.volumes[0];
  const free = vol?.freeBytes ?? 0;
  const total = vol?.totalBytes ?? 0;
  const st = STATUS[status.status];
  const safe = suggestions.filter((s) => s.risk === "safe").reduce((t, s) => t + s.sizeBytes, 0);
  const fc = status.forecast;
  let forecast: string | null = null;
  if (fc) {
    const days = Math.round(fc.daysLeft);
    const rate = Number((fc.bytesPerDay / GB).toFixed(1));
    forecast = "At about " + rate + " GB a day, this disk fills in " + (days <= 1 ? "about a day" : "about " + days + " days") + ".";
  }
  const rows = CATEGORIES.map((c) => {
    const list = suggestions.filter((s) => s.category === c.id);
    return { c, list, t: list.reduce((a, s) => a + s.sizeBytes, 0) };
  })
    .filter((r) => r.t > 0)
    .sort((a, b) => b.t - a.t);
  const max = rows.reduce((m, r) => Math.max(m, r.t), GB);
  const note = model.scanning
    ? phaseLabel(model.scanNote && model.scanNote !== "scan" ? model.scanNote : null)
    : "Last scan " + relativeTime(status.scan.lastFinishedAt);

  return (
    <section className="view" id="v-overview">
      <div className="hero">
        <Gauge freeBytes={free} lowBytes={settings.lowThresholdBytes} criticalBytes={settings.criticalThresholdBytes} />
        <div className="readout">
          <div>
            <span className={"pill st-" + st.key} id="st-pill">
              {st.label}
            </span>
          </div>
          <div className="bigfree">
            <span id="big-free">{(free / GB).toFixed(1)}</span> <small>GB free</small>
          </div>
          <p className="muted" id="free-of">
            {"of " + Math.round(total / GB) + " GB. " + ((total - free) / GB).toFixed(1) + " GB used."}
          </p>
          {forecast !== null && <p id="forecast">{forecast}</p>}
          <div className="actions">
            <button className="btn primary" data-action="go-suggestions" id="cta" onClick={model.goSuggestions}>
              {"Review " + gb(safe) + " of Safe items"}
            </button>
            <button className="btn" data-action="scan" id="scan-btn" disabled={model.scanning} onClick={() => model.scan("quick")}>
              Scan now
            </button>
          </div>
          <p className="muted" style={{ fontSize: "13px" }} id="scan-note" aria-live="polite">
            {note}
          </p>
          {vol && (vol.purgeableBytes ?? 0) > 0 && (
            <p className="muted" style={{ fontSize: "13px" }}>
              {"macOS also reports " + gb(vol.purgeableBytes!) + " of purgeable space" + (vol.snapshotCount ? " and " + vol.snapshotCount + " local snapshots" : "") + ". Finder may count it as free."}
            </p>
          )}
        </div>
      </div>
      <div id="ov-trash">
        <TrashBox model={model} />
      </div>
      <div className="two">
        <section>
          <h3>Where the reclaimable space is</h3>
          <div className="bars" id="bars">
            {rows.length ? (
              rows.map((r) => (
                <div className="bar-row" key={r.c.id}>
                  <div className="bar-top">
                    <span>{r.c.name}</span>
                    <span className="num">{gb(r.t)}</span>
                  </div>
                  <div className="bar">
                    {(["safe", "review", "risky"] as Risk[]).map((k) => {
                      const v = r.list.filter((s) => s.risk === k).reduce((a, s) => a + s.sizeBytes, 0);
                      return v ? (
                        <i key={k} style={{ width: ((v / max) * 100).toFixed(1) + "%", background: RISK_COLOR[k] }} title={RISK_NAME[k] + " " + gb(v)} />
                      ) : null;
                    })}
                  </div>
                </div>
              ))
            ) : (
              <p className="muted">Nothing left to suggest. Run another scan later.</p>
            )}
          </div>
          <div className="legend">
            <span>
              <i style={{ background: "var(--good)" }} />
              Safe
            </span>
            <span>
              <i style={{ background: "var(--warn)" }} />
              Review
            </span>
            <span>
              <i style={{ background: "var(--crit)" }} />
              Risky
            </span>
          </div>
        </section>
        <section>
          <h3>Free space, last 14 days</h3>
          <Trend points={status.trend} freeBytes={free} lowBytes={settings.lowThresholdBytes} />
        </section>
      </div>
    </section>
  );
}
