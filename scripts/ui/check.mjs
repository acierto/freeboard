import { readFileSync } from "node:fs";
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

async function page(query = "", size = { width: 1120, height: 900 }, locale = "en-US") {
  const ctx = await browser.newContext({ viewport: size, reducedMotion: "reduce", locale });
  await ctx.route("**/*", (r) => (new URL(r.request().url()).hostname === "127.0.0.1" ? r.continue() : r.abort()));
  const p = await ctx.newPage();
  await p.goto(`${base}/?${query}`);
  await p.waitForFunction(() => document.documentElement.dataset.busy === "0" && document.querySelector(".page .hero, .tray-window, #ob-root"));
  return { p, ctx };
}
const calls = (p) => p.evaluate(() => (window.__FIXTURE_CALLS__ ?? []).map((c) => c.cmd));
const focused = (p) => p.evaluate(() => {
  const e = document.activeElement;
  return e ? e.id || e.getAttribute("data-goal") || e.getAttribute("data-area") || e.getAttribute("data-act") || e.getAttribute("data-tab") || e.textContent.trim() : null;
});
const LG_NONE = JSON.parse(readFileSync(new URL("../../src/i18n/en.json", import.meta.url), "utf8"))["lg.none"];
const idle = (p) => p.waitForFunction(() => document.documentElement.dataset.busy === "0");

// Tray view: Esc and the close button both ask the shell to close it.
{
  const { p, ctx } = await page("view=tray", { width: 362, height: 560 });
  const free = await p.textContent(".tb-free");
  check("menu bar window: free space, status and a Dinghy line", free.includes("12.4 GB free") && free.includes("Low") && (await p.textContent(".tb-line")).startsWith("Riding low."), free);
  check("menu bar window: the three biggest crates and the rest", (await p.$$(".tl li")).length === 4 && (await p.textContent(".tl-more")).startsWith("+ 1 more"), await p.textContent(".tl"));
  // The fixture's last scan is fixed (2026-11-04 11:42 UTC); these checks run on the real clock.
  check("menu bar window: when it last scanned", /^Scanned (just now|\d+ (min|hours?|days?) ago)$/.test(await p.textContent(".tb-foot .note")), await p.textContent(".tb-foot .note"));
  await p.click('[data-act="tray-questions"]');
  check("menu bar window: quick questions open the main window there", JSON.stringify((await p.evaluate(() => window.__FIXTURE_CALLS__)).find((c) => c.cmd === "open_main_view")?.args) === '{"view":"questions"}');
  await p.keyboard.press("Escape");
  check("tray view: Esc closes it", (await calls(p)).includes("close_tray_view"));
  await p.evaluate(() => (window.__FIXTURE_CALLS__ = []));
  await p.click("#tray-close");
  check("tray view: close button closes it", (await calls(p)).includes("close_tray_view"));
  await p.click('[data-act="confirm"]');
  check("tray view: Toss opens the pile in the main window", (await calls(p)).includes("review_pile"));
  await ctx.close();
}

// Menu bar window when Healthy: the Toss button is the quiet style, marked optional,
// and the forecast line is there in every status.
{
  const low = await page("view=tray", { width: 362, height: 560 });
  // Pace from this week's rate (spec v21): 21.6 GB a week is +3.1 GB a day; 12.4 GB lasts about 4 days.
  check("menu bar window: the pace says when the disk is full", (await low.p.textContent(".tb-pace")) === "At this week\u2019s pace (+3.1 GB a day), full in about 4 days.", await low.p.textContent(".tb-pace"));
  check("menu bar window: the zones gauge under 60 GB free", !!(await low.p.$(".tg")) && !(await low.p.$(".tcap")));
  check("menu bar window: Toss is the primary style when Low", await low.p.$eval(".tb-toss", (b) => b.classList.contains("primary")));
  await low.ctx.close();
  const { p, ctx } = await page("view=tray&free=120", { width: 362, height: 560 });
  check("menu bar window: Healthy status", (await p.textContent(".tb-free")).includes("Healthy"), await p.textContent(".tb-free"));
  check("menu bar window: Toss is the quiet style when Healthy", await p.$eval(".tb-toss", (b) => b.classList.contains("calm") && !b.classList.contains("primary")));
  check("menu bar window: Healthy says tossing is optional", !!(await p.$(".tb-c")));
  check("menu bar window: the pace shows when Healthy too", (await p.textContent(".tb-pace")) === "At this week\u2019s pace (+3.1 GB a day), full in about 6 weeks.", await p.textContent(".tb-pace"));
  check("menu bar window: the whole-disk bar at 60 GB or more", !(await p.$(".tg")) && (await p.textContent(".tcap-lbl")) === "392.0 GB usedof 512 GB", await p.textContent(".tcap-lbl"));
  check("menu bar window: This week, with the fastest area", (await p.textContent(".tb-grow")) === "This week +21.6 GBMostly Docker (+9.3 GB). See where \u203a", await p.textContent(".tb-grow"));
  await p.evaluate(() => (window.__FIXTURE_CALLS__ = []));
  await p.click('[data-act="tray-map"]');
  check("menu bar window: See where opens the Disk map", JSON.stringify((await p.evaluate(() => window.__FIXTURE_CALLS__)).find((c) => c.cmd === "open_main_view")?.args) === '{"view":"map"}');
  await ctx.close();
  // 189.8 GB free with +63.7 GB a week: about 3 weeks, never 3 days.
  const roomy = await page("view=tray&free=189.8&week=63.7", { width: 372, height: 676 });
  check("menu bar window: 189.8 GB at +63.7 GB a week is about 3 weeks", (await roomy.p.textContent(".tb-pace")) === "At this week\u2019s pace (+9.1 GB a day), full in about 3 weeks.", await roomy.p.textContent(".tb-pace"));
  await roomy.ctx.close();
  // Healthy but full within 14 days: the warning line and the filled Toss.
  const fast = await page("view=tray&free=40", { width: 372, height: 676 });
  check("menu bar window: filling fast while Healthy", (await fast.p.textContent(".tb-line")) === "Plenty of room for now, but it\u2019s filling fast." && (await fast.p.$eval(".tb-toss", (b) => b.classList.contains("primary"))) && !(await fast.p.$(".tb-c")), await fast.p.textContent(".tb-line"));
  await fast.ctx.close();
  // Not filling up: no forecast that contradicts the status.
  const flat = await page("view=tray&free=120&week=0", { width: 372, height: 676 });
  check("menu bar window: not filling up this week", (await flat.p.textContent(".tb-pace")) === "Not filling up this week." && !(await flat.p.$(".tb-grow")), await flat.p.textContent(".tb-pace"));
  await flat.ctx.close();
}

