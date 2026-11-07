// The Disk map tab (prototype `renderMap()`): coverage ring, a squarified
// treemap of real buttons, the selected area's items and its leave-alone
// space, blind spots with working actions, growth, places checked and the
// disk report. Markup, classes and copy follow the prototype; the numbers come
// from the backend's accounting (design D4, D5).
import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { api } from "../../lib/api";
import { track } from "../../lib/useBusy";
import { gb, plural } from "../../lib/format";
import type { AreaGrowth, BlindSpot, BlindSpotAction, DiskMap, PileState, PlaceChecked, Platform, Suggestion } from "../../lib/types";
import { GroupActions, Row, type Choices } from "../questions/Questions";
import { GB, coverText, coverage, defaultTile, ringDash, squarify, tileClass, tilesOf } from "./mapModel";

function Ring({ pct }: { pct: number }) {
  const { dash, done } = ringDash(pct);
  return (
    <svg viewBox="0 0 88 88" role="img" aria-label={pct.toFixed(1) + " percent of used space explained"}>
      <circle cx="44" cy="44" r="34" fill="none" stroke="var(--soft)" strokeWidth="14" />
      <circle cx="44" cy="44" r="34" fill="none" stroke={done ? "var(--safe)" : "var(--hull)"} strokeWidth="14" strokeDasharray={dash} transform="rotate(-90 44 44)" />
      <g stroke="var(--card)" strokeWidth="5">
        <path d="M44 3v14M44 71v14M3 44h14M71 44h14" />
      </g>
    </svg>
  );
}

/** Notes the prototype has no state for: an incomplete map, no deep scan
 * yet, and macOS space Finder counts differently. */
function CoverNotes({ map, platform }: { map: DiskMap; platform: Platform }) {
  const notes: string[] = [];
  if (map.incomplete)
    notes.push(
      `These areas add up to ${gb(map.overBytes)} more than the disk says is in use, so this map is incomplete. ${platform === "macos" ? "APFS clones and snapshots can be counted twice." : "Some files may be counted twice."}`,
    );
  if (!map.measuredAt) notes.push("I measure folders during a deep scan. Until one runs, most of this space is not explained.");
  if (map.purgeableBytes && map.purgeableBytes >= 0.05 * GB) notes.push(`macOS can free ${gb(map.purgeableBytes)} of purgeable space on its own. Finder counts it as free, so its totals differ from these.`);
  if (map.snapshotCount) notes.push(`macOS keeps ${plural(map.snapshotCount, "local snapshot")} of this disk.`);
  if (!notes.length) return null;
  return (
    <div className="stack" id="map-notes" style={{ gap: 4 }}>
      {notes.map((n) => (
        <p className="note" key={n}>
          {n}
        </p>
      ))}
    </div>
  );
}

function Treemap({ map, selected, onSelect }: { map: DiskMap; selected: string; onSelect: (id: string) => void }) {
  const boxes = squarify(tilesOf(map), 0, 0, 100, 100);
  const ref = useRef<HTMLDivElement>(null);
  // Labels must never clip (spec "Disk map rules"): a label that does not
  // fit its tile is hidden, as on the prototype's small tiles. The button
  // keeps its accessible name.
  useLayoutEffect(() => {
    const fit = () =>
      ref.current?.querySelectorAll<HTMLElement>(".tile").forEach((t) => {
        t.classList.remove("tiny-fit");
        const span = t.firstElementChild as HTMLElement | null;
        if (!span || t.classList.contains("tiny")) return;
        // The span clips inside its 2 px border. A label line may run into
        // it by its leading (a quarter of the line box) without cutting a
        // glyph, as on the prototype's short tiles.
        const box = span.getBoundingClientRect();
        const clipped = [...span.children].some((c) => {
          const r = c.getBoundingClientRect();
          const lead = r.height / 4;
          return r.bottom > box.bottom - 2 + lead || r.right > box.right - 2 + 1;
        });
        // A name that only fits by breaking inside a word ("Proje / cts",
        // found on a real Mac) does not fit either.
        const name = span.querySelector("b");
        let broken = false;
        if (name) {
          name.style.overflowWrap = "normal";
          broken = name.scrollWidth > name.clientWidth + 1;
          name.style.overflowWrap = "";
        }
        if (clipped || broken) t.classList.add("tiny-fit");
      });
    fit();
    window.addEventListener("resize", fit);
    return () => window.removeEventListener("resize", fit);
  });
  return (
    <div className="map" id="map" ref={ref}>
      {boxes.map((b) => (
        <button
          key={b.t.id}
          className={tileClass(b)}
          data-tile={b.t.id}
          aria-pressed={selected === b.t.id}
          // The name contains the visible label word for word (WCAG 2.5.3),
          // and still names tiles whose label is hidden.
          aria-label={`${b.t.name} ${b.t.v.toFixed(1)} GB`}
          style={{ left: b.x.toFixed(2) + "%", top: b.y.toFixed(2) + "%", width: b.w.toFixed(2) + "%", height: b.h.toFixed(2) + "%", "--c": b.t.c } as CSSProperties}
          onClick={() => onSelect(b.t.id)}
        >
          <span>
            {/* The space collapses on screen and separates the words for assistive tools. */}
            <b>{b.t.name}</b>{" "}
            <i className="num">{b.t.v.toFixed(1) + " GB"}</i>
          </span>
        </button>
      ))}
    </div>
  );
}

