// The menu bar window (prototype `trayHTML()`, spec "Menu bar window" with
// revision v21): am I OK, and what would one click get me? It runs in its
// own small window opened from the tray icon, and closes with its close
// button, Esc, or a click outside (the window losing focus, handled by the
// shell). Header and footer stay put; only the middle scrolls, and the
// window fits the card's height (at most 640 px).
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { GB, STATUS, gb } from "../../lib/format";
import { ago, areaName, fx, t, tn, unitGB } from "../../i18n";
import { itemName } from "../../i18n/backend";
import { api, on } from "../../lib/api";
import { track } from "../../lib/useBusy";
import type { DiskStatus, Pace, UpdateStatus } from "../../lib/types";
import { useHarbor } from "../home/useHarbor";
import { pileItems, questionDeck } from "../home/homeModel";

/** Under this much free space the zoomed 0–60+ GB gauge; at or above it, the whole disk. */
const MAX = 60;
const x = (v: number) => Math.min(100, Math.max(0, (v / MAX) * 100));
/** Filling within this many days while Healthy: "filling fast". */
const FAST_DAYS = 14;

/** "Scanned 18 min ago" from the last finished scan. */
export function scannedAgo(iso: string | null, now = Date.now()): string {
  if (!iso) return t("ob.m.new");
  return t("t.scanned", { t: ago(Math.floor((now - Date.parse(iso)) / 60000)) });
}

/** "5 days", "3 weeks", "2 months": days under 14, weeks under 60, months above. */
export function durText(d: number): string {
  if (d < 14) return t(d === 1 ? "t.d1" : "t.dn", { n: d });
  if (d < 60) return t("t.wn", { n: Math.round(d / 7) });
  return t("t.mn", { n: Math.round(d / 30) });
}

/** The pace line from the measured rate (spec v21): rate = this week's
 *  growth ÷ 7, days = free ÷ rate. `days` is 0 when not filling up. */
export function paceOf(pace: Pace | null, freeBytes: number): { text: string; days: number } | null {
  if (!pace) return null;
  const rate = pace.bytesPerDay;
  const days = rate > 0.05 * GB ? Math.max(1, Math.round(freeBytes / rate)) : 0;
  if (!days || days > 365) return { text: t("t.pace0"), days: 0 };
  return { text: t("t.pace", { r: "+" + gb(rate), d: durText(days) }), days };
}

const LINE: Record<DiskStatus, (pile: string, fast: boolean) => string> = {
  critical: () => t("t.crit"),
  low: (pile) => t("t.warn", { x: pile }),
  healthy: (_, fast) => (fast ? t("t.goodfast") : t("t.good")),
};

/** The version chip and its banner (docs/decisions.md "Updates only when asked": the network is used only
 *  when the chip, Update or Restart to update is clicked). */
function useUpdate() {
  const [u, setU] = useState<UpdateStatus | null>(null);
  const [open, setOpen] = useState(false);
  const prev = useRef<UpdateStatus["stage"] | null>(null);
  const apply = (s: UpdateStatus) => {
    // A check that finds an update opens the banner.
    if (s.stage === "available" && prev.current === "checking") setOpen(true);
    prev.current = s.stage;
    setU(s);
  };
  useEffect(() => {
    void api.getUpdate().then(apply, () => undefined);
    const off = on("update_changed", apply);
    return () => void off.then((f) => f());
  }, []);
  const run = (p: Promise<UpdateStatus>) => void p.then(apply, () => undefined);
  return { u, open, setOpen, run };
}

