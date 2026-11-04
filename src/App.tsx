import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, on } from "./lib/api";
import { preselected, selectable } from "./lib/format";
import { track } from "./lib/useBusy";
import type { AppStatus, CleanupPlan, HistoryPage, RunDetail, Settings, Suggestion, TrashSummary, View } from "./lib/types";
import { Overview } from "./screens/Overview";
import { Suggestions, type Filter } from "./screens/Suggestions";
import { PlanScreen } from "./screens/Plan";
import { History } from "./screens/History";
import { SettingsScreen } from "./screens/Settings";
import { Onboarding } from "./screens/Onboarding";

export interface AppModel {
  status: AppStatus;
  settings: Settings;
  suggestions: Suggestion[];
  sel: Set<string>;
  selected: Suggestion[];
  trash: TrashSummary;
  trashAsk: boolean;
  scanNote: string | null;
  scanning: boolean;
  show(v: View): void;
  goSuggestions(): void;
  goPlan(): void;
  toggle(id: string, on: boolean): void;
  setTrashAsk(v: boolean): void;
  emptyTrash(): void;
  scan(kind?: "quick" | "deep"): void;
  updateSettings(patch: Parameters<typeof api.updateSettings>[0]): void;
  setSettings(s: Settings): void;
}

export function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [view, setView] = useState<View>("overview");
  const [filter, setFilter] = useState<Filter>("All");
  const [plan, setPlan] = useState<CleanupPlan | null>(null);
  const [planError, setPlanError] = useState<string | null>(null);
  const [run, setRun] = useState<RunDetail | null>(null);
  const [trash, setTrash] = useState<TrashSummary>({ bytes: 0, items: 0, confirmationToken: null });
  const [trashAsk, setTrashAsk] = useState(false);
  const [history, setHistory] = useState<HistoryPage>({ runs: [], page: 0, hasMore: false });
  const [scanNote, setScanNote] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [skipOnboarding, setSkipOnboarding] = useState(false);
  const runIdRef = useRef<string | null>(null);

  const refreshStatus = useCallback(() => track(api.getStatus()).then(setStatus), []);
  const refreshTrash = useCallback(() => track(api.getTrash()).then(setTrash), []);
  const refreshHistory = useCallback((page = 0) => track(api.listHistory(page)).then(setHistory), []);
  const refreshSuggestions = useCallback(
    (resetSelection: boolean) =>
      track(api.getSuggestions()).then((list) => {
        setSuggestions(list);
        setSel((old) => {
          if (resetSelection) return new Set(list.filter(preselected).map((s) => s.id));
          const ids = new Set(list.filter(selectable).map((s) => s.id));
          return new Set([...old].filter((id) => ids.has(id)));
        });
      }),
    [],
  );

  const handled = useRef(new Set<string>());
  const refreshRun = useCallback(async () => {
    const id = runIdRef.current;
    if (!id) return;
    const r = await track(api.getRun(id));
    if (runIdRef.current !== id) return;
    setRun(r);
    // A run can finish before its ID reaches the interface, so completion is
    // detected from the run state rather than only from cleanup_finished.
    if (r.state !== "running" && !handled.current.has(id)) {
      handled.current.add(id);
      setSel(new Set());
      await Promise.all([
        track(api.getStatus()).then(setStatus),
        track(api.getTrash()).then(setTrash),
        track(api.listHistory(0)).then(setHistory),
        track(api.getSuggestions()).then(setSuggestions),
      ]);
    }
  }, []);

  useEffect(() => {
    void track(
      Promise.all([
        refreshStatus(),
        track(api.getSettings()).then(setSettings),
        refreshSuggestions(true),
        refreshTrash(),
        refreshHistory(0),
      ]),
    );
    const subs = [
      on("status_changed", () => void refreshStatus()),
      on("scan_progress", (p) => {
        setScanning(true);
        setScanNote(p.detector ? p.detector : p.phase);
      }),
      on("scan_finished", () => {
        setScanning(false);
        setScanNote(null);
        setPlan(null);
        void refreshStatus();
        void refreshSuggestions(true);
      }),
      on("cleanup_item_updated", (e) => {
        if (e.runId === runIdRef.current) void refreshRun();
      }),
      on("cleanup_finished", (e) => {
        if (e.runId === runIdRef.current) void refreshRun();
      }),
      on("navigate", (e) => setView(e.view)),
    ];
    return () => subs.forEach((s) => void s.then((off) => off()));
  }, [refreshStatus, refreshSuggestions, refreshTrash, refreshHistory, refreshRun]);

  const selected = useMemo(() => suggestions.filter((s) => sel.has(s.id) && selectable(s)), [suggestions, sel]);

  const makePlan = useCallback(
    async (ids: string[], acks: string[] = []) => {
      setPlanError(null);
      if (!ids.length) {
        setPlan(null);
        return;
      }
      try {
        let p = await track(api.createPlan(ids));
        for (const id of acks) p = await track(api.acknowledgeItem(p.id, id));
        setPlan(p);
      } catch (e) {
        setPlan(null);
        setPlanError(String(e));
      }
    },
    [],
  );

  const show = useCallback(
    (v: View) => {
      setView(v);
      if (v === "plan" && !run) void makePlan(selected.map((s) => s.id));
      if (v === "history") void refreshHistory(0);
    },
    [run, selected, makePlan, refreshHistory],
  );

  if (!status || !settings) return null;

  const finished = run && run.state !== "running";
  const goSuggestions = () => {
    if (finished) {
      setRun(null);
      runIdRef.current = null;
    }
    setView("suggestions");
  };

  const model: AppModel = {
    status,
    settings,
    suggestions,
    sel,
    selected,
    trash,
    trashAsk,
    scanNote,
    scanning: scanning || !!status.scan.running,
    show,
    goSuggestions,
    goPlan: () => {
      setRun(null);
      runIdRef.current = null;
      setView("plan");
      void makePlan(selected.map((s) => s.id));
    },
    toggle: (id, checked) => {
      if (finished) {
        setRun(null);
        runIdRef.current = null;
      }
      setSel((old) => {
        const n = new Set(old);
        if (checked) n.add(id);
        else n.delete(id);
        return n;
      });
    },
    setTrashAsk,
    emptyTrash: async () => {
      const t = await track(api.getTrash());
      if (t.confirmationToken) await track(api.emptyFreeboardTrash(t.confirmationToken));
      setTrashAsk(false);
      await Promise.all([refreshTrash(), refreshStatus()]);
    },
    scan: async (kind = "quick") => {
      setScanning(true);
      setScanNote("scan");
      try {
        await track(api.startScan(kind));
      } catch {
        setScanning(false);
        setScanNote(null);
      }
    },
    updateSettings: async (patch) => {
      const s = await track(api.updateSettings(patch));
      setSettings(s);
      await refreshStatus();
    },
    setSettings,
  };

  const needsOnboarding = status.firstRun && !settings.onboardingComplete && !skipOnboarding;
  const count = selected.length;

  return (
    <div className="page">
      <div className="app">
        <nav className="nav" aria-label="Freeboard" id="nav">
          {(
            [
              ["overview", "Overview"],
              ["suggestions", "Suggestions"],
              ["plan", "Cleanup plan "],
              ["history", "History"],
              ["settings", "Settings"],
            ] as [View, string][]
          ).map(([v, label]) => (
            <button
              key={v}
              data-view={v}
              aria-current={!needsOnboarding && view === v ? "page" : undefined}
              onClick={() => {
                setSkipOnboarding(true);
                show(v);
              }}
            >
              {label}
              {v === "plan" && (
                <span id="nav-count" className="num">
                  {count ? count : ""}
                </span>
              )}
            </button>
          ))}
        </nav>
        <main>
          {needsOnboarding ? (
            <Onboarding
              model={model}
              onDone={async (complete) => {
                if (complete) await model.updateSettings({ onboardingComplete: true });
                setSkipOnboarding(true);
                if (complete) model.scan("deep");
              }}
            />
          ) : view === "overview" ? (
            <Overview model={model} />
          ) : view === "suggestions" ? (
            <Suggestions model={model} filter={filter} setFilter={setFilter} />
          ) : view === "plan" ? (
            <PlanScreen
              model={model}
              plan={plan}
              planError={planError}
              run={run}
              onAck={(id, checked) => {
                if (!plan) return;
                if (checked) void track(api.acknowledgeItem(plan.id, id)).then(setPlan);
                else void makePlan(plan.itemIds, plan.acknowledgedIds.filter((a) => a !== id));
              }}
              onRefresh={() => void makePlan(selected.map((s) => s.id))}
              onConfirm={async () => {
                if (!plan) return;
                try {
                  const id = await track(api.runPlan(plan.id));
                  runIdRef.current = id;
                  await refreshRun();
                } catch (e) {
                  setPlanError(String(e));
                }
              }}
              onCancel={() => {
                if (runIdRef.current) void track(api.cancelRun(runIdRef.current)).then(() => refreshRun());
              }}
              onOverview={() => setView("overview")}
              onPlanMore={() => {
                setRun(null);
                runIdRef.current = null;
                setPlan(null);
                setView("suggestions");
              }}
            />
          ) : view === "history" ? (
            <History page={history} platform={status.platform} onPage={(p) => void refreshHistory(p)} />
          ) : (
            <SettingsScreen model={model} />
          )}
        </main>
      </div>
    </div>
  );
}