// Updates (spec "Menu bar window" v21; docs/decisions.md "Updates only when
// asked"): nothing is checked until the chip is clicked.
{
  const { p, ctx } = await page("view=tray", { width: 372, height: 676 });
  await p.waitForSelector(".tb-ver");
  const calls = async () => (await p.evaluate(() => window.__FIXTURE_CALLS__)).map((c) => c.cmd);
  check("updates: nothing is checked on opening", !(await calls()).some((c) => c === "check_for_update" || c === "download_update"), JSON.stringify(await calls()));
  check("updates: the chip shows the installed version", (await p.textContent(".tb-ver")) === "v1.4.0");
  await p.click('[data-act="upd-check"]');
  check("updates: Checking…", (await p.textContent(".tb-ver")) === "Checking\u2026", await p.textContent(".tb-ver"));
  await p.waitForSelector('.tb-ver.new[data-act="upd-open"]');
  check("updates: an update turns the chip accent and opens the banner", (await p.textContent(".tb-ver")) === "1.4.1 available" && (await p.textContent(".tb-upd-t")) === "Freeboard 1.4.1 is readyYou have 1.4.0. New: faster worktree scans, removal queue.", await p.textContent(".tb-upd"));
  await p.click('[data-act="upd-go"]');
  check("updates: Downloading", (await p.textContent(".tb-ver")) === "Downloading 1.4.1\u2026" && !(await p.$(".tb-upd")), await p.textContent(".tb-ver"));
  await p.waitForSelector('[data-act="upd-restart"]');
  check("updates: Restart to update", (await p.textContent(".tb-ver")) === "Restart to update");
  await p.click('[data-act="upd-restart"]');
  await p.waitForFunction(() => document.querySelector(".tb-ver")?.textContent?.includes("up to date"));
  check("updates: after the restart, up to date", (await p.textContent(".tb-ver")) === "v1.4.1 \u00b7 up to date \u2713", await p.textContent(".tb-ver"));
  await ctx.close();
}

// Languages (spec "Languages"): Settings → Language applies at once; setup
// step 1 has a picker preset to the system's language; sizes follow it.
{
  const { p, ctx } = await page();
  await p.click('[data-act="settings"]');
  const tiles = await p.$$eval(".lang-tile", (l) => l.map((x) => [x.dataset.applang, x.getAttribute("aria-checked"), x.querySelector("b").textContent, x.querySelector("small").textContent]));
  check("language: six tiles, Same as my Mac chosen, each name in its own language", JSON.stringify(tiles) === JSON.stringify([["system", "true", "Same as my Mac", "English"], ["en", "false", "English", "12.4 GB free"], ["nl", "false", "Nederlands", "12,4 GB vrij"], ["de", "false", "Deutsch", "12,4 GB frei"], ["fr", "false", "Français", "12,4 Go libres"], ["es", "false", "Español", "12,4 GB libres"]]), JSON.stringify(tiles));
  await p.evaluate(() => (window.__FIXTURE_CALLS__ = []));
  await p.click('[data-applang="de"]');
  await idle(p);
  check("language: picking one saves it", JSON.stringify((await p.evaluate(() => window.__FIXTURE_CALLS__)).find((c) => c.cmd === "update_settings")?.args) === '{"patch":{"language":"de"}}');
  check("language: everything switches at once, the sheet stays open", (await p.textContent("#sheet-title")) === "Einstellungen" && (await p.textContent("#make-title")) === "Platz schaffen" && (await p.getAttribute('[data-applang="de"]', "aria-checked")) === "true" && (await p.getAttribute("html", "lang")) === "de");
  await p.click("#settings-done");
  check("language: sizes take a decimal comma", (await p.textContent("#free")) === "12,4" && (await p.textContent("#cta .tmain")).startsWith("Toss ") === false && /^\d+,\d GB über Bord$/.test(await p.textContent("#cta .tmain")), await p.textContent("#cta .tmain"));
  await ctx.close();
}
{
  const { p, ctx } = await page("view=tray&lang=fr");
  check("language: the menu bar window in French, with Go and translated item names", (await p.textContent(".tb-free")).includes("12,4 Go libres") && (await p.textContent(".tl")).includes("Cache de build Docker") && /^Analysé il y a \d+ (min|h|heures?|jours?)$/.test(await p.textContent(".tb-foot .note")), await p.textContent(".tb-foot .note"));
  await ctx.close();
}
{
  const { p, ctx } = await page("firstrun=1", undefined, "de-DE");
  check("language: setup follows the system's language, the picker preset to it", (await p.textContent("#ob-title")) === "Wissen, bevor die Platte voll ist." && (await p.inputValue("#ob-lang-sel")) === "de");
  await p.evaluate(() => (window.__FIXTURE_CALLS__ = []));
  await p.selectOption("#ob-lang-sel", "fr");
  await idle(p);
  check("language: switching in setup applies at once and is saved", (await p.textContent("#ob-title")) === "Soyez prévenu avant que le disque soit plein." && JSON.stringify((await p.evaluate(() => window.__FIXTURE_CALLS__)).find((c) => c.cmd === "update_settings")?.args) === '{"patch":{"language":"fr"}}');
  await p.click("#ob-next");
  check("language: the picker is on step 1 only", !(await p.$("#ob-lang-sel")));
  await ctx.close();
}

