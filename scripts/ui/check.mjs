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
