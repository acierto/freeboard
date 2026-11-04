import { GB, gb, gbLabel } from "../lib/format";
import type { TrendPoint } from "../lib/types";

// 14-day free-space trend, drawn with the prototype's exact geometry and
// styling. The markup string contains only computed numbers.
export function Trend({ points, freeBytes, lowBytes }: { points: TrendPoint[]; freeBytes: number; lowBytes: number }) {
  if (points.length < 2) return <p className="muted">Not enough history yet. The trend appears after a few hours of monitoring.</p>;
  const free = freeBytes / GB;
  const warn = lowBytes / GB;
  const pts = points.map((p) => p.freeBytes / GB);
  const max = Math.max(45, free, warn, ...pts);
  const W = 320, H = 118, L = 8, R = 62, T = 10, B = 22;
  const x = (i: number) => L + (i / (pts.length - 1)) * (W - L - R);
  const y = (v: number) => T + (1 - v / max) * (H - T - B);
  const line = pts.map((v, i) => (i ? "L" : "M") + x(i).toFixed(1) + " " + y(v).toFixed(1)).join(" ");
  const end = pts.length - 1;
  const yw = y(warn);
  const label = "Free space over the last 14 days, from " + gb(points[0].freeBytes) + " to " + gb(freeBytes);
  const html =
    '<svg class="trend" viewBox="0 0 ' + W + " " + H + '" role="img" aria-label="' + label + '">' +
    '<path d="' + line + " L" + x(end) + " " + (H - B) + " L" + L + " " + (H - B) + ' Z" fill="var(--water)" fill-opacity=".18"/>' +
    '<line x1="' + L + '" x2="' + (W - R) + '" y1="' + (H - B) + '" y2="' + (H - B) + '" stroke="var(--line)"/>' +
    '<line x1="' + L + '" x2="' + (W - R) + '" y1="' + yw + '" y2="' + yw + '" stroke="var(--warn)" stroke-dasharray="4 3"/>' +
    '<path d="' + line + '" fill="none" stroke="var(--accent)" stroke-width="2"/>' +
    '<circle cx="' + x(end) + '" cy="' + y(free) + '" r="3.5" fill="var(--accent)"/>' +
    '<text x="' + (x(end) + 7) + '" y="' + (y(free) + 4) + '" style="fill:var(--ink)">' + gb(freeBytes) + "</text>" +
    '<text x="' + x(0) + '" y="' + (yw - 4) + '">Low ' + gbLabel(lowBytes) + " GB</text>" +
    '<text x="' + L + '" y="' + (H - 7) + '">14 days ago</text><text x="' + (W - R) + '" y="' + (H - 7) + '" text-anchor="end">Today</text></svg>';
  return <div id="trend" dangerouslySetInnerHTML={{ __html: html }} />;
}
