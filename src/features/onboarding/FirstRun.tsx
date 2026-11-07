// First-run setup in five steps (prototype `obOpen()` / `obRender()`, spec
// "First-run setup"): Hello, Waterline, Folders, Promises, First look. Dinghy
// sits in its sea on the left and talks; the right half holds the buoys, the
// step and the footer. The first look is the normal quick scan. Folders are
// added with the backend's native picker, never a typed path (invariant 3).
import { useCallback, useEffect, useRef, useState, type ReactElement } from "react";
import { api, on } from "../../lib/api";
import { GB, gb } from "../../lib/format";
import { LANGS, LANG_NAMES, lang, t, tn, unitGB, type Lang } from "../../i18n";
import { itemName } from "../../i18n/backend";
import { track } from "../../lib/useBusy";
import type { AppStatus, FolderSuggestion, Settings, Suggestion, ToolFound } from "../../lib/types";
import { displayPath } from "../settings/Folders";

const PRESETS = [
  { id: "relaxed", k: "ob.relaxed", low: 10, crit: 4 },
  { id: "rec", k: "ob.balanced", low: 20, crit: 8, rec: true },
  { id: "roomy", k: "ob.roomy", low: 50, crit: 20 },
] as const;
type PresetId = (typeof PRESETS)[number]["id"];
const FOUND_SHOWN = 6;

// What a scan event's detector family is called while it runs.
const FAMILIES = ["docker", "git", "project", "cache", "files", "mobile", "ai", "tools", "apps", "system"];
const familyName = (f: string) => (f === "docker" ? "Docker" : t("ob.fam." + f));

/** A translated sentence whose <b>…</b> parts are bold (prototype strings). */
function rich(s: string) {
  return s.split(/<b>(.*?)<\/b>/).map((part, i) => (i % 2 ? <b key={i}>{part}</b> : part));
}

/** The 49 in the prototype's sentences is the real count of places here. */
const places = (s: string, n: number | string) => s.replace("49", String(n));

function wave(y: number, a: number): string {
  let d = "M-40 " + y;
  for (let x = -40; x < 480; x += 60) d += " q15 -" + a + " 30 0 t30 0";
  return d + " V420 H-40Z";
}

type Face = "happy" | "think" | "focus" | "proud";

