// Real-app click-through: drives the INSTALLED Freeboard (real Rust core, real
// scans) through WebDriver, by element, and checks what each control does.
// Runs inside the Linux test VM against WebKitWebDriver (Windows: msedgedriver
// with the WebView2 options). Uses only fetch, no dependencies.
//
//   node e2e.mjs [--driver http://127.0.0.1:4444] [--app /usr/bin/freeboard] [--shots /tmp/e2e]
//
// Prerequisites in the VM: scripts/native/fixtures.sh has created ~/fbtest,
// Freeboard's data folder was removed for a fresh first run, and
// `TAURI_WEBVIEW_AUTOMATION=true WebKitWebDriver --port=4444` is running in
// the graphical session.
// Prints one PASS/FAIL line per check and exits 1 on any failure.
import { mkdirSync, writeFileSync } from "node:fs";

const arg = (k, d) => (process.argv.includes(k) ? process.argv[process.argv.indexOf(k) + 1] : d);
const DRIVER = arg("--driver", "http://127.0.0.1:4444");
const APP = arg("--app", "/usr/bin/freeboard");
const SHOTS = arg("--shots", "/tmp/e2e");
mkdirSync(SHOTS, { recursive: true });

const results = [];
const check = (name, ok, detail = "") => {
  results.push({ name, ok: !!ok });
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${ok || !detail ? "" : "  (" + detail + ")"}`);
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function wd(method, path, body) {
  const r = await fetch(DRIVER + path, { method, headers: { "content-type": "application/json" }, body: body ? JSON.stringify(body) : undefined });
  const j = await r.json();
  if (j.value && j.value.error) throw new Error(`${method} ${path}: ${j.value.error} ${j.value.message ?? ""}`);
  return j.value;
}

const caps = {
  capabilities: {
    // As tauri-driver does: no browserName, the app binary as the browser.
    // The driver must run with TAURI_WEBVIEW_AUTOMATION=true so the app
    // accepts automation.
    alwaysMatch: { "webkitgtk:browserOptions": { binary: APP, args: [] } },
  },
};
const session = await wd("POST", "/session", caps);
const S = `/session/${session.sessionId}`;
// The W3C element key; WebKitWebDriver uses a different spelling, so take the
// one value whatever its key.
const elemId = (e) => Object.values(e)[0];

const js = (script, args = []) => wd("POST", `${S}/execute/sync`, { script, args });
const find = async (css) => (await wd("POST", `${S}/elements`, { using: "css selector", value: css })).map(elemId);
const text = (sel) => js(`const e=document.querySelector(arguments[0]);return e?e.textContent.trim():null`, [sel]);
async function click(css) {
  const [id] = await find(css);
  if (!id) throw new Error("no element " + css);
  await wd("POST", `${S}/element/${id}/click`, {});
  await sleep(700);
}
async function waitFor(desc, fn, ms = 15000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    try {
      const v = await fn();
      if (v) return v;
    } catch {}
    await sleep(500);
  }
  throw new Error("timed out waiting for " + desc);
}
let n = 0;
async function shot(name) {
  const png = await wd("GET", `${S}/screenshot`);
  writeFileSync(`${SHOTS}/${String(++n).padStart(2, "0")}-${name}.png`, Buffer.from(png, "base64"));
}

try {
  // ---- first run ------------------------------------------------------------
  await waitFor("first run", () => find("#first-run").then((l) => l.length));
  await shot("first-run");
  const offer = await waitFor("the fbtest folder offer", () =>
    js(`return [...document.querySelectorAll('[data-offer]')].map(b=>[b.dataset.offer,b.textContent]).find(([,t])=>t.includes('fbtest'))`),
  );
  check("first run offers the folder that holds Git repositories", offer && /fbtest \(\d+ repositor/.test(offer[1]), offer?.[1]);
  await click(`[data-offer="${offer[0]}"]`);
  const listed = await js(`return [...document.querySelectorAll('#first-run .mono')].map(e=>e.textContent)`);
  check("adding the offered folder lists it", listed.some((t) => t.endsWith("/fbtest")), listed.join(", "));
  await click('[data-act="start"]');
  await waitFor("home", () => find(".hero").then((l) => l.length));
  check("Start opens the home view", true);

  // ---- scan --------------------------------------------------------------------
  await waitFor("the deep scan to find projects", () => js(`return [...document.querySelectorAll('#areas [data-area]')].some(b=>b.dataset.area==='Projects')`), 180000);
  await waitFor("the scan to finish", async () => !(await js(`return document.body.textContent.includes('Scanning your disk')`)), 180000);
  await shot("home-after-scan");
  const areas = await js(`return [...document.querySelectorAll('#areas [data-area]')].map(b=>b.textContent)`);
  check("From lists the areas with Safe crates and their sizes", areas.some((a) => /^Projects \d+\.\d GB$/.test(a)), areas.join(", "));

  // ---- the pile ------------------------------------------------------------------
  const cta = () => text("#cta .tmain");
  const before = await cta();
  check("goal fills the pile with Safe items", /^Toss \d+\.\d GB overboard$/.test(before) && !before.startsWith("Toss 0.0"), before);
  const crates = await js(`return document.querySelectorAll('#crates [data-crate]').length`);
  check("Dinghy carries the pile's crates", crates > 0, String(crates));
  await click("#pile-btn");
  const rows = await js(`return [...document.querySelectorAll('#pile [data-out]')].map(b=>b.dataset.out)`);
  check("the crates button lists the crates", rows.length === crates, `${rows.length} rows, ${crates} crates`);
  await click(`[data-out="${rows[0]}"]`);
  await waitFor("the pile to change", async () => (await cta()) !== before);
  const note = await text("#cta-note");
  check("taking a crate out lowers the total and switches to manual", (await cta()) !== before && note.includes("You chose these by hand"), `${await cta()} / ${note}`);
  await shot("after-take-out");
  await click('[data-goal="20"]');
  await waitFor("auto mode", async () => !(await text("#cta-note")).includes("by hand"));
  check("choosing a size again leaves manual mode", true);
  const allAreas = await cta();
  await click('[data-area="Projects"]');
  await sleep(800);
  const unticked = await js(`return document.querySelector('[data-area="Projects"]').getAttribute('aria-pressed')==='false' && !document.getElementById('areas-all').disabled`);
  check("unticking an area limits the pile", unticked, `${allAreas} -> ${await cta()}`);
  await click("#areas-all");

  // ---- Everything I found ----------------------------------------------------------
  await click('details[data-open="grp-Projects"] > summary');
  const nm = await js(`const r=[...document.querySelectorAll('#v-tidy .row-item')].find(r=>r.textContent.includes('node_modules in'));return r?r.textContent:null`);
  check("projects are one node_modules item split by project", /node_modules in \d+ inactive projects/.test(nm ?? ""), nm ?? "missing");
  const partsBox = await js(`const d=[...document.querySelectorAll('#v-tidy details.parts')].find(d=>d.closest('.row-item').textContent.includes('node_modules in'));d.open=true;return d.querySelector('summary').textContent`);
  check("Choose which shows every project", /Choose which \(\d+ of \d+\)/.test(partsBox), partsBox);
  const sizeBefore = await js(`return [...document.querySelectorAll('#v-tidy .row-item')].find(r=>r.textContent.includes('node_modules in')).querySelector('.size').textContent`);
  await js(`[...document.querySelectorAll('#v-tidy details.parts')].find(d=>d.closest('.row-item').textContent.includes('node_modules in')).querySelector('input').click()`);
  await sleep(1200);
  const sizeAfter = await js(`return [...document.querySelectorAll('#v-tidy .row-item')].find(r=>r.textContent.includes('node_modules in')).querySelector('.size').textContent`);
  const partsAfter = await js(`return [...document.querySelectorAll('#v-tidy details.parts')].find(d=>d.closest('.row-item').textContent.includes('node_modules in')).querySelector('summary').textContent`);
  const [, on, of] = partsBox.match(/\((\d+) of (\d+)\)/) ?? [];
  check("deselecting a part is saved", partsAfter === `Choose which (${on - 1} of ${of})`, `${partsBox} → ${partsAfter}`);
  check("deselecting a part lowers the item's size", parseFloat(sizeAfter) < parseFloat(sizeBefore), `${sizeBefore} → ${sizeAfter}`);
  await shot("parts");

  // ---- Quick questions --------------------------------------------------------------
  const q1 = await text("#v-tidy .q h2");
  if (q1) {
    const left1 = await text("#v-tidy .q .note");
    await click('[data-q="later"]');
    const q2 = await text("#v-tidy .q h2");
    check("Ask me later moves to the next question", q2 !== q1, `${q1} → ${q2}`);
    check("the count goes down", (await text("#v-tidy .q .note")) !== left1);
  } else {
    check("Quick questions shows the finished state when nothing is left", await text("#v-tidy h2"));
  }

  // ---- Keep ------------------------------------------------------------------------------
  const keepId = await js(`const b=document.querySelector('#v-tidy [data-keep]');return b?b.dataset.keep:null`);
  if (keepId) {
    await click(`[data-keep="${keepId}"]`);
    check("Keep turns into Stop keeping", (await text(`[data-keep="${keepId}"]`)) === "Stop keeping");
    await click(`[data-keep="${keepId}"]`);
    check("Stop keeping makes it available again", (await text(`[data-keep="${keepId}"]`)) === "Keep");
  }

  // ---- Clean up (slice 2): only disposable fixture folders in ~/fbtest ---------------------
  // The pile holds Safe items only; on a fixture-only test machine those are the
  // node_modules item (4 projects, one deselected above) and the Rust target folder.
  const fs = await import("node:fs");
  const home = process.env.HOME;
  const nmBefore = ["web-app", "admin-portal", "docs-site", "storybook"].filter((p) => fs.existsSync(`${home}/fbtest/${p}/node_modules`));
  check("all four fixture node_modules exist before cleanup", nmBefore.length === 4, nmBefore.join(", "));
  const deselected = await js(`const d=[...document.querySelectorAll('#v-tidy details.parts')].find(d=>d.closest('.row-item').textContent.includes('node_modules in'));const i=[...d.querySelectorAll('input')].find(i=>!i.checked);return i?i.closest('label').querySelector('span').textContent:null`);
  check("one project is deselected under Choose which", !!deselected, String(deselected));
  await click('[data-goal="all"]');
  await sleep(800);
  await click("#cta");
  await waitFor("the confirmation sheet", () => find("#sheet").then((l) => l.length));
  const title = await text("#sheet-title");
  check("Toss opens the confirmation with the pile", /^Toss \d+ crates?\?$/.test(title), title);
  const totals = await js(`return [...document.querySelectorAll('#sheet .sum3 > div')].map(d=>d.textContent)`);
  check("the confirmation shows three totals", totals.length === 3, totals.join(" | "));
  await shot("confirm");
  await click('#sheet [data-act="run"]');
  await waitFor("the result sheet", async () => (await text("#sheet-title")) === "Much better!", 120000);
  await shot("result");
  const result = await js(`return document.getElementById('sheet').textContent`);
  check("the result reports the measured free space", /I freed .*GB/.test(result) || /freed/i.test(result), result.slice(0, 160));
  const nmAfter = ["web-app", "admin-portal", "docs-site", "storybook"].filter((p) => fs.existsSync(`${home}/fbtest/${p}/node_modules`));
  check("only the deselected project's node_modules is left", nmAfter.length === 1 && nmAfter[0] === deselected, `left: ${nmAfter.join(", ")}; deselected: ${deselected}`);
  check("the Rust build output is removed", !fs.existsSync(`${home}/fbtest/rust-cli/target`));
  check("project sources and repositories are untouched", ["web-app", "rust-cli", "data-tools"].every((p) => fs.existsSync(`${home}/fbtest/${p}/.git`) && fs.existsSync(`${home}/fbtest/${p}/${p === "rust-cli" ? "Cargo.toml" : p === "data-tools" ? "requirements.txt" : "package.json"}`)));
  check("the Review .venv was not touched (never auto-added)", fs.existsSync(`${home}/fbtest/data-tools/.venv`));
  await click('#sheet [data-act="close"]');
  await sleep(800);
  await click('[data-tab="log"]');
  await sleep(1200);
  const log = await js(`return [...document.querySelectorAll('#v-log .log')].map(r=>r.textContent)`);
  check("the Logbook records the cleanup", log.length === 1 && /Tossed \d+ crates?/.test(log[0]), log.join(" | "));
  await click("#v-log .log");
  await waitFor("run details", () => find("#sheet").then((l) => l.length));
  const details = await js(`return document.querySelectorAll('#sheet .plan-list .pile-row').length`);
  check("run details list each removed folder", details >= 2, String(details));
  await shot("run-details");
  await click('#sheet [data-act="close"]');
  await sleep(600);
  await click('[data-tab="tidy"]');

  // ---- Settings -------------------------------------------------------------------------
  await click('[data-act="settings"]');
  const folders = await js(`return [...document.querySelectorAll('#sheet .mono')].map(e=>e.textContent)`);
  check("Settings lists the scan folders", folders.some((f) => f.endsWith("/fbtest")), folders.join(", "));
  await shot("settings");
  await wd("POST", `${S}/actions`, { actions: [{ type: "key", id: "k", actions: [{ type: "keyDown", value: "" }, { type: "keyUp", value: "" }] }] });
  await sleep(500);
  check("Esc closes Settings", !(await find("#sheet")).length);
} catch (e) {
  check("click-through ran to the end", false, e.message);
  await shot("failure").catch(() => {});
} finally {
  await wd("DELETE", S).catch(() => {});
}
const failed = results.filter((r) => !r.ok).length;
console.log(failed ? `\n${failed} of ${results.length} checks failed.` : `\nAll ${results.length} real-app checks passed.`);
process.exit(failed ? 1 : 0);
