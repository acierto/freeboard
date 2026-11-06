// First-run setup (spec "Default settings": the first run explains what
// Freeboard reads, asks for scan folders, and asks for no permission). Not in
// the prototype, so it uses the prototype's cards, lists and buttons.
import { api } from "../../lib/api";
import { track } from "../../lib/useBusy";
import type { Platform, Settings } from "../../lib/types";
import { FolderList, useFolderOffers } from "../settings/Folders";

export function FirstRun({ settings, platform, onSettings, onDone }: { settings: Settings; platform: Platform; onSettings: (s: Settings) => void; onDone: () => void }) {
  const offers = useFolderOffers(settings);
  const finish = (scan: boolean) =>
    void track(api.updateSettings({ onboardingComplete: true }))
      .then((s) => {
        onSettings(s);
        if (scan) void track(api.startScan("deep")).catch(() => undefined);
        onDone();
      })
      .catch(() => undefined);
  return (
    <div className="page">
      <header className="top">
        <div className="brand">Freeboard</div>
      </header>
      <section className="card stack" id="first-run">
        <h2>Welcome aboard</h2>
        <p>I warn you before your disk fills up, and show you what you can remove safely. Nothing is removed until you confirm it.</p>
        <div className="two">
          <div className="stack" style={{ gap: 6 }}>
            <b>What I read</b>
            <p className="muted">Free space on your disk, every 5 minutes. Reports from Docker, Git, pnpm, pip and other developer tools. Sizes and dates of files in the folders you choose.</p>
          </div>
          <div className="stack" style={{ gap: 6 }}>
            <b>What I never do</b>
            <p className="muted">Remove anything without a plan you confirm. Send your data anywhere: everything stays on this computer. Ask for system permissions before a feature needs them.</p>
          </div>
        </div>
        <FolderList list="scanFolders" title="Folders to scan for projects and worktrees" empty="No folders chosen yet." settings={settings} platform={platform} onSettings={onSettings} offers={offers} />
        <div className="row">
          <button className="btn primary" data-act="start" disabled={!settings.scanFolders.length} onClick={() => finish(true)}>
            Start
          </button>
          <button className="btn" data-act="skip" onClick={() => finish(false)}>
            Skip for now
          </button>
        </div>
      </section>
    </div>
  );
}
