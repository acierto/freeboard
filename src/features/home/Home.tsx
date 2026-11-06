// The home view (prototype `.page`): top bar, the harbor stage with Dinghy,
// the helm (free space, status, forecast, goal, Where from, Toss, the pile,
// Trash), and the section tabs. Markup and class names follow the prototype
// so the visual verifier can drive both with the same steps.
import { useEffect, useState } from "react";
import { Dinghy } from "../../components/Dinghy";
import { GB, GROUPS, STATUS, gb, groupLabel, trashWord } from "../../lib/format";
import type { Goal, Group, View } from "../../lib/types";
import { SettingsSheet } from "../settings/SettingsSheet";
import { FirstRun } from "../onboarding/FirstRun";
import { QuickQuestions } from "../questions/Questions";
import { Logbook } from "../logbook/Logbook";
import { areaChoices, bubbleText, coveragePercent, forecastText, moodOf, pileItems, pileNote, questionDeck, scanLabel, tossLabel } from "./homeModel";
import { useHarbor } from "./useHarbor";

type Tab = "tidy" | "map" | "log";
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
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    if (navigate === "pile") setShowPile(true);
    if (navigate) window.scrollTo({ top: 0 });
  }, [navigate]);

  if (!h.data) return h.error ? <p className="page muted">{`Freeboard could not reach its background service. ${h.error}`}</p> : null;
  const { status, settings, suggestions, pile, map } = h.data;
  if (!settings.onboardingComplete) return <FirstRun settings={settings} platform={status.platform} onSettings={h.setSettings} onDone={() => void h.reload()} />;
  const scanning = !!status.scan.running;

  const home = status.volumes[0];
  const freeBytes = home?.freeBytes ?? 0;
  const freeGb = freeBytes / GB;
  const mood = moodOf(status.status);
  const st = STATUS[status.status];
  const items = pileItems(pile, suggestions);
  const deck = questionDeck(pile, suggestions).length;
  const coverage = map && map.notExplainedBytes > 0 ? Math.round(coveragePercent(map.usedBytes, map.notExplainedBytes)) : null;
  const areas = areaChoices(suggestions);
  const anyArea = pile.areas.length > 0;

  const toggleArea = (g: Group | null) => {
    if (!g) return h.setAreas([]);
    const next = pile.areas.includes(g) ? pile.areas.filter((x) => x !== g) : [...pile.areas, g];
    // Keep the groups in display order.
    h.setAreas(GROUPS.map((x) => x.id).filter((x) => next.includes(x)));
  };

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
          <Dinghy mood={mood} freeGb={freeGb} crates={items.map((s) => ({ id: s.id, sizeGb: s.sizeBytes / GB }))} bubble={bubbleText(mood, freeBytes, pile.totalBytes)} />

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
            <div className="stack" style={{ gap: 6 }}>
              <span style={{ fontWeight: 800 }}>How much room do you need?</span>
              <div className="chips" id="goals">
                {GOALS.map(([g, label]) => (
                  <button key={label} className="chip" data-goal={g === "all" ? "all" : g / GB} aria-pressed={pile.goalBytes === g} onClick={() => h.setGoal(g)}>
                    {label}
                  </button>
                ))}
              </div>
            </div>
            <div className="stack" style={{ gap: 6 }}>
              <span style={{ fontWeight: 800 }}>Where from?</span>
              <div className="chips" id="areas">
                <button className="chip" data-area="" aria-pressed={!anyArea} onClick={() => toggleArea(null)}>
                  Everywhere
                </button>
                {areas.map((g) => (
                  <button key={g} className="chip" data-area={groupLabel(g)} aria-pressed={pile.areas.includes(g)} onClick={() => toggleArea(g)}>
                    {groupLabel(g)}
                  </button>
                ))}
              </div>
            </div>
            {/* The confirmation sheet arrives with cleanup (task 5.4); until then Toss shows the pile. */}
            <button className="btn primary big" id="cta" data-act="confirm" disabled={!items.length} onClick={() => setShowPile(true)}>
              {tossLabel(items.length, pile.totalBytes)}
            </button>
            <p className="note" id="cta-note">
              {pileNote(pile, suggestions)}
            </p>
            {scanning && (
              <p className="note" id="scan-progress" aria-live="polite">
                {scanLabel(h.data.progress)}
              </p>
            )}
            <button className="link" data-act="toggle-pile" id="pile-btn" style={{ alignSelf: "flex-start" }} onClick={() => setShowPile((v) => !v)}>
              {showPile ? "Hide what is inside" : "See what is inside"}
            </button>
            <div id="pile" hidden={!showPile}>
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
                  {/* Emptying needs its own confirmation sheet (task 5.5). */}
                  <button className="link" data-act="empty">
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
          <QuickQuestions items={suggestions} pile={pile} act={{ add: h.add, takeOut: h.takeOut, decide: h.decide, parts: h.parts }} />
        </section>
        <section id="v-map" className="two" hidden={tab !== "map"}>
          {/* The disk map needs its Rust accounting (slice 3, tasks 5.9–5.12). */}
          <div className="card stack">
            <h3>The disk map is on its way</h3>
            <p className="muted">I can't yet account for all the space on your disk. Until I can, use Everything I found under Quick questions to see what I found.</p>
          </div>
        </section>
        <section id="v-log" className="card" hidden={tab !== "log"}>
          {tab === "log" && <Logbook platform={status.platform} refresh={status.trashBytes} />}
        </section>
      </div>
      {settingsOpen && (
        <SettingsSheet settings={settings} platform={status.platform} scanning={scanning} onChange={h.updateSettings} onSettings={h.setSettings} onClose={() => setSettingsOpen(false)} />
      )}
    </>
  );
}
