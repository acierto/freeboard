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
import { track } from "../../lib/useBusy";
import type { FolderDetails, FolderInfo, FolderList, Platform, QuickScanSchedule, ScanKind, Settings, SettingsPatch } from "../../lib/types";
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
const folderSize = (b: number) => (b >= GB ? `${Math.round(b / GB)} GB` : gb(b));

/** "14 repositories, 96 GB"; just the size for a folder without repositories. */
export function folderMeta(f: FolderInfo | undefined): string {
  if (!f) return "";
  if (f.repos === 0) return f.bytes == null ? "Size known after a deep scan" : folderSize(f.bytes);
  const repos = `${f.repos} ${f.repos === 1 ? "repository" : "repositories"}`;
  return `${repos}, ${f.bytes == null ? "size after a deep scan" : folderSize(f.bytes)}`;
}

export function ago(iso: string | null, now = Date.now()): string {
  if (!iso) return "never";
  const min = Math.floor((now - Date.parse(iso)) / 60000);
  if (min < 1) return "just now";
  if (min < 60) return `${plural(min, "minute")} ago`;
  const h = Math.floor(min / 60);
  if (h < 24) return `${plural(h, "hour")} ago`;
  return `${plural(Math.floor(h / 24), "day")} ago`;
}

const QUICK_EVERY: Record<QuickScanSchedule, string> = {
  hourly: "Quick scans run every hour.",
  every6_hours: "Quick scans run every 6 hours.",
  daily: "Quick scans run once a day.",
  on_request: "Quick scans run when you ask.",
};

function Stepper({ id, label, value, lo, hi, d, onStep }: { id: string; label: string; value: number; lo: number; hi: number; d: number; onStep: (v: number) => void }) {
  return (
    <span className="stepper" role="group" aria-label={label}>
      <button id={`${id}-dn`} aria-label="Lower" disabled={value - d < lo} onClick={() => onStep(value - d)}>
        −
      </button>
      <output className="num" id={`${id}-out`} aria-live="polite">{`${value} GB`}</output>
      <button id={`${id}-up`} aria-label="Higher" disabled={value + d > hi} onClick={() => onStep(value + d)}>
        +
      </button>
    </span>
  );
}

