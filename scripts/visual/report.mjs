import { writeFileSync } from "node:fs";
import { join } from "node:path";

const esc = (s) => String(s ?? "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/** Writes result.json, report.md and report.html into <out>/report. */
export function writeReport(out, result) {
  const dir = join(out, "report");
  writeFileSync(join(dir, "result.json"), JSON.stringify(result, null, 2));
  const rev = (r) => result.reviews?.[r.key] ?? result.reviews?.[r.id];
  const md = [
    "# Visual verification report",
    "",
    `- Generated: ${result.generatedAt}`,
    `- Revision: ${result.revision}`,
    `- Prototype SHA-256: \`${result.sourceSha256}\``,
    `- Engine: ${result.engine.engine} ${result.engine.version} (Playwright ${result.engine.playwright}) on ${result.engine.os}`,
    `- Mode: ${result.mode}${result.partial ? " (PARTIAL: --only filter)" : ""}`,
    `- Browser comparisons: **${result.browserStatus.toUpperCase()}** — ${result.counts.pass} pass, ${result.counts.fail} fail, ${result.counts.incomplete} incomplete`,
    `- Overall visual gate: **${result.overallStatus.toUpperCase()}**${result.overallNote ? " — " + result.overallNote : ""}`,
    "",
    ...(result.problems.length ? ["## Problems", "", ...result.problems.map((p) => "- " + p), ""] : []),
    "## Browser comparisons",
    "",
    "| Capture | Surface / state | Theme | Viewport | Kind | Tolerance | Diff px | Result | Review | Evidence |",
    "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ...result.rows.map(
      (r) =>
        `| ${r.key} | ${r.surface ?? ""}: ${r.state ?? ""} | ${r.theme ?? ""} | ${r.viewport ?? ""} | ${r.supplemental ? "supplemental" : "prototype"} | ${r.tolerance ? `t=${r.tolerance.threshold}, max=${r.tolerance.maxDiffPixels}` : ""} | ${r.diffPixels ?? ""} | ${r.result.toUpperCase()}: ${r.reason ?? ""} | ${rev(r) ? rev(r).disposition + " (" + rev(r).reviewer + ")" : "not reviewed"} | [ref](../baseline/${r.key}.png) · [actual](../actual/${r.key}.png) · [diff](../diff/${r.key}.png) |`,
    ),
    "",
    "## Reduced-motion checks",
    "",
    ...(result.motionChecks?.length
      ? ["| Side | Preference | Wave animation | Waterline transition | Result |", "| --- | --- | --- | --- | --- |", ...result.motionChecks.map((m) => `| ${m.side} | ${m.motion} | ${m.wave} | ${m.sea} | ${m.ok ? "PASS" : "FAIL"} |`)]
      : ["Not run (partial run)."]),
    "",
    "## Native evidence",
    "",
    "| Surface | Platform | State | Result |",
    "| --- | --- | --- | --- |",
    ...result.native.map((n) => `| ${n.surface} | ${n.platform} | ${n.state} | ${n.result.toUpperCase()}: ${n.reason} |`),
    "",
  ].join("\n");
  writeFileSync(join(dir, "report.md"), md);

  const cls = { pass: "#1F7A50", fail: "#B02A22", incomplete: "#8A5700" };
  const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Visual verification</title>
<style>body{font:14px system-ui,sans-serif;margin:24px;color:#13262F}table{border-collapse:collapse;width:100%}td,th{border-bottom:1px solid #ccd;padding:6px;vertical-align:top;text-align:left}
.r{font-weight:600}img{max-width:300px;border:1px solid #ccd;display:block}details summary{cursor:pointer}.imgs{display:flex;gap:8px;flex-wrap:wrap}</style></head><body>
<h1>Visual verification</h1>
<p>Revision ${esc(result.revision)} · prototype ${esc(result.sourceSha256.slice(0, 12))}… · ${esc(result.engine.engine)} ${esc(result.engine.version)} · ${esc(result.engine.os)} · mode ${esc(result.mode)}${result.partial ? " · <b>PARTIAL</b>" : ""}</p>
<p class="r">Browser comparisons: <span style="color:${cls[result.browserStatus]}">${result.browserStatus.toUpperCase()}</span> (${result.counts.pass} pass, ${result.counts.fail} fail, ${result.counts.incomplete} incomplete).
Overall gate: <span style="color:${cls[result.overallStatus]}">${result.overallStatus.toUpperCase()}</span>. ${esc(result.overallNote ?? "")}</p>
${result.problems.length ? "<ul>" + result.problems.map((p) => `<li style="color:#B02A22">${esc(p)}</li>`).join("") + "</ul>" : ""}
<table><thead><tr><th>Capture</th><th>State</th><th>Kind</th><th>Result</th><th>Review</th><th>Images</th></tr></thead><tbody>
${result.rows
  .map(
    (r) => `<tr><td>${esc(r.key)}</td><td>${esc(r.surface)}: ${esc(r.state)}${r.note ? `<br><small>${esc(r.note)}</small>` : ""}</td><td>${r.supplemental ? "supplemental" : "prototype"}</td>
<td class="r" style="color:${cls[r.result]}">${r.result.toUpperCase()}<br><small>${esc(r.reason)}</small></td><td>${rev(r) ? esc(rev(r).disposition + " — " + rev(r).reviewer + ": " + (rev(r).notes ?? "")) : "not reviewed"}</td>
<td><details><summary>reference / actual / diff</summary><div class="imgs"><img src="../baseline/${esc(r.key)}.png" alt="reference"><img src="../actual/${esc(r.key)}.png" alt="actual"><img src="../diff/${esc(r.key)}.png" alt="diff"></div></details></td></tr>`,
  )
  .join("\n")}
</tbody></table>
<h2>Native evidence</h2><table><tbody>${result.native.map((n) => `<tr><td>${esc(n.surface)}</td><td>${esc(n.platform)}</td><td>${esc(n.state)}</td><td style="color:${cls.incomplete}">${n.result.toUpperCase()}: ${esc(n.reason)}</td></tr>`).join("")}</tbody></table>
</body></html>`;
  writeFileSync(join(dir, "report.html"), html);
}
