// The cleanup flow: confirmation, progress, result, and Empty it. The
// backend builds and owns every plan; this only sends plan and item IDs
// (invariant 3), acknowledgments, and the user's confirm and cancel.
import { useCallback, useEffect, useRef, useState } from "react";
import { api, on } from "../../lib/api";
import { track } from "../../lib/useBusy";
import type { CleanupPlan, Platform, RunDetail, TrashSummary } from "../../lib/types";

type Err = { code: string; message: string };
const asErr = (e: unknown): Err =>
  e && typeof e === "object" && "message" in e ? { code: String((e as { code?: unknown }).code ?? "error"), message: String((e as { message: unknown }).message) } : { code: "error", message: String(e) };

export type Flow =
  | { kind: "confirm"; plan: CleanupPlan; permanent: boolean; acks: Record<string, boolean>; busy: boolean; error: Err | null; platform: Platform }
  | { kind: "run"; plan: CleanupPlan; runId: string; run: RunDetail | null; canceling: boolean; platform: Platform }
  | { kind: "result"; plan: CleanupPlan; run: RunDetail; platform: Platform }
  | { kind: "trash"; trash: TrashSummary; busy: boolean; error: string | null; platform: Platform }
  | { kind: "error"; message: string; platform: Platform };

export interface Cleanup {
  flow: Flow | null;
  fromPile(): void;
  fromItems(ids: string[]): void;
  permanent(ids: string[]): void;
  openTrash(): void;
  ack(itemId: string, on: boolean): void;
  /** Drop one item from the confirmation; the rest is planned again. */
  leaveOut(itemId: string): void;
  run(): void;
  replan(): void;
  cancel(): void;
  emptyTrash(): void;
  close(): void;
}

/** `onChanged` reloads the home view after a run or after emptying the Trash. */
export function useCleanup(platform: Platform, onChanged: () => void): Cleanup {
  const [flow, setFlow] = useState<Flow | null>(null);
  // The latest flow, updated as it is set, so replies that arrive before the
  // next render (a run's first record) see the state they belong to.
  const flowRef = useRef<Flow | null>(null);
  const set = useCallback((f: Flow | null) => {
    flowRef.current = f;
    setFlow(f);
  }, []);
  const update = useCallback((fn: (f: Flow | null) => Flow | null) => set(fn(flowRef.current)), [set]);
  // How the open plan was made, so an expired plan can be made again.
  const remake = useRef<(() => Promise<CleanupPlan>) | null>(null);
  // Whether it came from the pile (Toss) rather than chosen items.
  const fromPile = useRef(false);
  const changed = useRef(onChanged);
  changed.current = onChanged;

  const open = useCallback(
    (make: () => Promise<CleanupPlan>, permanent = false) => {
      remake.current = make;
      track(make()).then(
        (plan) => set({ kind: "confirm", plan, permanent, acks: Object.fromEntries(plan.acknowledgedIds.map((id) => [id, true])), busy: false, error: null, platform }),
        (e) => set({ kind: "error", message: asErr(e).message, platform }),
      );
    },
    [platform],
  );

  // Follow the run: each item update and the finish reload its record.
  const refresh = useCallback((runId: string) => {
    void track(api.getRun(runId)).then((run) => {
      const f = flowRef.current;
      if (!f || f.kind !== "run" || f.runId !== runId) return;
      if (run.state === "running") set({ ...f, run });
      else {
        set({ kind: "result", plan: f.plan, run, platform: f.platform });
        changed.current();
      }
    });
  }, []);

  useEffect(() => {
    const offs = [
      on("cleanup_item_updated", (u) => {
        const f = flowRef.current;
        if (f?.kind === "run" && f.runId === u.runId) refresh(u.runId);
      }),
      on("cleanup_finished", (u) => {
        const f = flowRef.current;
        if (f?.kind === "run" && f.runId === u.runId) refresh(u.runId);
      }),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  }, [refresh]);

  const c: Cleanup = {
    flow,
    fromPile: () => {
      fromPile.current = true;
      open(() => api.createPlanFromPile());
    },
    fromItems: (ids) => {
      fromPile.current = false;
      open(() => api.createPlan(ids));
    },
    leaveOut: (itemId) => {
      const f = flowRef.current;
      if (f?.kind !== "confirm") return;
      const rest = f.plan.items.map((s) => s.id).filter((id) => id !== itemId);
      if (!rest.length) return set(null);
      if (f.permanent) return open(() => api.createPermanentPlan(rest), true);
      // From the pile: take it out of the pile too, so it does not come back.
      if (fromPile.current) void track(api.takeOutOfPile([itemId])).then(() => open(() => api.createPlanFromPile()), (e) => set({ kind: "error", message: asErr(e).message, platform }));
      else open(() => api.createPlan(rest));
    },
    permanent: (ids) => {
      fromPile.current = false;
      open(() => api.createPermanentPlan(ids), true);
    },
    openTrash: () =>
      void track(api.getTrash()).then(
        (trash) => set({ kind: "trash", trash, busy: false, error: trash.confirmationToken ? null : "There is nothing of mine in the Trash to empty.", platform }),
        (e) => set({ kind: "error", message: asErr(e).message, platform }),
      ),
    ack: (itemId, checked) => {
      const f = flowRef.current;
      if (f?.kind !== "confirm") return;
      set({ ...f, acks: { ...f.acks, [itemId]: checked } });
      // The backend records each acknowledgment against this plan; unticking
      // only disables the confirm button here.
      if (checked && !f.plan.acknowledgedIds.includes(itemId))
        void track(api.acknowledgeItem(f.plan.id, itemId)).then(
          (plan) => update((g) => (g?.kind === "confirm" && g.plan.id === plan.id ? { ...g, plan } : g)),
          (e) => update((g) => (g?.kind === "confirm" ? { ...g, error: asErr(e) } : g)),
        );
    },
    run: () => {
      const f = flowRef.current;
      if (f?.kind !== "confirm") return;
      set({ ...f, busy: true, error: null });
      track(api.runPlan(f.plan.id)).then(
        (runId) => {
          set({ kind: "run", plan: f.plan, runId, run: null, canceling: false, platform: f.platform });
          refresh(runId);
        },
        (e) => update((g) => (g?.kind === "confirm" ? { ...g, busy: false, error: asErr(e) } : g)),
      );
    },
    replan: () => {
      const make = remake.current;
      const f = flowRef.current;
      if (make) open(make, f?.kind === "confirm" && f.permanent);
    },
    cancel: () => {
      const f = flowRef.current;
      if (f?.kind !== "run") return;
      set({ ...f, canceling: true });
      void track(api.cancelRun(f.runId));
    },
    emptyTrash: () => {
      const f = flowRef.current;
      if (f?.kind !== "trash" || !f.trash.confirmationToken) return;
      set({ ...f, busy: true });
      track(api.emptyFreeboardTrash(f.trash.confirmationToken)).then(
        () => {
          set(null);
          changed.current();
        },
        (e) => update((g) => (g?.kind === "trash" ? { ...g, busy: false, error: asErr(e).message } : g)),
      );
    },
    close: () => {
      if (flowRef.current?.kind === "run") return; // Esc and the scrim do nothing while a cleanup runs.
      set(null);
    },
  };
  return c;
}
