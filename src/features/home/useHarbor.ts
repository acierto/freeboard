// Loads what the home view shows and keeps it current from backend events.
// The backend owns the pile (design D3); this only reads it and forwards the
// user's choices. The disk map, blind spots, places checked and growth come
// from the backend's accounting (design D4, D5).
import { useCallback, useEffect, useState } from "react";
import { api, errorText, on } from "../../lib/api";
import { track } from "../../lib/useBusy";
import type { AppStatus, AreaGrowth, BlindSpot, BlindSpotAction, DiskMap, Goal, Group, ItemDecision, PileState, PlaceChecked, Settings, SettingsPatch, Suggestion } from "../../lib/types";

export type Progress = { phase: string; detector: string | null; currentCount: number } | null;

export interface Harbor {
  status: AppStatus;
  settings: Settings;
  suggestions: Suggestion[];
  pile: PileState;
  map: DiskMap | null;
  spots: BlindSpot[];
  places: PlaceChecked[];
  growth: AreaGrowth[];
  /** The running scan's latest progress, or null. */
  progress: Progress;
}

export function useHarbor() {
  const [data, setData] = useState<Harbor | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [status, settings, suggestions, pile, map, spots, places, growth] = await track(
        Promise.all([api.getStatus(), api.getSettings(), api.getSuggestions(), api.getPile(), api.getDiskMap(), api.getBlindSpots(), api.getPlacesChecked(), api.getGrowth(7)]),
      );
      setData({ status, settings, suggestions, pile, map, spots, places, growth, progress: status.scan.running ? { phase: status.scan.phase ?? "", detector: null, currentCount: 0 } : null });
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
      on("blind_spots_changed", (spots) => setData((d) => d && { ...d, spots })),
      // Show "Scanning…" as soon as a scan starts.
      on("scan_progress", (p) =>
        setData((d) => d && { ...d, progress: p, status: { ...d.status, scan: { ...d.status.scan, running: p.scanId, phase: p.phase } } }),
      ),
      on("scan_finished", () => void load()),
      on("cleanup_finished", () => void load()),
    ];
    // Access granted in System Settings shows up when Freeboard is back in
    // front: the backend re-checks blind spots on every read (design D5).
    const recheck = () =>
      void track(api.getBlindSpots())
        .then((spots) => setData((d) => d && { ...d, spots }))
        .catch(() => undefined);
    window.addEventListener("focus", recheck);
    return () => {
      window.removeEventListener("focus", recheck);
      offs.forEach((p) => p.then((off) => off()));
    };
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
    setAreas: (a: Group[], none = false) => withPile(api.setPileAreas(a, none)),
    takeOut: (id: string) => withPile(api.takeOutOfPile([id])),
    add: (ids: string[]) => withPile(api.addToPile(ids)),
    // Decisions and parts answer with the item; the backend then emits
    // pile_changed, which reloads the pile and the suggestions.
    decide: (id: string, d: ItemDecision) => void track(api.setItemDecision(id, d)).catch((e) => setError(errorText(e))),
    parts: (id: string, partIds: string[]) => void track(api.setItemParts(id, partIds)).catch((e) => setError(errorText(e))),
    resolveSpot: (id: string, action: BlindSpotAction) =>
      void track(api.resolveBlindSpot(id, action))
        .then((r) => setData((d) => d && { ...d, map: r.map, spots: r.blindSpots }))
        // A resolved spot can add places (the folder's findings).
        .then(() => track(api.getPlacesChecked()))
        .then((places) => setData((d) => d && { ...d, places }))
        .catch((e) => setError(errorText(e))),
    setSettings: (settings: Settings) => setData((d) => d && { ...d, settings }),
    updateSettings: (patch: SettingsPatch) =>
      void track(api.updateSettings(patch))
        .then((settings) => setData((d) => d && { ...d, settings }))
        .then(() => track(api.getStatus()))
        .then((status) => setData((d) => d && status && { ...d, status }))
        .catch(() => undefined),
  };
}
