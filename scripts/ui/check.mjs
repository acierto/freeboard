// Interface behaviour checks against the fixture build (dist-visual):
// keyboard order and focus, sheets (focus trap, Esc, focus return), the tray
// view's close paths, and the pile responding to the helm's controls.
//
// Usage: npm run verify:ui   (exits 1 on any failed check)
import { launch, startServer } from "../visual/lib.mjs";

const { server, base } = await startServer();
const browser = await launch();
const results = [];
const check = (name, ok, detail = "") => results.push({ name, ok: !!ok, detail });

async function page(query = "", size = { width: 1120, height: 900 }) {
  const ctx = await browser.newContext({ viewport: size, reducedMotion: "reduce" });
  await ctx.route("**/*", (r) => (new URL(r.request().url()).hostname === "127.0.0.1" ? r.continue() : r.abort()));
  const p = await ctx.newPage();
  await p.goto(`${base}/?${query}`);
  await p.waitForFunction(() => document.documentElement.dataset.busy === "0" && document.querySelector(".page .hero, .tray-window"));
  return { p, ctx };
}
const calls = (p) => p.evaluate(() => (window.__FIXTURE_CALLS__ ?? []).map((c) => c.cmd));
const focused = (p) => p.evaluate(() => {
  const e = document.activeElement;
  return e ? e.id || e.getAttribute("data-goal") || e.getAttribute("data-area") || e.getAttribute("data-act") || e.getAttribute("data-tab") || e.textContent.trim() : null;
});
const idle = (p) => p.waitForFunction(() => document.documentElement.dataset.busy === "0");

// Tray view: Esc and the close button both ask the shell to close it.
{
  const { p, ctx } = await page("view=tray", { width: 312, height: 260 });
  await p.keyboard.press("Escape");
  check("tray view: Esc closes it", (await calls(p)).includes("close_tray_view"));
  await p.evaluate(() => (window.__FIXTURE_CALLS__ = []));
  await p.click("#tray-close");
  check("tray view: close button closes it", (await calls(p)).includes("close_tray_view"));
  await p.click('[data-act="confirm"]');
  check("tray view: Toss opens the pile in the main window", (await calls(p)).includes("review_pile"));
  await ctx.close();
}

// Home: keyboard order and visible focus.
{
  const { p, ctx } = await page();
  const order = [];
  for (let i = 0; i < 18; i++) {
    await p.keyboard.press("Tab");
    order.push(await focused(p));
  }
  const want = ["settings", "20", "50", "100", "all", "Everywhere", "Docker", "Projects", "Caches", "AI models", "Phones and emulators", "Old tools and apps", "Your files", "System", "cta", "pile-btn", "tab-tidy", "tab-map"];
  check("home: Tab order follows the prototype's reading order", JSON.stringify(order) === JSON.stringify(want), order.join(" | "));
  await p.focus("#cta");
  await p.keyboard.press("Shift+Tab");
  await p.keyboard.press("Tab");
  const outline = await p.evaluate(() => getComputedStyle(document.activeElement).outlineStyle);
  check("home: keyboard focus is visible", outline === "solid", outline);

  // Settings sheet: opens from the gear, traps focus, Esc closes, focus returns.
  await p.focus('[data-act="settings"]');
  await p.keyboard.press("Enter");
  await p.waitForSelector("#sheet");
  check("settings: focus moves into the sheet", await p.evaluate(() => document.getElementById("sheet").contains(document.activeElement)));
  let inside = true;
  for (let i = 0; i < 8; i++) {
    await p.keyboard.press("Tab");
    inside &&= await p.evaluate(() => document.getElementById("sheet").contains(document.activeElement));
  }
  check("settings: Tab stays inside the sheet", inside);
  await p.keyboard.press("Escape");
  check("settings: Esc closes the sheet", !(await p.$("#sheet")));
  check("settings: focus returns to the gear", (await focused(p)) === "settings");

  // The pile: take a crate out, then choose a size again.
  await p.click("#pile-btn");
  const before = await p.textContent("#cta");
  await p.click('[data-out="d-cache"]');
  await idle(p);
  const note = await p.textContent("#cta-note");
  check("pile: taking a crate out lowers the total and does not refill", (await p.textContent("#cta")) === "Toss 39.2 GB overboard" && before === "Toss 54.0 GB overboard", await p.textContent("#cta"));
  check("pile: manual mode says so", note.includes("You chose these by hand. Pick a size again to refill the pile."), note);
  await p.click('[data-goal="50"]');
  await idle(p);
  check("pile: choosing a size again refills it", (await p.textContent("#cta")) === "Toss 54.0 GB overboard", await p.textContent("#cta"));
  await p.click('[data-area="Caches"]');
  await idle(p);
  check("pile: Where from limits filling to that group", (await p.textContent("#cta")) === "Toss 34.2 GB overboard", await p.textContent("#cta-note"));
  await ctx.close();
}

