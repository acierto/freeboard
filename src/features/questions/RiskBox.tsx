// "What Risky means here" (prototype `riskBox()`): where the item sits on the
// Safe / Review / Risky scale, why this one sits there, and what to do first
// to make removing it safer. The reasons and commands come from the core
// (risk_notes.rs); the commands are only shown, Freeboard never runs them.
import { t } from "../../i18n";
import { tb } from "../../i18n/backend";
import type { Risk, Suggestion } from "../../lib/types";

const RISK = (k: Risk) => ({ name: t("risk." + k), short: t("risk." + k + ".s"), long: t("risk." + k + ".l") });
const ORDER: Risk[] = ["safe", "review", "risky"];

export function RiskBox({ s, open }: { s: Suggestion; open: boolean }) {
  if (s.risk === "safe") return null;
  const lv = RISK(s.risk);
  // As the prototype, an item without its own reasons still says one thing.
  const why = s.riskWhy.length ? s.riskWhy.map(tb) : [s.risk === "risky" ? t("risk.noundo") : t("risk.back", { x: tb(s.recovery) })];
  return (
    <details className={`riskx ${s.risk}`} open={open} data-risk-box={s.id}>
      <summary>{t("risk.means", { x: lv.name })}</summary>
      <ol className="rscale" aria-label={t("risk.scale")}>
        {ORDER.map((k) => (
          <li key={k} className={k === s.risk ? `rs ${k} on` : `rs ${k}`} aria-current={k === s.risk ? "true" : undefined}>
            <b>{RISK(k).name}</b>
            <span>{RISK(k).short}</span>
          </li>
        ))}
      </ol>
      <p>{lv.long}</p>
      <div className="rx-sub">{t("risk.whyone", { x: lv.name })}</div>
      <ul className="rx-why">
        {why.map((w) => (
          <li key={w}>{w}</li>
        ))}
      </ul>
      {s.safer.length > 0 && (
        <>
          <div className="rx-sub">{t("risk.safer")}</div>
          <ol className="rx-steps">
            {s.safer.map((x) => (
              <li key={x.command}>
                <span>{tb(x.label)}</span>
                <code className="mono">{x.command}</code>
              </li>
            ))}
          </ol>
        </>
      )}
    </details>
  );
}
