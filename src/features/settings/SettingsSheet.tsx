// Settings behind the gear button (prototype `settingsHTML()`): warning
// levels on a zone bar with steppers, folders scanned and skipped, what is
// always protected, and scanning on request. Changes save as they happen.
//
// Unlike the prototype, folders are added through the backend's native
// folder picker, never by typing a path (the interface sends no paths), and
// folder rows count Git repositories, which is what the scan finds.
import { useEffect, useState } from "react";
import { Sheet } from "../../components/Sheet";
import { api } from "../../lib/api";
import { GB, gb, plural } from "../../lib/format";
import { LANGS, LANG_NAMES, ago as agoIn, fx, lang, t, tn, unitGB, type Lang, type LanguagePref } from "../../i18n";
import { track } from "../../lib/useBusy";
import type { DeepScanSchedule, FolderDetails, FolderInfo, FolderList, Platform, QuickScanSchedule, ScanKind, Settings, SettingsPatch, VolumeChoice } from "../../lib/types";
import { displayPath } from "./Folders";

const MAX = 60;
const pct = (v: number) => Math.min(100, (v / MAX) * 100);

const FOLDER_SVG = (
  <svg className="ficon" viewBox="0 0 34 28" aria-hidden="true">
    <path d="M2 6a3 3 0 0 1 3-3h8l3 3h13a3 3 0 0 1 3 3v14a3 3 0 0 1-3 3H5a3 3 0 0 1-3-3Z" fill="var(--crate)" stroke="var(--crate-edge)" strokeWidth="2" />
  </svg>
);
const LOCK_SVG = (
  <svg viewBox="0 0 12 12" aria-hidden="true">
    <rect x="1.5" y="5" width="9" height="6.5" rx="1.5" fill="var(--muted)" />
    <path d="M3.5 5V3.6a2.5 2.5 0 0 1 5 0V5" fill="none" stroke="var(--muted)" strokeWidth="1.5" />
  </svg>
);

/** Folder sizes in whole GB from 1 GB up, as the prototype writes them. */
const folderSize = (b: number) => (b >= GB ? `${Math.round(b / GB)} ${unitGB()}` : gb(b));

/** "14 repositories, 96 GB"; just the size for a folder without repositories. */
export function folderMeta(f: FolderInfo | undefined): string {
  if (!f) return "";
  if (f.repos === 0) return f.bytes == null ? t("s.size.later") : folderSize(f.bytes);
  const repos = tn("ob.m.repos1", "ob.m.reposN", f.repos);
  return `${repos}, ${f.bytes == null ? t("s.size.later2") : folderSize(f.bytes)}`;
}

export function ago(iso: string | null, now = Date.now()): string {
  if (!iso) return t("s.never.scan");
  const min = Math.floor((now - Date.parse(iso)) / 60000);
  if (lang() !== "en") return agoIn(min);
  if (min < 1) return "just now";
  if (min < 60) return `${plural(min, "minute")} ago`;
  const h = Math.floor(min / 60);
  if (h < 24) return `${plural(h, "hour")} ago`;
  return `${plural(Math.floor(h / 24), "day")} ago`;
}

/** "Last scan 18 minutes ago. Quick scans run every hour." */
function lastScanLine(when: string, q: QuickScanSchedule): string {
  if (q === "hourly") return t("s.last", { t: when });
  const every = { every6_hours: "s.q.6h", daily: "s.q.daily", on_request: "s.q.ask" }[q];
  return t("s.lastonly", { t: when }) + " " + t(every);
}

const QUICK: QuickScanSchedule[] = ["hourly", "every6_hours", "daily", "on_request"];
const DEEP: DeepScanSchedule[] = ["on_request", "daily"];

/** In the background (supplemental; the prototype has no such group): scan
 * schedules, battery, login, notifications and other disks to watch. Rows in
 * the sheet's own list style: label and hint left, control right. */