// ----- cleanup sheets (slice 2) ------------------------------------------------
const callsOf = (p, name) => p.evaluate((n) => (window.__FIXTURE_CALLS__ ?? []).filter((c) => c.cmd === n).map((c) => c.args), name);
const sheetTitle = (p) => p.evaluate(() => document.getElementById("sheet-title")?.textContent ?? null);
const inSheet = (p) => p.evaluate(() => !!document.getElementById("sheet")?.contains(document.activeElement));
async function tabStaysInside(p, n = 10) {
  let inside = true;
  for (let i = 0; i < n; i++) {
    await p.keyboard.press("Tab");
    inside &&= await inSheet(p);
  }
  return inside;
}

// Toss: the confirmation for the pile traps focus, Esc closes it, focus returns.
{
  const { p, ctx } = await page();
  await p.focus("#cta");
  await p.keyboard.press("Enter");
  await p.waitForSelector("#sheet");
  await idle(p);
  const plan = await callsOf(p, "create_plan");
  check("toss: plans the pile by source, sending no item IDs", plan.length === 1 && plan[0].source === "pile" && plan[0].itemIds.length === 0, JSON.stringify(plan));
  check("toss: the confirmation names the crates", (await sheetTitle(p)) === "Toss 4 crates?", await sheetTitle(p));
  check("toss: three totals", (await p.textContent(".sum3")).replace(/\s+/g, " ") === "54.0 GBFree right away0.0 GBHeld in the Trash0Cannot be recovered", await p.textContent(".sum3"));
  check("toss: commands use the code font, prose actions do not", await p.evaluate(() => {
    const subs = [...document.querySelectorAll(".plan-list .sub")];
    return subs.find((e) => e.textContent === "docker builder prune")?.classList.contains("mono") && !subs.find((e) => e.textContent === "Delete the folders")?.classList.contains("mono");
  }));
  check("toss: focus moves into the sheet", await inSheet(p));
  check("toss: Tab stays inside the sheet", await tabStaysInside(p));
  await p.keyboard.press("Escape");
  check("toss: Esc closes the confirmation", !(await p.$("#sheet")));
  check("toss: focus returns to Toss", (await focused(p)) === "cta", await focused(p));
  check("toss: closing runs nothing", (await callsOf(p, "run_plan")).length === 0);
  await ctx.close();
}

