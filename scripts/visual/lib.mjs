// Shared capture machinery for the visual verifier.
import { createServer } from "node:http";
import { createHash } from "node:crypto";
import { readFileSync, existsSync, statSync } from "node:fs";
import { extname, join, normalize, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";

export const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
export const PROTOTYPE = join(ROOT, "docs/reference/freeboard-prototype.html");
/** SHA-256 recorded in docs/reference/README.md. */
export const PROTOTYPE_SHA256 = "d38593d8df51f68b58fe64a961aed4819a234ee44b26e8097189bc983d5c946b";
export const FIXED_TIME = Date.parse("2026-11-04T12:00:00Z");
export const BASELINE_DIR = join(ROOT, "visual/baseline");
export const TOLERANCE_FILE = join(ROOT, "visual/tolerances.json");

/** Fonts every capture must actually load (no silent fallback). */
export const REQUIRED_FONTS = [
  ["Fredoka", "600"],
  ["Fredoka", "700"],
  ["Nunito", "600"],
  ["Nunito", "700"],
  ["Nunito", "800"],
  ["DM Mono", "400"],
];

/** What every capture crops to: the whole page (the prototype has no app frame). */
export const CROP = "page";

/** Hidden in the reference. Prototype-only controls (spec "Build reference"):
 * Pretend free space, Start over, and the "example data" note under the brand.
 * Also the in-page "Menu bar" button, which stands in for the native tray icon
 * (decision 2026-11-06); the tray view itself is compared on its own. */
export const PROTOTYPE_ONLY = ".top-right .sim, .top-right [data-act=\"reset\"], .top > div > .note, .top-right .tray";

export const sha256 = (buf) => createHash("sha256").update(buf).digest("hex");

const TYPES = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".css": "text/css", ".woff2": "font/woff2", ".png": "image/png", ".svg": "image/svg+xml" };

/** The reference adapter: the original prototype with local fonts, a theme
 * attribute, the artifact host's base reset (src/styles/reset.css — the
 * prototype is written for a host page that supplies it, and without it its
 * `hidden` sections stay visible) and the prototype-only controls hidden.
 * The file itself is never modified. */