/** Dinghy in its sea, posed for the step (prototype `obSea()`). */
function Sea({ step, found, done }: { step: number; found: number; done: boolean }) {
  const y = step === 4 ? 238 + Math.min(found, 4) * 5 : 228;
  const face: Face = step === 4 && done ? "happy" : step === 4 ? "focus" : step === 1 ? "think" : step === 3 ? "proud" : "happy";
  const ink = "var(--boat-ink)";
  const faces: Record<Face, ReactElement> = {
    happy: (
      <>
        <circle cx="-82" cy="-30" r="6" fill="#FFB4A8" />
        <circle cx="-32" cy="-30" r="6" fill="#FFB4A8" />
        <path d="M-78 -46q7 -9 14 0M-50 -46q7 -9 14 0" fill="none" stroke={ink} strokeWidth="3.5" strokeLinecap="round" />
        <path d="M-68 -30q11 12 22 0" fill={ink} />
      </>
    ),
    think: (
      <>
        <circle cx="-71" cy="-44" r="5" fill={ink} />
        <circle cx="-43" cy="-44" r="5" fill={ink} />
        <path d="M-66 -26h18" stroke={ink} strokeWidth="3.5" strokeLinecap="round" />
        <path d="M-80 -56l14 -3M-34 -59l-14 3" stroke={ink} strokeWidth="3" strokeLinecap="round" />
      </>
    ),
    focus: (
      <>
        <circle cx="-71" cy="-42" r="6" fill={ink} />
        <circle cx="-43" cy="-42" r="6" fill={ink} />
        <circle cx="-69" cy="-44" r="2" fill="#fff" />
        <circle cx="-41" cy="-44" r="2" fill="#fff" />
        <path d="M-62 -26q5 4 10 0" fill="none" stroke={ink} strokeWidth="3.5" strokeLinecap="round" />
      </>
    ),
    proud: (
      <>
        <circle cx="-82" cy="-30" r="6" fill="#FFB4A8" />
        <circle cx="-32" cy="-30" r="6" fill="#FFB4A8" />
        <path d="M-78 -42q7 -7 14 0M-50 -42q7 -7 14 0" fill="none" stroke={ink} strokeWidth="3.5" strokeLinecap="round" />
        <path d="M-70 -28q13 8 26 0" fill="none" stroke={ink} strokeWidth="3.5" strokeLinecap="round" />
      </>
    ),
  };
  const spots: [number, number][] = [
    [22, 19],
    [62, 15],
    [100, 11],
    [42, 9],
  ];
  return (
    <svg viewBox="0 70 400 330" preserveAspectRatio="xMidYMax slice" aria-hidden="true">
      <circle cx="320" cy="150" r="26" fill="var(--sun)" />
      <g fill="var(--cloud)">
        <ellipse cx="96" cy="170" rx="30" ry="9" />
        <ellipse cx="108" cy="163" rx="16" ry="9" />
        <ellipse cx="300" cy="200" rx="24" ry="8" />
      </g>
      <path d={wave(272, 10)} fill="var(--sea-a)" />
      <g className="ob-boat" style={{ transform: `translate(200px,${y}px) scale(.78)` }}>
        <g className="bob">
          <path d="M-118 -6H134Q128 58 72 66H-78Q-112 58 -118 -6Z" fill="var(--hull)" />
          <path d="M-118 -6H134L131 8H-116Z" fill="var(--hull-dark)" />
          <rect x="-104" y="-82" width="94" height="78" rx="20" fill="var(--cabin)" stroke={ink} strokeWidth="3" />
          <path d="M-114 -74Q-58 -104 0 -74Z" fill="var(--hull)" stroke={ink} strokeWidth="3" strokeLinejoin="round" />
          <rect x="-34" y="-112" width="20" height="30" rx="5" fill="var(--hull-dark)" stroke={ink} strokeWidth="3" />
          {step === 0 && (
            <g>
              <path d="M-24 -112v-40" stroke={ink} strokeWidth="3" />
              <path d="M-24 -152l30 9 -30 9z" fill="var(--accent)">
                {/* The flag waves, except with reduced motion. */}
                {!window.matchMedia("(prefers-reduced-motion: reduce)").matches && (
                  <animateTransform attributeName="transform" type="skewY" values="0;-6;0" dur="1.4s" repeatCount="indefinite" />
                )}
              </path>
            </g>
          )}
          {faces[face]}
          {step === 2 && (
            <g transform="translate(-6 -40) rotate(-18)">
              <rect x="0" y="-7" width="46" height="14" rx="5" fill={ink} />
              <circle cx="46" cy="0" r="9" fill="#9FD8F5" stroke={ink} strokeWidth="3" />
            </g>
          )}
          {step === 3 && (
            <g transform="translate(70 -58)">
              <path d="M0 -26l22 8v12c0 14 -10 24 -22 30c-12 -6 -22 -16 -22 -30v-12z" fill="var(--safe)" stroke={ink} strokeWidth="3" />
              <path d="M-9 0l6 7 12 -14" fill="none" stroke="#fff" strokeWidth="4" strokeLinecap="round" strokeLinejoin="round" />
            </g>
          )}
          {spots.map(([x, base], n) => {
            const shown = step === 4 && n < found;
            const cy = n === 3 ? -62 : -32;
            void base;
            return (
              <g key={n} className="ob-crate" style={{ transform: `translate(${x}px,${shown ? cy : cy - 160}px)`, opacity: shown ? 1 : 0 }}>
                <rect x="-17" y="-15" width="34" height="30" rx="5" fill="var(--crate)" stroke="var(--crate-edge)" strokeWidth="3" />
                <path d="M-15 -4h30M-15 5h30" stroke="var(--crate-edge)" strokeWidth="2" />
              </g>
            );
          })}
        </g>
      </g>
      <path className="wave" d={wave(290, 12)} fill="var(--sea-b)" fillOpacity=".94" />
    </svg>
  );
}