// Remove now on a volume: its own acknowledgment gates the confirm button.
{
  const { p, ctx } = await page();
  await p.click('details[data-open="grp-Docker"] > summary');
  await p.click('[data-now="d-vol"]');
  await p.waitForSelector("#sheet");
  await idle(p);
  const plan = await callsOf(p, "create_plan");
  check("remove now: plans just that item", plan.length === 1 && plan[0].source === "items" && JSON.stringify(plan[0].itemIds) === '["d-vol"]', JSON.stringify(plan));
  check("remove now: one crate, Remove it", (await sheetTitle(p)) === "Toss 1 crate?" && (await p.textContent("#go")) === "Remove it");
  check("remove now: confirm stays disabled until acknowledged", await p.isDisabled("#go"));
  await p.focus("#sheet-ack-d-vol");
  await p.keyboard.press("Space");
  await idle(p);
  check("remove now: the acknowledgment is sent to the backend", JSON.stringify(await callsOf(p, "acknowledge_item")) === JSON.stringify([{ planId: "plan-1", itemId: "d-vol" }]), JSON.stringify(await callsOf(p, "acknowledge_item")));
  check("remove now: acknowledged, confirm is enabled", !(await p.isDisabled("#go")));
  await p.keyboard.press("Space");
  check("remove now: unticking disables confirm again", await p.isDisabled("#go"));
  await p.click('[data-act="close"]');
  check("remove now: Not now closes; focus returns to Remove now", !(await p.$("#sheet")) && (await focused(p)) === "l-now-d-vol", await focused(p));
  await ctx.close();
}

// Remove all here: excludes items needing their own confirmation.
{
  const { p, ctx } = await page();
  await p.click('details[data-open="grp-Docker"] > summary');
  check("remove all here: shows the eligible total", (await p.textContent("#clean-grp-Docker")) === "Remove all here (44.5 GB)", await p.textContent("#clean-grp-Docker"));
  await p.click("#clean-grp-Docker");
  await p.waitForSelector("#sheet");
  await idle(p);
  const ids = (await callsOf(p, "create_plan"))[0]?.itemIds ?? [];
  check("remove all here: no volume or blocked item", !ids.includes("d-vol") && ids.length === 5, JSON.stringify(ids));
  await ctx.close();
}

// Progress: Esc and the scrim do nothing while running; Cancel stops between items.
{
  const { p, ctx } = await page("hold=1");
  await p.click("#cta");
  await p.waitForSelector("#go");
  await p.click("#go");
  await p.waitForSelector("#run-bar");
  await idle(p);
  check("progress: the sheet says what it is checking", (await p.textContent("#run-now")) === "Checking node_modules in quiet projects", await p.textContent("#run-now"));
  check("progress: the bar counts crates", (await p.getAttribute("#run-bar", "style")).includes("25%"));
  check("progress: the first crate leaves the boat", await p.evaluate(() => document.querySelector('[data-crate="b-node"]')?.classList.contains("crate-fly") && !document.querySelector('[data-crate="d-cache"]')?.classList.contains("crate-fly")));
  check("progress: the boat rises with measured free space", (await p.textContent("#free")) === "31.1", await p.textContent("#free"));
  await p.keyboard.press("Escape");
  await p.mouse.click(5, 5);
  check("progress: Esc and the scrim do nothing while running", (await sheetTitle(p)) === "Tossing crates");
  check("progress: focus stays in the sheet", await inSheet(p));
  await p.click('[data-act="cancel"]');
  await p.waitForFunction(() => document.getElementById("sheet-title")?.textContent === "Stopped");
  await idle(p);
  check("cancel: asks the backend to stop", (await callsOf(p, "cancel_run")).length === 1);
  check("cancel: the result says the rest did not start", (await p.textContent("#sheet")).includes("3 items did not start because you stopped the cleanup."), await p.textContent("#sheet"));
  await p.click("#go");
  check("cancel: Done closes and focus returns to Toss", !(await p.$("#sheet")) && (await focused(p)) === "cta", await focused(p));
  await ctx.close();
}

