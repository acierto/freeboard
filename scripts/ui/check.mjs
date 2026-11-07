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
  for (let i = 0; i < 13; i++) {
    await p.keyboard.press("Tab");
    order.push(await focused(p));
  }
  // "All areas" is disabled while every area is ticked, so Tab skips it.
  const want = ["settings", "20", "50", "100", "all", "Docker", "Projects", "Caches", "cta", "pile-btn", "tab-tidy", "tab-map", "tab-log"];
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
  const toss = () => p.textContent("#cta .tmain");
  await p.click("#pile-btn");
  check("pile: the crates button opens the list", (await p.getAttribute("#pile-btn", "aria-expanded")) === "true" && !(await p.$eval("#pile", (e) => e.hidden)));
  const before = await toss();
  await p.click('[data-out="d-cache"]');
  await idle(p);
  const note = await p.textContent("#cta-note");
  check("pile: taking a crate out lowers the total and does not refill", (await toss()) === "Toss 39.2 GB overboard" && before === "Toss 54.0 GB overboard", await toss());
  check("pile: manual mode says so", note.includes("You chose these by hand. Pick a size again to refill the pile."), note);
  check("pile: the meter says chosen by hand", (await p.textContent("#meter")).includes("chosen by hand"), await p.textContent("#meter"));
  await p.click('[data-goal="50"]');
  await idle(p);
  check("pile: choosing a size again refills it", (await toss()) === "Toss 54.0 GB overboard", await toss());
  // From: every area starts ticked; unticking Caches leaves Docker and Projects.
  await p.click('[data-area="Caches"]');
  await idle(p);
  const meter = await p.textContent("#meter");
  check("pile: unticking an area stops filling from it", (await toss()) === "Toss 48.9 GB overboard", await toss());
  check("pile: the meter shows the gap to the goal", meter.includes("48.9 GB ready to toss") && meter.includes("goal 50 GB") && meter.includes("1.1 GB more needs your OK"), meter);
  await p.click('[data-area="Docker"]');
  await p.click('[data-area="Projects"]');
  await idle(p);
  check("pile: unticking every area says so", (await p.textContent("#cta-note")).includes("No areas picked.") && (await p.isDisabled("#cta")), await p.textContent("#cta-note"));
  await p.click("#areas-all");
  await idle(p);
  check("pile: Use all areas ticks them all again", (await toss()) === "Toss 54.0 GB overboard" && (await p.isDisabled("#areas-all")), await toss());
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
  check("toss: the confirmation names the items", (await sheetTitle(p)) === "Toss 4 items?", await sheetTitle(p));
  const sum = await p.textContent(".cf-sum");
  check("toss: free right away, and what kind of items", sum.includes("54.0 GB free right away") && sum.includes("All rebuildable by their tools") && !sum.includes("cannot be recovered"), sum);
  check("toss: the button says how much", (await p.textContent("#go")) === "Toss 54.0 GB", await p.textContent("#go"));
  check("toss: commands use the code font, prose actions do not", await p.evaluate(() => {
    const runs = [...document.querySelectorAll(".cf-lines dd")];
    return runs.find((e) => e.textContent === "docker builder prune")?.classList.contains("mono") && !runs.find((e) => e.textContent === "Delete the folders")?.classList.contains("mono");
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
  // What Risky means: folded on a Review question, open on a Risky item.
  const qBox = await p.$eval("#v-tidy .q", (q) => {
    const b = q.querySelector(".riskx");
    return b ? { open: b.open, title: b.querySelector("summary").textContent } : null;
  });
  check("risk: a Review question explains Review, folded", qBox?.title === "What \u201cReview\u201d means here" && qBox.open === false, JSON.stringify(qBox));
  await p.click('details[data-open="grp-Docker"] > summary');
  check("rows: details are folded until a row is opened", !(await p.$('[data-now="d-vol"]')));
  await p.click("#l-x-d-vol");
  check("rows: opening a row shows its details and actions", (await p.getAttribute("#l-x-d-vol", "aria-expanded")) === "true" && !!(await p.$('[data-now="d-vol"]')));
  const box = await p.$eval('[data-risk-box="d-vol"]', (b) => ({
    open: b.open,
    on: b.querySelector(".rx-lvl.on b")?.textContent,
    why: b.querySelectorAll(".rx-why li").length,
    safer: [...b.querySelectorAll(".rx-safer code")].map((c) => c.textContent),
  }));
  check("risk: a Risky item explains itself, open, with its place on the scale", box.open && box.on === "Risky" && box.why === 3, JSON.stringify(box));
  check("risk: it says how to keep a copy first", box.safer.length === 2 && box.safer[1].includes("tar czf /backup/billing_pgdata.tgz"), JSON.stringify(box.safer));
  await p.click("#l-x-d-cache");
  check("risk: Safe items have no explanation box", !(await p.$('[data-risk-box="d-cache"]')));
  await p.click('[data-now="d-vol"]');
  await p.waitForSelector("#sheet");
  await idle(p);
  const plan = await callsOf(p, "create_plan");
  check("remove now: plans just that item", plan.length === 1 && plan[0].source === "items" && JSON.stringify(plan[0].itemIds) === '["d-vol"]', JSON.stringify(plan));
  check("remove now: one item, named, with its size on the button", /^Remove .+\?$/.test(await sheetTitle(p)) && /^Remove \d+\.\d GB$/.test(await p.textContent("#go")), `${await sheetTitle(p)} / ${await p.textContent("#go")}`);
  check("remove now: confirm stays disabled until acknowledged", await p.isDisabled("#go"));
  await p.focus("#sheet-ack-d-vol");
  await p.keyboard.press("Space");
  await idle(p);
  check("remove now: the acknowledgment is sent to the backend", JSON.stringify(await callsOf(p, "acknowledge_item")) === JSON.stringify([{ planId: "plan-1", itemId: "d-vol" }]), JSON.stringify(await callsOf(p, "acknowledge_item")));
  check("remove now: acknowledged, confirm is enabled", !(await p.isDisabled("#go")));
  await p.keyboard.press("Space");
  check("remove now: unticking disables confirm again", await p.isDisabled("#go"));
  await p.click('[data-act="close"]');
  check("remove now: Cancel closes; focus returns to Remove now", !(await p.$("#sheet")) && (await focused(p)) === "l-now-d-vol", await focused(p));
  await ctx.close();
}

// Remove all here: excludes items needing their own confirmation.
{
  const { p, ctx } = await page();
  await p.click('details[data-open="grp-Docker"] > summary');
  check("remove all: counts the eligible items", (await p.textContent("#clean-grp-Docker")) === "Remove all 5…" && (await p.textContent("#add-grp-Docker")) === "Add all 5 to the pile (44.5 GB)", await p.textContent("#add-grp-Docker"));
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
  check("progress: which one of how many", (await p.textContent("#run-of")) === "1 of 4", await p.textContent("#run-of"));
  check("progress: the deck shows a crate per item, the first one gone", (await p.$$eval("#deck .cube", (c) => c.length)) === 4 && (await p.getAttribute("#cube-0", "class")) === "cube gone", await p.getAttribute("#cube-0", "class"));
  check("progress: space freed so far and the tally", (await p.textContent("#run-freed")) === "18.7 GB" && (await p.textContent("#run-tally")) === "1 tossed", `${await p.textContent("#run-freed")} / ${await p.textContent("#run-tally")}`);
  check("progress: the first crate leaves the boat", await p.evaluate(() => document.querySelector('[data-crate="b-node"]')?.classList.contains("crate-fly") && !document.querySelector('[data-crate="d-cache"]')?.classList.contains("crate-fly")));
  check("progress: the boat rises with measured free space", (await p.textContent("#free")) === "31.1", await p.textContent("#free"));
  await p.keyboard.press("Escape");
  await p.mouse.click(5, 5);
  check("progress: Esc and the scrim do nothing while running", (await sheetTitle(p)) === "Tossing crates");
  check("progress: focus stays in the sheet", await inSheet(p));
  check("progress: Stop says it waits for the item that is running", (await p.textContent('[data-act="cancel"]')) === "Stop after this one");
  await p.click('[data-act="cancel"]');
  await p.waitForSelector(".res");
  await idle(p);
  check("cancel: asks the backend to stop", (await callsOf(p, "cancel_run")).length === 1);
  const stopped = await p.textContent("#sheet");
  check("cancel: the result lists what did not start", stopped.includes("Not started") && stopped.includes("You stopped the run before these"), stopped);
  check("cancel: Try again offers what did not start", (await p.textContent("#retry")) === "Try 3 again", await p.textContent("#retry"));
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
  await p.waitForFunction(() => document.getElementById("sheet-title")?.textContent === "Done, mostly");
  await idle(p);
  const text = await p.textContent("#sheet");
  check("result: measured change, before and after", text.includes("54.0 GB freed") && text.includes("12.4 GB free before") && text.includes("66.4 GB free now"), text);
  check("result: what stayed on board, and why", text.includes("Stayed on board") && text.includes("Worktree old-experiment A new commit appeared after the scan."), text);
  check("result: what went is folded", !(await p.$eval("details.res.safe", (d) => d.open)) && (await p.$eval("details.res.review", (d) => d.open)));
  check("result: Docker Desktop note", text.includes("Docker Desktop can take a while"));
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
  await p.click("#l-x-b-venv");
  await p.click('[data-now="b-venv"]');
  await p.waitForSelector("#go");
  await p.click("#go");
  await p.waitForSelector('[data-act="permanent"]');
  await idle(p);
  const text = await p.textContent("#sheet");
  check("no Trash: the item could not go and says why", text.includes("Could not go") && text.includes(".venv in data-tools") && text.includes("Trash unavailable"), text);
  check("no Trash: nothing is deleted as a fallback", (await callsOf(p, "create_permanent_plan")).length === 0);
  await p.click('[data-act="permanent"]');
  await p.waitForFunction(() => document.getElementById("sheet-title")?.textContent === "Delete 1 item permanently?");
  await idle(p);
  check("no Trash: permanent deletion is a new plan", JSON.stringify(await callsOf(p, "create_permanent_plan")) === JSON.stringify([{ itemIds: ["b-venv"] }]));
  check("no Trash: it needs its own acknowledgment", (await p.isDisabled("#go")) && (await p.textContent(".cf-flags")).includes("1 cannot be recovered"));
  check("no Trash: the code font is not used for prose", !(await p.evaluate(() => document.querySelector(".cf-lines dd").classList.contains("mono"))));
  await ctx.close();
}

// A failure: reasons are grouped, and Try again plans only what could not go.
{
  const { p, ctx } = await page("fail=d-cache");
  await p.click("#cta");
  await p.waitForSelector("#go");
  await p.click("#go");
  await p.waitForSelector("#retry");
  await idle(p);
  const text = await p.textContent("details.res.risky");
  check("failure: Could not go names the reason once", text.includes("Could not go") && text.includes("Docker did not answer within 20 seconds"), text);
  await p.click("#retry");
  await p.waitForFunction(() => /^Remove .+\?$/.test(document.getElementById("sheet-title")?.textContent ?? ""));
  await idle(p);
  const plans = await callsOf(p, "create_plan");
  check("try again: plans just the item that could not go", plans.length === 2 && plans[1].source === "items" && JSON.stringify(plans[1].itemIds) === '["d-cache"]', JSON.stringify(plans));
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
  await p.focus('[data-tile="unknown"]');
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
  const causes = await p.$$eval("#tile-detail .detail-row > span:first-child", (r) => r.map((e) => e.textContent));
  check("map: Not explained lists its causes", JSON.stringify(causes) === JSON.stringify(["In folders I don\u2019t scan","Locked by macOS", "Not traced to a folder", "Files I don\u2019t recognize"]), JSON.stringify(causes));
  check("map: areas under 3% are chips, not slivers", !(await p.$('#map [data-tile="browser"]')) && (await p.$$(".small-areas .tile-chip")).length === 2);
  await p.click('.tile-chip[data-tile="fonts"]');
  await idle(p);
  check("map: a chip opens its area", (await p.textContent("#tile-detail h3")) === "Fonts and plug-ins" && (await p.getAttribute('.tile-chip[data-tile="fonts"]', "aria-pressed")) === "true");
  const why = await p.textContent("#why");
  check("map: the Why card says how much and that nothing is lost", why.startsWith("Why 160.0 GB isn\u2019t explained") && why.includes("None of it is lost."), why.slice(0, 80));
  check("map: the folder list shows four, the rest behind Show more", (await p.$eval("#why .fix-list", (e) => e.querySelectorAll(".fix").length)) === 4 && (await p.textContent('[data-why-more="outside"]')) === "Show 4 more, 10.7 GB");
  await p.evaluate(() => (window.__FIXTURE_CALLS__ = []));
  await p.click('[data-spot="mystery"]');
  await idle(p);
  const sent = await p.evaluate(() => window.__FIXTURE_CALLS__.find((c) => c.cmd === "resolve_blind_spot")?.args);
  check("map: a blind spot is resolved by ID and action only", JSON.stringify(sent) === JSON.stringify({ id: "mystery", action: "mark_mine" }), JSON.stringify(sent));
  check("map: a fixed spot moves to the fixed list and coverage rises", (await p.textContent("#why")).includes("1 fixed") && (await p.textContent(".cover b")) === "68.8% explained", await p.textContent(".cover b"));
  await p.click("#mine-vms");
  await idle(p);
  const skip = await p.evaluate(() => window.__FIXTURE_CALLS__.filter((c) => c.cmd === "resolve_blind_spot").map((c) => c.args));
  check("map: Skip counts a folder as yours (mark_mine)", JSON.stringify(skip[1]) === JSON.stringify({ id: "vms", action: "mark_mine" }), JSON.stringify(skip));
  await p.click("#all-outside");
  await idle(p);
  const all = await p.evaluate(() => window.__FIXTURE_CALLS__.find((c) => c.cmd === "resolve_blind_spots")?.args);
  check("map: Scan all sends every open folder in one request", all?.action === "scan_folder" && all.spotIds.length === 7 && !all.spotIds.includes("vms"), JSON.stringify(all));
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
