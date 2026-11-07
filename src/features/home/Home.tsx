// The home view (prototype `.page`): top bar, the harbor stage with Dinghy,
// the helm (free space, status, forecast, goal, Where from, Toss, the pile,
// Trash), and the section tabs. Markup and class names follow the prototype
// so the visual verifier can drive both with the same steps.
import { useEffect, useState } from "react";
import { Dinghy } from "../../components/Dinghy";
import { GB, STATUS, gb, groupLabel, trashWord } from "../../lib/format";
import type { Goal, Group, View } from "../../lib/types";
import { SettingsSheet } from "../settings/SettingsSheet";
import { FirstRun } from "../onboarding/FirstRun";
import { QuickQuestions } from "../questions/Questions";
import { Logbook } from "../logbook/Logbook";
import { DiskMapView } from "../map/DiskMapView";
import { coverageBadge } from "../map/mapModel";
import { areaChoices, areaTicked, bubbleText, forecastText, meterOf, moodOf, pileItems, pileNote, questionDeck, reviewOnlyAreas, safeIn, scanLabel, toggledAreas, tossNote } from "./homeModel";
import { useHarbor } from "./useHarbor";
import { CleanupSheets } from "../cleanup/CleanupSheets";
import { progressOf } from "../cleanup/cleanupModel";
import { useCleanup } from "../cleanup/useCleanup";
import type { Crate } from "../../components/Dinghy";

type Tab = "tidy" | "map" | "log";
// The wave strip along the bottom of the Toss button (prototype `.splash`).
const SPLASH = "M0 6 q6 -5 12 0" + " t12 0".repeat(33) + " V10 H0Z";
const GOALS: [Goal, string][] = [
  [20 * GB, "20 GB"],
  [50 * GB, "50 GB"],
  [100 * GB, "100 GB"],
  ["all", "All I can"],
];

