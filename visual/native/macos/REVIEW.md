# macOS native evidence (rebuild)

Earlier evidence for the hull-gauge interface was removed on 2026-11-06 (it is in git history).

## 2026-11-06 — tray view and hidden mode (milestone 2, tasks 2.5 and 2.7)

macOS 27, Apple silicon, release build (`npx tauri build --bundles app`), commit after `cadcd9c`.

| Check | Result |
| --- | --- |
| Left click on the menu-bar item opens the tray view under it | Pass: `tray-view-healthy.png`. 312 × 236 window at the icon, transparent around the card (only the card and its shadow show). Status "Healthy", "89.2 GB free", Toss disabled with "Toss 0.0 GB" (the Rust pile arrives in milestone 4). |
| Closes with a click outside | Pass (window count 2 → 1 after a click on the main window) |
| Closes with Esc | Pass |
| Closes with its × button | Pass |
| Clicking the icon again closes it | Pass |
| Menu-bar title shows free space | Pass ("89.2 GB" beside the ring) |
| Hidden memory and idle CPU | 21 MB footprint, 0.053% CPU over 304 s after the webview was destroyed (see `docs/performance.md`) |

Clicks were real mouse events (CGEvent), because an accessibility "press" on a status item does not reach the tray's click handler.

Still to capture: Low and Critical rings, the right-click menu, and a notification and its click (they need a disk below 20 GB or changed thresholds).
