import { useEffect, useState } from "react";
import type { AppModel } from "../App";
import { api } from "../lib/api";
import type { FolderSuggestion } from "../lib/types";
import { track } from "../lib/useBusy";

// Folders in the home folder that hold Git repositories, offered while no
// project folder is chosen (supplemental, built from the prototype's .actions
// and .btn). Worktrees, dependencies and build output are found only inside
// chosen folders; skipping that step used to hide them silently (found by the
// user, 2026-11-05). The interface sends back only the offer's index.
export function useFolderOffers(model: AppModel) {
  const [offers, setOffers] = useState<FolderSuggestion[]>([]);
  const chosen = model.settings.scanFolders.length;
  useEffect(() => {
    let live = true;
    api
      .folderSuggestions()
      .then((o) => live && setOffers(o))
      .catch(() => live && setOffers([]));
    return () => {
      live = false;
    };
  }, [chosen]);
  return offers;
}

export function offerLabel(o: FolderSuggestion) {
  return "Add " + o.label + " (" + o.repos + (o.repos === 1 ? " repository)" : " repositories)");
}

/** Banner for Suggestions: shown whenever no project folder is chosen. */
export function NoFoldersNote({ model }: { model: AppModel }) {
  const offers = useFolderOffers(model);
  if (model.settings.scanFolders.length) return null;
  const add = (p: Promise<unknown>) =>
    void track(p.then((s) => model.setSettings(s as never))).then(() => model.scan("deep"));
  return (
    <div className="notes" id="no-folders-note">
      <p className="group-note">
        No project folders are chosen, so Git worktrees, dependencies and build output are not checked.
      </p>
      <div className="actions">
        {offers.map((o, i) => (
          <button key={o.label} className={"btn" + (i === 0 ? " primary" : "")} data-action={"offer-" + i} onClick={() => add(api.addSuggestedFolder(i))}>
            {offerLabel(o)}
          </button>
        ))}
        <button className={"btn" + (offers.length ? "" : " primary")} data-action="choose-folder" onClick={() => add(api.addFolder("scanFolders"))}>
          Choose a folder
        </button>
      </div>
    </div>
  );
}
