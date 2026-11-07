// The disk map's geometry and wording as pure functions. `squarify`, the tile
// classes and the coverage figures are ported from the prototype's
// `renderMap()`, `squarify()` and `explained()`, so the same numbers give the
// same tiles.
import type { BlindSpot, BlindSpotKind, DiskMap } from "../../lib/types.ts";

export const GB = 1e9;

/** Tile colours, by area (prototype TILES `c`). */
export const TILE_COLOR: Record<string, string> = {
  projects: "var(--t-a)",
  docker: "var(--t-b)",
  ai: "var(--t-e)",
  caches: "var(--t-c)",
  files: "var(--t-d)",
  mobile: "var(--t-f)",
  apps: "var(--t-g)",
  system: "var(--t-h)",
  browser: "var(--t-d)",
  fonts: "var(--t-e)",
  unknown: "",
};

export interface Tile {
  id: string;
  name: string;
  c: string;
  /** Size in GB, rounded to 0.1 (prototype `tileSize`). */
  v: number;
}
export interface Box {
  t: Tile;
  x: number;
  y: number;
  w: number;
  h: number;
}

const round1 = (n: number) => Math.round(n * 10) / 10;

/** The map's tiles in prototype TILES order, Not explained last. */
export function tilesOf(map: DiskMap): Tile[] {
  const out = map.areas.map((a) => ({ id: a.id, name: a.label, c: TILE_COLOR[a.id] ?? "var(--t-a)", v: Math.max(0, round1(a.bytes / GB)) }));
  out.push({ id: "unknown", name: "Not explained", c: "", v: Math.max(0, round1(map.notExplainedBytes / GB)) });
  return out;
}

/** Squarified treemap in a w × h box (prototype `squarify`, unchanged). */
export function squarify(list: Tile[], x: number, y: number, w: number, h: number): Box[] {
  const out: Box[] = [];
  let rest = list.filter((t) => t.v > 0).sort((a, b) => b.v - a.v);
  const total = rest.reduce((t, r) => t + r.v, 0);
  const k = (w * h) / total;
  const worst = (row: Tile[], side: number) => {
    const s = row.reduce((t, r) => t + r.v * k, 0);
    let mx = 0;
    let mn = Infinity;
    row.forEach((r) => {
      const a = r.v * k;
      mx = Math.max(mx, a);
      mn = Math.min(mn, a);
    });
    return Math.max((side * side * mx) / (s * s), (s * s) / (side * side * mn));
  };
  while (rest.length) {
    const side = Math.min(w, h);
    const row = [rest[0]];
    let n = 1;
    while (n < rest.length && worst(row.concat([rest[n]]), side) <= worst(row, side)) {
      row.push(rest[n]);
      n++;
    }
    rest = rest.slice(n);
    const area = row.reduce((t, r) => t + r.v * k, 0);
    const thick = area / side;
    let pos = 0;
    row.forEach((r) => {
      const len = (r.v * k) / thick;
      if (w >= h) out.push({ t: r, x, y: y + pos, w: thick, h: len });
      else out.push({ t: r, x: x + pos, y, w: len, h: thick });
      pos += len;
    });
    if (w >= h) {
      x += thick;
      w -= thick;
    } else {
      y += thick;
      h -= thick;
    }
  }
  return out;
}

/** Tile classes (prototype): Not explained is striped; small tiles hide
 * their labels; short wide tiles put them on one line. */
export function tileClass(b: Box): string {
  const a = b.w * b.h;
  return "tile" + (b.t.id === "unknown" ? " unknown" : "") + (a < 240 ? " tiny" : "") + (b.h < 14 && b.w > 25 ? " flat" : "");
}

/** Coverage as the prototype computes it: used is the sum of the tiles. */
export function explained(tiles: Tile[]) {
  const used = tiles.reduce((t, x) => t + x.v, 0);
  const un = tiles.find((t) => t.id === "unknown")?.v ?? 0;
  return { used, un, pct: used ? ((used - un) / used) * 100 : 100 };
}

/** Coverage from the disk's real used space and what is explained. On a
 * complete map this equals the tile sum the prototype uses; on a map whose
 * areas add up to more than the disk uses (APFS clones, snapshots), the tile
 * sum would claim an impossible "in use" figure and 100% (found on a real
 * Mac, 2026-11-07), so the backend's figures are used. */
export function coverage(map: DiskMap) {
  const used = map.usedBytes / 1e9;
  const un = map.notExplainedBytes / 1e9;
  return { used, un, pct: used ? ((used - un) / used) * 100 : 100 };
}

export function coverText(map: DiskMap): { pct: string; line: string } {
  const ex = coverage(map);
  return { pct: ex.pct.toFixed(1) + "% explained", line: "I can account for " + (ex.used - ex.un).toFixed(1) + " of the " + ex.used.toFixed(1) + " GB in use." };
}

