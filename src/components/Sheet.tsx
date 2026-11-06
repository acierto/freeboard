// A modal sheet (prototype `.scrim` + `.sheet`). It traps focus, closes with
// Esc or a click on the scrim unless `locked` (a cleanup is running), and
// returns focus to the control that opened it (spec "Accessibility").
import { useEffect, useRef, type ReactNode } from "react";

const FOCUSABLE = 'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea, a[href], [tabindex]:not([tabindex="-1"])';

export function Sheet({ title, onClose, locked = false, children }: { title: string; onClose: () => void; locked?: boolean; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  const opener = useRef<Element | null>(null);
  const close = useRef(onClose);
  close.current = onClose;
  const isLocked = useRef(locked);
  isLocked.current = locked;

  useEffect(() => {
    opener.current = document.activeElement;
    ref.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      const sheet = ref.current;
      if (!sheet) return;
      if (e.key === "Escape" && !isLocked.current) {
        e.preventDefault();
        close.current();
        return;
      }
      if (e.key !== "Tab") return;
      const items = [...sheet.querySelectorAll<HTMLElement>(FOCUSABLE)];
      if (!items.length) {
        e.preventDefault();
        sheet.focus();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      if (e.shiftKey && (active === first || active === sheet)) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      } else if (!sheet.contains(active)) {
        e.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      const o = opener.current;
      if (o instanceof HTMLElement && o.isConnected) o.focus();
    };
  }, []);

  return (
    <div
      className="scrim"
      id="scrim"
      onClick={(e) => {
        if (e.target === e.currentTarget && !isLocked.current) onClose();
      }}
    >
      <div className="sheet" role="dialog" aria-modal="true" aria-labelledby="sheet-title" tabIndex={-1} id="sheet" ref={ref}>
        <h2 id="sheet-title">{title}</h2>
        {children}
      </div>
    </div>
  );
}