function Background({ settings, onChange }: { settings: Settings; onChange: (p: SettingsPatch) => void }) {
  const [volumes, setVolumes] = useState<VolumeChoice[] | null>(null);
  useEffect(() => {
    track(api.getVolumes()).then(setVolumes, () => setVolumes([]));
  }, []);
  const toggle = (id: string, label: string, hint: string, on: boolean, set: (v: boolean) => void) => (
    <label className="frow setrow" htmlFor={id}>
      <span>
        <b>{label}</b>
        <small>{hint}</small>
      </span>
      <input type="checkbox" className="switch" id={id} checked={on} onChange={(e) => set(e.target.checked)} />
    </label>
  );
  return (
    <section className="st-sec" aria-labelledby="st-bg" data-supplemental="background">
      <h3 id="st-bg">{t("s.bg.h")}</h3>
      <p>{t("s.bg.p")}</p>
      <div className="st-list">
        <div className="frow setrow">
          <label htmlFor="quick-every">
            <b>{t("s.bg.quick")}</b>
            <small>{t("s.bg.quick.s")}</small>
          </label>
          <select id="quick-every" value={settings.quickScan} onChange={(e) => onChange({ quickScan: e.target.value as QuickScanSchedule })}>
            {QUICK.map((q) => (
              <option key={q} value={q}>
                {t("s.bg.q." + q)}
              </option>
            ))}
          </select>
        </div>
        <div className="frow setrow">
          <label htmlFor="deep-every">
            <b>{t("s.bg.deep")}</b>
            <small>{t("s.bg.deep.s")}</small>
          </label>
          <select id="deep-every" value={settings.deepScan} onChange={(e) => onChange({ deepScan: e.target.value as DeepScanSchedule })}>
            {DEEP.map((q) => (
              <option key={q} value={q}>
                {t("s.bg.d." + q)}
              </option>
            ))}
          </select>
        </div>
        {toggle("pause-battery", t("s.bg.battery"), t("s.bg.battery.s"), settings.pauseDeepScansOnBattery, (v) => onChange({ pauseDeepScansOnBattery: v }))}
        {toggle("start-login", t("s.bg.login"), t("s.bg.login.s"), settings.startAtLogin, (v) => onChange({ startAtLogin: v }))}
        {toggle("notify", t("s.bg.notify"), t("s.bg.notify.s"), settings.notifications, (v) => onChange({ notifications: v }))}
      </div>
      <div className="st-sub">{t("s.bg.disks")}</div>
      <div className="st-list">
        {volumes && volumes.length === 0 && <div className="empty">{t("s.bg.nodisks")}</div>}
        {(volumes ?? []).map((v) =>
          toggle(`disk-${v.id}`, v.name, t("s.bg.disk.s", { x: gb(v.freeBytes), t: gb(v.totalBytes) }), v.watched, (on) =>
            void track(api.setVolumeWatched(v.id, on)).then(setVolumes, () => undefined),
          ),
        )}
      </div>
    </section>
  );
}

/** A translated sentence with a folder path (in mono) where `{p}` stands. */
function withPath(key: string, path: string) {
  const [a, b = ""] = t(key).split("{p}");
  return (
    <span>
      {a}
      <span className="mono">{path}</span>
      {b}
    </span>
  );
}

/** "12,4 GB frei": how sizes look in a language (prototype `sample()`). */
const sample = (l: Lang) => `${fx(12.4, l)} ${l === "fr" ? "Go libres" : t("gb.free", undefined, l)}`;

/** Settings → Language (spec "Language design in the app"): six tiles, each
 * language in its own name with a preview of its number format. */
function LanguageSection({ pref, system, onPick }: { pref: LanguagePref; system: Lang; onPick: (l: LanguagePref) => void }) {
  const tile = (v: LanguagePref, name: string, sub: string, l?: Lang) => (
    <button key={v} className="lang-tile" role="radio" aria-checked={pref === v} data-applang={v} lang={l} onClick={() => onPick(v)}>
      <b>{name}</b>
      <small className="num">{sub}</small>
    </button>
  );
  return (
    <section className="st-sec" aria-labelledby="st-lang">
      <h3 id="st-lang">{t("s.lang.h")}</h3>
      <p>{t("s.lang.p")}</p>
      <div className="lang-tiles" role="radiogroup" aria-labelledby="st-lang">
        {tile("system", t("s.lang.system"), LANG_NAMES[system])}
        {LANGS.map((l) => tile(l, LANG_NAMES[l], sample(l), l))}
      </div>
    </section>
  );
}

function Stepper({ id, label, value, lo, hi, d, onStep }: { id: string; label: string; value: number; lo: number; hi: number; d: number; onStep: (v: number) => void }) {
  return (
    <span className="stepper" role="group" aria-label={label}>
      <button id={`${id}-dn`} aria-label={t("a.lower")} disabled={value - d < lo} onClick={() => onStep(value - d)}>
        −
      </button>
      <output className="num" id={`${id}-out`} aria-live="polite">{`${value} ${unitGB()}`}</output>
      <button id={`${id}-up`} aria-label={t("a.higher")} disabled={value + d > hi} onClick={() => onStep(value + d)}>
        +
      </button>
    </span>
  );
}

