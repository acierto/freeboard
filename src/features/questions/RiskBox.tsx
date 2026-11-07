// "What Risky means here" (prototype `riskBox()`): where the item sits on the
// Safe / Review / Risky scale, why this one sits there, and what to do first
// to make removing it safer. The reasons and commands come from the core
// (risk_notes.rs); the commands are only shown, Freeboard never runs them.
import type { Risk, Suggestion } from "../../lib/types";

const RISK: Record<Risk, { name: string; short: string; long: string }> = {
  safe: { name: "Safe", short: "Comes back by itself", long: "Freeboard can show this rebuilds or downloads again on its own. Nothing of yours is lost." },
  review: {
    name: "Review",
    short: "Comes back, with effort",
    long: "Nothing is lost for good, but getting it back costs you something: a download, a rebuild, or a trip to the Trash. Worth a glance.",
  },
  risky: {
    name: "Risky",
    short: "May not come back",
    long: "Freeboard can’t prove this is safe to remove, and removing it can’t be undone. It may be the only copy of something you need.",
  },
};
const ORDER: Risk[] = ["safe", "review", "risky"];

export function RiskBox({ s, open }: { s: Suggestion; open: boolean }) {
  if (s.risk === "safe") return null;
  const lv = RISK[s.risk];
  // As the prototype, an item without its own reasons still says one thing.
  const why = s.riskWhy.length ? s.riskWhy : [s.risk === "risky" ? "It cannot be undone." : "To get it back: " + s.recovery];
  return (
    <details className={`riskx ${s.risk}`} open={open} data-risk-box={s.id}>
      <summary>{`What “${lv.name}” means here`}</summary>
      <ol className="rscale" aria-label="Risk scale">
        {ORDER.map((k) => (
          <li key={k} className={k === s.risk ? `rs ${k} on` : `rs ${k}`} aria-current={k === s.risk ? "true" : undefined}>
            <b>{RISK[k].name}</b>
            <span>{RISK[k].short}</span>
          </li>
        ))}
      </ol>
      <p>{lv.long}</p>
      <div className="rx-sub">{`Why this one is ${lv.name}`}</div>
      <ul className="rx-why">
        {why.map((w) => (
          <li key={w}>{w}</li>
        ))}
      </ul>
      {s.safer.length > 0 && (
        <>
          <div className="rx-sub">Make it safer first</div>
          <ol className="rx-steps">
            {s.safer.map((x) => (
              <li key={x.command}>
                <span>{x.label}</span>
                <code className="mono">{x.command}</code>
              </li>
            ))}
          </ol>
        </>
      )}
    </details>
  );
}