export function referenceHtml({ theme, supplemental }) {
  const src = readFileSync(PROTOTYPE, "utf8");
  const fontsCss = readFileSync(join(ROOT, "src/styles/fonts.css"), "utf8").replaceAll("../fonts/", "/__fonts/");
  const resetCss = readFileSync(join(ROOT, "src/styles/reset.css"), "utf8");
  const supCss = supplemental ? readFileSync(join(ROOT, "src/styles/supplemental.css"), "utf8") : "";
  let html = src
    .replace(/<link rel="preconnect"[^>]*>\n?/g, "")
    .replace(/<link rel="stylesheet" href="https:\/\/fonts\.googleapis\.com[^>]*>/, `<style data-adapter="fonts">${fontsCss}</style>`);
  if (html.includes("fonts.googleapis.com")) throw new Error("reference adapter failed to replace the remote font link");
  const adapter =
    `<script>document.documentElement.dataset.theme=${JSON.stringify(theme)};</script>` +
    `<style data-adapter="host-reset">${resetCss}</style>` +
    `<style data-adapter="prototype-only">${PROTOTYPE_ONLY}{display:none!important}</style>` +
    (supCss ? `<style data-adapter="supplemental">${supCss}</style>` : "");
  // The prototype has no <head>; put the adapter before its <title>.
  if (!html.startsWith("<title>")) throw new Error("reference adapter: unexpected prototype start");
  return adapter + html;
}

export function startServer() {
  const dist = join(ROOT, "dist-visual");
  if (!existsSync(join(dist, "index.html"))) throw new Error("dist-visual is missing; run `npm run build:visual`");
  const server = createServer((req, res) => {
    const url = new URL(req.url, "http://x");
    try {
      if (url.pathname === "/__ref/") {
        res.writeHead(200, { "content-type": TYPES[".html"] });
        res.end(referenceHtml({ theme: url.searchParams.get("theme") ?? "light", supplemental: url.searchParams.get("sup") === "1" }));
        return;
      }
      let file;
      if (url.pathname.startsWith("/__fonts/")) file = join(ROOT, "src/fonts", normalize(url.pathname.slice(9)));
      else file = join(dist, normalize(url.pathname === "/" ? "index.html" : url.pathname));
      if (!file.startsWith(ROOT) || !existsSync(file) || statSync(file).isDirectory()) {
        res.writeHead(404);
        res.end();
        return;
      }
      res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" });
      res.end(readFileSync(file));
    } catch (e) {
      res.writeHead(500);
      res.end(String(e));
    }
  });
  return new Promise((ok) => server.listen(0, "127.0.0.1", () => ok({ server, base: `http://127.0.0.1:${server.address().port}` })));
}

export async function launch() {
  // --disable-partial-raster: partial tile re-raster after DOM changes antialiases
  // rounded corners differently (1-unit colour noise); full raster is deterministic.
  const browser = await chromium.launch({ args: ["--font-render-hinting=none", "--disable-lcd-text", "--force-color-profile=srgb", "--disable-partial-raster"] });
  return browser;
}

export function engineInfo(browser) {
  return { engine: "chromium", version: browser.version(), playwright: "1.64.1", os: `${process.platform}-${process.arch}` };
}

async function newPage(browser, cap, side, { blockFonts = false } = {}) {
  const ctx = await browser.newContext({
    viewport: { width: cap.width, height: cap.height },
    deviceScaleFactor: 1,
    colorScheme: cap.theme,
    reducedMotion: cap.motion ? "no-preference" : "reduce",
    locale: "en-US",
    timezoneId: "UTC",
  });
  // Local only: abort every request that leaves the test server.
  await ctx.route("**/*", (route) => {
    const u = new URL(route.request().url());
    if (u.hostname !== "127.0.0.1") return route.abort();
    if (blockFonts && u.pathname.endsWith(".woff2")) return route.abort();
    return route.continue();
  });
  const page = await ctx.newPage();
  if (side === "ref") {
    await page.clock.install({ time: FIXED_TIME });
  } else {
    await page.clock.setFixedTime(FIXED_TIME);
  }
  return { ctx, page };
}

async function settle(page, side) {
  if (side === "impl") await page.waitForFunction(() => document.documentElement.dataset.busy === "0", null, { timeout: 10000 });
  // requestAnimationFrame is faked (and paused) by the reference clock, so
  // wait in real time for the compositor instead.
  if (side === "impl") await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  else await page.waitForTimeout(50);
  if (side === "impl") await page.waitForFunction(() => document.documentElement.dataset.busy === "0", null, { timeout: 10000 });
}

async function runStep(page, side, step) {
  if (step.clockRun !== undefined) {
    if (side === "ref") await page.clock.runFor(step.clockRun);
    return;
  }
  if (step.click) {
    await page.evaluate((sel) => {
      const el = document.querySelector(sel);
      if (!el) throw new Error("missing element " + sel);
      el.click();
    }, step.click);
  } else if (step.check) {
    await page.evaluate(([sel, on]) => {
      const el = document.querySelector(sel);
      if (!el) throw new Error("missing element " + sel);
      if (el.checked !== on) el.click();
    }, step.check);
  } else if (step.range) {
    await page.evaluate(([sel, v]) => {
      const el = document.querySelector(sel);
      if (!el) throw new Error("missing element " + sel);
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(el, v);
      el.dispatchEvent(new Event("input", { bubbles: true }));
      el.dispatchEvent(new Event("change", { bubbles: true }));
    }, step.range);
  } else if (step.key) {
    await page.keyboard.press(step.key);
  } else if (step.eval) {
    if (side !== "ref") throw new Error("eval steps are reference-only");
    await page.evaluate(([src, arg]) => (0, eval)("(" + src + ")")(arg), [step.eval, step.arg]);
  } else {
    throw new Error("unknown step " + JSON.stringify(step));
  }
  await settle(page, side);
}

/** Verify the bundled fonts really loaded. Returns the missing ones. */
async function missingFonts(page) {
  return page.evaluate(async (want) => {
    await Promise.all(want.map(([f, w]) => document.fonts.load(`${w} 16px "${f}"`).catch(() => [])));
    await document.fonts.ready;
    const loaded = [...document.fonts].filter((ff) => ff.status === "loaded").map((ff) => `${ff.family.replace(/"/g, "")}|${ff.weight}`);
    return want.filter(([f, w]) => !loaded.includes(`${f}|${w}`)).map(([f, w]) => `${f} ${w}`);
  }, REQUIRED_FONTS);
}

/**
 * Capture one side. Returns { png, missingFonts, error }.
 * `mutate` (selftest only) runs in the page right before capture.
 */
export async function capture(browser, base, cap, side, opts = {}) {
  const { ctx, page } = await newPage(browser, cap, side, opts);
  try {
    if (side === "ref") {
      await page.goto(`${base}/__ref/?theme=${cap.theme}&sup=${cap.supplemental ? 1 : 0}`);
      // Freeze the prototype's simulated timers; `clockRun` steps advance them.
      await page.clock.pauseAt(FIXED_TIME + 60000);
      if (opts.checkReferenceIntact !== false) {
        const ok = await page.evaluate(() => !!document.querySelector(".page .hero") && !!document.getElementById("boat"));
        if (!ok) throw new Error("reference prototype did not render");
      }
    } else {
      const q = new URLSearchParams(cap.query ?? "");
      q.set("theme", cap.theme);
      await page.goto(`${base}/?${q}`);
      await page.waitForSelector(".page .hero, .tray-window", { timeout: 10000 });
    }
    await settle(page, side);
    for (const step of side === "ref" ? cap.ref : cap.impl) await runStep(page, side, step);
    if (opts.mutate) {
      await page.evaluate(opts.mutate);
      await settle(page, side);
    }
    const missing = await missingFonts(page);
    // Force a full relayout and repaint so antialiasing never depends on the
    // paint history of the steps (partial repaints rasterize rounded corners
    // slightly differently in Chromium).
    await page.evaluate(() => {
      const root = document.body;
      root.style.display = "none";
      void root.offsetHeight;
      root.style.display = "";
    });
    await page.setViewportSize({ width: cap.width, height: cap.height + 1 });
    await page.setViewportSize({ width: cap.width, height: cap.height });
    await settle(page, side);
    // `clip`: compare only the union of these elements (parts of the page whose
    // neighbours arrive in later milestones); otherwise the whole page.
    const clip = cap.clip ? await page.evaluate((sels) => {
      const rects = sels.flatMap((s) => [...document.querySelectorAll(s)]).map((e) => e.getBoundingClientRect()).filter((r) => r.width && r.height);
      if (!rects.length) return null;
      const x = Math.min(...rects.map((r) => r.left)), y = Math.min(...rects.map((r) => r.top));
      const r = Math.max(...rects.map((r) => r.right)), b = Math.max(...rects.map((r) => r.bottom));
      return { x: Math.floor(x + scrollX), y: Math.floor(y + scrollY), width: Math.ceil(r - x), height: Math.ceil(b - y) };
    }, cap.clip) : undefined;
    if (cap.clip && !clip) throw new Error("nothing to capture for " + cap.clip.join(", "));
    const png = await page.screenshot({ fullPage: true, clip, animations: "disabled", caret: "hide" });
    return { png, missingFonts: missing };
  } catch (e) {
    return { png: null, missingFonts: [], error: String(e.message ?? e) };
  } finally {
    await ctx.close();
  }
}

export function loadJson(file, dflt) {
  return existsSync(file) ? JSON.parse(readFileSync(file, "utf8")) : dflt;
}

/**
 * Behavioural motion check on both sides: with reduced motion Dinghy's bob,
 * the waves and the boat's rise must not animate; with motion allowed they must.
 */
export async function motionChecks(browser, base) {
  const out = [];
  for (const side of ["ref", "impl"]) {
    for (const motion of [false, true]) {
      const cap = { width: 1120, height: 900, theme: "light", motion, query: "", ref: [], impl: [] };
      const { ctx, page } = await newPage(browser, cap, side);
      try {
        await page.goto(side === "ref" ? `${base}/__ref/?theme=light` : `${base}/?theme=light`);
        const found = await page.waitForSelector("#boat", { state: "attached", timeout: 10000 }).then(() => true, () => false);
        if (!found) {
          out.push({ side, motion: motion ? "no-preference" : "reduce", ok: false, error: "Dinghy (#boat) is not on the page" });
          continue;
        }
        const s = await page.evaluate(() => ({
          bob: getComputedStyle(document.querySelector(".bob")).animationName,
          wave: getComputedStyle(document.querySelector(".wave")).animationName,
          boat: getComputedStyle(document.getElementById("boat")).transitionDuration,
        }));
        const ok = motion ? s.bob === "bob" && s.wave === "drift" && s.boat !== "0s" : s.bob === "none" && s.wave === "none" && s.boat === "0s";
        out.push({ side, motion: motion ? "no-preference" : "reduce", ...s, ok });
      } finally {
        await ctx.close();
      }
    }
  }
  return out;
}
