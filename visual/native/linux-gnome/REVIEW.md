# Linux GNOME native evidence (rebuild)

Earlier evidence for the hull-gauge interface was removed on 2026-11-06 (it is in git history).

## 2026-11-06 — tray view, menu and hidden mode (milestone 2, tasks 2.5 and 2.7)

Ubuntu 24.04.5 LTS ARM64, GNOME on X11 with the AppIndicator extension, QEMU VM without 3D acceleration (`scripts/linux/vm/`). `Freeboard-linux-arm64.deb` from release workflow run 42232860033 (commit `2868ead`).

| Check | Result |
| --- | --- |
| The window renders | **Failed first**: blank white window (`before-fix-blank-window.png`); WebKitGTK logged `libEGL warning: DRI2: failed to create screen`. With `WEBKIT_DISABLE_DMABUF_RENDERER=1` it renders correctly (`tray-view-and-home.png`). Fixed: Freeboard now sets that variable at start-up on Linux (`src-tauri/src/main.rs`, decision 2026-11-06). |
| Tray ring and menu | Pass (`tray-menu.png`): status line "Healthy · 34.3 GB free", Show Summary, Open Freeboard, Scan Now, Pause Notifications, Quit. GNOME does not report tray clicks, so the menu opens on any click. |
| Show Summary opens the tray view at the top right | Pass (`tray-view-and-home.png`): "Your tray", "34.3 GB free", Healthy, Toss disabled (the Rust pile arrives in milestone 4). Transparent around the card. |
| Tray view closes with a click outside | Pass |
| Closes with Esc | Pass |
| Closes with its × button | Pass |
| Home view | Pass: Dinghy happy at Healthy, bubble "All tidy! Nothing to toss right now.", fonts bundled, layout as in the prototype. No forecast yet (needs 3 days of samples). |
| Hidden mode | Alt+F4 hid the window; 60 s later Freeboard restarted as `freeboard --tray --after-pid 3781`. 22.9 MB PSS, 0.328% CPU over 305 s (`docs/performance.md`). |

Still to capture: Low and Critical rings, a notification and its click, KDE Plasma.