/** The tab badge: shown only while some space is unexplained. */
export function coverageBadge(map: DiskMap | null): number | null {
  if (!map) return null;
  const ex = coverage(map);
  return ex.un > 0 ? Math.round(ex.pct) : null;
}

/** The ring's arc (prototype `ring`): circumference and dash. */
export function ringDash(pct: number): { dash: string; done: boolean } {
  const c = 2 * Math.PI * 34;
  return { dash: ((c * pct) / 100).toFixed(1) + " " + c.toFixed(1), done: pct >= 99.95 };
}

/** The area to open first: the one with the most to remove (the prototype
 * opens AI models, the largest in its example). */
export function defaultTile(map: DiskMap, sizes: { mapArea: string; sizeBytes: number }[]): string {
  let best = "";
  let most = -1;
  for (const a of map.areas) {
    const s = sizes.filter((x) => x.mapArea === a.id).reduce((t, x) => t + x.sizeBytes, 0);
    if (s > most) {
      most = s;
      best = a.id;
    }
  }
  return best || "unknown";
}

/** Areas under 3% of the map are not drawn as slivers; they are listed as
 * chips under it (prototype `mapAreas`). Not explained is always drawn. */
export const SMALL_AREA = 0.03;
export function splitAreas(tiles: Tile[]): { tiles: Tile[]; small: Tile[] } {
  const used = tiles.reduce((t, x) => t + x.v, 0);
  const small = tiles.filter((x) => x.id !== "unknown" && x.v > 0 && x.v / used < SMALL_AREA);
  return { tiles: tiles.filter((x) => !small.includes(x)), small };
}

/** Why space is not explained, by cause (prototype `CAUSES`). */
export interface Cause {
  id: "outside" | "locked" | "strange" | "tools" | "untraced";
  title: string;
  why: string;
  /** Swatch colour; "stripes" for the striped one. */
  c: string;
  /** Sizes here are estimates ("about"). */
  est: boolean;
  /** Open blind spots in this cause, biggest first. */
  open: BlindSpot[];
  /** Spots already fixed (or being fixed). */
  fixed: BlindSpot[];
  bytes: number;
}

const CAUSE_OF: Record<BlindSpotKind, Cause["id"]> = {
  unscanned_folder: "outside",
  needs_permission: "locked",
  unknown_files: "strange",
  tool_unavailable: "tools",
};
const CAUSE_TEXT: Record<Cause["id"], [title: string, why: string, c: string, est: boolean]> = {
  outside: ["In folders I don\u2019t scan", "I only look inside the folders in Settings. A quick size check found these elsewhere in your home folder.", "var(--t-c)", false],
  locked: ["Locked by macOS", "Mail, Messages, Photos and app data stay hidden until Freeboard has Full Disk Access, so this size is an estimate.", "var(--t-e)", true],
  strange: ["Files I don\u2019t recognize", "I can read these, but no detector knows which app made them.", "stripes", false],
  tools: ["Tools I could not ask", "A tool did not answer during the last scan, so what it manages is not counted yet.", "var(--t-g)", false],
  untraced: ["Not traced to a folder", "Space in places I do not walk, such as app data in Library and system folders. I can\u2019t say which folder holds it.", "var(--t-h)", false],
};

const sizeOf = (s: BlindSpot) => s.sizeBytes ?? 0;

/** The Why-not-explained card's causes, in the prototype's order. What no
 * blind spot accounts for is shown honestly as "Not traced to a folder". */
export function causesOf(map: DiskMap, spots: BlindSpot[]): { causes: Cause[]; left: number } {
  const order: Cause["id"][] = ["outside", "locked", "untraced", "strange", "tools"];
  const causes = order.map((id): Cause => {
    const [title, why, c, est] = CAUSE_TEXT[id];
    const mine = spots.filter((s) => CAUSE_OF[s.kind] === id);
    const open = mine.filter((s) => s.state === "open").sort((a, b) => sizeOf(b) - sizeOf(a));
    return { id, title, why, c, est, open, fixed: mine.filter((s) => s.state !== "open"), bytes: open.reduce((t, s) => t + sizeOf(s), 0) };
  });
  const traced = causes.reduce((t, c) => t + c.bytes, 0);
  const untraced = causes.find((c) => c.id === "untraced")!;
  untraced.bytes = Math.max(0, map.notExplainedBytes - traced);
  const shown = causes.filter((c) => c.open.length || c.fixed.length || (c.id === "untraced" && c.bytes >= 0.05 * GB));
  return { causes: shown, left: shown.reduce((t, c) => t + c.bytes, 0) };
}
