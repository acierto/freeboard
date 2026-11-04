import { useState } from "react";
import { api } from "../lib/api";
import { gb, runClass, runLabel, shortDate, trashWord } from "../lib/format";
import { track } from "../lib/useBusy";
import type { HistoryPage, Platform, RunDetail } from "../lib/types";

const dateLabel = (iso: string) => {
  const d = shortDate(iso);
  return d === shortDate(new Date(Date.now()).toISOString()) ? "Today" : d;
};

export function History({ page, platform, onPage }: { page: HistoryPage; platform: Platform; onPage(p: number): void }) {
  const [detail, setDetail] = useState<RunDetail | null>(null);
  const w = trashWord(platform);
  const open = (id: string) => void track(api.getRun(id)).then(setDetail);

  if (detail) {
    const measured = detail.measuredBytesFreed ?? 0;
    return (
      <section className="view" id="v-history">
        <div>
          <h2>{"Cleanup on " + shortDate(detail.startedAt)}</h2>
          <p className="muted num">
            {"Freed " + gb(measured) + ", measured on the disk. " + gb(detail.trashBytes) + " sent to the " + w + ". " +
              detail.skipped + " skipped, " + detail.failed + " failed."}
          </p>
        </div>
        <div>
          {detail.items.map((r) => (
            <div className="plan-row" key={r.itemId}>
              <div>
                <b>{r.title}</b>
                <div className="sub mono">{r.actionLabel}</div>
                {r.reason && <div className="sub">{r.reason}</div>}
              </div>
              <span className={runClass(r.state)}>{runLabel(r.state, platform)}</span>
            </div>
          ))}
        </div>
        <div className="actions">
          <button className="btn" data-action="history-back" onClick={() => setDetail(null)}>
            Back to history
          </button>
        </div>
      </section>
    );
  }

  return (
    <section className="view" id="v-history">
      <h2>History</h2>
      <p className="muted">Every cleanup keeps a local record of what ran and what changed.</p>
      <div className="table-wrap" id="history">
        {page.runs.length ? (
          <table>
            <thead>
              <tr>
                <th scope="col">Date</th>
                <th className="n" scope="col">Items</th>
                <th className="n" scope="col">Freed</th>
                <th className="n" scope="col">{"Sent to " + w}</th>
                <th className="n" scope="col">Skipped</th>
              </tr>
            </thead>
            <tbody>
              {page.runs.map((h) => (
                <tr
                  key={h.id}
                  className="link"
                  tabIndex={0}
                  data-run={h.id}
                  aria-label={"Open details for the cleanup on " + shortDate(h.startedAt)}
                  onClick={() => open(h.id)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      open(h.id);
                    }
                  }}
                >
                  <td>{dateLabel(h.startedAt)}</td>
                  <td className="n">{h.items}</td>
                  <td className="n">{gb(h.measuredBytesFreed ?? 0)}</td>
                  <td className="n">{gb(h.trashBytes)}</td>
                  <td className="n">{h.skipped}</td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="muted">No cleanups yet. Each cleanup you confirm is recorded here.</p>
        )}
      </div>
      {(page.page > 0 || page.hasMore) && (
        <div className="actions">
          <button className="btn" disabled={page.page === 0} onClick={() => onPage(page.page - 1)}>
            Newer
          </button>
          <button className="btn" disabled={!page.hasMore} onClick={() => onPage(page.page + 1)}>
            Older
          </button>
        </div>
      )}
    </section>
  );
}
