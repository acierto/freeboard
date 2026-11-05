# Linux (KDE Plasma) native evidence

Captured 2026-11-05 in the same disposable QEMU VM as the GNOME evidence (`scripts/linux/vm/`): Ubuntu 24.04.5 ARM64, KDE Plasma 5.27.12 and KWin 5.27.11 on X11 (Kubuntu packages `plasma-desktop`, `plasma-workspace`, `kwin-x11`, logged in through GDM), software rendering. Freeboard installed from the `.deb`. Real backend; for this run the VM also had git, Node/npm, pip and Docker 29.1 with test images, a volume and a stopped container.

| Capture | Review |
| --- | --- |
| tray-menu.png | Ring in the Plasma system tray (StatusNotifierItem, no extension needed). Native menu: status line, Open Freeboard, Scan Now, Pause Notifications (checkbox), Quit. |
| tray-mode-no-window.png | `freeboard --tray`: tray icon only, no window. |
| notification-popup-sequence.png | Plasma popup on entering Low: app name, title, body, Freeboard icon. It expires after about 5 s (Plasma default; the history keeps it). Screenshots 1 s apart. |
| notification-click-suggestions.png | Clicking the popup (D-Bus `ActionInvoked "default"`) opened a window from tray mode, on Suggestions. |
| suggestions-real-data.png | Scan Now with real Docker, npm and pip data: images Review, volume Risky (never preselected), caches Safe. |
| reopen-after-tray-restart.png | Open Freeboard from the tray after the automatic restart into tray mode; selection count kept. (The welcome screen shows because first-run setup was never finished in this VM.) |

Also verified (`linux-evidence/kde-lifecycle.txt`): closing the window hides it; 60 s later Freeboard restarts as `freeboard --tray --after-pid <old>` (logged); a second launch hands over to the running copy (one process); Quit ends the process and removes the tray icon.

Tray mode memory and CPU: 26.2 MB PSS average over 300 s. Idle CPU 0.11% over 300 s once settled (0.74% in the first 300 s, which includes start-up).

Defect found here and fixed: Docker 29 leaves untagged images out of `docker image ls`, so dangling images were never suggested (they were found in the VM after the fix; on the Mac the two dangling images are used by containers and are correctly excluded).

Minor, open: hovering the tray icon shows "freeboard" (lowercase, the indicator's ID) instead of the status tooltip, because Plasma shows the StatusNotifierItem title.

Not covered: Plasma 6 / Wayland, dark appearance.
