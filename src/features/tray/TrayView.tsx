// The menu bar window (prototype `trayHTML()`, spec "Menu bar window"): am I
// OK, and what would one click get me? It runs in its own small window opened
// from the tray icon, and closes with its close button, Esc, or a click
// outside (the window losing focus, handled by the shell).
import { useEffect } from "react";
import { STATUS, gb } from "../../lib/format";
import { ago, areaName, fx, itemName, t, tn, unitGB } from "../../i18n";
import { api } from "../../lib/api";
import { track } from "../../lib/useBusy";
import type { DiskStatus } from "../../lib/types";
import { useHarbor } from "../home/useHarbor";
import { pileItems, questionDeck } from "../home/homeModel";

const MAX = 60;
const x = (v: number) => Math.min(100, Math.max(0, (v / MAX) * 100));

/** "Scanned 18 min ago" from the last finished scan. */
export function scannedAgo(iso: string | null, now = Date.now()): string {
  if (!iso) return t("ob.m.new");
  return t("t.scanned", { t: ago(Math.floor((now - Date.parse(iso)) / 60000)) });
}

const LINE: Record<DiskStatus, (pile: string) => string> = {
  critical: () => t("t.crit"),
  low: (pile) => t("t.warn", { x: pile }),
  healthy: () => t("t.good"),
};

export function TrayView() {
  const { data, reload } = useHarbor();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") void api.closeTrayView();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);
  if (!data) return null;
  const { status, pile, settings, suggestions, growth } = data;
  const freeBytes = status.volumes[0]?.freeBytes ?? 0;
  const f = freeBytes / 1e9;
  const st = STATUS[status.status];
  const items = pileItems(pile, suggestions);
  const total = pile.totalBytes / 1e9;
  const low = settings.lowThresholdBytes / 1e9;
  const crit = settings.criticalThresholdBytes / 1e9;
  const now = x(f);
  const after = x(f + total);
  const deck = questionDeck(pile, suggestions);
  const deckBytes = deck.reduce((t, s) => t + s.sizeBytes, 0);
  const g = growth.find((a) => a.bytes > 0);
  const calm = st.key === "good";
  const scanning = !!status.scan.running;
  const days = status.forecast ? Math.max(1, Math.round(status.forecast.daysLeft)) : null;
  const scan = () => void track(api.startScan("quick")).then(() => setTimeout(reload, 400), () => undefined);
  return (
    <div className="tray-window">
      <div className="popover" id="popover">
        <div className="tb-head">
          <svg className="tb-boat" viewBox="0 0 40 30" aria-hidden="true">
            <path d="M20 3v15" stroke="var(--ink)" strokeWidth="2" />
            <path d="M21 4l10 12H21Z" fill="var(--accent)" />
            <path d="M3 18h34l-5 9H8Z" fill="var(--hull)" stroke="var(--ink)" strokeWidth="1.5" strokeLinejoin="round" />
          </svg>
          <b>Freeboard</b>
          <button className="tb-icon" data-act="tray-settings" aria-label={t("t.settings")} onClick={() => void api.openMainView("settings")}>
            {"⚙"}
          </button>
          <button className="tb-icon" id="tray-close" data-act="close-tray" aria-label={t("t.close")} onClick={() => void api.closeTrayView()}>
            {"×"}
          </button>
        </div>
        <section className={`tb-status ${st.key}`}>
          <div className="tb-free">
            <span className="num">
              <b>{fx(f)}</b> {t("gb.free")}
            </span>
            <span className={"pill " + st.key}>{st.label}</span>
          </div>
          <div className="tg" role="img" aria-label={t("t.aria", { x: gb(freeBytes), l: Math.round(low), c: Math.round(crit) })}>
            <div className="tg-track">
              <i className="z crit" style={{ width: x(crit) + "%" }}></i>
              <i className="z warn" style={{ left: x(crit) + "%", width: x(low) - x(crit) + "%" }}></i>
              <i className="z good" style={{ left: x(low) + "%", right: 0 }}></i>
              {items.length > 0 && after > now && <i className="ghost" style={{ left: now + "%", width: after - now + "%" }}></i>}
              <span className={`tg-mark ${st.key}`} style={{ left: now + "%" }}></span>
            </div>
            <div className="tg-scale num">
              <span style={{ left: 0 }}>0</span>
              <span style={{ left: x(crit) + "%" }}>{Math.round(crit)}</span>
              <span style={{ left: x(low) + "%" }}>{Math.round(low)}</span>
              <span style={{ right: 0 }}>{`${MAX}+ ${unitGB()}`}</span>
            </div>
          </div>
          <p className="tb-line">
            {LINE[status.status](gb(pile.totalBytes)) + " "}
            {days !== null && <span className="muted num">{t("t.full", { n: days })}</span>}
          </p>
        </section>
        {items.length ? (
          <section className="tb-pile">
            <div className="tb-sub">
              <span>{t("t.ready")}</span>
              <span className="num">{tn("h.crate1", "h.craten", items.length)}</span>
            </div>
            <ul className="tl">
              {items.slice(0, 3).map((s) => (
                <li key={s.id}>
                  <span className="tl-n">{itemName(s)}</span>
                  <span className="num">{gb(s.sizeBytes)}</span>
                </li>
              ))}
              {items.length > 3 && (
                <li className="tl-more">
                  <span>{t("t.more", { n: items.length - 3 })}</span>
                  <span className="num">{gb(items.slice(3).reduce((t, s) => t + s.sizeBytes, 0))}</span>
                </li>
              )}
            </ul>
            <button className={`btn ${calm ? "calm" : "primary"} tb-toss`} data-act="confirm" onClick={() => void api.reviewPile()}>
              {t("h.toss", { x: gb(pile.totalBytes) })}
            </button>
            {calm && <p className="note tb-c">{t("t.optional")}</p>}
          </section>
        ) : (
          <section className="tb-pile">
            <p className="muted">{t("t.nosafe")}</p>
          </section>
        )}
        <section className="tb-links">
          {deck.length > 0 && (
            <button className="tb-link" data-act="tray-questions" onClick={() => void api.openMainView("questions")}>
              <span>{tn("t.q1", "t.qn", deck.length)}</span>
              <span className="num muted">{`${gb(deckBytes)} ›`}</span>
            </button>
          )}
          {g && (
            <div className="tb-link static">
              <span>{t("t.grew", { a: areaName(g.label) })}</span>
              <span className="num muted">{t("t.week", { x: gb(g.bytes) })}</span>
            </div>
          )}
        </section>
        <div className="tb-foot">
          <span className="note">{scanning ? t("t.scanning") : scannedAgo(status.scan.lastFinishedAt)}</span>
          <button className="link small" data-act="tray-scan" disabled={scanning} onClick={scan}>
            {t("t.scannow")}
          </button>
          <button className="btn tb-open" data-act="tray-open" onClick={() => void api.openMainView("home")}>
            {t("t.open")}
          </button>
        </div>
      </div>
    </div>
  );
}
