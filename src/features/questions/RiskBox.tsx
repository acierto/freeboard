// "What Risky means here" (decided by the user 2026-11-07): where the item
// sits on the Safe / Review / Risky scale, why this one sits there, and what
// to do first to make removing it safer. Open on Risky items, folded on
// Review, absent on Safe. The commands are only shown; Freeboard never runs
// them.
import { useState } from "react";
import type { Risk, Suggestion } from "../../lib/types";

const LEVELS: { id: Risk; name: string; short: string; long: string }[] = [
  { id: "safe", name: "Safe", short: "comes back by itself", long: "Its tool downloads or rebuilds it on its own when needed." },
  { id: "review", name: "Review", short: "comes back, with effort", long: "It comes back, but it takes a download, a rebuild or a restore from the Trash." },
  {
    id: "risky",
    name: "Risky",
    short: "may not come back",
    long: "Freeboard can’t prove it is safe to remove, it can’t be undone, and it may be the only copy of something.",
  },
];

function Command({ label, command }: { label: string; command: string }) {
  const [copied, setCopied] = useState(false);
  const copy = () =>
    navigator.clipboard?.writeText(command).then(
      () => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      },
      () => undefined,
    );
  return (
    <li>
      <span>{label}</span>
      <span className="rx-cmd">
        <code className="mono">{command}</code>
        <button className="mini" onClick={() => void copy()} aria-label={`Copy the command: ${label}`}>
          {copied ? "Copied" : "Copy"}
        </button>
      </span>
    </li>
  );
}

export function RiskBox({ s }: { s: Suggestion }) {
  if (s.risk === "safe") return null;
  const level = LEVELS.find((l) => l.id === s.risk)!;
  return (
    <details className={`riskx ${s.risk}`} open={s.risk === "risky"} data-risk-box={s.id}>
      <summary>{`What “${level.name}” means here`}</summary>
      <div className="rx-scale" role="list" aria-label="Risk levels">
        {LEVELS.map((l) => (
          <span key={l.id} role="listitem" className={`rx-lvl ${l.id}${l.id === s.risk ? " on" : ""}`} aria-current={l.id === s.risk ? "true" : undefined}>
            <b>{l.name}</b>
            <small>{l.short}</small>
          </span>
        ))}
      </div>
      <p className="rx-def">{level.long}</p>
      {s.riskWhy.length > 0 && (
        <>
          <b className="rx-h">{`Why this one is ${level.name}`}</b>
          <ul className="rx-why">
            {s.riskWhy.map((w) => (
              <li key={w}>{w}</li>
            ))}
          </ul>
        </>
      )}
      {s.safer.length > 0 && (
        <>
          <b className="rx-h">Make it safer first</b>
          <ul className="rx-safer">
            {s.safer.map((x) => (
              <Command key={x.command} label={x.label} command={x.command} />
            ))}
          </ul>
          <p className="rx-note">Run these in Terminal yourself. Freeboard never runs them.</p>
        </>
      )}
    </details>
  );
}
