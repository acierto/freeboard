// Settings behind the gear button (prototype `settingsSheet()`), plus the
// folder controls the prototype only describes: scan and excluded folders,
// and scanning on request. Warning levels save as they change. Schedules,
// startup and notifications are task 6.4.
import { useState } from "react";
import { Sheet } from "../../components/Sheet";
import { api } from "../../lib/api";
import { GB } from "../../lib/format";
import { track } from "../../lib/useBusy";
import type { Platform, Settings, SettingsPatch } from "../../lib/types";
import { FolderList, useFolderOffers } from "./Folders";

export function SettingsSheet({
  settings,
  platform,
  scanning,
  onChange,
  onSettings,
  onClose,
}: {
  settings: Settings;
  platform: Platform;
  scanning: boolean;
  onChange: (p: SettingsPatch) => void;
  onSettings: (s: Settings) => void;
  onClose: () => void;
}) {
  const [low, setLow] = useState(Math.round(settings.lowThresholdBytes / GB));
  const [crit, setCrit] = useState(Math.round(settings.criticalThresholdBytes / GB));
  const offers = useFolderOffers(settings);
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
        <FolderList list="scanFolders" title="Folders I scan" empty="No folders chosen yet. I only look for projects and worktrees inside the folders you choose." settings={settings} platform={platform} onSettings={onSettings} offers={offers} />
        <FolderList list="exclusions" title="Folders I never look in" empty="Nothing excluded." settings={settings} platform={platform} onSettings={onSettings} />
        <div>
          <b>Never touched</b>
          <p className="muted">System folders, repository roots, .git and paths you exclude.</p>
        </div>
        <div className="row">
          <button className="btn" data-act="scan-quick" disabled={scanning} onClick={() => void track(api.startScan("quick")).catch(() => undefined)}>
            Scan now
          </button>
          <button className="btn" data-act="scan-deep" disabled={scanning} onClick={() => void track(api.startScan("deep")).catch(() => undefined)}>
            Run deep scan now
          </button>
          {scanning && <span className="note">Scanning…</span>}
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