// Worktrees (spec "Worktrees view"): status tiles filter, a repository's
// ready ones are selected at once, removal is the normal plan and run, one
// at a time; a timeout is listed with Try N again.
{
  const { p, ctx } = await page();
  check("worktrees: the tab badge counts the Ready ones", (await p.textContent("#tab-wt")) === "Worktrees15", await p.textContent("#tab-wt"));
  check("worktrees: they are not counted as quick questions", (await p.textContent("#tab-tidy")) === "Quick questions19", await p.textContent("#tab-tidy"));
  await p.click("#tab-wt");
  await idle(p);
  check("worktrees: one summary line", (await p.textContent(".wt-head .muted")) === "20 in 6 repositories · 23.0 GB", await p.textContent(".wt-head .muted"));
  const tiles = await p.$$eval(".wt-leg", (l) => l.map((x) => x.textContent));
  check("worktrees: four status tiles with count and size", JSON.stringify(tiles) === JSON.stringify(["Ready to remove15 · 17.0 GB", "Needs a look2 · 2.0 GB", "Has your work2 · 3.5 GB", "Main checkouts1 · 0.5 GB"]), JSON.stringify(tiles));
  await p.click('[data-wt-filter="work"]');
  check("worktrees: a tile filters the list", (await p.$$(".wt-row")).length === 2 && (await p.getAttribute('[data-wt-filter="work"]', "aria-pressed")) === "true");
  await p.click('[data-wt-filter="work"]');
  check("worktrees: a second click clears the filter", (await p.$$(".wt-row")).length === 20);
  await p.click('[data-wt-filter="agent"]');
  check("worktrees: Made by agents shows the agent branches", (await p.$$(".wt-row")).length === 12);
  await p.click('[data-wt-filter="all"]');
  check("worktrees: Has your work and main can't be selected", (await p.isDisabled('[data-wt-sel="w3"]')) && (await p.isDisabled('[data-wt-sel="w7"]')) && !(await p.$('[data-wt-sel^="main:"]')));
  await p.click('[data-wt-open="w7"]');
  check("worktrees: uncommitted files: the exact command has --force", (await p.textContent(".wt-row.open .wt-cmd")) === "git -C ~/proj/knowback worktree remove --force .worktrees/admin-users-data");
  await p.click('[data-wt-ack="w7"]');
  check("worktrees: acknowledging the files makes it selectable", !(await p.isDisabled('[data-wt-sel="w7"]')));
  await p.click('[data-wt-sel="w7"]');
  await p.evaluate(() => (window.__FIXTURE_CALLS__ = []));
  await p.click('[data-wt-act="remove"]');
  await p.waitForFunction(() => /^Done\. 1 removed/.test(document.querySelector(".wt-now")?.textContent ?? ""));
  const acked = (await p.evaluate(() => window.__FIXTURE_CALLS__)).filter((c) => c.cmd === "acknowledge_item");
  check("worktrees: the plan records the acknowledgment before it runs", acked.length === 1 && acked[0].args.itemId === "w7", JSON.stringify(acked));
  await p.click('[data-wt-act="dismiss"]');
  check("worktrees: Needs a look can be selected by hand", !(await p.isDisabled('[data-wt-sel="w6"]')) && !(await p.isDisabled('[data-wt-sel="w19"]')));
  await p.click('[data-wt-open="w5"]');
  const after = await p.textContent(".wt-row.open .wt-after");
  check("worktrees: an unmerged branch stays, with how to reopen it", after === "Only the folder goes. Branch codex/ios-testflight-20261019 stays with its 3 commits. Reopen it with git worktree add .codex/worktrees/ios-testflight codex/ios-testflight-20261019.", after);
  check("worktrees: the exact commands, with no branch delete", (await p.textContent(".wt-row.open .wt-cmd")) === "git -C ~/proj/leftyou worktree remove .codex/worktrees/ios-testflight");
  await p.click('[data-wt-open="w5"]');
  await p.click('[data-wt-selrepo="sheltersync"]');
  check("worktrees: Select N ready selects that repository's ready ones", (await p.textContent(".wt-dock")).startsWith("8 selected · 3.9 GB") && (await p.textContent('[data-wt-act="remove"]')) === "Remove 8 worktrees 3.9 GB", await p.textContent(".wt-dock"));
  await p.evaluate(() => (window.__FIXTURE_CALLS__ = []));
  await p.click('[data-wt-act="remove"]');
  await p.waitForSelector('.wt-dock.fin [data-wt-act="retry"]');
  await idle(p);
  const sent = await p.evaluate(() => window.__FIXTURE_CALLS__);
  const order = sent.filter((c) => c.cmd === "create_plan").map((c) => c.args.itemIds.join(","));
  check("worktrees: one confirmed plan per worktree, biggest first", JSON.stringify(order) === '["w12","w13","w14","w15","w16","w17","w18","w20"]', JSON.stringify(order));
  const dockText = await p.textContent(".wt-dock");
  check("worktrees: the result in the dock, the timeout named, with Try 1 again", dockText.startsWith("Done. 7 removed, 1 could not go") && dockText.includes("worktree-agent-a447c27fa307f5ebd timed out after 120 s") && dockText.includes("Try 1 again"), dockText);
  check("worktrees: removed ones stay, struck through, until the dock is closed", (await p.$$(".wt-row.removed")).length === 7 && (await p.textContent('.wt-row.removed:has([data-wt-open="w12"]) .wt-why')).endsWith("Removed. Branch kept.") && (await p.textContent("#tab-wt")) === "Worktrees8", await p.textContent("#tab-wt"));
  check("worktrees: the one that timed out says nothing changed", (await p.textContent('.wt-row:has([data-wt-open="w18"]) .wt-why')).endsWith("Timed out after 120 s. Still here, nothing changed."));
  await p.click('[data-wt-act="retry"]');
  await p.waitForFunction(() => /^Done\. 8 removed/.test(document.querySelector(".wt-now")?.textContent ?? ""));
  check("worktrees: Try again removes the one that timed out", !!(await p.$('.wt-row.removed:has([data-wt-open="w18"])')));
  await p.click('[data-wt-act="dismiss"]');
  check("worktrees: Close clears the dock and the removed rows", !(await p.$(".wt-dock")) && !(await p.$(".wt-row.removed")) && !(await p.$('[data-wt-open="w12"]')));
  await ctx.close();
}

