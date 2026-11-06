// The tray view (prototype `#popover`): status, free space, the pile total and
// a button to review the pile. It runs in its own small window opened from the
// tray icon, and closes with its close button, Esc, or a click outside (the
// window losing focus, handled by the shell).
import { useEffect } from "react";
import { STATUS, gb, trayWord } from "../../lib/format";
import { api } from "../../lib/api";
import { useHarbor } from "../home/useHarbor";

export function TrayView() {
  const { data } = useHarbor();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") void api.closeTrayView();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);
  if (!data) return null;
  const { status, pile } = data;
  const free = status.volumes[0]?.freeBytes ?? 0;
  const st = STATUS[status.status];
  const word = trayWord(status.platform);
  return (
    <div className="tray-window">
      <div className="popover" id="popover">
        <div className="row between">
          <b>{`Your ${word}`}</b>
          <button className="x" id="tray-close" data-act="close-tray" aria-label={`Close ${word}`} onClick={() => void api.closeTrayView()}>
            {"×"}
          </button>
        </div>
        <p>Most days, this is all you need to open.</p>
        <div className="row between">
          <span className="num">{`${gb(free)} free`}</span>
          <span className={"pill " + st.key}>{st.label}</span>
        </div>
        <button className="btn primary" data-act="confirm" disabled={!pile.itemIds.length} onClick={() => void api.reviewPile()}>
          {`Toss ${gb(pile.totalBytes)}`}
        </button>
      </div>
    </div>
  );
}
