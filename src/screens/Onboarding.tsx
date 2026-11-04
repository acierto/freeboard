import type { AppModel } from "../App";
import { FolderList } from "./Settings";

// First run (supplemental design built from the prototype's existing
// components): explains what Freeboard reads, asks for scan folders, and
// requests operating system permissions only when a feature needs them.
export function Onboarding({ model, onDone }: { model: AppModel; onDone(complete: boolean): void }) {
  const hasFolders = model.settings.scanFolders.length > 0;
  return (
    <section className="view" id="v-onboarding">
      <div>
        <h2>Welcome to Freeboard</h2>
        <p className="muted">Freeboard warns you before your disk fills up and shows what you can remove safely.</p>
      </div>
      <div className="settings">
        <div className="set">
          <span style={{ fontWeight: 500 }}>What Freeboard reads</span>
          <ul>
            <li>Free space on your disks, every 5 minutes</li>
            <li>Reports from Docker, Git, pnpm, pip and other developer tools</li>
            <li>Sizes and dates of files in the folders you choose</li>
          </ul>
        </div>
        <div className="set">
          <span style={{ fontWeight: 500 }}>What it never does</span>
          <ul>
            <li>Remove anything without a plan you confirm</li>
            <li>Send your data anywhere. Everything stays on this computer.</li>
            <li>Ask for system permissions before a feature needs them</li>
          </ul>
        </div>
        <div className="set">
          <span style={{ fontWeight: 500 }}>Folders to scan</span>
          <FolderList model={model} list="scanFolders" empty="No folders chosen yet" />
        </div>
      </div>
      <div className="actions">
        <button className="btn primary" data-action="onboarding-done" disabled={!hasFolders} onClick={() => onDone(true)}>
          Start monitoring
        </button>
        <button className="btn" data-action="onboarding-skip" onClick={() => onDone(false)}>
          Skip for now
        </button>
      </div>
    </section>
  );
}
