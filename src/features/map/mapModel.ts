// The disk map's geometry and wording as pure functions. `squarify`, the tile
// classes and the coverage figures are ported from the prototype's
// `renderMap()`, `squarify()` and `explained()`, so the same numbers give the
// same tiles.
import type { DiskMap } from "../../lib/types.ts";

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

export function coverText(tiles: Tile[]): { pct: string; line: string } {
  const ex = explained(tiles);
  return { pct: ex.pct.toFixed(1) + "% explained", line: "I can account for " + (ex.used - ex.un).toFixed(1) + " of the " + ex.used.toFixed(1) + " GB in use." };
}

/** The tab badge: shown only while some space is unexplained. */
export function coverageBadge(map: DiskMap | null): number | null {
  if (!map) return null;
  const ex = explained(tilesOf(map));
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