function VersionChip({ up }: { up: ReturnType<typeof useUpdate> }) {
  const { u, open, setOpen, run } = up;
  if (!u) return null;
  const v = "v" + u.current;
  switch (u.stage) {
    case "checking":
      return (
        <span className="tb-ver" role="status">
          <span className="spin" aria-hidden="true"></span>
          {t("t.checking")}
        </span>
      );
    case "downloading":
      return (
        <span className="tb-ver" role="status">
          <span className="spin" aria-hidden="true"></span>
          {t("t.downloading", { v: u.version ?? "" })}
        </span>
      );
    case "available":
      return (
        <button className="tb-ver new" data-act="upd-open" aria-expanded={open} onClick={() => setOpen(!open)}>
          {t("t.avail", { v: u.version ?? "" })}
        </button>
      );
    case "ready":
      return (
        <button className="tb-ver new" data-act="upd-restart" onClick={() => run(api.restartToUpdate())}>
          {t("t.restart")}
        </button>
      );
    case "installFailed":
      return (
        <button className="tb-ver" data-act="upd-open" aria-expanded={open} onClick={() => setOpen(!open)}>
          {`${v} · ${t("t.updfail")}`}
        </button>
      );
    default: {
      const tail = u.stage === "latest" ? ` · ${t("t.uptodate")} ✓` : u.stage === "checkFailed" ? ` · ${t("t.checkfail")}` : "";
      return (
        <button className="tb-ver" data-act="upd-check" title={t("t.check")} aria-label={tail ? undefined : `${v}. ${t("t.check")}`} onClick={() => run(api.checkForUpdate())}>
          {v + tail}
        </button>
      );
    }
  }
}

function UpdateBanner({ up }: { up: ReturnType<typeof useUpdate> }) {
  const { u, open, setOpen, run } = up;
  if (!u || !open || !u.version || !(u.stage === "available" || u.stage === "installFailed")) return null;
  const note = t("t.youhave", { v: u.current }) + (u.notes ? " " + t("t.new", { x: u.notes }) : "");
  return (
    <section className="tb-upd">
      <div className="tb-upd-t">
        <b>{t("t.updh", { v: u.version })}</b>
        <span className="note">{note}</span>
      </div>
      <button
        className="btn small primary"
        data-act="upd-go"
        onClick={() => {
          setOpen(false);
          run(api.downloadUpdate());
        }}
      >
        {t("t.update")}
      </button>
      <button className="icon-x small" data-act="upd-later" aria-label={t("t.later")} onClick={() => setOpen(false)}>
        {"×"}
      </button>
    </section>
  );
}