// The removal queue (prototype Version 20, spec "Removal queue"): the list
// stays usable while worktrees are removed one at a time.
{
  const { p, ctx } = await page("wtms=700");
  await p.click("#tab-wt");
  await p.click('[data-wt-sel="w1"]');
  await p.click('[data-wt-sel="w2"]');
  await p.click('[data-wt-sel="w5"]');
  await p.click('[data-wt-act="remove"]');
  await p.waitForSelector(".wt-dock.run");
  const now0 = await p.textContent(".wt-now");
  check("queue: the dock names the one being removed, and how many wait", now0.startsWith("Removing worktree-agent-aeee3547468b1220d (loitr)") && (await p.textContent(".wt-now small")) === "1 of 3 · 2 waiting", now0);
  check("queue: the summary card says it runs in the background", (await p.textContent(".wt-hint")) === "Removing in the background. Keep ticking worktrees; they join the queue.");
  check("queue: waiting rows show their place and Take out", (await p.textContent('.wt-row.queued:has([data-wt-open="w2"]) .wt-state')) === "1" && !!(await p.$('.wt-row [data-wt-unq="w2"]')));
  check("queue: checkboxes and Select N ready stay active", !(await p.isDisabled('[data-wt-sel="w12"]')) && !(await p.isDisabled('[data-wt-selrepo="sheltersync"]')));
  await p.click('[data-wt-sel="w12"]');
  check("queue: the main button becomes Add to queue", (await p.textContent('[data-wt-act="enqueue"]')) === "+ Add 1 to queue 0.6 GB", await p.textContent(".wt-dock"));
  await p.click('[data-wt-act="enqueue"]');
  check("queue: added ones go to the end", (await p.textContent('.wt-row.queued:has([data-wt-open="w12"]) .wt-state')) === "3", await p.textContent(".wt-now small"));
  await p.click('[data-wt-act="qtoggle"]');
  const ql = await p.$$eval(".wt-qlist li .mono", (l) => l.map((x) => x.textContent));
  check("queue: Show queue lists the waiting ones", JSON.stringify(ql) === JSON.stringify(["docs/scaffolder-api-tags", "codex/ios-testflight-20261019", "marketing/website-concept-20261005"]), JSON.stringify(ql));
  await p.click('.wt-qlist [data-wt-unq="w5"]');
  check("queue: × takes one out", !(await p.$('.wt-row.queued:has([data-wt-open="w5"])')) && !!(await p.$('[data-wt-sel="w5"]')));
  await p.click('[data-wt-act="pause"]');
  check("queue: Pause stops after the current one", (await p.textContent(".wt-now small")).includes("pausing after this one"), await p.textContent(".wt-now small"));
  await p.waitForFunction(() => document.querySelector(".wt-now b")?.textContent === "Paused.");
  const paused = await p.textContent(".wt-now");
  check("queue: paused, nothing half-removed", paused.startsWith("Paused. 1 of 3 done") && (await p.textContent(".wt-now small")) === "2 waiting. Nothing is half-removed." && (await p.textContent('[data-wt-act="resume"]')) === "Resume", paused);
  await p.click('[data-wt-act="resume"]');
  await p.waitForSelector(".wt-dock.fin", { timeout: 10000 });
  check("queue: when empty it becomes the result", (await p.textContent(".wt-now")).startsWith("Done. 3 removed") && (await p.$$(".wt-row.removed")).length === 3, await p.textContent(".wt-now"));
  await ctx.close();
}

// Worktrees found by an older version: the tab says so and offers a deep scan.
{
  const { p, ctx } = await page("wtstale=141");
  check("worktrees: an older-version worktree is not listed in Quick questions", !(await p.textContent("#v-tidy")).includes("docs/old-format"));
  await p.click("#tab-wt");
  await idle(p);
  const txt = await p.textContent("#wt-stale");
  check("worktrees: older results are named, not shown as none", txt.includes("I found 141 worktrees with an older version") && !(await p.$("text=No worktrees found")), txt);
  await p.evaluate(() => (window.__FIXTURE_CALLS__ = []));
  await p.click('[data-act="wt-sort"]');
  await idle(p);
  check("worktrees: Sort them now starts a deep scan", JSON.stringify((await p.evaluate(() => window.__FIXTURE_CALLS__)).find((c) => c.cmd === "start_scan")?.args) === '{"kind":"deep"}');
  await ctx.close();
}

