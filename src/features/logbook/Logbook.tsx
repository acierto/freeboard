// The Logbook (prototype `renderLog()`): one row per cleanup, newest first,
// with the measured space freed, the Trash amount and skipped items. Run
// details (task 4.5) open from a row once cleanup lands in this interface.
import { useEffect, useState } from "react";
import { api, errorText } from "../../lib/api";
import { gb, plural, trashWord } from "../../lib/format";
import { track } from "../../lib/useBusy";
import type { LogbookPage, Platform, RunSummary } from "../../lib/types";

const DAY = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric" });

function when(iso: string, now = Date.now()): string {
  const d = new Date(iso);
  if (new Date(now).toDateString() === d.toDateString()) return "Today";
  return DAY.format(d);
}

function line(r: RunSummary): string {
  const done = r.items - r.skipped;
  if (r.state === "canceled") return `Stopped after ${plural(done, "crate")}`;
  if (r.state === "interrupted") return `Interrupted after ${plural(done, "crate")}`;
  return `Tossed ${plural(done, "crate")}`;
}

function note(r: RunSummary, platform: Platform): string {
  const parts: string[] = [];
  if (r.trashBytes > 0) parts.push(`${gb(r.trashBytes)} in the ${trashWord(platform)}.`);
  parts.push(r.skipped ? `${plural(r.skipped, "item")} skipped because ${r.skipped === 1 ? "it" : "they"} changed.` : "Nothing skipped.");
  return parts.join(" ");
}

export function Logbook({ platform, refresh }: { platform: Platform; refresh: number }) {
  const [page, setPage] = useState<LogbookPage | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    track(api.listLogbook(0)).then(setPage, (e) => setError(errorText(e)));
  }, [refresh]);
  return (
    <>
      <h2 style={{ marginBottom: 8 }}>Logbook</h2>
      {error && <p className="muted">{`I couldn't read the Logbook: ${error}`}</p>}
      {page && page.runs.length === 0 && <p className="muted">No cleanups yet. Each cleanup you confirm is recorded here.</p>}
      {page?.runs.map((r) => (
        <div className="log" key={r.id}>
          <span className="when muted">{when(r.startedAt)}</span>
          <span>
            {line(r)}
            <br />
            <span className="note">{note(r, platform)}</span>
          </span>
          <span className="num">{r.measuredBytesFreed === null ? "measuring" : gb(Math.max(0, r.measuredBytesFreed))}</span>
        </div>
      ))}
    </>
  );
}