interface Folder {
  key: string;
  path: string;
  meta: string;
  bytes: number | null;
  on: boolean;
  /** Offered by the backend (added at the first look), or already chosen. */
  offer: number | null;
}

export function FirstRun({ settings, status, onSettings, onDone }: { settings: Settings; status: AppStatus; onSettings: (s: Settings) => void; onDone: () => void }) {
  const [step, setStep] = useState(0);
  const [preset, setPreset] = useState<PresetId>(() => PRESETS.find((p) => p.low * GB === settings.lowThresholdBytes && p.crit * GB === settings.criticalThresholdBytes)?.id ?? "rec");
  const [offers, setOffers] = useState<FolderSuggestion[] | null>(null);
  const [offerOn, setOfferOn] = useState<Record<number, boolean>>({});
  const [tools, setTools] = useState<ToolFound[]>([]);
  const [sw, setSw] = useState({ menubar: settings.menuBarFreeSpace, login: settings.startAtLogin });
  const [fda, setFda] = useState(false);
  const [scan, setScan] = useState({ total: 0, done: 0, label: "", finished: false });
  const [found, setFound] = useState<Suggestion[]>([]);
  const [shown, setShown] = useState(0);
  const next = useRef<HTMLButtonElement>(null);
  const platform = status.platform;
  const vol = status.volumes[0];
  const free = (vol?.freeBytes ?? 0) / GB;
  const total = (vol?.totalBytes ?? 0) / GB;

  useEffect(() => {
    void api.folderSuggestions().then(
      (o) => {
        setOffers(o);
        setOfferOn(Object.fromEntries(o.map((_, i) => [i, true])));
      },
      () => setOffers([]),
    );
    void api.getTools().then(setTools, () => setTools([]));
  }, []);

  const folders: Folder[] = [
    ...settings.scanFolders.map((p) => ({ key: "s:" + p, path: displayPath(p, platform), meta: t("ob.m.mine"), bytes: null, on: true, offer: null })),
    ...(offers ?? []).map((o, i) => ({
      key: "o:" + o.label,
      path: o.label,
      meta: tn("ob.m.repos1", "ob.m.reposN", o.repos),
      bytes: o.bytes,
      on: offerOn[i] ?? true,
      offer: i,
    })),
  ];
  const chosen = folders.filter((f) => f.on).length;

  const skip = useCallback(() => {
    void track(api.updateSettings({ onboardingComplete: true })).then((s) => {
      onSettings(s);
      onDone();
    });
  }, [onSettings, onDone]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && step < 4) skip();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [step, skip]);

  // The first look: progress from the scan's events, then the findings drop in.
  useEffect(() => {
    if (step !== 4) return;
    const offs = [
      on("scan_progress", (p) => {
        if (p.phase === "detectors") setScan((s) => ({ ...s, total: p.currentCount }));
        if (p.phase === "detector_done") setScan((s) => ({ ...s, done: Math.max(s.done, p.currentCount), label: FAMILIES.includes((p.detector ?? "").split(".")[0]) ? (p.detector ?? "").split(".")[0] : s.label }));
      }),
      on("scan_finished", () => {
        void api.getSuggestions().then((all) => {
          const open = all.filter((s) => !s.blockedReason && s.sizeBytes > 0).sort((a, b) => b.sizeBytes - a.sizeBytes);
          setFound(open);
          setScan((s) => ({ ...s, done: s.total || s.done, finished: true }));
        });
      }),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  }, [step]);
  useEffect(() => {
    if (!scan.finished || shown >= Math.min(FOUND_SHOWN, found.length)) return;
    const t = setTimeout(() => setShown((n) => n + 1), window.matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : 140);
    return () => clearTimeout(t);
  }, [scan.finished, shown, found.length]);
  const done = scan.finished && shown >= Math.min(FOUND_SHOWN, found.length);
  useEffect(() => {
    if (done) next.current?.focus();
  }, [done]);

  const pr = PRESETS.find((p) => p.id === preset)!;

  /** Save the choices, then run the first look (the normal quick scan). */
  const startFirstLook = async () => {
    setStep(4);
    setScan({ total: 0, done: 0, label: "", finished: false });
    setShown(0);
    let s = await track(api.updateSettings({ lowThresholdBytes: pr.low * GB, criticalThresholdBytes: pr.crit * GB, menuBarFreeSpace: sw.menubar, startAtLogin: sw.login }));
    for (const f of folders) if (f.offer !== null && f.on) s = await track(api.addSuggestedFolder(f.offer)).catch(() => s);
    for (let i = settings.scanFolders.length - 1; i >= 0; i--) if (!folders[i].on) s = await track(api.removeFolder("scanFolders", i)).catch(() => s);
    onSettings(s);
    await track(api.startScan("quick")).catch(() => setScan((x) => ({ ...x, finished: true })));
  };

  const finish = () =>
    void track(api.updateSettings({ onboardingComplete: true })).then((s) => {
      onSettings(s);
      if (s.scanFolders.length) void track(api.startScan("deep")).catch(() => undefined);
      onDone();
    });

  const onNext = () => {
    if (step === 3) void startFirstLook();
    else if (step === 4) finish();
    else setStep(step + 1);
    setTimeout(() => next.current?.focus(), 0);
  };

  const f = gb(free * GB);
  const bubble = [
    t("ob.b0"),
    free < pr.crit ? t("ob.b1.crit", { f }) : free < pr.low ? t("ob.b1.warn", { f }) : t("ob.b1.good", { f }),
    t("ob.b2"),
    t("ob.b3"),
    done
      ? found.length
        ? t("ob.b4.done", { x: gb(found.reduce((n, s) => n + s.sizeBytes, 0)) })
        : t("h.b.tidy")
      : places(t("ob.b4.run", { n: scan.done }), scan.total || "…"),
  ][step];

  const label = step === 0 ? t("ob.next0") : step === 3 ? t("ob.next3") : step === 4 ? (done ? t("ob.next4") : t("ob.next4run")) : t("ob.next");
  const hint = step === 2 ? tn("ob.fchosen1", "ob.fchosenN", chosen) : step === 4 && done ? t("ob.nothing") : "";
  const steps = t("ob.steps").split("|");
  // Setup step 1's language picker applies at once, before anything else is read.
  const pickLang = (l: Lang) => void track(api.updateSettings({ language: l })).then(onSettings, () => undefined);

  return (
    <div className="ob-root" id="ob-root" role="dialog" aria-modal="true" aria-labelledby="ob-title">
      <div className="ob">
        <div className="ob-sea">
          <p className="ob-bubble" aria-live="polite">
            {bubble}
          </p>
          <Sea step={step} found={shown} done={done} />
        </div>
        <div className="ob-side">
          <div className="ob-top">
            <ol className="ob-route" aria-label={t("ob.progress")}>
              {steps.map((n, i) => (
                <li key={n} className={i < step ? "done" : i === step ? "on" : ""} aria-current={i === step ? "step" : undefined}>
                  <span className="buoy" title={n}>
                    {i < step ? "✓" : i + 1}
                  </span>
                  {i < steps.length - 1 && <span className="leg"></span>}
                </li>
              ))}
            </ol>
            {step === 0 && <LangPicker onPick={pickLang} />}
            {step < 4 && (
              <button className="ob-skip" data-ob="skip" onClick={skip}>
                {t("ob.skip")}
              </button>
            )}
          </div>
          <div className="ob-body">
            {step === 0 && <Hello free={free} total={total} name={vol?.name ?? ""} />}
            {step === 1 && <Waterline free={free} preset={preset} onPreset={setPreset} />}
            {step === 2 && <Folders folders={folders} tools={tools} onToggle={(f, on) => (f.offer !== null ? setOfferOn((m) => ({ ...m, [f.offer!]: on })) : null)} onPick={() => void track(api.addFolder("scanFolders")).then(onSettings, () => undefined)} onRemovePicked={(i) => void track(api.removeFolder("scanFolders", i)).then(onSettings, () => undefined)} pickedCount={settings.scanFolders.length} />}
            {step === 3 && <Promises sw={sw} onSw={setSw} fda={fda} onFda={() => setFda(true)} macos={platform === "macos"} />}
            {step === 4 && <FirstLook scan={scan} found={found} shown={shown} done={done} />}
          </div>
          <div className="ob-foot">
            {step > 0 && step < 4 && (
              <button className="btn" data-ob="back" onClick={() => setStep(step - 1)}>
                {t("ob.back")}
              </button>
            )}
            <span className="grow hint">{hint}</span>
            <button className="btn primary" id="ob-next" data-ob="next" ref={next} disabled={step === 4 && !done} onClick={onNext}>
              {label}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

/** Setup step 1: a compact globe and language picker next to Skip setup,
 * preset to the language in use (spec "Language design in the app"). */
function LangPicker({ onPick }: { onPick: (l: Lang) => void }) {
  return (
    <label className="ob-lang" title={t("ob.lang")}>
      <svg viewBox="0 0 20 20" width="16" height="16" aria-hidden="true">
        <circle cx="10" cy="10" r="8" fill="none" stroke="currentColor" strokeWidth="1.6" />
        <path d="M2 10h16M10 2c2.6 2.4 2.6 13.6 0 16M10 2c-2.6 2.4-2.6 13.6 0 16" fill="none" stroke="currentColor" strokeWidth="1.6" />
      </svg>
      <span className="sr-only">{t("ob.lang")}</span>
      <select id="ob-lang-sel" value={lang()} onChange={(e) => onPick(e.target.value as Lang)}>
        {LANGS.map((l) => (
          <option key={l} value={l} lang={l}>
            {LANG_NAMES[l]}
          </option>
        ))}
      </select>
    </label>
  );
}

function Hello({ free, total, name }: { free: number; total: number; name: string }) {
  const used = Math.max(0, total - free);
  return (
    <>
      <span className="ob-kicker">{t("ob.k0")}</span>
      <h2 id="ob-title">{t("ob.h0")}</h2>
      <p>{t("ob.p0")}</p>
      <div className="ob-disk">
        <div className="row1">
          <span>
            <b className="num">{gb(free * GB)}</b>
            {" " + t("ob.free")}
          </span>
          <span className="muted num">{name ? t("ob.of", { n: Math.round(total) }).replace("Macintosh HD", name) : t("ob.of", { n: Math.round(total) }).replace(", Macintosh HD", "")}</span>
        </div>
        <div className="ob-bar" aria-hidden="true">
          <i style={{ width: (total ? (used / total) * 100 : 0).toFixed(1) + "%" }}></i>
        </div>
      </div>
      <div className="ob-promises">
        <div className="ob-promise">
          <span className="ic" aria-hidden="true">
            🔔
          </span>
          <b>{t("ob.pr1")}</b>
          {t("ob.pr1.s")}
        </div>
        <div className="ob-promise">
          <span className="ic" aria-hidden="true">
            ♻️
          </span>
          <b>{t("ob.pr2")}</b>
          {t("ob.pr2.s")}
        </div>
        <div className="ob-promise">
          <span className="ic" aria-hidden="true">
            🏠
          </span>
          <b>{t("ob.pr3")}</b>
          {t("ob.pr3.s")}
        </div>
      </div>
    </>
  );
}

function Waterline({ free, preset, onPreset }: { free: number; preset: PresetId; onPreset: (p: PresetId) => void }) {
  const pr = PRESETS.find((p) => p.id === preset)!;
  const MAX = 160;
  const x = (v: number) => Math.min(100, (v / MAX) * 100);
  const stt = free < pr.crit ? [t("st.crit"), "var(--risky)"] : free < pr.low ? [t("st.warn"), "var(--review)"] : [t("st.good"), "var(--safe)"];
  return (
    <>
      <span className="ob-kicker">{t("ob.k1")}</span>
      <h2 id="ob-title">{t("ob.h1")}</h2>
      <p>{rich(t("ob.p1"))}</p>
      <div className="ob-presets" role="radiogroup" aria-label={t("ob.levels")}>
        {PRESETS.map((p) => (
          <button key={p.id} className="ob-preset" role="radio" aria-checked={p.id === preset} aria-pressed={p.id === preset} data-ob-preset={p.id} onClick={() => onPreset(p.id)}>
            {"rec" in p && p.rec ? (
              <span className="rec">{t("ob.rec")}</span>
            ) : (
              <span className="rec" style={{ visibility: "hidden" }}>
                ·
              </span>
            )}
            <b>{t(p.k)}</b>
            <span className="nums num">{t("ob.nums", { l: p.low, c: p.crit })}</span>
            <span className="note">{t(p.k + ".n")}</span>
          </button>
        ))}
      </div>
      <div className="ob-gauge">
        <div className="track">
          <i className="z crit" style={{ width: x(pr.crit) + "%" }}></i>
          <i className="z warn" style={{ left: x(pr.crit) + "%", width: x(pr.low) - x(pr.crit) + "%" }}></i>
          <i className="z good" style={{ left: x(pr.low) + "%" }}></i>
          <span className="you" style={{ left: `calc(${x(free)}% - 2px)` }} title={t("ob.you")}></span>
        </div>
        <div className="lbl num">
          <span>{t("ob.zero")}</span>
          <span>{`${MAX}+ ${unitGB()}`}</span>
        </div>
      </div>
      <p className="ob-verdict">
        {t("ob.now") + " "}
        <span style={{ color: stt[1] }}>{stt[0]}</span> <span className="muted num">{`(${gb(free * GB)} ${t("ob.free")})`}</span>
      </p>
    </>
  );
}

function Folders({
  folders,
  tools,
  onToggle,
  onPick,
  onRemovePicked,
  pickedCount,
}: {
  folders: Folder[];
  tools: ToolFound[];
  onToggle: (f: Folder, on: boolean) => void;
  onPick: () => void;
  onRemovePicked: (i: number) => void;
  pickedCount: number;
}) {
  return (
    <>
      <span className="ob-kicker">{t("ob.k2")}</span>
      <h2 id="ob-title">{t("ob.h2")}</h2>
      <p>{t("ob.p2")}</p>
      <div className="ob-folders">
        {folders.map((f, n) => (
          <label className="ob-folder" key={f.key}>
            <input
              type="checkbox"
              data-ob-folder={n}
              checked={f.on}
              onChange={(e) => (f.offer === null ? n < pickedCount && !e.target.checked && onRemovePicked(n) : onToggle(f, e.target.checked))}
            />
            <span className="fi" aria-hidden="true"></span>
            <span className="ob-ftext">
              <span className="fp">{f.path}</span>
              <span className="fm">{f.meta}</span>
            </span>
            <span className="fs num">{f.bytes !== null ? `${Math.round(f.bytes / GB)} ${unitGB()}` : ""}</span>
          </label>
        ))}
        <button className="ob-add" data-ob="add" onClick={onPick}>
          {t("ob.addf")}
        </button>
      </div>
      {tools.length > 0 && (
        <>
          <div className="ob-sub">{t("ob.tools")}</div>
          <div className="ob-tools">
            {tools.map((x) => (
              <span key={x.name} className={x.found ? "ob-tool on" : "ob-tool"} title={x.found ? undefined : t("ob.notinst")}>
                {x.name}
              </span>
            ))}
          </div>
        </>
      )}
    </>
  );
}

function Promises({ sw, onSw, fda, onFda, macos }: { sw: { menubar: boolean; login: boolean }; onSw: (s: { menubar: boolean; login: boolean }) => void; fda: boolean; onFda: () => void; macos: boolean }) {
  return (
    <>
      <span className="ob-kicker">{t("ob.k3")}</span>
      <h2 id="ob-title">{t("ob.h3")}</h2>
      <div className="ob-two">
        <div className="ob-list yes">
          <h3>{t("ob.read")}</h3>
          <ul>
            <li>{t("ob.r1")}</li>
            <li>{t("ob.r2")}</li>
            <li>{t("ob.r3")}</li>
          </ul>
        </div>
        <div className="ob-list no">
          <h3>{t("ob.never")}</h3>
          <ul>
            <li>{t("ob.n1")}</li>
            <li>{t("ob.n2")}</li>
            <li>{t("ob.n3")}</li>
          </ul>
        </div>
      </div>
      <div className="ob-switches">
        <label className="ob-switch">
          <span>
            {macos ? t("ob.sw1") : t("ob.sw1.tray")}
            <small>{t("ob.sw1.s")}</small>
          </span>
          <input className="tg-sw" type="checkbox" data-ob-sw="menubar" checked={sw.menubar} onChange={(e) => onSw({ ...sw, menubar: e.target.checked })} />
        </label>
        <label className="ob-switch">
          <span>
            {t("ob.sw2")}
            <small>{t("ob.sw2.s")}</small>
          </span>
          <input className="tg-sw" type="checkbox" data-ob-sw="login" checked={sw.login} onChange={(e) => onSw({ ...sw, login: e.target.checked })} />
        </label>
        {macos && (
          <div className="ob-switch">
            <span>
              {t("ob.fda")}
              <small>{t("ob.fda.s")}</small>
            </span>
            <button className="btn" data-ob="fda" disabled={fda} onClick={onFda}>
              {fda ? t("ob.later2") : t("ob.later")}
            </button>
          </div>
        )}
      </div>
    </>
  );
}

function FirstLook({ scan, found, shown, done }: { scan: { total: number; done: number; label: string; finished: boolean }; found: Suggestion[]; shown: number; done: boolean }) {
  const total = found.reduce((t, s) => t + s.sizeBytes, 0);
  const safe = found.filter((s) => s.risk === "safe").reduce((t, s) => t + s.sizeBytes, 0);
  const of = scan.total || 0;
  return (
    <>
      <span className="ob-kicker">{t("ob.k4")}</span>
      <h2 id="ob-title">{done ? t("ob.h4.done") : t("ob.h4.run")}</h2>
      <div className="ob-scan">
        <div className="ob-prog" role="progressbar" aria-valuemin={0} aria-valuemax={of || 1} aria-valuenow={scan.done}>
          <i style={{ width: (of ? (scan.done / of) * 100 : scan.finished ? 100 : 0).toFixed(1) + "%" }}></i>
        </div>
        <div className="ob-scanline">
          <span>{scan.finished ? places(t("ob.checked"), of || scan.done) : scan.label ? t("ob.checking", { x: familyName(scan.label) }) : t("ob.starting")}</span>
          <span className="num">{of ? `${scan.done} / ${of}` : ""}</span>
        </div>
      </div>
      <div className="ob-found">
        {found.slice(0, shown).map((s) => (
          <div key={s.id}>
            <span>{itemName(s)}</span>
            <span className="num">{gb(s.sizeBytes)}</span>
          </div>
        ))}
      </div>
      {done && (
        <div className="ob-total">
          <b className="num">{gb(total)}</b>
          <span>
            {(found.length ? t("ob.total") : t("ob.tidy")) + " "}
            {found.length > 0 && <span className="muted">{t("ob.rebuild", { x: gb(safe) })}</span>}
          </span>
        </div>
      )}
    </>
  );
}
