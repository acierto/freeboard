// The Logbook (prototype `renderLog()`): one row per cleanup, newest first,
// with the measured space freed, the Trash amount and skipped items. A row
// opens its run details: each item with its action and result (spec
// "Required behavior missing from the prototype").
import { lang, t } from "../../i18n";
import { itemName, tb } from "../../i18n/backend";
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { Sheet } from "../../components/Sheet";
import { api, errorText } from "../../lib/api";
import { gb, trashWord } from "../../lib/format";
import { track } from "../../lib/useBusy";
import type { LogbookPage, Platform, RunDetail, RunSummary } from "../../lib/types";
import { expectedFreed, logNote, resultWord } from "../cleanup/cleanupModel";

// Dates in the language in use ("Nov 7", "7. Nov.", "7 nov.").
const DAY = () => new Intl.DateTimeFormat(lang() === "en" ? "en-US" : lang(), { month: "short", day: "numeric" });
const DAY_TIME = () => new Intl.DateTimeFormat(lang() === "en" ? "en-US" : lang(), { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", hourCycle: "h23" });

function when(iso: string, now = Date.now()): string {
  const d = new Date(iso);
  if (new Date(now).toDateString() === d.toDateString()) return t("lg.today");
  return DAY().format(d);
}

function line(r: { removed: number; state: string }): string {
  const n = r.removed;
  if (r.state === "canceled") return n === 1 ? t("lg.stopped1") : t("lg.stoppedN", { n });
  if (r.state === "interrupted") return n === 1 ? t("lg.int1") : t("lg.intN", { n });
  if (r.state === "running") return t("cs.t.run");
  return n === 1 ? t("lg.tossed1") : t("lg.tossedN", { n });
}

/** Crates removed in a run, counting a split item once. */
function removedOf(run: RunDetail): number {
  return new Set(run.items.filter((i) => i.state === "done" || i.state === "moved_to_trash").map((i) => i.parentId ?? i.itemId)).size;
}

function RunDetails({ run, platform, onClose }: { run: RunDetail; platform: Platform; onClose: () => void }) {
  const freed = run.measuredBytesFreed === null ? null : Math.max(0, run.measuredBytesFreed);
  const expected = expectedFreed(run);
  const differs = freed !== null && expected > 0 && Math.abs(freed - expected) >= 0.05e9;
  return (
    <Sheet title={line({ removed: removedOf(run), state: run.state })} onClose={onClose}>
      <p className="muted">{DAY_TIME().format(new Date(run.startedAt))}</p>
      <div className="sum3">
        <div>
          <b className="num">{freed === null ? "…" : gb(freed)}</b>
          {t("lg.freed")}
        </div>
        <div>
          <b className="num">{gb(run.trashBytes)}</b>
          {t("lg.held", { t: trashWord(platform) })}
        </div>
        <div>
          <b className="num">{run.skipped}</b>
          {t("lg.skipped")}
        </div>
      </div>
      {differs && <p className="note">{t("lg.expected", { x: gb(expected) })}</p>}
      {run.involvesDocker && <p className="note">{t("lg.docker")}</p>}
      {/* Focusable so the list scrolls by keyboard when it is long. */}
      <div className="plan-list" tabIndex={0} role="region" aria-label={t("lg.items")}>
        {run.items.map((i) => (
          <div className="pile-row" style={{ gridTemplateColumns: "minmax(0,1fr) auto" }} key={i.itemId}>
            <span>
              {i.parentId ? tb(i.title) : itemName({ id: i.itemId, title: i.title })}
              <span className={i.action === "native_command" ? "sub mono" : "sub"}>{tb(i.actionLabel)}</span>
              <span className="sub">{resultWord(i, platform, trashWord)}</span>
            </span>
            <span className="num">{gb(i.expectedBytes)}</span>
          </div>
        ))}
      </div>
      <div className="row">
        <button className="btn primary" id="go" data-act="close" onClick={onClose}>
          {t("cs.done")}
        </button>
      </div>
    </Sheet>
  );
}

export function Logbook({ platform, refresh }: { platform: Platform; refresh: string }) {
  const [page, setPage] = useState<LogbookPage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState<RunDetail | null>(null);
  useEffect(() => {
    track(api.listLogbook(0)).then(setPage, (e) => setError(errorText(e)));
  }, [refresh]);
  const show = (r: RunSummary) => void track(api.getRun(r.id)).then(setOpen, (e) => setError(errorText(e)));
  return (
    <>
      <h2 style={{ marginBottom: 8 }}>{t("lg.title")}</h2>
      {error && <p className="muted">{t("lg.error", { e: tb(error) })}</p>}
      {page && page.runs.length === 0 && <p className="muted">{t("lg.none")}</p>}
      {page?.runs.map((r) => (
        <div
          className="log"
          key={r.id}
          role="button"
          tabIndex={0}
          data-run={r.id}
          onClick={() => show(r)}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              show(r);
            }
          }}
        >
          <span className="when muted">{when(r.startedAt)}</span>
          <span>
            {line(r)}
            {/* A block, not <br>: the prototype renders without a doctype,
                where a small-font second line is shorter than in standards
                mode; as a block it has that height in both. */}
            <span className="note" style={{ display: "block" }}>
              {logNote(r, platform, trashWord)}
            </span>
          </span>
          <span className="num">{r.measuredBytesFreed === null ? t("lg.measuring") : gb(Math.max(0, r.measuredBytesFreed))}</span>
        </div>
      ))}
      {/* Like the other sheets, outside the page (the prototype's #sheet-root). */}
      {open && createPortal(<RunDetails run={open} platform={platform} onClose={() => setOpen(null)} />, document.body)}
    </>
  );
}
