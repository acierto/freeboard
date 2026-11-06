import { useEffect, useState } from "react";
import { on } from "./lib/api";
import type { View } from "./lib/types";
import { Home } from "./features/home/Home";
import { TrayView } from "./features/tray/TrayView";

/** True in the tray view's window. Tauri names it "tray"; the visual
 * verifier, which has no Tauri, opens it with `?view=tray`. */
function isTrayWindow(): boolean {
  const label = (window as { __TAURI_INTERNALS__?: { metadata?: { currentWindow?: { label?: string } } } }).__TAURI_INTERNALS__?.metadata?.currentWindow?.label;
  return label === "tray" || new URLSearchParams(location.search).get("view") === "tray";
}

/** The main window shows the home view; the tray window shows the tray view. */
export function App() {
  const tray = isTrayWindow();
  const [navigate, setNavigate] = useState<View | null>(null);
  useEffect(() => {
    if (tray) return;
    // A notification click or the tray view's button sends the window here.
    const off = on("navigate", (n) => setNavigate(n.view));
    return () => void off.then((f) => f());
  }, [tray]);
  return tray ? <TrayView /> : <Home navigate={navigate} />;
}
