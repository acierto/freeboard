import type { CSSProperties } from "react";
import { GB, SCALE_GB, gbLabel } from "../lib/format";

// Hull gauge from docs/reference/freeboard-prototype.html. Geometry is copied
// exactly: deck line at y=40 (0 GB free), 220 px for the top 120 GB.
const HULL = "M56 40H402L372 276Q362 288 342 288H56Z";
export const yOf = (gbFree: number) => 40 + (Math.min(gbFree, SCALE_GB) / SCALE_GB) * 220;

let WAVE = "M-40 0";
for (let x = -40; x < 500; x += 40) WAVE += " q10 -8 20 0 t20 0";
WAVE += " V320 H-40 Z";

export function Gauge({ freeBytes, lowBytes, criticalBytes }: { freeBytes: number; lowBytes: number; criticalBytes: number }) {
  const yw = yOf(lowBytes / GB);
  const yc = yOf(criticalBytes / GB);
  const style = { "--wy": yOf(freeBytes / GB) + "px" } as CSSProperties;
  return (
    <svg className="gauge" id="gauge" viewBox="0 0 420 300" role="img" aria-label="Hull gauge. The waterline rises toward the deck as free space runs out." style={style}>
      <defs>
        <clipPath id="hull-clip">
          <path d={HULL} />
        </clipPath>
        <clipPath id="sea-clip">
          <rect x="52" y="0" width="368" height="300" />
        </clipPath>
      </defs>
      <text x="0" y="14" fill="var(--muted)">GB free, top 120 GB of disk</text>
      <g clipPath="url(#sea-clip)">
        <g className="sea">
          <path className="wave" fill="var(--water)" d={WAVE} />
        </g>
      </g>
      <path d={HULL} fill="var(--hull)" />
      <g clipPath="url(#hull-clip)">
        <rect id="sub" x="56" y="0" width="350" height="300" fill="var(--oxide)" />
      </g>
      <g id="ticks">
        {[0, 30, 60, 90, 120].map((v) => {
          const y = yOf(v);
          return [
            <line key={"l" + v} x1="46" x2="56" y1={y} y2={y} stroke="var(--muted)" />,
            <text key={"t" + v} x="42" y={y + 4} textAnchor="end" fill="var(--muted)">
              {v}
            </text>,
          ];
        })}
      </g>
      <g clipPath="url(#sea-clip)">
        <g className="sea">
          <path className="wave" fill="var(--water)" fillOpacity=".16" stroke="var(--water)" strokeWidth="2" d={WAVE} />
        </g>
      </g>
      <line x1="52" x2="406" y1="40" y2="40" stroke="var(--ink)" strokeWidth="3" />
      <text x="404" y="31" textAnchor="end" fill="var(--muted)">
        Deck line: disk full
      </text>
      <g id="loads">
        <line x1="56" x2="396" y1={yw} y2={yw} stroke="var(--hull-ink)" strokeDasharray="6 4" />
        <text x="384" y={yw + 13} textAnchor="end" fill="var(--hull-ink)">
          {"Low " + gbLabel(lowBytes) + " GB"}
        </text>
        <line x1="56" x2="398" y1={yc} y2={yc} stroke="var(--hull-ink)" strokeDasharray="2 3" />
        <text x="66" y={yc + 13} fill="var(--hull-ink)">
          {"Critical " + gbLabel(criticalBytes) + " GB"}
        </text>
      </g>
    </svg>
  );
}
