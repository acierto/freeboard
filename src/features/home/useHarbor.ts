// Loads what the home view shows and keeps it current from backend events.
// The backend owns the pile (design D3); this only reads it and forwards the
// user's choices. Commands that the Rust core gains in later milestones
// (pile: milestone 4, disk map: milestone 3) fall back to an empty pile and no
// map until they exist, instead of failing the whole view.
import { useCallback, useEffect, useState } from "react";
import { api, on } from "../../lib/api";
import { track } from "../../lib/useBusy";
import type { AppStatus, DiskMap, Goal, Group, PileState, Settings, SettingsPatch, Suggestion } from "../../lib/types";

export interface Harbor {
  status: AppStatus;
  settings: Settings;
  suggestions: Suggestion[];
  pile: PileState;
  map: DiskMap | null;
}

const emptyPile = (s: Settings): PileState => ({ goalBytes: s.goal, areas: s.whereFrom, mode: "auto", addedIds: [], takenOutIds: [], itemIds: [], totalBytes: 0 });

export function useHarbor() {
  const [data, setData] = useState<Harbor | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [status, settings, suggestions] = await track(Promise.all([api.getStatus(), api.getSettings(), api.getSuggestions().catch(() => [] as Suggestion[])]));
      const [pile, map] = await track(Promise.all([api.getPile().catch(() => emptyPile(settings)), api.getDiskMap().catch(() => null)]));
      setData({ status, settings, suggestions, pile, map });
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void load();
    const offs = [
      on("status_changed", () => void track(api.getStatus()).then((status) => setData((d) => d && { ...d, status }))),
      on("pile_changed", (pile) => setData((d) => d && { ...d, pile })),
      on("disk_map_changed", (map) => setData((d) => d && { ...d, map })),
      on("scan_finished", () => void load()),
      on("cleanup_finished", () => void load()),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  }, [load]);

  const withPile = useCallback((p: Promise<PileState>) => {
    void track(p)
      .then((pile) => setData((d) => d && { ...d, pile }))
      .catch(() => undefined);
  }, []);

  return {
    data,
    error,
    reload: load,
    setGoal: (g: Goal) => withPile(api.setPileGoal(g)),
    setAreas: (a: Group[]) => withPile(api.setPileAreas(a)),
    takeOut: (id: string) => withPile(api.takeOutOfPile([id])),
    updateSettings: (patch: SettingsPatch) =>
      void track(api.updateSettings(patch))
        .then((settings) => setData((d) => d && { ...d, settings }))
        .then(() => track(api.getStatus()))
        .then((status) => setData((d) => d && status && { ...d, status }))
        .catch(() => undefined),
  };
}
