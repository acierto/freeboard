// Settings behind the gear button (prototype `settingsSheet()`). Warning levels
// save as they change. The full Settings sheet (folders, schedules, startup,
// notifications) is task 6.4.
import { useState } from "react";
import { Sheet } from "../../components/Sheet";
import { GB } from "../../lib/format";
import type { Platform, Settings, SettingsPatch } from "../../lib/types";

/** Show home-relative paths as ~/..., like the prototype. */
function displayPath(p: string, platform: Platform): string {
  if (platform === "windows") return p;
  const m = /^(\/Users\/[^/]+|\/home\/[^/]+)(\/.*)?$/.exec(p);
  return m ? "~" + (m[2] ?? "") : p;
}

export function SettingsSheet({ settings, platform, onChange, onClose }: { settings: Settings; platform: Platform; onChange: (p: SettingsPatch) => void; onClose: () => void }) {
  const [low, setLow] = useState(Math.round(settings.lowThresholdBytes / GB));
  const [crit, setCrit] = useState(Math.round(settings.criticalThresholdBytes / GB));
  const folders = settings.scanFolders.map((f) => displayPath(f, platform)).join(", ");
  return (
    <Sheet title="Settings" onClose={onClose}>
      <div className="set stack">
        <div>
          <label htmlFor="low-gb">
            Warn me below <b className="num" id="low-out">{`${low} GB`}</b>
          </label>
          <input
            id="low-gb"
            type="range"
            min={15}
            max={60}
            step={5}
            value={low}
            onChange={(e) => {
              const v = +e.target.value;
              setLow(v);
              onChange({ lowThresholdBytes: v * GB });
            }}
          />
        </div>
        <div>
          <label htmlFor="crit-gb">
            Urgent warning below <b className="num" id="crit-out">{`${crit} GB`}</b>
          </label>
          <input
            id="crit-gb"
            type="range"
            min={3}
            max={10}
            step={1}
            value={crit}
            onChange={(e) => {
              const v = +e.target.value;
              setCrit(v);
              onChange({ criticalThresholdBytes: v * GB });
            }}
          />
        </div>
        <div>
          <b>Folders I scan</b>
          <p className="mono">{folders || "No folders chosen yet"}</p>
        </div>
        <div>
          <b>Never touched</b>
          <p className="muted">System folders, repository roots, .git and paths you exclude.</p>
        </div>
      </div>
      <div className="row">
        <button className="btn primary" onClick={onClose}>
          Done
        </button>
      </div>
    </Sheet>
  );
}