export function TrayView() {
  const { data, reload } = useHarbor();
  const up = useUpdate();
  const card = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") void api.closeTrayView();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);
  // The window follows the card's height (the card caps it at 640 px).
  useLayoutEffect(() => {
    const el = card.current;
    if (!el) return;
    const fit = () => void api.fitTrayView(el.getBoundingClientRect().height).catch(() => undefined);
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => ro.disconnect();
  }, [!!data]);
  if (!data) return null;
  const { status, pile, settings, suggestions, growth } = data;
  const vol = status.volumes[0];
  const freeBytes = vol?.freeBytes ?? 0;
  const totalBytes = vol?.totalBytes ?? 0;
  const f = freeBytes / GB;
  const st = STATUS[status.status];
  const items = pileItems(pile, suggestions);
  const total = pile.totalBytes / GB;
  const low = settings.lowThresholdBytes / GB;
  const crit = settings.criticalThresholdBytes / GB;
  const deck = questionDeck(pile, suggestions);
  const deckBytes = deck.reduce((t, s) => t + s.sizeBytes, 0);
  const pace = paceOf(status.pace, freeBytes);
  const fast = !!pace && pace.days > 0 && pace.days < FAST_DAYS;
  const calm = st.key === "good" && !fast;
  const week = status.pace && status.pace.weekBytes > 0 ? status.pace.weekBytes : 0;
  const top = growth.filter((g) => g.bytes > 0).slice(0, 4);
  const scanning = !!status.scan.running;
  const scan = () => void track(api.startScan("quick")).then(() => setTimeout(reload, 400), () => undefined);

  let viz;
  if (f >= MAX && totalBytes > 0) {
    // The whole disk: used in grey, the pile striped, free in green.
    const usedPct = ((totalBytes - freeBytes) / totalBytes) * 100;
    const tossPct = (Math.min(pile.totalBytes, totalBytes - freeBytes) / totalBytes) * 100;
    viz = (
      <div className="tcap" role="img" aria-label={t("t.caparia", { x: gb(freeBytes), t: Math.round(totalBytes / GB) })}>
        <div className="tcap-bar">
          <i className="used" style={{ width: (usedPct - tossPct).toFixed(1) + "%" }}></i>
          {items.length > 0 && <i className="toss" style={{ width: tossPct.toFixed(1) + "%" }}></i>}
        </div>
        <div className="tcap-lbl num">
          <span>{t("t.used", { x: gb(totalBytes - freeBytes) })}</span>
          <span>{t("t.of", { n: Math.round(totalBytes / GB) })}</span>
        </div>
      </div>
    );
  } else {
    const now = x(f);
    const after = x(f + total);
    viz = (
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
    );
  }

  return (
    <div className="tray-window">
      <div className="popover" id="popover" ref={card}>
        <div className="tb-head">
          <svg className="tb-boat" viewBox="0 0 40 30" aria-hidden="true">
            <path d="M20 3v15" stroke="var(--ink)" strokeWidth="2" />
            <path d="M21 4l10 12H21Z" fill="var(--accent)" />
            <path d="M3 18h34l-5 9H8Z" fill="var(--hull)" stroke="var(--ink)" strokeWidth="1.5" strokeLinejoin="round" />
          </svg>
          <b>Freeboard</b>
          <VersionChip up={up} />
          <button className="tb-icon" data-act="tray-settings" aria-label={t("t.settings")} onClick={() => void api.openMainView("settings")}>
            {"⚙"}
          </button>
          <button className="tb-icon" id="tray-close" data-act="close-tray" aria-label={t("t.close")} onClick={() => void api.closeTrayView()}>
            {"×"}
          </button>
        </div>
        <UpdateBanner up={up} />
        <div className="tb-scroll">
          <section className={`tb-status ${st.key}${fast && st.key === "good" ? " fast" : ""}`}>
            <div className="tb-free">
              <span className="num">
                <b>{fx(f)}</b> {t("gb.free")}
              </span>
              <span className={"pill " + st.key}>{st.label}</span>
            </div>
            {viz}
            <p className="tb-line">{LINE[status.status](gb(pile.totalBytes), fast)}</p>
            {pace && <p className="tb-pace num">{pace.text}</p>}
          </section>
          {week > 0 && (
            <button className="tb-grow" data-act="tray-map" onClick={() => void api.openMainView("map")}>
              <span className="tb-gl">
                <b>
                  {t("t.thisweek")} <span className="num up">{"+" + gb(week)}</span>
                </b>
                {top.length > 0 && <span className="tb-gtxt">{t("t.mostly", { a: areaName(top[0].label), x: gb(top[0].bytes) })}</span>}
              </span>
              {top.length > 0 && (
                <span className="tb-gbar" aria-hidden="true">
                  {top.map((g, i) => (
                    <i key={g.id} style={{ flex: g.bytes, opacity: 1 - i * 0.2 }}></i>
                  ))}
                </span>
              )}
            </button>
          )}
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
          {deck.length > 0 && (
            <section className="tb-links">
              <button className="tb-link" data-act="tray-questions" onClick={() => void api.openMainView("questions")}>
                <span>{tn("t.q1", "t.qn", deck.length)}</span>
                <span className="num muted">{`${gb(deckBytes)} ›`}</span>
              </button>
            </section>
          )}
        </div>
        <div className="tb-foot">
          <div className="tb-fl">
            <span className="note">{scanning ? t("t.scanning") : scannedAgo(status.scan.lastFinishedAt)}</span>
            <button className="link small" data-act="tray-scan" disabled={scanning} onClick={scan}>
              {t("t.scannow")}
            </button>
          </div>
          <button className="btn tb-open" data-act="tray-open" onClick={() => void api.openMainView("home")}>
            {t("t.open")}
          </button>
        </div>
      </div>
    </div>
  );
}