// A full run: result, a reset pile, and a Logbook entry with its details.
{
  const { p, ctx } = await page("skip=wt-exp");
  await p.click('details[data-open="grp-Projects"] > summary');
  await p.check("#l-pile-wt-exp");
  await idle(p);
  await p.click("#cta");
  await p.waitForSelector("#go");
  await p.click("#go");
  await p.waitForFunction(() => document.getElementById("sheet-title")?.textContent === "Much better!");
  await idle(p);
  const text = await p.textContent("#sheet");
  check("result: measured change and new free space", text.includes("I freed 54.0 GB. You now have 66.4 GB free."), text);
  check("result: skipped item and its reason", text.includes("Worktree old-experiment stayed on board. A new commit appeared after the scan."), text);
  check("result: Docker Desktop note", text.includes("Docker Desktop may take a while"));
  check("result: focus is in the sheet", await inSheet(p));
  await p.keyboard.press("Escape");
  await idle(p);
  check("result: Esc closes it once the run is over", !(await p.$("#sheet")));
  check("after a run: the skipped item stays listed as changed", (await p.textContent("#v-tidy")).includes("Changed, scan again"));
  check("after a run: the pile refills from Safe items", (await p.textContent("#cta")).startsWith("Toss "), await p.textContent("#cta"));
  await p.click("#tab-log");
  await idle(p);
  const first = await p.textContent(".log");
  check("logbook: the run is listed first with its skip", first.includes("Today") && first.includes("Tossed 4 crates") && first.includes("1 skipped because it changed"), first);
  await p.focus(".log");
  await p.keyboard.press("Enter");
  await p.waitForSelector("#sheet");
  await idle(p);
  const details = await p.textContent("#sheet");
  check("run details: each item with its action and result", details.includes("docker builder prune") && details.includes("Removed") && details.includes("Skipped: A new commit appeared after the scan."), details);
  check("run details: focus moves into the sheet", await inSheet(p));
  await p.keyboard.press("Escape");
  check("run details: Esc closes and focus returns to the row", !(await p.$("#sheet")) && (await p.evaluate(() => document.activeElement?.classList.contains("log"))));
  await ctx.close();
}

// Empty it: its own confirmation, only Freeboard's items; Recycle Bin on Windows.
{
  const { p, ctx } = await page("trash=2.4");
  await p.click('[data-act="empty"]');
  await p.waitForSelector("#sheet");
  await idle(p);
  check("empty it: its own confirmation", (await sheetTitle(p)) === "Empty 2.4 GB from the Trash?", await sheetTitle(p));
  check("empty it: nothing is emptied before confirming", (await callsOf(p, "empty_freeboard_trash")).length === 0);
  await p.keyboard.press("Escape");
  check("empty it: Esc closes; focus returns to Empty it", !(await p.$("#sheet")) && (await focused(p)) === "empty", await focused(p));
  await p.click('[data-act="empty"]');
  await p.waitForSelector("#go");
  await p.click("#go");
  await p.waitForFunction(() => !document.getElementById("sheet"));
  await idle(p);
  const sent = await callsOf(p, "empty_freeboard_trash");
  check("empty it: sends the confirmation token", sent.length === 1 && typeof sent[0].token === "string" && sent[0].token.length > 0, JSON.stringify(sent));
  check("empty it: the Trash line goes away", (await p.textContent("#trash")).trim() === "");
  await ctx.close();
}
{
  const { p, ctx } = await page("trash=2.4&platform=windows");
  check("windows: the helm says Recycle Bin", (await p.textContent("#trash")).includes("in the Recycle Bin"));
  await p.click('[data-act="empty"]');
  await p.waitForSelector("#sheet");
  check("windows: Empty it says Recycle Bin", (await sheetTitle(p)) === "Empty 2.4 GB from the Recycle Bin?", await sheetTitle(p));
  await ctx.close();
}

