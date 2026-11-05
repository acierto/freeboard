# Linux (GNOME) native evidence

Captured 2026-11-05 in a disposable QEMU VM (`scripts/linux/vm/`): Ubuntu 24.04.5 ARM64, GNOME on Xorg with the Ubuntu AppIndicator extension, WebKitGTK 2.52, software rendering (no GPU). Freeboard installed from the `.deb` built by `scripts/linux/verify.sh`. Real backend; the VM has no developer tools installed.

| Capture | Review |
| --- | --- |
| first.png | Window renders in WebKitGTK with the bundled fonts; app icon in the dock. |
| tray-menu.png | Tray ring in the top bar; native menu shows status line, Open Freeboard, Scan Now, Pause Notifications, Quit. (The "System program problem" dialog is Ubuntu's report for `fwupdmgr`, which crashed before Freeboard was installed.) |
| tray-paused.png | Low status after raising the Low threshold to 40 GB; amber ring; Pause Notifications checked and persisted. |
| notification2.png | GNOME notification on entering Low, once; text without a "0.0 GB" claim before any scan. |
| after-click3.png | Clicking the notification opens Suggestions, past the unfinished welcome screen. |
| scanned.png | Quick scan with no tools installed: each missing tool reported once, other detectors continue. (Sentence after a finished scan was later changed to "The last scan found nothing to reclaim…".) |
| reopened.png | Window recreated from the tray after the webview was destroyed (60 s after closing). |

Also verified: Quit exits Freeboard and all WebKit helper processes; real freedesktop Trash round trip (`native-trash` step in `scripts/linux/verify.sh`) empties only Freeboard's item.

Defects found here and fixed: notification click did nothing on Linux (now handled through the desktop notification service's default action); notification click stayed on the welcome screen; "0.0 GB can be freed" notification text; "No suggestions yet. Run a scan" after a scan had run.

Hidden memory: first measured at 100 MB PSS (81.5 MB main process, 18.5 MB WebKitNetworkProcess) after the webview was destroyed. Fixed by restarting into tray mode 60 s after the window is hidden: 22.8 MB PSS, idle CPU 0.026% over 300 s. Verified: the restart (logged, `--tray --after-pid`), Open Freeboard from the tray-mode process, and a second launch handing over to the running copy (one process, window shown).

Not covered: KDE Plasma, dark appearance.
