// Loads what the home view shows and keeps it current from backend events.
// The backend owns the pile (design D3); this only reads it and forwards the
// user's choices. The disk map (and its coverage badge) arrives with slice 3.
import { useCallback, useEffect, useState } from "react";
import { api, errorText, on } from "../../lib/api";
import { track } from "../../lib/useBusy";
import type { AppStatus, DiskMap, Goal, Group, ItemDecision, PileState, Settings, SettingsPatch, Suggestion } from "../../lib/types";

export type Progress = { phase: string; detector: string | null; currentCount: number } | null;

export interface Harbor {
  status: AppStatus;
  settings: Settings;
  suggestions: Suggestion[];
  pile: PileState;
  map: DiskMap | null;
  /** The running scan's latest progress, or null. */
  progress: Progress;
}

export function useHarbor() {
  const [data, setData] = useState<Harbor | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [status, settings, suggestions, pile] = await track(Promise.all([api.getStatus(), api.getSettings(), api.getSuggestions(), api.getPile()]));
      setData({ status, settings, suggestions, pile, map: null, progress: status.scan.running ? { phase: status.scan.phase ?? "", detector: null, currentCount: 0 } : null });
      setError(null);
    } catch (e) {
      setError(errorText(e));
    }
  }, []);

  useEffect(() => {
    void load();
    const offs = [
      on("status_changed", () => void track(api.getStatus()).then((status) => setData((d) => d && { ...d, status }))),
      // A changed pile can mean changed decisions or parts: reload suggestions too.
      on("pile_changed", (pile) => {
        setData((d) => d && { ...d, pile });
        void track(api.getSuggestions()).then((suggestions) => setData((d) => d && { ...d, suggestions }));
      }),
      on("disk_map_changed", (map) => setData((d) => d && { ...d, map })),
      // Show "Scanning…" as soon as a scan starts.
      on("scan_progress", (p) =>
        setData((d) => d && { ...d, progress: p, status: { ...d.status, scan: { ...d.status.scan, running: p.scanId, phase: p.phase } } }),
      ),
      on("scan_finished", () => void load()),
      on("cleanup_finished", () => void load()),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  }, [load]);

  const withPile = useCallback((p: Promise<PileState>) => {
    void track(p)
      .then((pile) => setData((d) => d && { ...d, pile }))
      .catch((e) => setError(errorText(e)));
  }, []);

  return {
    data,
    error,
    reload: load,
    setGoal: (g: Goal) => withPile(api.setPileGoal(g)),
    setAreas: (a: Group[]) => withPile(api.setPileAreas(a)),
    takeOut: (id: string) => withPile(api.takeOutOfPile([id])),
    add: (ids: string[]) => withPile(api.addToPile(ids)),
    // Decisions and parts answer with the item; the backend then emits
    // pile_changed, which reloads the pile and the suggestions.
    decide: (id: string, d: ItemDecision) => void track(api.setItemDecision(id, d)).catch((e) => setError(errorText(e))),
    parts: (id: string, partIds: string[]) => void track(api.setItemParts(id, partIds)).catch((e) => setError(errorText(e))),
    setSettings: (settings: Settings) => setData((d) => d && { ...d, settings }),
    updateSettings: (patch: SettingsPatch) =>
      void track(api.updateSettings(patch))
        .then((settings) => setData((d) => d && { ...d, settings }))
        .then(() => track(api.getStatus()))
        .then((status) => setData((d) => d && status && { ...d, status }))
        .catch(() => undefined),
  };
}