export function Home({ navigate }: { navigate: View | null }) {
  const h = useHarbor();
  const [tab, setTab] = useState<Tab>("tidy");
  const [showPile, setShowPile] = useState(false);
  // Pointing at Toss makes the crates on deck bounce (prototype `.eager`).
  const [eager, setEager] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const cleanup = useCleanup(h.data?.status.platform ?? "macos", () => void h.reload());
  // While a cleanup runs, the crates on deck are the ones that were there
  // when it started; each leaves as its item is removed (prototype `run()`).
  const [deckAtRun, setDeckAtRun] = useState<Crate[] | null>(null);
  const running = cleanup.flow?.kind === "run";

  useEffect(() => {
    if (navigate === "pile") setShowPile(true);
    if (navigate) window.scrollTo({ top: 0 });
  }, [navigate]);

  if (!h.data) return h.error ? <p className="page muted">{`Freeboard could not reach its background service. ${h.error}`}</p> : null;
  const { status, settings, suggestions, pile, map } = h.data;
  if (!settings.onboardingComplete) return <FirstRun settings={settings} platform={status.platform} onSettings={h.setSettings} onDone={() => void h.reload()} />;
  const scanning = !!status.scan.running;

  const home = status.volumes[0];
  const run = cleanup.flow?.kind === "run" ? cleanup.flow : null;
  // The boat rises as measured free space comes back during a run.
  const freeBytes = run?.run?.freeBytesNow ?? home?.freeBytes ?? 0;
  const freeGb = freeBytes / GB;
  const mood = moodOf(status.status);
  const st = STATUS[status.status];
  const items = pileItems(pile, suggestions);
  const pileCrates: Crate[] = items.map((s) => ({ id: s.id, sizeGb: s.sizeBytes / GB }));
  if (running && !deckAtRun) setDeckAtRun(pileCrates);
  if (!running && deckAtRun) setDeckAtRun(null);
  const gone = run ? progressOf(run.plan, run.run).gone : null;
  const crates = running && deckAtRun ? deckAtRun.map((c) => ({ ...c, leaving: !!gone?.has(c.id) })) : pileCrates;
  const deck = questionDeck(pile, suggestions).length;
  // Prototype: the badge shows while some space is unexplained.
  const coverage = coverageBadge(map);
  const areas = areaChoices(suggestions);
  const others = reviewOnlyAreas(suggestions);
  const anyArea = pile.areas.length > 0 || pile.noAreas;
  const meter = meterOf(pile, suggestions);
  const note = pileNote(pile, suggestions);

  const toggleArea = (g: Group | null) => {
    if (!g) return h.setAreas([]);
    const next = toggledAreas(pile, g, areas);
    h.setAreas(next.areas, next.none);
  };
  const toQuestions = () => {
    setTab("tidy");
    const t = document.getElementById("tab-tidy");
    t?.scrollIntoView({ block: "start", behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
    t?.focus({ preventScroll: true });
  };
  const canToss = items.length > 0 && !running;
  const eagerOn = () => canToss && !window.matchMedia("(prefers-reduced-motion: reduce)").matches && setEager(true);

  return (
    <>
      <div className="page">
        <header className="top">
          <div>
            <div className="brand">Freeboard</div>
          </div>
          <div className="top-right">
            <button className="icon-btn" data-act="settings" aria-label="Settings" onClick={() => setSettingsOpen(true)}>
              {"⚙"}
            </button>
          </div>
        </header>

        <section className="hero">
          <Dinghy mood={mood} freeGb={freeGb} crates={crates} bubble={bubbleText(mood, freeBytes, pile.totalBytes)} eager={eager && canToss} />

          <div className="card helm">
            <div className="row between">
              <div className="big-free">
                <span id="free" className="num">
                  {freeGb.toFixed(1)}
                </span>{" "}
                <small>GB free</small>
              </div>
              <span className={"pill " + st.key} id="status">
                {st.label}
              </span>
            </div>
            <p className="muted" id="forecast">
              {forecastText(status.forecast)}
            </p>
            <div className="make" role="group" aria-labelledby="make-title">
              <h3 id="make-title">Make room</h3>
              <div className="field">
                <span className="flabel" id="goal-label">
                  How much
                </span>
                <div className="seg" id="goals" role="group" aria-labelledby="goal-label">
                  {GOALS.map(([g, label]) => (
                    <button key={label} data-goal={g === "all" ? "all" : g / GB} aria-pressed={pile.goalBytes === g && pile.mode !== "manual"} onClick={() => h.setGoal(g)}>
                      {label}
                    </button>
                  ))}
                </div>
              </div>
              <div className="field">
                <div className="flabel-row">
                  <span className="flabel" id="area-label">
                    From
                  </span>
                  <button className="link small" id="areas-all" data-area="" disabled={!anyArea} onClick={() => toggleArea(null)}>
                    {anyArea ? "Use all areas" : "All areas"}
                  </button>
                </div>
                <div className="areas" id="areas" role="group" aria-labelledby="area-label">
                  {areas.map((g) => (
                    <button key={g} className="area" data-area={groupLabel(g)} aria-pressed={areaTicked(pile, g)} onClick={() => toggleArea(g)}>
                      <span className="box">
                        <svg viewBox="0 0 12 12" aria-hidden="true">
                          <path d="M2 6.5l2.5 2.5L10 3" fill="none" stroke="var(--accent-ink)" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" />
                        </svg>
                      </span>
                      {groupLabel(g)} <small className="num">{gb(safeIn(g, suggestions))}</small>
                    </button>
                  ))}
                  {others.length > 0 && (
                    <p className="flabel-note" title={others.map(groupLabel).join(", ")}>
                      {`${others.length === 1 ? `${groupLabel(others[0])} needs` : `${groupLabel(others[0])} and ${others.length - 1} more areas need`} your OK first.`}
                    </p>
                  )}
                </div>
              </div>
              <div className="field" id="meter">
                <div className="meter-top">
                  <span className="num">
                    <b>{gb(meter.total)}</b> ready to toss
                  </span>
                  <span className="num muted">{meter.right}</span>
                </div>
                <div className="meter" role="img" aria-label={`${gb(meter.total)} of ${gb(meter.target)} ready`}>
                  <i className={meter.manual ? "picked" : ""} style={{ width: meter.fill.toFixed(1) + "%" }} />
                </div>
                {meter.short > 0 && (
                  <div className="gaprow">
                    <span className="num">{`${gb(meter.short)} more needs your OK`}</span>
                    <button className="link small" data-act="goto-questions" onClick={toQuestions}>
                      Review items
                    </button>
                  </div>
                )}
              </div>
            </div>
            <div className="cta-row">
              <button
                className="btn primary big toss"
                id="cta"
                data-act="confirm"
                // Not disabled while a cleanup runs: the run's sheet covers
                // the page, and the click below is ignored then.
                disabled={!items.length}
                onClick={() => !running && cleanup.fromPile()}
                onPointerOver={eagerOn}
                onFocus={eagerOn}
                onPointerOut={() => setEager(false)}
                onBlur={() => setEager(false)}
              >
                {items.length ? (
                  <>
                    <svg className="tcrate" viewBox="0 0 30 26" aria-hidden="true">
                      <rect x="1.5" y="1.5" width="27" height="23" rx="5" fill="var(--crate)" stroke="var(--crate-edge)" strokeWidth="2.5" />
                      <path d="M1.5 9.5H28.5M1.5 16.5H28.5" stroke="var(--crate-edge)" strokeWidth="1.6" />
                    </svg>
                    <span className="tlabel">
                      <span className="tmain">
                        {`Toss ${gb(pile.totalBytes)}`}
                        <span className="tlong"> overboard</span>
                      </span>{" "}
                      {/* The visible text is the button's name (WCAG 2.5.3); the space separates its two lines for assistive tools. */}
                      <small className="tnote">{tossNote(items)}</small>
                    </span>
                    <svg className="splash" viewBox="0 0 400 10" preserveAspectRatio="none" aria-hidden="true">
                      <path d={SPLASH} fill="var(--accent-ink)" />
                    </svg>
                  </>
                ) : (
                  "Nothing to toss"
                )}
              </button>
              <button className="btn pile-toggle" data-act="toggle-pile" id="pile-btn" aria-controls="pile" aria-expanded={showPile && items.length > 0} disabled={!items.length} onClick={() => setShowPile((v) => !v)}>
                {`${items.length} crate${items.length === 1 ? "" : "s"}`}
              </button>
            </div>
            <p className="note" id="cta-note">
              {note}
            </p>
            {scanning && (
              <p className="note" id="scan-progress" aria-live="polite">
                {scanLabel(h.data.progress)}
              </p>
            )}
            <div id="pile" hidden={!showPile || !items.length}>
              {items.map((s) => (
                <div className="pile-row" key={s.id}>
                  <span>
                    {s.title}
                    <span className="sub">{s.recovery}</span>
                  </span>
                  <span className="num">{gb(s.sizeBytes)}</span>
                  <button className="x" data-out={s.id} aria-label={`Take ${s.title} out of the pile`} title="Take out of the pile" onClick={() => h.takeOut(s.id)}>
                    {"×"}
                  </button>
                </div>
              ))}
            </div>
            <div id="trash">
              {status.trashBytes > 0 && (
                <div className="row between note">
                  <span className="num">{`${gb(status.trashBytes)} is in the ${trashWord(status.platform)} and not free yet.`}</span>
                  <button className="link" data-act="empty" onClick={cleanup.openTrash}>
                    Empty it
                  </button>
                </div>
              )}
            </div>
          </div>
        </section>

        <div className="tabs" role="tablist" aria-label="Freeboard sections">
          <button className="tab" role="tab" data-tab="tidy" id="tab-tidy" aria-selected={tab === "tidy"} onClick={() => setTab("tidy")}>
            Quick questions
            {deck > 0 && <span className="dot num">{deck}</span>}
          </button>
          <button className="tab" role="tab" data-tab="map" id="tab-map" aria-selected={tab === "map"} onClick={() => setTab("map")}>
            Disk map
            {coverage !== null && <span className="dot num">{coverage}%</span>}
          </button>
          <button className="tab" role="tab" data-tab="log" id="tab-log" aria-selected={tab === "log"} onClick={() => setTab("log")}>
            Logbook
          </button>
        </div>

        <section id="v-tidy" className="two" hidden={tab !== "tidy"}>
          <QuickQuestions items={suggestions} pile={pile} act={{ add: h.add, takeOut: h.takeOut, decide: h.decide, parts: h.parts, remove: cleanup.fromItems, toss: cleanup.fromPile }} />
        </section>
        <section id="v-map" className="two" hidden={tab !== "map"}>
          {map && (
            <DiskMapView
              map={map}
              spots={h.data.spots}
              places={h.data.places}
              growth={h.data.growth}
              items={suggestions}
              pile={pile}
              platform={status.platform}
              act={{ add: h.add, takeOut: h.takeOut, decide: h.decide, parts: h.parts, remove: cleanup.fromItems, toss: cleanup.fromPile }}
              onResolve={h.resolveSpot}
            />
          )}
        </section>
        <section id="v-log" className="card" hidden={tab !== "log"}>
          {tab === "log" && <Logbook platform={status.platform} refresh={`${status.trashBytes}|${status.volumes[0]?.freeBytes ?? 0}`} />}
        </section>
      </div>
      <CleanupSheets c={cleanup} />
      {settingsOpen && (
        <SettingsSheet settings={settings} platform={status.platform} freeBytes={freeBytes} scanning={scanning} onChange={h.updateSettings} onSettings={h.setSettings} onClose={() => setSettingsOpen(false)} />
      )}
    </>
  );
}
