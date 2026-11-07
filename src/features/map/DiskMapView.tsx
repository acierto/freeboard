// The Disk map tab (prototype `renderMap()`): coverage ring, a squarified
// treemap of real buttons, the selected area's items and its leave-alone
// space, blind spots with working actions, growth, places checked and the
// disk report. Markup, classes and copy follow the prototype; the numbers come
// from the backend's accounting (design D4, D5).
import { areaName, t as tx } from "../../i18n";
import { itemName, tb } from "../../i18n/backend";
import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { api } from "../../lib/api";
import { track } from "../../lib/useBusy";
import { GROUPS, gb } from "../../lib/format";
import { Sheet } from "../../components/Sheet";
import type { AreaGrowth, BlindSpot, BlindSpotAction, DiskMap, PileState, PlaceChecked, Platform, Suggestion } from "../../lib/types";
import { GroupActions, Row, type Choices } from "../questions/Questions";
import { GB, causesOf, coverText, coverage, defaultTile, ringDash, splitAreas, squarify, tileClass, tilesOf, type Cause } from "./mapModel";

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
      `${tx("m.incomplete", { x: gb(map.overBytes) })} ${platform === "macos" ? tx("m.twice.mac") : tx("m.twice")}`,
    );
  if (!map.measuredAt) notes.push(tx("m.nodeep"));
  if (map.purgeableBytes && map.purgeableBytes >= 0.05 * GB) notes.push(tx("m.purge", { x: gb(map.purgeableBytes) }));
  if (map.snapshotCount) notes.push(map.snapshotCount === 1 ? tx("m.snap1") : tx("m.snapN", { n: map.snapshotCount }));
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
  const areas = splitAreas(tilesOf(map));
  const boxes = squarify(areas.tiles, 0, 0, 100, 100);
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
    <>
      <div className="map" id="map" ref={ref}>
        {boxes.map((b) => (
          <button
            key={b.t.id}
            className={tileClass(b)}
            data-tile={b.t.id}
            aria-pressed={selected === b.t.id}
            // The name contains the visible label word for word (WCAG 2.5.3),
            // and still names tiles whose label is hidden.
            aria-label={`${b.t.name} ${gb(b.t.v * GB)}`}
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
      {areas.small.length > 0 && (
        <div className="small-areas">
          <span>{tx("m.small")}</span>
          {areas.small.map((x) => (
            <button key={x.id} className="tile-chip" data-tile={x.id} aria-pressed={selected === x.id} onClick={() => onSelect(x.id)}>
              <i style={{ ["--c" as string]: x.c }}></i>
              {`${x.name} `}
              <span className="num muted">{x.v.toFixed(1) + " GB"}</span>
            </button>
          ))}
        </div>
      )}
    </>
  );
}

function Detail({ map, tile, items, spots, chosen, act, focus, platform }: { map: DiskMap; tile: string; items: Suggestion[]; spots: BlindSpot[]; chosen: Set<string>; act: Choices; focus: { id: string; n: number } | null; platform: Platform }) {
  const tiles = tilesOf(map);
  const t = tiles.find((x) => x.id === tile) ?? tiles[0];
  const size = t.v * GB;
  let rows;
  if (t.id === "unknown") {
    const { causes, left } = causesOf(map, spots, platform);
    const shown = causes.filter((c) => c.bytes > 0);
    rows = shown.length ? (
      <>
        {shown.map((c) => (
          <div className="detail-row" key={c.id}>
            <span>{c.title}</span>
            <span className="num">{c.est ? tx("m.about", { x: gb(c.bytes) }) : gb(c.bytes)}</span>
          </div>
        ))}
        <p className="note" style={{ marginTop: 8 }}>
          {(() => {
            const [a, b = ""] = tx("m.seewhy").split("{x}");
            return (
              <>
                {a}
                <b>{tx("m.why", { x: gb(left) })}</b>
                {b}
              </>
            );
          })()}
        </p>
      </>
    ) : (
      <p className="muted">{tx("m.nothingleft")}</p>
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
          // A row opened from Places checked starts expanded (a new key remounts it).
          <Row key={focus?.id === s.id ? `${s.id}#${focus.n}` : s.id} s={s} ctx="m" chosen={chosen} act={act} max={inside[0]?.sizeBytes ?? 0} startOpen={focus?.id === s.id} />
        ))}
        {/* More than one protected item share one folded row, as under
            Quick questions (a real disk had 139; found by the user). */}
        {blocked.length > 1 && (
          <details className="parts" data-open={"m-protected-" + t.id}>
            <summary>{tx("m.notouch", { n: blocked.length, x: gb(blocked.reduce((n, s) => n + s.sizeBytes, 0)) })}</summary>
            {blocked.map((s) => (
              <div className="detail-row" key={s.id}>
                <span>{itemName(s)}</span>
                <span className="num">{gb(s.sizeBytes)}</span>
              </div>
            ))}
          </details>
        )}
        {rest > 0.05 * GB && (
          <div className="detail-row">
            <span className="muted">{tx("m.leave", { x: area?.leaveAloneLabel ? tb(area.leaveAloneLabel) : tx("m.other") })}</span>
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

const TICK = (
  <svg viewBox="0 0 18 18" aria-hidden="true">
    <circle cx="9" cy="9" r="8" fill="var(--safe)" />
    <path d="M5 9.3l2.6 2.5L13 6.4" fill="none" stroke="var(--card)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

type Resolve = (ids: string[], action: BlindSpotAction) => void;

/** The short line under a fix's place: where the button leads, or what the
 * files are ("~/x has 4.2 GB of .dat files from an unknown tool."). */
function noteOf(s: BlindSpot): string | null {
  if (s.kind === "needs_permission") return s.action === "open_settings" ? tx("m.opens") : null;
  if (s.kind === "unknown_files") {
    const m = /of (\.\S+ )?files from an unknown tool\.$/.exec(s.text);
    return m ? (m[1] ? tx("m.unk.ext", { e: m[1].trim() }) : tx("m.unk")) : null;
  }
  return null;
}

/** One blind spot as a row: its place, size and fix (prototype `whyCard`). */
function FixRow({ s, cause, onResolve }: { s: BlindSpot; cause: Cause; onResolve: Resolve }) {
  const path = cause.id === "outside" || cause.id === "strange";
  // "Skip" counts a folder outside the scan list as the user's own files.
  const mine = cause.id === "outside";
  const size = s.sizeBytes === null ? tx("m.unknownsize") : cause.est ? tx("m.about", { x: gb(s.sizeBytes) }) : gb(s.sizeBytes);
  return (
    <div className={path ? "fix" : "fix wide"}>
      <span className={path ? "lbl mono" : "lbl"}>
        {tb(s.location || s.title)}
        {noteOf(s) && <small>{noteOf(s)}</small>}
      </span>
      <span className="num sz">{size}</span>
      <span className="acts">
        <button className="mini go" id={`spot-${s.id}`} data-spot={s.id} onClick={() => onResolve([s.id], s.action)}>
          {s.action === "scan_folder" ? tx("m.scan") : s.action === "open_settings" ? tx("m.allow") : s.action === "mark_mine" ? tx("m.mine") : tb(s.actionLabel)}
        </button>
        {mine && (
          <button className="mini" id={`mine-${s.id}`} data-spot-mine={s.id} title={tx("m.skip.t")} onClick={() => onResolve([s.id], "mark_mine")}>
            {tx("m.skip")}
          </button>
        )}
      </span>
    </div>
  );
}

const LIMIT = 4;

function CauseBlock({ c, onResolve }: { c: Cause; onResolve: Resolve }) {
  const [more, setMore] = useState(false);
  const shown = more ? c.open : c.open.slice(0, LIMIT);
  const rest = c.open.slice(LIMIT);
  const left = c.bytes > 0 || c.open.length > 0;
  const bulk = c.open.length > 1 && c.open.every((s) => s.action === "scan_folder");
  return (
    <div className={left ? "cause" : "cause cleared"}>
      <div className="cause-head">
        <span className={c.c === "stripes" ? "sw stripes" : "sw"} style={{ ["--c" as string]: c.c }}></span>
        <b>
          {c.title}
          {c.open.length > 1 && (
            <>
              {" "}
              <span className="muted" style={{ fontWeight: 600 }}>
                {c.open.length}
              </span>
            </>
          )}
        </b>
        <span className="num">{!left ? tx("m.done") : c.bytes > 0 ? (c.est ? tx("m.about", { x: gb(c.bytes) }) : gb(c.bytes)) : tx("m.unknownsize")}</span>
        {left && <p>{c.why}</p>}
        {bulk && (
          <button className="btn primary bulk" id={`all-${c.id}`} data-spot-all={c.id} onClick={() => onResolve(c.open.map((s) => s.id), "scan_folder")}>
            {tx("m.scanall", { n: c.open.length })}
          </button>
        )}
      </div>
      {c.open.length > 0 && (
        <div className="fix-list">
          {shown.map((s) => (
            <FixRow key={s.id} s={s} cause={c} onResolve={onResolve} />
          ))}
          {rest.length > 0 && (
            <button className="link small fix-more" data-why-more={c.id} onClick={() => setMore((v) => !v)}>
              {more ? tx("m.fewer") : tx("m.more", { n: rest.length, x: gb(rest.reduce((n, s) => n + (s.sizeBytes ?? 0), 0)) })}
            </button>
          )}
        </div>
      )}
      {c.fixed.length > 0 && (
        <details className="fixed-list">
          <summary className="fx">
            {TICK}
            {tx("m.fixed", { n: c.fixed.length })}
          </summary>
          {c.fixed.map((s) => (
            <p key={s.id}>
              <span className={c.id === "outside" || c.id === "strange" ? "mono" : undefined}>{tb(s.location || s.title)}</span>
              {`: ${s.state === "working" ? tx("m.scanning") : tb(s.doneText ?? "")}`}
            </p>
          ))}
        </details>
      )}
    </div>
  );
}

/** Why space is not explained, by cause, each with its fix (prototype
 * `whyCard`). Replaces one dashed card per blind spot. */
function WhyCard({ map, spots, onResolve, platform }: { map: DiskMap; spots: BlindSpot[]; onResolve: Resolve; platform: Platform }) {
  const { causes, left } = causesOf(map, spots, platform);
  // Not traced has no fix of its own: say so instead of promising one.
  const untraced = causes.some((c) => c.id === "untraced" && c.bytes > 0);
  const fixable = causes.some((c) => c.open.length > 0);
  if (left < 0.05 * GB && !causes.some((c) => c.open.length))
    return (
      <div className="card stack why" id="why">
        <h3>{tx("m.allclear")}</h3>
        <p className="allclear">
          {TICK}
          {tx("m.allclear.p")}
        </p>
      </div>
    );
  return (
    <div className="card stack why" id="why">
      <h3>{tx("m.why", { x: gb(left) })}</h3>
      <p className="lede">{untraced ? tx("m.lede") + (fixable ? " " + tx("m.lede.fix") : "") : tx("m.lede.all")}</p>
      <div className="wbar" aria-hidden="true">
        {causes
          .filter((c) => c.bytes > 0)
          .map((c) => (
            <i key={c.id} className={c.c === "stripes" ? "stripes" : ""} style={{ width: ((c.bytes / left) * 100).toFixed(1) + "%", ["--c" as string]: c.c }} title={`${c.title} ${gb(c.bytes)}`}></i>
          ))}
      </div>
      {causes.map((c) => (
        <CauseBlock key={c.id} c={c} onResolve={onResolve} />
      ))}
      <p className="why-foot">{untraced ? (fixable ? tx("m.foot.closer") + " " : "") + tx("m.foot") : tx("m.foot.all")}</p>
    </div>
  );
}

function Growing({ growth }: { growth: AreaGrowth[] }) {
  const top = growth.filter((g) => g.bytes > 0).slice(0, 4);
  const mx = top[0]?.bytes ?? 1;
  return (
    <section>
      <div className="sc-label">{tx("m.grow")}</div>
      {top.length ? (
        top.map((g) => (
          <div className="grow" key={g.id}>
            <span>{areaName(g.label)}</span>
            <span className="num">{"+" + (g.bytes / GB).toFixed(1) + " GB"}</span>
            <i style={{ width: ((g.bytes / mx) * 100).toFixed(0) + "%" }}></i>
          </div>
        ))
      ) : (
        <p className="muted">{tx("m.grow.none")}</p>
      )}
    </section>
  );
}

/** Folders outside the scan list that are still open blind spots. */
const notYetOf = (spots: BlindSpot[]) => spots.filter((s) => s.kind === "unscanned_folder" && s.state === "open");

/** Places checked as one line with a dot strip (prototype `coverageHTML`). */
function Coverage({ places, spots, onOpen }: { places: PlaceChecked[]; spots: BlindSpot[]; onOpen: () => void }) {
  const hit = places.filter((p) => p.state === "found").length;
  const off = places.filter((p) => p.state === "unavailable").length;
  const clean = places.length - hit - off;
  const notYet = notYetOf(spots).length;
  const parts = [tx("m.hit", { n: hit }), tx("m.clean", { n: clean })];
  if (off) parts.push(tx("m.off", { n: off }));
  if (notYet) parts.push(tx("m.notyet", { n: notYet }));
  return (
    <section className="sc-row cov">
      <div>
        <b className="num">{tx("m.places", { n: places.length })}</b>
        <span className="note num">{parts.join(" \u00b7 ")}</span>
        <span className="dots" aria-hidden="true">
          {places.map((p) => (
            <i key={p.detector} className={p.state === "found" ? "hit" : p.state === "unavailable" ? "todo" : ""}></i>
          ))}
          {Array.from({ length: notYet }, (_, n) => (
            <i key={"todo" + n} className="todo"></i>
          ))}
        </span>
      </div>
      <button className="btn" id="places-btn" data-act="places" onClick={onOpen}>
        {tx("m.seeall")}
      </button>
    </section>
  );
}

/** Every place checked (prototype `placesSheet()`): what had something,
 * grouped by area, what was clean, and the folders not looked in yet. */
function PlacesSheet({ places, items, spots, onGo, onClose }: { places: PlaceChecked[]; items: Suggestion[]; spots: BlindSpot[]; onGo: (s: Suggestion) => void; onClose: () => void }) {
  const clean = places.filter((p) => p.state === "clean").map((p) => tb(p.label));
  const off = places.filter((p) => p.state === "unavailable").map((p) => tb(p.label));
  const notYet = notYetOf(spots);
  const total = items.length + clean.length;
  const sum = (l: Suggestion[]) => l.reduce((t, s) => t + s.sizeBytes, 0);
  return (
    <Sheet title={tx("pl.title", { n: total })} onClose={onClose} className="framed" closeLabel={tx("cs.close")}>
      <div className="st-body pl">
        <div className="pl-sum">
          <span>
            <b className="num">{items.length}</b>
            {" " + tx("pl.hit")}
          </span>
          <span>
            <b className="num">{clean.length}</b>
            {" " + tx("pl.clean")}
          </span>
          {notYet.length > 0 && (
            <span>
              <b className="num">{notYet.length}</b>
              {" " + tx("pl.notyet")}
            </span>
          )}
        </div>
        <section>
          <h3 className="pl-h">
            {tx("pl.found") + " "}
            <span className="muted num">{gb(sum(items))}</span>
          </h3>
          <p className="note">{tx("pl.tap")}</p>
          {GROUPS.map(({ id, label }) => {
            const l = items.filter((s) => s.group === id).sort((a, b) => b.sizeBytes - a.sizeBytes);
            if (!l.length) return null;
            return (
              <div className="pl-group" key={id}>
                <div className="pl-gh">
                  <span>{areaName(label)}</span>
                  <span className="num">{gb(sum(l))}</span>
                </div>
                {l.map((s) => (
                  <button className="pl-row" data-place={s.id} key={s.id} onClick={() => onGo(s)}>
                    <span className={`rdot ${s.risk}`}></span>
                    <span className="pl-n">{itemName(s)}</span>
                    <span className="num">{gb(s.sizeBytes)}</span>
                    <span className="pl-go" aria-hidden="true">
                      ›
                    </span>
                  </button>
                ))}
              </div>
            );
          })}
        </section>
        <section>
          <h3 className="pl-h">{tx("pl.nothing")}</h3>
          <ul className="pl-clean">
            {clean.map((n) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
        </section>
        {off.length > 0 && (
          <section>
            <h3 className="pl-h">{tx("pl.off")}</h3>
            <p className="note">{tx("pl.off.p", { list: off.join(", ") })}</p>
          </section>
        )}
        {notYet.length > 0 && (
          <section>
            <h3 className="pl-h">{tx("pl.notyet.h")}</h3>
            <p className="note">
              {`${notYet.map((s) => s.location).join(", ")}. `}
              <button className="link" data-act="close" onClick={onClose}>
                {tx("pl.fixit")}
              </button>
            </p>
          </section>
        )}
      </div>
    </Sheet>
  );
}

function SecondOpinion() {
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
    <>
      <section className="sc-row">
        <div>
          <b>{tx("so.title")}</b>
          <span className="note">{tx("so.p")}</span>
        </div>
        <button className="btn" data-act="report" onClick={() => void copy()}>
          {tx("so.copy")}
        </button>
      </section>
      <div id="report-box">
        {box && "copied" in box && <p className="note">{tx("so.copied")}</p>}
        {box && "text" in box && <textarea id="report-text" className="report-text" aria-label={tx("so.box")} readOnly value={box.text} ref={area} />}
      </div>
    </>
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
  onResolve: Resolve;
}

export function DiskMapView({ map, spots, places, growth, items, pile, platform, act, onResolve }: MapProps) {
  const [tile, setTile] = useState<string | null>(null);
  const [placesOpen, setPlacesOpen] = useState(false);
  const [focus, setFocus] = useState<{ id: string; n: number } | null>(null);
  // Places checked: open the item on the map, already expanded.
  const go = (s: Suggestion) => {
    setPlacesOpen(false);
    setTile(s.mapArea);
    setFocus((f) => ({ id: s.id, n: (f?.n ?? 0) + 1 }));
    setTimeout(() => document.getElementById("tile-detail")?.scrollIntoView({ block: "start" }), 0);
  };
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
        <Detail map={map} tile={selected} items={items} spots={spots} chosen={chosen} act={act} focus={focus} platform={platform} />
      </div>
      <div className="stack">
        <WhyCard map={map} spots={spots} onResolve={onResolve} platform={platform} />
        <div className="card scan-card">
          <Growing growth={growth} />
          <Coverage places={places} spots={spots} onOpen={() => setPlacesOpen(true)} />
          <SecondOpinion />
        </div>
      </div>
      {placesOpen && <PlacesSheet places={places} items={items} spots={spots} onGo={go} onClose={() => setPlacesOpen(false)} />}
    </>
  );
}