// Languages, the whole app: no visible text or accessible name is still an
// English interface string, on every screen (found by the user on 1.2.0).
{
  const en = JSON.parse(readFileSync(new URL("../../src/i18n/en.json", import.meta.url), "utf8"));
  for (const [l, locale] of [["de", "de-DE"], ["fr", "fr-FR"]]) {
    const other = JSON.parse(readFileSync(new URL(`../../src/i18n/${l}.json`, import.meta.url), "utf8"));
    // English strings without placeholders whose translation differs.
    const english = new Set(Object.entries(en).filter(([k, v]) => !/\{/.test(v) && /[A-Za-z]{4}/.test(v) && other[k] !== v).map(([, v]) => v.trim()));
    const leftovers = new Map();
    const sweep = async (p, where) => {
      const found = await p.evaluate((list) => {
        const set = new Set(list);
        const out = [];
        const walk = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
        for (let n = walk.nextNode(); n; n = walk.nextNode()) {
          const el = n.parentElement;
          if (!el || el.closest("[hidden], script, style") || !el.getClientRects().length) continue;
          const v = n.textContent.trim();
          if (set.has(v)) out.push(v);
        }
        for (const el of document.querySelectorAll("[aria-label], [title]")) for (const a of ["aria-label", "title"]) if (set.has((el.getAttribute(a) ?? "").trim())) out.push(`${a}="${el.getAttribute(a)}"`);
        return out;
      }, [...english]);
      for (const f of found) leftovers.set(f, where);
    };
    const visit = async (query, where, steps = []) => {
      const { p, ctx } = await page(query, undefined, locale);
      for (const st of steps) {
        if (typeof st === "string") await p.click(st);
        else await st(p);
        await idle(p);
      }
      await sweep(p, where);
      await ctx.close();
    };
    await visit(`lang=${l}`, "home");
    await visit(`lang=${l}`, "home pile", ["#pile-btn"]);
    await visit(`lang=${l}`, "quick questions rows", ['details[data-open="grp-Docker"] > summary', "#l-x-d-vol", '[data-risk-box="d-vol"] > summary', 'details[data-open="grp-Projects"] > summary', "#l-x-b-node"]);
    await visit(`lang=${l}`, "disk map", ["#tab-map"]);
    await visit(`lang=${l}`, "not explained", ["#tab-map", '[data-tile="unknown"]']);
    await visit(`lang=${l}`, "places", ["#tab-map", "#places-btn"]);
    await visit(`lang=${l}`, "report", ["#tab-map", '[data-act="report"]']);
    await visit(`lang=${l}`, "logbook", ["#tab-log"]);
    await visit(`lang=${l}`, "worktrees", ["#tab-wt", '[data-wt-open="w1"]', '[data-wt-open="w3"]', '[data-wt-selrepo="sheltersync"]']);
    await visit(`lang=${l}`, "run details", ["#tab-log", '[data-run="run-2"]']);
    await visit(`lang=${l}`, "settings", ['[data-act="settings"]']);
    await visit(`lang=${l}`, "confirm", ["#cta", async (p) => p.waitForSelector("#go")]);
    await visit(`lang=${l}&hold=1`, "running", ["#cta", async (p) => p.waitForSelector("#go"), "#go", async (p) => p.waitForSelector("#run-bar")]);
    await visit(`lang=${l}&skip=wt-exp&trash=1&fail=d-cache`, "result", ["#cta", async (p) => p.waitForSelector("#go"), "#go", async (p) => p.waitForSelector('#go[data-act="close"]'), "details.res.safe > summary"]);
    await visit(`lang=${l}&trash=2.4`, "empty trash", ['[data-act="empty"]', async (p) => p.waitForSelector("#go")]);
    await visit(`view=tray&lang=${l}`, "menu bar window");
    await visit(`firstrun=1&lang=${l}`, "setup 1");
    await visit(`firstrun=1&lang=${l}`, "setup 3", ["#ob-next", "#ob-next"]);
    await visit(`firstrun=1&lang=${l}`, "setup 4", ["#ob-next", "#ob-next", "#ob-next"]);
    check(`language (${l}): no English interface text left on any screen`, leftovers.size === 0, [...leftovers].map(([t, w]) => `${w}: ${t}`).join(" | "));
  }
}

// First run: five steps; choices are saved when the first look starts.
{
  const { p, ctx } = await page("firstrun=1");
  const route = () => p.$$eval(".ob-route li", (l) => l.map((x) => x.className).join(","));
  check("first run: opens on Hello with five buoys", (await p.textContent("#ob-title")) === "Know before your disk is full." && (await route()) === "on,,,,");
  await p.click("#ob-next");
  await p.click('[data-ob-preset="roomy"]');
  check("first run: a preset reacts", (await p.textContent(".ob-verdict")).startsWith("Right now: Critical") && (await p.getAttribute('[data-ob-preset="roomy"]', "aria-checked")) === "true");
  await p.click("#ob-next");
  check("first run: folders that hold repositories, all ticked", (await p.$$eval(".ob-folder input", (i) => i.map((x) => x.checked))).join() === "true,true,true");
  await p.click(".ob-folder:nth-child(3) input");
  check("first run: unticking updates the count", (await p.textContent(".ob-foot .hint")) === "2 folders chosen");
  check("first run: tools found, missing ones struck", (await p.$$(".ob-tool.on")).length === 9 && (await p.$$(".ob-tool:not(.on)")).length === 2);
  await p.click("#ob-next");
  await p.click('[data-ob-sw="login"]');
  await p.evaluate(() => (window.__FIXTURE_CALLS__ = []));
  await p.click("#ob-next");
  await p.waitForSelector("#ob-next:not([disabled])", { timeout: 10000 });
  const sent = await p.evaluate(() => window.__FIXTURE_CALLS__);
  const upd = sent.find((c) => c.cmd === "update_settings")?.args.patch;
  check("first run: the preset and switches are saved", upd?.lowThresholdBytes === 50e9 && upd?.criticalThresholdBytes === 20e9 && upd?.startAtLogin === false && upd?.menuBarFreeSpace === true, JSON.stringify(upd));
  check("first run: only the ticked folders are added", JSON.stringify(sent.filter((c) => c.cmd === "add_suggested_folder").map((c) => c.args.index)) === "[0,1]", JSON.stringify(sent.filter((c) => c.cmd === "add_suggested_folder")));
  check("first run: the first look is the normal quick scan", JSON.stringify(sent.find((c) => c.cmd === "start_scan")?.args) === '{"kind":"quick"}');
  check("first run: the first look ends with what could come back", (await p.textContent("#ob-title")) === "Here\u2019s what I found." && (await p.$$(".ob-found div")).length === 6 && /GB of it rebuilds by itself/.test(await p.textContent(".ob-total")));
  await p.click("#ob-next");
  await p.waitForSelector(".page .hero");
  check("first run: Show me opens the main view", !(await p.$("#ob-root")));
  await ctx.close();
}
{
  const { p, ctx } = await page("firstrun=1");
  // First run listens for Esc from an effect that runs after the first
  // paint; two frames make sure it is attached (a slow CI Mac pressed Esc
  // before it, 2026-11-08). Pressing twice would save twice.
  await p.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  await p.keyboard.press("Escape");
  await p.waitForSelector(".page .hero");
  const sent = await p.evaluate(() => window.__FIXTURE_CALLS__.filter((c) => c.cmd === "update_settings").map((c) => c.args.patch));
  check("first run: Esc skips setup and saves nothing else", JSON.stringify(sent) === '[{"onboardingComplete":true}]', JSON.stringify(sent));
  await ctx.close();
}

// Home: keyboard order and visible focus.
{
  const { p, ctx } = await page();
  const order = [];
  for (let i = 0; i < 14; i++) {
    await p.keyboard.press("Tab");
    order.push(await focused(p));
  }
  // "All areas" is disabled while every area is ticked, so Tab skips it.
  const want = ["settings", "20", "50", "100", "all", "Docker", "Projects", "Caches", "cta", "pile-btn", "tab-tidy", "tab-map", "tab-wt", "tab-log"];
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

// A Risky question card starts with its box open (spec "Risk explainer").
{
  const { p, ctx } = await page();
  // Skip questions with Ask me later until one is Risky.
  let riskyQ = null;
  for (let i = 0; i < 40 && !riskyQ; i++) {
    const q = await p.$eval("#v-tidy .q", (q) => { const b = q.querySelector(".riskx"); return b ? { risky: b.classList.contains("risky"), open: b.open, id: b.dataset.riskBox } : { risky: false }; }).catch(() => null);
    if (!q) break;
    if (q.risky) riskyQ = q;
    else { await p.click('#v-tidy .q [data-q="later"]'); await idle(p); }
  }
  check("risk: a Risky question card starts with its box open", riskyQ?.open === true, JSON.stringify(riskyQ));
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
  const closed = await p.$eval('[data-risk-box="d-vol"]', (b) => b.open);
  await p.click('[data-risk-box="d-vol"] > summary');
  const box = await p.$eval('[data-risk-box="d-vol"]', (b) => ({
    open: b.open,
    on: b.querySelector(".rs.on b")?.textContent,
    why: b.querySelectorAll(".rx-why li").length,
    safer: [...b.querySelectorAll(".rx-steps code")].map((c) => c.textContent),
  }));
  check("risk: in a row the box starts folded, as the prototype", closed === false);
  check("risk: a Risky item explains itself, with its place on the scale", box.open && box.on === "Risky" && box.why === 3, JSON.stringify(box));
  check("risk: it says how to keep a copy first", box.safer.length === 2 && box.safer[1].includes("tar czf /b/billing_pgdata.tgz"), JSON.stringify(box.safer));
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

// Toss with one item that needs its own acknowledgment: leaving it out lets
// the rest go (found on a real Mac: the whole toss was blocked by one item).
{
  const { p, ctx } = await page();
  await p.click('details[data-open="grp-Docker"] > summary');
  await p.click('[data-pile="d-vol"]');
  await idle(p);
  await p.click("#cta");
  await p.waitForSelector("#sheet");
  await idle(p);
  check("leave out: one unacknowledged item blocks the toss", await p.isDisabled("#go"));
  check("leave out: the sheet says how to go on", (await p.textContent(".cf-hint"))?.includes("leave it out"));
  const before = await p.$$eval(".cf-item", (l) => l.length);
  await p.evaluate(() => (window.__FIXTURE_CALLS__ = []));
  await p.click('[data-leave="d-vol"]');
  await p.waitForFunction((n) => document.querySelectorAll(".cf-item").length === n - 1, before);
  await idle(p);
  check("leave out: it is taken out of the pile", JSON.stringify(await callsOf(p, "take_out_of_pile")) === '[{"itemIds":["d-vol"]}]', JSON.stringify(await callsOf(p, "take_out_of_pile")));
  check("leave out: the rest is planned again, without it", !(await p.$('[data-leave="d-vol"]')) && (await callsOf(p, "create_plan")).length === 1);
  check("leave out: the rest can be tossed", !(await p.isDisabled("#go")) && !(await p.$(".cf-hint")));
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

// Disk map, Why card: Not traced has no fix, so the card never promises one;
// with Full Disk Access it names what is left as macOS's own.
{
  const { p, ctx } = await page();
  await p.click("#tab-map");
  await idle(p);
  const lede = await p.textContent("#why .lede");
  check("why: with a part that has no fix, the card does not promise one for each", !lede.includes("Each part below has a fix") && lede.includes("The parts with a button"), lede);
  check("why: without access, Not traced blames Library", (await p.textContent("#why")).includes("Not traced to a folder"));
  await ctx.close();
}
{
  const { p, ctx } = await page("fda=1");
  await p.click("#tab-map");
  await idle(p);
  const why = await p.textContent("#why");
  check("why: with Full Disk Access, what is left is kept by macOS", why.includes("Kept by macOS") && why.includes("Even Full Disk Access does not reach it") && !why.includes("Not traced to a folder"), why);
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
  // Places checked: one line, a See all sheet, and a found item opens on the map.
  const cov = await p.textContent(".scan-card .cov");
  check("places: one summary line with See all", /^\d+ places checked\d+ had something · \d+ clean/.test(cov) && !!(await p.$("#places-btn")), cov);
  await p.click("#places-btn");
  await p.waitForSelector(".pl-sum");
  check("places: the sheet groups what was found and lists what was clean", (await p.$$(".pl-group")).length >= 3 && (await p.$$(".pl-clean li")).length > 0);
  await p.click('[data-place="d-vol"]');
  await idle(p);
  check("places: a found item opens on the map, already expanded", !(await p.$("#sheet")) && (await p.textContent("#tile-detail h3")) === "Docker" && (await p.getAttribute("#m-x-d-vol", "aria-expanded")) === "true" && !!(await p.$("#m-now-d-vol")));
  await p.click('[data-act="report"]');
  await idle(p);
  await p.waitForSelector("#report-box textarea, #report-box p");
  const report = await p.evaluate(() => document.querySelector("#report-text")?.value ?? "copied");
  check("map: the disk report is shown or copied", report === "copied" || JSON.parse(report).free_gb === 12.4, report.slice(0, 80));
  await ctx.close();
}

// Keep me tidy (spec "Keep me tidy rules"): both off; turning one on first
// says exactly what it removes, and only Turn on saves it.
{
  const { p, ctx } = await page();
  await p.click("#tab-tidy");
  await p.waitForSelector("#keep-tidy #auto-caches");
  check("tidy: both rules are off", !(await p.isChecked("#auto-caches")) && !(await p.isChecked("#auto-docker")));
  await p.evaluate(() => (window.__FIXTURE_CALLS__ = []));
  await p.click("#auto-caches");
  const says = await p.textContent("#keep-tidy .tidy-ask");
  check("tidy: turning on lists what it removes before saving", says.includes("pnpm store, pip cache, npm cache, Homebrew cache") && !(await calls(p)).includes("set_tidy_rule"), says);
  await p.click('[data-act="tidy-cancel"]');
  check("tidy: Cancel leaves it off", !(await p.isChecked("#auto-caches")) && !(await calls(p)).includes("set_tidy_rule") && !(await p.$("#keep-tidy .tidy-ask")));
  await p.click("#auto-docker");
  await p.click('[data-act="tidy-on"]');
  await idle(p);
  const set = (await p.evaluate(() => window.__FIXTURE_CALLS__)).filter((c) => c.cmd === "set_tidy_rule");
  check("tidy: Turn on saves that rule only", set.length === 1 && JSON.stringify(set[0].args) === '{"ruleId":"docker_build_cache","enabled":true}' && (await p.isChecked("#auto-docker")), JSON.stringify(set));
  await p.click("#auto-docker");
  await idle(p);
  const off = (await p.evaluate(() => window.__FIXTURE_CALLS__)).filter((c) => c.cmd === "set_tidy_rule");
  check("tidy: turning off saves at once", off.length === 2 && off[1].args.enabled === false && !(await p.isChecked("#auto-docker")));
  await ctx.close();
}

// Settings, In the background: schedules and switches save at once; other
// disks are switched by ID, never by path.
{
  const { p, ctx } = await page();
  await p.click('[data-act="settings"]');
  await p.waitForSelector("#st-bg");
  await idle(p);
  await p.evaluate(() => (window.__FIXTURE_CALLS__ = []));
  await p.selectOption("#quick-every", "daily");
  await p.selectOption("#deep-every", "daily");
  await p.click("#pause-battery");
  await idle(p);
  const patches = (await p.evaluate(() => window.__FIXTURE_CALLS__)).filter((c) => c.cmd === "update_settings").map((c) => JSON.stringify(c.args.patch));
  check("settings: schedules and the battery switch save at once", JSON.stringify(patches) === JSON.stringify(['{"quickScan":"daily"}', '{"deepScan":"daily"}', '{"pauseDeepScansOnBattery":false}']), JSON.stringify(patches));
  check("settings: the scan line follows the schedule", (await p.textContent("#scan-status")).endsWith("Quick scans run once a day."), await p.textContent("#scan-status"));
  await p.click("#disk-vol-archive");
  await idle(p);
  const vol = (await p.evaluate(() => window.__FIXTURE_CALLS__)).filter((c) => c.cmd === "set_volume_watched");
  check("settings: a disk is watched by its ID", vol.length === 1 && JSON.stringify(vol[0].args) === '{"volumeId":"vol-archive","watched":true}' && (await p.isChecked("#disk-vol-archive")), JSON.stringify(vol));
  await ctx.close();
}

// States (task 6.5): scan running, places that could not be checked, a new
// install, a scan that found nothing, and Windows words.
{
  let { p, ctx } = await page("unavail=1");
  await p.click("#tab-tidy");
  const lines = await p.$$eval("#v-tidy .unchecked .note", (e) => e.map((x) => x.textContent));
  check("states: places not checked, once per cause", JSON.stringify(lines) === JSON.stringify(["Docker is installed but not running.", "pip is not installed."]), JSON.stringify(lines));
  await ctx.close();
  ({ p, ctx } = await page("fresh=1"));
  await p.click("#tab-tidy");
  check("states: a new install says how to start", (await p.textContent("#v-tidy .card .note")).startsWith("Nothing yet."), await p.textContent("#v-tidy .card .note"));
  await p.click('[data-tab="log"]');
  await idle(p);
  const log = await p.textContent("#v-log");
  check("states: an empty Logbook says so", log.includes(LG_NONE), log);
  await ctx.close();
  ({ p, ctx } = await page("nothing=1"));
  await p.click("#tab-tidy");
  check("states: a scan that found nothing says so", (await p.textContent("#v-tidy .card .note")) === "The last scan found nothing to clean up.", await p.textContent("#v-tidy .card .note"));
  await ctx.close();
  ({ p, ctx } = await page("scanning=1"));
  check("states: a running scan shows its progress", !!(await p.$("#scan-progress")));
  await ctx.close();
  ({ p, ctx } = await page("platform=windows&trash=2.4"));
  check("states: Windows says Recycle Bin", (await p.textContent("#trash")).includes("Recycle Bin"), await p.textContent("#trash"));
  await ctx.close();
}


// Download page languages (spec "Languages"): opens in the browser's language
// when it is one of en, nl, de, fr, es (else English), remembers a manual
// choice, and writes sizes with the language's decimal separator.
{
  const { createServer } = await import("node:http");
  const pages = { "/": readFileSync(new URL("../../site/index.html", import.meta.url)), "/docs/": readFileSync(new URL("../../site/docs/index.html", import.meta.url)) };
  const srv = createServer((req, res) => { const pg = pages[req.url.split("?")[0]]; res.writeHead(pg ? 200 : 404, { "content-type": "text/html; charset=utf-8" }); res.end(pg ?? ""); });
  await new Promise((r) => srv.listen(0, "127.0.0.1", r));
  const url = `http://127.0.0.1:${srv.address().port}/`;
  const open = async (ctx) => { const p = await ctx.newPage(); await ctx.route("**/*", (r) => (new URL(r.request().url()).hostname === "127.0.0.1" ? r.continue() : r.abort())); await p.goto(url); await p.waitForFunction(() => document.getElementById("langCode")?.textContent); return p; };
  let ctx = await browser.newContext({ locale: "nl-NL" });
  let p = await open(ctx);
  check("site: opens in the browser's language (Dutch)", (await p.getAttribute("html", "lang")) === "nl" && (await p.textContent("#langCode")) === "NL" && (await p.textContent(".hero h1")).includes("waterlijn"), await p.textContent(".hero h1"));
  check("site: sizes take a decimal comma in Dutch", (await p.textContent("body")).includes("12,4 GB") && !(await p.textContent(".hero")).includes("12.4 GB"));
  await p.click("#langBtn");
  await p.click('[data-lang="fr"]');
  check("site: picking a language switches at once, French writes Go", (await p.getAttribute("html", "lang")) === "fr" && (await p.textContent("body")).includes("12,4 Go"));
  await p.reload();
  await p.waitForFunction(() => document.getElementById("langCode")?.textContent);
  check("site: the manual choice is remembered", (await p.textContent("#langCode")) === "FR");
  // The docs page follows the choice made on the home page (found 2026-11-09:
  // it was English only, with no switcher).
  await p.goto(url + "docs/");
  await p.waitForFunction(() => document.getElementById("langCode")?.textContent);
  check("docs: follows the language chosen on the home page", (await p.getAttribute("html", "lang")) === "fr" && (await p.textContent("#langCode")) === "FR" && (await p.textContent(".doc h1")) === "Documentation" && (await p.textContent("#start")) === "Pour commencer", await p.textContent("#start"));
  await p.click("#langBtn");
  await p.click('[data-lang="de"]');
  check("docs: its switcher changes the page and is remembered", (await p.textContent("#start")) === "Erste Schritte" && (await p.evaluate(() => localStorage.getItem("fb-lang"))) === "de", await p.textContent("#start"));
  await ctx.close();
  ctx = await browser.newContext({ locale: "es-ES" });
  p = await ctx.newPage();
  await ctx.route("**/*", (r) => (new URL(r.request().url()).hostname === "127.0.0.1" ? r.continue() : r.abort()));
  await p.goto(url + "docs/");
  await p.waitForFunction(() => document.getElementById("langCode")?.textContent);
  const untranslated = await p.evaluate(() => [...document.querySelectorAll(".doc h2, .doc h3, .doc p, .doc li, .doc th, .doc td, .toc a, .card b, .card span, .nav-links > a, footer span")].filter((e) => !e.closest("[data-i18n]") && /[A-Za-z]{3}/.test(e.textContent) && !e.querySelector("[data-i18n]")).map((e) => e.textContent.slice(0, 40)));
  check("docs: opens in the browser's language (Spanish), every text translated", (await p.textContent("#start")) === "Primeros pasos" && untranslated.length === 0, JSON.stringify(untranslated));
  await ctx.close();
  ctx = await browser.newContext({ locale: "ja-JP" });
  p = await open(ctx);
  check("site: other languages get English", (await p.getAttribute("html", "lang")) === "en" && (await p.textContent("body")).includes("12.4 GB"));
  await ctx.close();
  srv.close();
}

await browser.close();
server.close();
for (const r of results) console.log(`${r.ok ? "PASS" : "FAIL"}  ${r.name}${r.ok || !r.detail ? "" : "  (" + r.detail + ")"}`);
const failed = results.filter((r) => !r.ok).length;
console.log(failed ? `\n${failed} interface check(s) failed.` : `\nAll ${results.length} interface checks passed.`);
process.exit(failed ? 1 : 0);
