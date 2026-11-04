import type { AppModel } from "../App";
import { api } from "../lib/api";
import { GB, displayPath } from "../lib/format";
import { track } from "../lib/useBusy";
import type { FolderList, QuickScanSchedule, DeepScanSchedule } from "../lib/types";

const QUICK: [QuickScanSchedule, string][] = [
  ["hourly", "Every hour"],
  ["every6_hours", "Every 6 hours"],
  ["daily", "Every day"],
  ["on_request", "Only when I ask"],
];
const DEEP: [DeepScanSchedule, string][] = [
  ["on_request", "Only when I ask"],
  ["daily", "Every day"],
];

export function FolderList({ model, list, empty }: { model: AppModel; list: FolderList; empty: string }) {
  const items = model.settings[list];
  const p = model.status.platform;
  return (
    <>
      <ul className="mono">
        {items.length ? (
          items.map((f, i) => (
            <li className="folder-row" key={f}>
              <span>{displayPath(f, p)}</span>
              <button
                className="chip small"
                aria-label={"Remove " + displayPath(f, p)}
                onClick={() => void track(api.removeFolder(list, i)).then(model.setSettings)}
              >
                Remove
              </button>
            </li>
          ))
        ) : (
          <li className="muted">{empty}</li>
        )}
      </ul>
      <div className="actions">
        <button className="btn" data-action={"add-" + list} onClick={() => void track(api.addFolder(list)).then(model.setSettings)}>
          Add folder
        </button>
      </div>
    </>
  );
}

export function SettingsScreen({ model }: { model: AppModel }) {
  const s = model.settings;
  const warn = Math.round(s.lowThresholdBytes / GB);
  const crit = Math.round(s.criticalThresholdBytes / GB);
  return (
    <section className="view" id="v-settings">
      <h2>Settings</h2>
      <div className="settings">
        <div className="set">
          <label htmlFor="warn-gb">Low space warning</label>
          <input
            id="warn-gb"
            type="range"
            min="15"
            max="60"
            step="5"
            value={warn}
            onChange={(e) => model.updateSettings({ lowThresholdBytes: +e.target.value * GB })}
          />
          <span className="muted num" id="warn-out">
            {"Warn below " + warn + " GB free"}
          </span>
        </div>
        <div className="set">
          <label htmlFor="crit-gb">Critical warning</label>
          <input
            id="crit-gb"
            type="range"
            min="3"
            max="10"
            step="1"
            value={crit}
            onChange={(e) => model.updateSettings({ criticalThresholdBytes: +e.target.value * GB })}
          />
          <span className="muted num" id="crit-out">
            {"Warn urgently below " + crit + " GB free"}
          </span>
        </div>
        <div className="set">
          <label htmlFor="scan-freq">Quick scan</label>
          <select id="scan-freq" value={s.quickScan} onChange={(e) => model.updateSettings({ quickScan: e.target.value as QuickScanSchedule })}>
            {QUICK.map(([v, l]) => (
              <option key={v} value={v}>
                {l}
              </option>
            ))}
          </select>
        </div>
        <div className="set">
          <span style={{ fontWeight: 500 }}>Background behavior</span>
          <label className="toggle" style={{ fontWeight: 400 }}>
            <input type="checkbox" id="set-notify" checked={s.notifications} onChange={(e) => model.updateSettings({ notifications: e.target.checked })} />
            Send notifications
          </label>
          <label className="toggle" style={{ fontWeight: 400 }}>
            <input
              type="checkbox"
              id="set-battery"
              checked={s.pauseDeepScansOnBattery}
              onChange={(e) => model.updateSettings({ pauseDeepScansOnBattery: e.target.checked })}
            />
            Pause deep scans on battery
          </label>
          <label className="toggle" style={{ fontWeight: 400 }}>
            <input type="checkbox" id="set-login" checked={s.startAtLogin} onChange={(e) => model.updateSettings({ startAtLogin: e.target.checked })} />
            Start at login
          </label>
        </div>
        <div className="set">
          <span style={{ fontWeight: 500 }}>Folders to scan</span>
          <FolderList model={model} list="scanFolders" empty="No folders chosen yet" />
        </div>
        <div className="set">
          <span style={{ fontWeight: 500 }}>Never touched</span>
          <ul className="mono muted">
            <li>System and application folders</li>
            <li>Repository roots and .git</li>
            <li>Paths you exclude</li>
          </ul>
        </div>
        <div className="set">
          <label htmlFor="deep-freq">Deep scan</label>
          <select id="deep-freq" value={s.deepScan} onChange={(e) => model.updateSettings({ deepScan: e.target.value as DeepScanSchedule })}>
            {DEEP.map(([v, l]) => (
              <option key={v} value={v}>
                {l}
              </option>
            ))}
          </select>
          <div className="actions">
            <button className="btn" data-action="deep-scan" disabled={model.scanning} onClick={() => model.scan("deep")}>
              Run deep scan now
            </button>
          </div>
        </div>
        <div className="set">
          <span style={{ fontWeight: 500 }}>Excluded folders</span>
          <FolderList model={model} list="exclusions" empty="Nothing excluded" />
        </div>
      </div>
    </section>
  );
}