// No usable Trash: the item stops, and permanent deletion is a separate plan.
{
  const { p, ctx } = await page("notrash=1");
  await p.click('details[data-open="grp-Projects"] > summary');
  await p.click('[data-now="b-venv"]');
  await p.waitForSelector("#go");
  await p.click("#go");
  await p.waitForSelector('[data-act="permanent"]');
  await idle(p);
  const text = await p.textContent("#sheet");
  check("no Trash: the item was not removed and says why", text.includes(".venv in data-tools was not removed. Trash unavailable"), text);
  check("no Trash: nothing is deleted as a fallback", (await callsOf(p, "create_permanent_plan")).length === 0);
  await p.click('[data-act="permanent"]');
  await p.waitForFunction(() => document.getElementById("sheet-title")?.textContent === "Delete 1 item permanently?");
  await idle(p);
  check("no Trash: permanent deletion is a new plan", JSON.stringify(await callsOf(p, "create_permanent_plan")) === JSON.stringify([{ itemIds: ["b-venv"] }]));
  check("no Trash: it needs its own acknowledgment", (await p.isDisabled("#go")) && (await p.textContent(".sum3")).includes("1Cannot be recovered"));
  check("no Trash: the code font is not used for prose", !(await p.evaluate(() => document.querySelector(".plan-list .sub").classList.contains("mono"))));
  await ctx.close();
}

// Disk map: tiles are real buttons reachable by keyboard; Enter selects one;
// blind spots and the report go to the backend by ID only.
{
  const { p, ctx } = await page();
  await p.click("#tab-map");
  await idle(p);
  const tiles = await p.$$eval("#map .tile", (ts) => ts.map((t) => ({ tag: t.tagName, name: t.getAttribute("aria-label"), tab: t.tabIndex })));
  check("map: every tile is a focusable button with a name and size", tiles.length === 9 && tiles.every((t) => t.tag === "BUTTON" && t.tab === 0 && / GB$/.test(t.name)), JSON.stringify(tiles));
  check("map: the area with the most to remove is selected first", (await p.textContent("#tile-detail h3")) === "AI models");
  await p.focus('[data-tile="projects"]');
  let reached = null;
  for (let i = 0; i < 12 && reached !== "docker"; i++) {
    await p.keyboard.press("Tab");
    reached = await p.evaluate(() => document.activeElement?.getAttribute("data-tile"));
  }
  check("map: Tab moves from tile to tile", reached === "docker", String(reached));
  await p.keyboard.press("Enter");
  await idle(p);
  check("map: Enter selects the focused tile", (await p.textContent("#tile-detail h3")) === "Docker" && (await p.getAttribute('[data-tile="docker"]', "aria-pressed")) === "true");
  const outline = await p.evaluate(() => getComputedStyle(document.activeElement).outlineStyle);
  check("map: the focused tile shows a visible focus ring", outline === "solid", outline);
  await p.focus('[data-tile="unknown"]');
  await p.keyboard.press(" ");
  await idle(p);
  check("map: Not explained lists the blind spots", (await p.$$("#tile-detail .detail-row")).length === 3);
  await p.evaluate(() => (window.__FIXTURE_CALLS__ = []));
  await p.click('[data-spot="mystery"]');
  await idle(p);
  const sent = await p.evaluate(() => window.__FIXTURE_CALLS__.find((c) => c.cmd === "resolve_blind_spot")?.args);
  check("map: a blind spot is resolved by ID and action only", JSON.stringify(sent) === JSON.stringify({ id: "mystery", action: "mark_mine" }), JSON.stringify(sent));
  check("map: a resolved blind spot shows as done and coverage rises", (await p.$$(".spot.done")).length === 1 && (await p.textContent(".cover b")) === "96.7% explained", await p.textContent(".cover b"));
  await p.click('[data-act="report"]');
  await idle(p);
  await p.waitForSelector("#report-box textarea, #report-box p");
  const report = await p.evaluate(() => document.querySelector("#report-text")?.value ?? "copied");
  check("map: the disk report is shown or copied", report === "copied" || JSON.parse(report).free_gb === 12.4, report.slice(0, 80));
  await ctx.close();
}

await browser.close();
server.close();
for (const r of results) console.log(`${r.ok ? "PASS" : "FAIL"}  ${r.name}${r.ok || !r.detail ? "" : "  (" + r.detail + ")"}`);
const failed = results.filter((r) => !r.ok).length;
console.log(failed ? `\n${failed} interface check(s) failed.` : `\nAll ${results.length} interface checks passed.`);
process.exit(failed ? 1 : 0);
