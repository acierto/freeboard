// Scan and excluded folders. The backend opens the folder picker and keeps
// the paths; the interface sends only which list and an index (invariant 3).
// Shared by Settings and first-run setup.
import { useEffect, useState } from "react";
import { api } from "../../lib/api";
import { track } from "../../lib/useBusy";
import type { FolderList, FolderSuggestion, Platform, Settings } from "../../lib/types";

/** Show home-relative paths as ~/..., like the prototype. */
export function displayPath(p: string, platform: Platform): string {
  if (platform === "windows") return p;
  const m = /^(\/Users\/[^/]+|\/home\/[^/]+)(\/.*)?$/.exec(p);
  return m ? "~" + (m[2] ?? "") : p;
}

export const offerLabel = (o: FolderSuggestion) => `Add ${o.label} (${o.repos} ${o.repos === 1 ? "repository" : "repositories"})`;

/** Home folders that hold Git repositories and are not chosen yet. */
export function useFolderOffers(settings: Settings) {
  const [offers, setOffers] = useState<FolderSuggestion[]>([]);
  const n = settings.scanFolders.length;
  useEffect(() => {
    let live = true;
    api.folderSuggestions().then((o) => live && setOffers(o), () => live && setOffers([]));
    return () => {
      live = false;
    };
  }, [n]);
  return offers;
}

export function FolderList({
  list,
  title,
  empty,
  settings,
  platform,
  onSettings,
  offers = [],
}: {
  list: FolderList;
  title: string;
  empty: string;
  settings: Settings;
  platform: Platform;
  onSettings: (s: Settings) => void;
  offers?: FolderSuggestion[];
}) {
  const folders = list === "scanFolders" ? settings.scanFolders : settings.exclusions;
  const run = (p: Promise<Settings>) => void track(p).then(onSettings, () => undefined);
  return (
    <div className="stack" style={{ gap: 6 }}>
      <b>{title}</b>
      {folders.length === 0 && <p className="muted">{empty}</p>}
      {folders.map((f, i) => (
        <div className="row between" key={f}>
          <span className="mono">{displayPath(f, platform)}</span>
          <button className="mini" data-remove={`${list}-${i}`} aria-label={`Remove ${displayPath(f, platform)}`} onClick={() => run(api.removeFolder(list, i))}>
            Remove
          </button>
        </div>
      ))}
      <div className="row">
        {offers.map((o, i) => (
          <button key={o.label} className="mini go" data-offer={i} onClick={() => run(api.addSuggestedFolder(i))}>
            {offerLabel(o)}
          </button>
        ))}
        <button className="mini" data-add={list} onClick={() => run(api.addFolder(list))}>
          Add folder
        </button>
      </div>
    </div>
  );
}