function Detail({ map, tile, items, spots, chosen, act }: { map: DiskMap; tile: string; items: Suggestion[]; spots: BlindSpot[]; chosen: Set<string>; act: Choices }) {
  const tiles = tilesOf(map);
  const t = tiles.find((x) => x.id === tile) ?? tiles[0];
  const size = t.v * GB;
  let rows;
  if (t.id === "unknown") {
    const open = spots.filter((s) => s.state !== "done");
    const traced = open.reduce((n, s) => n + (s.sizeBytes ?? 0), 0);
    const rest = map.notExplainedBytes - traced;
    rows = (
      <>
        {open.map((s) => (
          <div className="detail-row" key={s.id}>
            <span>{s.text}</span>
            <span className="num">{s.sizeBytes === null ? "unknown" : gb(s.sizeBytes)}</span>
          </div>
        ))}
        {rest > 0.05 * GB && (
          <div className="detail-row">
            <span className="muted">Space I could not trace to a folder</span>
            <span className="num muted">{gb(rest)}</span>
          </div>
        )}
        {!open.length && rest <= 0.05 * GB && <p className="muted">Nothing is left unexplained.</p>}
      </>
    );
  } else {
    const area = map.areas.find((a) => a.id === t.id);
    const inside = items.filter((s) => s.mapArea === t.id).sort((a, b) => b.sizeBytes - a.sizeBytes);
    const blocked = inside.filter((s) => s.blockedReason);
    // As the prototype: what is not listed above (including parts left
    // unselected) is the space to leave alone.
    const rest = Math.max(0, size - inside.reduce((n, s) => n + s.sizeBytes, 0));
    rows = (
      <>
        <GroupActions items={inside} slug={"map-" + t.id} act={act} />
        {(blocked.length > 1 ? inside.filter((s) => !s.blockedReason) : inside).map((s) => (
          <Row key={s.id} s={s} ctx="m" chosen={chosen} act={act} />
        ))}
        {/* More than one protected item share one folded row, as under
            Quick questions (a real disk had 139; found by the user). */}
        {blocked.length > 1 && (
          <details className="parts" data-open={"m-protected-" + t.id}>
            <summary>{`${blocked.length} things I will not touch · ${gb(blocked.reduce((n, s) => n + s.sizeBytes, 0))}`}</summary>
            {blocked.map((s) => (
              <div className="detail-row" key={s.id}>
                <span>{s.title}</span>
                <span className="num">{gb(s.sizeBytes)}</span>
              </div>
            ))}
          </details>
        )}
        {rest > 0.05 * GB && (
          <div className="detail-row">
            <span className="muted">{`${area?.leaveAloneLabel || "Other"} (leave these)`}</span>
            <span className="num muted">{gb(rest)}</span>
          </div>
        )}
      </>
    );
  }
  return (
    <div className="card stack" id="tile-detail">
      <div className="row between">
        <h3>{t.name}</h3>
        <span className="num muted">{gb(size)}</span>
      </div>
      <div>{rows}</div>
    </div>
  );
}

function Spots({ spots, onResolve }: { spots: BlindSpot[]; onResolve: (id: string, action: BlindSpotAction) => void }) {
  const open = spots.filter((s) => s.state !== "done").length;
  return (
    <div className="card stack">
      <h3>{open ? `${open} thing${open === 1 ? "" : "s"} I cannot see yet` : "I can see everything now"}</h3>
      {spots.map((s) =>
        s.state === "done" ? (
          <div className="spot done" key={s.id}>
            <b>{s.title}</b>
            <span className="muted">{s.doneText}</span>
          </div>
        ) : s.state === "working" ? (
          <div className="spot" key={s.id} aria-live="polite">
            <b>{s.title}</b>
            <span>{s.text}</span>
            <span className="note">{s.doneText}</span>
          </div>
        ) : (
          <div className="spot" key={s.id}>
            <b>{s.title}</b>
            <span>{s.text}</span>
            <button className="btn" style={{ alignSelf: "flex-start" }} data-spot={s.id} onClick={() => onResolve(s.id, s.action)}>
              {s.actionLabel}
            </button>
          </div>
        ),
      )}
    </div>
  );
}