export function SettingsSheet({
  settings,
  platform,
  systemLanguage,
  freeBytes,
  scanning,
  onChange,
  onSettings,
  onClose,
}: {
  settings: Settings;
  platform: Platform;
  /** The system's language, for "Same as my Mac". */
  systemLanguage: Lang;
  freeBytes: number;
  scanning: boolean;
  onChange: (p: SettingsPatch) => void;
  onSettings: (s: Settings) => void;
  onClose: () => void;
}) {
  const [low, setLow] = useState(Math.round(settings.lowThresholdBytes / GB));
  const [crit, setCrit] = useState(Math.round(settings.criticalThresholdBytes / GB));
  const [details, setDetails] = useState<FolderDetails | null>(null);
  const [kind, setKind] = useState<ScanKind | null>(null);
  const folderKey = settings.scanFolders.join("\n") + "\u0000" + settings.exclusions.join("\n");
  useEffect(() => {
    let live = true;
    api.getFolderDetails().then((d) => live && setDetails(d), () => undefined);
    return () => {
      live = false;
    };
  }, [folderKey, scanning]);
  useEffect(() => {
    if (!scanning) setKind(null);
  }, [scanning]);

  const run = (p: Promise<Settings>) => void track(p).then(onSettings, () => undefined);
  const scan = (k: ScanKind) => {
    setKind(k);
    void track(api.startScan(k)).catch(() => setKind(null));
  };
  const free = freeBytes / GB;
  const removed = details?.removed ?? null;
  const undo = (list: FolderList) =>
    removed && removed.list === list ? (
      <div className="undo">
        <span>
          {withPath(list === "scanFolders" ? "s.noscan" : "s.noskip", removed.label)}
        </span>
        <button className="mini" id="undo-rm" onClick={() => run(api.undoRemoveFolder())}>
          {t("s.undo")}
        </button>
      </div>
    ) : null;
  const row = (list: FolderList, p: string, i: number, meta: string) => (
    <div className="frow" key={p}>
      {FOLDER_SVG}
      <span>
        <span className="path">{displayPath(p, platform)}</span>
        <small>{meta}</small>
      </span>
      <button
        className="icon-x"
        id={`${list === "scanFolders" ? "scan" : "skip"}-rm-${i}`}
        data-remove={`${list}-${i}`}
        aria-label={t(list === "scanFolders" ? "a.stopscan" : "a.stopskip", { p: displayPath(p, platform) })}
        title={t("a.remove")}
        onClick={() => run(api.removeFolder(list, i))}
      >
        ×
      </button>
    </div>
  );
  const status = scanning ? (kind === "quick" ? t("s.quickrun") : kind === "deep" ? t("s.deeprun") : t("s.anyrun")) : lastScanLine(ago(details?.lastScanAt ?? null), settings.quickScan);
  const suggestions = details?.suggestions ?? [];

  return (
    <Sheet title={t("s.title")} onClose={onClose} className="settings" closeLabel={t("s.close")}>
      <div className="st-body">
        <LanguageSection pref={settings.language} system={systemLanguage} onPick={(language) => onChange({ language })} />
        <section className="st-sec" aria-labelledby="st-warn">
          <h3 id="st-warn">{t("s.warn.h")}</h3>
          <p>{t("s.warn.p")}</p>
          <div className="zone-wrap">
            <span className="zone-you num" style={{ left: Math.max(6, Math.min(94, pct(free))).toFixed(1) + "%" }}>{t("s.you", { x: gb(freeBytes) })}</span>
            <div className="zone" aria-hidden="true">
              <i style={{ width: pct(crit) + "%", background: "var(--risky)" }} />
              <i style={{ width: pct(low) - pct(crit) + "%", background: "var(--review)" }} />
              <i style={{ flex: 1, background: "var(--safe)" }} />
            </div>
          </div>
          <div className="zone-ticks num" aria-hidden="true">
            <span style={{ left: 0 }}>0</span>
            <span style={{ left: pct(crit) + "%" }}>{crit}</span>
            <span style={{ left: pct(low) + "%" }}>{low}</span>
            <span style={{ left: "100%" }}>{`60+ ${unitGB()}`}</span>
          </div>
          <div className="limit">
            <span className="sw" style={{ background: "var(--review)" }} />
            <span>
              <b>{t("s.warnme")}</b>
              <small>{t("s.warnme.s")}</small>
            </span>
            <Stepper
              id="low"
              label={t("a.warnlevel")}
              value={low}
              lo={crit + 5}
              hi={MAX}
              d={5}
              onStep={(v) => {
                setLow(v);
                onChange({ lowThresholdBytes: v * GB });
              }}
            />
          </div>
          <div className="limit">
            <span className="sw" style={{ background: "var(--risky)" }} />
            <span>
              <b>{t("s.urgent")}</b>
              <small>{t("s.urgent.s")}</small>
            </span>
            <Stepper
              id="crit"
              label={t("a.urgentlevel")}
              value={crit}
              lo={2}
              hi={low - 2}
              d={1}
              onStep={(v) => {
                setCrit(v);
                onChange({ criticalThresholdBytes: v * GB });
              }}
            />
          </div>
        </section>

        <section className="st-sec" aria-labelledby="st-scan">
          <h3 id="st-scan">{t("s.scan.h")}</h3>
          <p>{t("s.scan.p")}</p>
          <div className="st-list">
            {settings.scanFolders.length ? settings.scanFolders.map((p, i) => row("scanFolders", p, i, folderMeta(details?.scan[i]))) : <div className="empty">{t("s.empty1")}</div>}
          </div>
          {undo("scanFolders")}
          {suggestions.length > 0 && (
            <>
              <div className="st-sub">{t("s.found")}</div>
              <div className="st-list">
                {suggestions.map((o, i) => (
                  <div className="frow sugg" key={o.label}>
                    {FOLDER_SVG}
                    <span>
                      <span className="path">{o.label}</span>
                      <small>{folderMeta({ repos: o.repos, bytes: details?.suggestionBytes[i] ?? null })}</small>
                    </span>
                    <button className="mini go" id={`sugg-add-${i}`} data-offer={i} aria-label={t("a.add", { p: o.label })} onClick={() => run(api.addSuggestedFolder(i))}>
                      {t("ob.addb")}
                    </button>
                  </div>
                ))}
              </div>
            </>
          )}
          <div>
            <button className="btn" id="add-scan" data-add="scanFolders" onClick={() => run(api.addFolder("scanFolders"))}>
              {t("s.add")}
            </button>
          </div>
        </section>

        <section className="st-sec" aria-labelledby="st-skip">
          <h3 id="st-skip">{t("s.skip.h")}</h3>
          <p>{t("s.skip.p")}</p>
          <div className="st-list">{settings.exclusions.length ? settings.exclusions.map((p, i) => row("exclusions", p, i, t("s.never"))) : <div className="empty">{t("s.empty2")}</div>}</div>
          {undo("exclusions")}
          <div>
            <button className="btn" id="add-skip" data-add="exclusions" onClick={() => run(api.addFolder("exclusions"))}>
              {t("s.skipadd")}
            </button>
          </div>
        </section>

        <section className="st-sec" aria-labelledby="st-lock">
          <h3 id="st-lock">{t("s.lock.h")}</h3>
          <p>{t("s.lock.p")}</p>
          <div className="lockchips">
            {["s.l1", "s.l2", "s.l3", "s.l4"].map((k) => (
              <span key={k}>
                {LOCK_SVG}
                {t(k)}
              </span>
            ))}
          </div>
        </section>

        <section className="st-sec" aria-labelledby="st-run">
          <h3 id="st-run">{t("s.run.h")}</h3>
          <p id="scan-status" aria-live="polite">
            {status}
          </p>
          <div className="scanrow">
            <button className="scanbtn" id="scan-quick" data-act="scan-quick" disabled={scanning} onClick={() => scan("quick")}>
              <b>{t("s.quick")}</b>
              <small>{t("s.quick.s")}</small>
            </button>
            <button className="scanbtn" id="scan-deep" data-act="scan-deep" disabled={scanning} onClick={() => scan("deep")}>
              <b>{t("s.deep")}</b>
              <small>{t("s.deep.s2")}</small>
            </button>
          </div>
        </section>

        <Background settings={settings} onChange={onChange} />
      </div>
      <div className="st-foot">
        <button className="link small" id="rerun-setup" style={{ marginRight: "auto" }} onClick={() => void track(api.updateSettings({ onboardingComplete: false })).then((s) => { onSettings(s); onClose(); }, () => undefined)}>
          {t("s.rerun")}
        </button>
        <button className="btn primary" id="settings-done" onClick={onClose}>
          {t("s.done")}
        </button>
      </div>
    </Sheet>
  );
}