export function SettingsSheet({
  settings,
  platform,
  freeBytes,
  scanning,
  onChange,
  onSettings,
  onClose,
}: {
  settings: Settings;
  platform: Platform;
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
          {list === "scanFolders" ? "No longer scanning " : "No longer skipping "}
          <span className="mono">{removed.label}</span>
        </span>
        <button className="mini" id="undo-rm" onClick={() => run(api.undoRemoveFolder())}>
          Undo
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
        aria-label={`${list === "scanFolders" ? "Stop scanning" : "Stop skipping"} ${displayPath(p, platform)}`}
        title="Remove"
        onClick={() => run(api.removeFolder(list, i))}
      >
        ×
      </button>
    </div>
  );
  const status = scanning ? (kind === "quick" ? "Quick scan running…" : kind === "deep" ? "Deep scan running…" : "A scan is running…") : `Last scan ${ago(details?.lastScanAt ?? null)}. ${QUICK_EVERY[settings.quickScan]}`;
  const suggestions = details?.suggestions ?? [];

  return (
    <Sheet title="Settings" onClose={onClose} className="settings" closeLabel="Close settings">
      <div className="st-body">
        <section className="st-sec" aria-labelledby="st-warn">
          <h3 id="st-warn">When Dinghy warns you</h3>
          <p>Dinghy gets worried in the amber zone and panics in the red one.</p>
          <div className="zone-wrap">
            <span className="zone-you num" style={{ left: Math.max(6, Math.min(94, pct(free))).toFixed(1) + "%" }}>{`You: ${gb(freeBytes)}`}</span>
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
            <span style={{ left: "100%" }}>60+ GB</span>
          </div>
          <div className="limit">
            <span className="sw" style={{ background: "var(--review)" }} />
            <span>
              <b>Warn me</b>
              <small>Low, Dinghy looks worried</small>
            </span>
            <Stepper
              id="low"
              label="Warning level"
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
              <b>Warn me urgently</b>
              <small>Critical, Dinghy panics</small>
            </span>
            <Stepper
              id="crit"
              label="Urgent level"
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
          <h3 id="st-scan">Folders I scan</h3>
          <p>Freeboard looks for projects, worktrees and caches here.</p>
          <div className="st-list">
            {settings.scanFolders.length ? settings.scanFolders.map((p, i) => row("scanFolders", p, i, folderMeta(details?.scan[i]))) : <div className="empty">No folders yet. Add one, or Freeboard only checks tool caches.</div>}
          </div>
          {undo("scanFolders")}
          {suggestions.length > 0 && (
            <>
              <div className="st-sub">Found outside these folders</div>
              <div className="st-list">
                {suggestions.map((o, i) => (
                  <div className="frow sugg" key={o.label}>
                    {FOLDER_SVG}
                    <span>
                      <span className="path">{o.label}</span>
                      <small>{folderMeta({ repos: o.repos, bytes: details?.suggestionBytes[i] ?? null })}</small>
                    </span>
                    <button className="mini go" id={`sugg-add-${i}`} data-offer={i} aria-label={`Add ${o.label}`} onClick={() => run(api.addSuggestedFolder(i))}>
                      Add
                    </button>
                  </div>
                ))}
              </div>
            </>
          )}
          <div>
            <button className="btn" id="add-scan" data-add="scanFolders" onClick={() => run(api.addFolder("scanFolders"))}>
              Add a folder
            </button>
          </div>
        </section>

        <section className="st-sec" aria-labelledby="st-skip">
          <h3 id="st-skip">Folders I skip</h3>
          <p>Freeboard never looks inside these.</p>
          <div className="st-list">{settings.exclusions.length ? settings.exclusions.map((p, i) => row("exclusions", p, i, "Never looked inside")) : <div className="empty">Nothing skipped yet.</div>}</div>
          {undo("exclusions")}
          <div>
            <button className="btn" id="add-skip" data-add="exclusions" onClick={() => run(api.addFolder("exclusions"))}>
              Skip a folder
            </button>
          </div>
        </section>

        <section className="st-sec" aria-labelledby="st-lock">
          <h3 id="st-lock">Always protected</h3>
          <p>Freeboard never removes these, whatever you choose.</p>
          <div className="lockchips">
            {["System folders", "Repository roots", ".git folders", "Folders you skip"].map((t) => (
              <span key={t}>
                {LOCK_SVG}
                {t}
              </span>
            ))}
          </div>
        </section>

        <section className="st-sec" aria-labelledby="st-run">
          <h3 id="st-run">Scanning</h3>
          <p id="scan-status" aria-live="polite">
            {status}
          </p>
          <div className="scanrow">
            <button className="scanbtn" id="scan-quick" data-act="scan-quick" disabled={scanning} onClick={() => scan("quick")}>
              <b>Quick scan</b>
              <small>Known caches and tools, about 10 seconds</small>
            </button>
            <button className="scanbtn" id="scan-deep" data-act="scan-deep" disabled={scanning} onClick={() => scan("deep")}>
              <b>Deep scan</b>
              <small>Every file in your folders and the disk map, a few minutes</small>
            </button>
          </div>
        </section>
      </div>
      <div className="st-foot">
        <button className="link small" id="rerun-setup" style={{ marginRight: "auto" }} onClick={() => void track(api.updateSettings({ onboardingComplete: false })).then((s) => { onSettings(s); onClose(); }, () => undefined)}>
          Run setup again
        </button>
        <button className="btn primary" id="settings-done" onClick={onClose}>
          Done
        </button>
      </div>
    </Sheet>
  );
}
