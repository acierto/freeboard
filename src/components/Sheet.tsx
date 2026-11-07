// A modal sheet (prototype `.scrim` + `.sheet`). It traps focus, closes with
// Esc or a click on the scrim unless `locked` (a cleanup is running), and
// returns focus to the control that opened it (spec "Accessibility").
import { useEffect, useRef, type ReactNode } from "react";

const FOCUSABLE = 'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea, a[href], [tabindex]:not([tabindex="-1"])';

/** `stage`: when a sheet moves on (confirmation, progress, result), focus
 * returns to the sheet itself, as the prototype's `openSheet()` does.
 * `fallbackFocus`: where focus goes on close when the opener is gone or
 * disabled (a removed row, a Toss button with nothing left to toss). */
export function Sheet({
  title,
  onClose,
  locked = false,
  stage,
  fallbackFocus,
  className,
  closeLabel,
  headRow = false,
  children,
}: {
  title: string;
  onClose: () => void;
  locked?: boolean;
  stage?: string;
  fallbackFocus?: string;
  /** Extra class on the sheet (the prototype's `openSheet(html, cls)`). */
  className?: string;
  /** With a label, the title row gets a close button (prototype `.st-head`). */
  closeLabel?: string;
  /** The prototype's `.st-head` title row, even without a close button. */
  headRow?: boolean;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const opener = useRef<Element | null>(null);
  const close = useRef(onClose);
  close.current = onClose;
  const isLocked = useRef(locked);
  isLocked.current = locked;
  const fallback = useRef(fallbackFocus);
  fallback.current = fallbackFocus;
  const first = useRef(true);

  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    ref.current?.focus();
  }, [stage]);

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
      const usable = (e: Element | null): e is HTMLElement => e instanceof HTMLElement && e.isConnected && !(e as HTMLButtonElement).disabled;
      if (usable(o)) o.focus();
      else {
        // Wait for the view to re-render after the sheet's change.
        setTimeout(() => {
          const now = opener.current;
          if (usable(now)) return now.focus();
          const f = fallback.current ? document.querySelector(fallback.current) : null;
          if (usable(f)) f.focus();
        }, 0);
      }
    };
  }, []);

  return (
    <div
      className="scrim"
      id="scrim"
      // A click on the scrim never takes focus out of the sheet.
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) e.preventDefault();
      }}
      onClick={(e) => {
        if (e.target === e.currentTarget && !isLocked.current) onClose();
      }}
    >
      <div className={className ? `sheet ${className}` : "sheet"} role="dialog" aria-modal="true" aria-labelledby="sheet-title" tabIndex={-1} id="sheet" ref={ref}>
        {closeLabel || headRow ? (
          <div className="st-head">
            <h2 id="sheet-title">{title}</h2>
            {closeLabel && (
              <button className="icon-x" data-act="close" aria-label={closeLabel} onClick={onClose}>
                ×
              </button>
            )}
          </div>
        ) : (
          <h2 id="sheet-title">{title}</h2>
        )}
        {children}
      </div>
    </div>
  );
}