function Growing({ growth }: { growth: AreaGrowth[] }) {
  const top = growth.filter((g) => g.bytes > 0).slice(0, 4);
  const mx = top[0]?.bytes ?? 1;
  return (
    <div className="card stack">
      <h3>Growing fastest this week</h3>
      {top.length ? (
        top.map((g) => (
          <div className="grow" key={g.id}>
            <span>{g.label}</span>
            <span className="num">{"+" + (g.bytes / GB).toFixed(1) + " GB"}</span>
            <i style={{ width: ((g.bytes / mx) * 100).toFixed(0) + "%" }}></i>
          </div>
        ))
      ) : (
        <p className="muted">I need a few days of scans to see what is growing.</p>
      )}
    </div>
  );
}

function Places({ places }: { places: PlaceChecked[] }) {
  const off = places.some((p) => p.state === "unavailable");
  return (
    <div className="card">
      <details data-open="places">
        <summary>{`${places.length} places I checked`}</summary>
        <p className="note" style={{ marginTop: 6 }}>
          {"Green means I found something. Grey means I looked and it was clean." + (off ? " Dashed means I could not look." : "")}
        </p>
        <div className="places">
          {places.map((p) => (
            <span className={"place" + (p.state === "found" ? " hit" : p.state === "unavailable" ? " off" : "")} key={p.detector}>
              {p.state === "unavailable" ? `${p.label} (could not check)` : p.label}
            </span>
          ))}
        </div>
      </details>
    </div>
  );
}

function Report() {
  const [box, setBox] = useState<{ copied: true } | { text: string } | null>(null);
  const area = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    if (box && "text" in box) area.current?.select();
  }, [box]);
  const copy = async () => {
    const text = JSON.stringify(await track(api.createDiskReport()), null, 2);
    // As the prototype: copy, or show the text to copy by hand.
    if (navigator.clipboard && navigator.clipboard.writeText)
      await navigator.clipboard.writeText(text).then(
        () => setBox({ copied: true }),
        () => setBox({ text }),
      );
    else setBox({ text });
  };
  return (
    <div className="card stack">
      <h3>Get a second opinion</h3>
      <p className="note">Copy a summary with sizes and folder names, then ask your AI agent what I missed. File contents are not included.</p>
      <button className="btn" style={{ alignSelf: "flex-start" }} data-act="report" onClick={() => void copy()}>
        Copy disk report
      </button>
      <div id="report-box">
        {box && "copied" in box && <p className="note">Copied. Paste it into your agent.</p>}
        {box && "text" in box && <textarea id="report-text" className="report-text" aria-label="Disk report" readOnly value={box.text} ref={area} />}
      </div>
    </div>
  );
}

export interface MapProps {
  map: DiskMap;
  spots: BlindSpot[];
  places: PlaceChecked[];
  growth: AreaGrowth[];
  items: Suggestion[];
  pile: PileState;
  platform: Platform;
  act: Choices;
  onResolve: (id: string, action: BlindSpotAction) => void;
}

export function DiskMapView({ map, spots, places, growth, items, pile, platform, act, onResolve }: MapProps) {
  const [tile, setTile] = useState<string | null>(null);
  const selected = tile ?? defaultTile(map, items);
  const chosen = new Set(pile.itemIds);
  const cover = coverText(map);
  const select = (id: string) => {
    setTile(id);
    const d = document.getElementById("tile-detail");
    if (d && window.innerWidth < 800) d.scrollIntoView({ block: "nearest" });
  };
  return (
    <>
      <div className="stack">
        <div className="card stack">
          <div className="cover">
            <Ring pct={coverage(map).pct} />
            <div>
              <b className="num">{cover.pct}</b>
              <span className="muted num">{cover.line}</span>
            </div>
          </div>
          <CoverNotes map={map} platform={platform} />
          <Treemap map={map} selected={selected} onSelect={select} />
        </div>
        <Detail map={map} tile={selected} items={items} spots={spots} chosen={chosen} act={act} />
      </div>
      <div className="stack">
        <Spots spots={spots} onResolve={onResolve} />
        <Growing growth={growth} />
        <Places places={places} />
        <Report />
      </div>
    </>
  );
}
