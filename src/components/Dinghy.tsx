// Dinghy and the harbor scene (prototype `.stage`): sky, stars at night, sun or
// moon, clouds, two waves, and the boat with its face and crates. Geometry is
// the prototype's; motion is CSS and respects reduced motion (harbor.css).
import { boatTransform, CRATE_SLOTS, crateLabels, wavePath, type Mood } from "./dinghyGeometry";

const INK = "var(--boat-ink)";

function Face({ mood }: { mood: Mood }) {
  const cheeks = (
    <>
      <circle cx="-82" cy="-30" r="6" fill="#FFB4A8" />
      <circle cx="-32" cy="-30" r="6" fill="#FFB4A8" />
    </>
  );
  if (mood === "good")
    return (
      <>
        {cheeks}
        <path d="M-78 -46q7 -9 14 0M-50 -46q7 -9 14 0" fill="none" stroke={INK} strokeWidth="3.5" strokeLinecap="round" />
        <path d="M-68 -30q11 12 22 0" fill={INK} />
      </>
    );
  if (mood === "warn")
    return (
      <>
        {cheeks}
        <circle cx="-71" cy="-44" r="5" fill={INK} />
        <circle cx="-43" cy="-44" r="5" fill={INK} />
        <path d="M-80 -56l13 -4M-34 -56l-13 -4" stroke={INK} strokeWidth="3" strokeLinecap="round" />
        <path d="M-66 -24q9 -8 18 0" fill="none" stroke={INK} strokeWidth="3" strokeLinecap="round" />
      </>
    );
  return (
    <>
      {cheeks}
      <circle cx="-71" cy="-44" r="7" fill="#fff" stroke={INK} strokeWidth="3" />
      <circle cx="-43" cy="-44" r="7" fill="#fff" stroke={INK} strokeWidth="3" />
      <circle cx="-71" cy="-43" r="2.5" fill={INK} />
      <circle cx="-43" cy="-43" r="2.5" fill={INK} />
      <ellipse cx="-57" cy="-24" rx="7" ry="6" fill={INK} />
      <path d="M-22 -62q6 8 0 11q-6 -3 0 -11Z" fill="#6FD0F0" />
    </>
  );
}

export interface Crate {
  id: string;
  sizeGb: number;
  leaving?: boolean;
}

export function Dinghy({ mood, freeGb, crates, bubble, eager = false }: { mood: Mood; freeGb: number; crates: Crate[]; bubble: string; eager?: boolean }) {
  const labels = crateLabels(crates.map((c) => c.sizeGb));
  return (
    <div className="stage" id="stage">
      <p className="bubble" id="bubble" aria-live="polite">
        {bubble}
      </p>
      <svg viewBox="110 40 400 290" preserveAspectRatio="xMidYMax meet" role="img" aria-label="Dinghy the boat. It sits lower in the water as the disk fills.">
        <g style={{ opacity: "var(--star)" }} fill="#FFF3C4">
          <circle cx="60" cy="130" r="2" />
          <circle cx="210" cy="104" r="1.6" />
          <circle cx="330" cy="150" r="2" />
          <circle cx="460" cy="96" r="1.6" />
          <circle cx="600" cy="150" r="2" />
          <circle cx="520" cy="190" r="1.5" />
          <circle cx="120" cy="190" r="1.5" />
        </g>
        <circle cx="456" cy="104" r="26" fill="var(--sun)" />
        <g fill="var(--cloud)">
          <ellipse cx="476" cy="160" rx="32" ry="10" />
          <ellipse cx="490" cy="152" rx="18" ry="10" />
          <ellipse cx="136" cy="150" rx="26" ry="9" />
          <ellipse cx="148" cy="143" rx="15" ry="9" />
        </g>
        <path className="wave slow" id="wave-back" fill="var(--sea-a)" d={wavePath(232, 7)} />
        <g id="boat" style={{ transform: boatTransform(freeGb) }}>
          <g className="bob">
            <path d="M-118 -6H134Q128 58 72 66H-78Q-112 58 -118 -6Z" fill="var(--hull)" />
            <path d="M-118 -6H134L131 8H-116Z" fill="var(--hull-dark)" />
            <rect x="-104" y="-82" width="94" height="78" rx="20" fill="var(--cabin)" stroke="var(--boat-ink)" strokeWidth="3" />
            <path d="M-114 -74Q-58 -104 0 -74Z" fill="var(--hull)" stroke="var(--boat-ink)" strokeWidth="3" strokeLinejoin="round" />
            <rect x="-34" y="-112" width="20" height="30" rx="5" fill="var(--hull-dark)" stroke="var(--boat-ink)" strokeWidth="3" />
            <g id="face">
              <Face mood={mood} />
            </g>
            <g id="crates" className={eager ? "eager" : undefined}>
              {crates.slice(0, labels.length).map((c, n) => (
                <g key={c.id} data-crate={c.id} className={c.leaving ? "crate-fly" : undefined}>
                  <g transform={`translate(${CRATE_SLOTS[n][0]} ${CRATE_SLOTS[n][1]})`}>
                    <rect width="32" height="26" rx="5" fill="var(--crate)" stroke="var(--crate-edge)" strokeWidth="2.5" />
                    <path d="M0 9H32M0 17H32" stroke="var(--crate-edge)" strokeWidth="1.5" />
                    <text x="16" y="17" textAnchor="middle" fontFamily="Nunito, sans-serif" fontWeight="800" fontSize="11" fill="#5B3A06">
                      {labels[n]}
                    </text>
                  </g>
                </g>
              ))}
            </g>
          </g>
        </g>
        <path className="wave" id="wave-front" fill="var(--sea-b)" fillOpacity=".92" d={wavePath(258, 9)} />
      </svg>
    </div>
  );
}
