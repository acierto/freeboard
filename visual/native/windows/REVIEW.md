# Windows 11 Arm64 — native review (2026-11-05, in progress)

Test VM: `scripts/windows/vm/` (QEMU 9.2 with HVF on Apple silicon, Windows 11 Pro 26H2 build 26300, 800×600 basic display).
Build: `Freeboard-windows-arm64-setup.exe` from the release workflow (test build of `main`, not published).

| Check | Result | Evidence |
| --- | --- | --- |
| Silent per-user install (`/S`), no administrator rights | pass: installs to `%LOCALAPPDATA%\Freeboard`, uninstall entry 0.1.0 | — |
| First launch shows onboarding, fonts and layout render | pass | `onboarding.png` |
| Free space matches Windows (`Get-PSDrive C`: 32.6 GB free, 35.8 GB used) | pass | `overview-not-scanned.png` |
| Start at login runs in tray mode (no window); tray menu with status, Open, Scan Now, Pause Notifications, Quit | pass | `tray-menu-after-login.png` |
| Open Freeboard from the tray | pass | — |
| Native folder picker | pass | `folder-picker.png` |
| Scan folder shown as `\\?\C:\Users\dev\projects` | **bug, fixed in source** (plain path stored); needs a re-test with the next build | — |
| Gradle caches offered when the only daemon log belongs to a stopped process | pass | `suggestions-scan.png` |
| Old files in `%TEMP%` (Windows temp detector) | pass | `suggestions-scan.png` |
| Missing tools reported once each (Docker, Git, pnpm, pip not installed) | pass | `suggestions-scan.png` |
| `node_modules` in a project without a lockfile is not offered | pass (by design: no lockfile, cannot reinstall) | — |
| Old installer in Downloads (`.msi`, times set 120 days back) | **not shown; unexplained**. Likely its last-access time was refreshed by Defender after the fixture set it; not yet confirmed | — |

Not yet covered: cleanup to the Recycle Bin, plan and run screens, notifications and notification click, restart into tray mode after the window closes, hidden-memory and idle-CPU measurements, x64 build, dark mode.

The VM is slow and its SSH server stalls at times, so input lags; results above were re-checked when in doubt.
