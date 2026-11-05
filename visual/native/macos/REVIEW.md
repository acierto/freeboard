# macOS native evidence (Tauri WKWebView)

Captured 2026-11-05 on macOS 27.0.1 (arm64), release build signed ad hoc, real backend and real disk data (not fixtures), light appearance. Window 1120×900 pt; images downscaled to 1120 px wide.

| Capture | Review |
| --- | --- |
| onboarding-light.png | Matches the supplemental onboarding design; bundled Barlow Condensed / IBM Plex fonts load offline in WKWebView. |
| overview-light.png | Hull gauge, pill with status text, big free figure, CTA with real Safe total, trend and category bars match the prototype styling. 192.5 GB free is above the 120 GB scale, so the waterline sits below the hull (the "over 120" gauge state). |
| suggestions-light.png | Grouping, sizes, "about" estimates, risk pills with shapes, blocked reason (running Gradle daemon), single Docker-unavailable note. |
| settings-light.png | Layout matches. Range sliders and selects use native macOS WebKit control styling, which differs from Chromium; accepted as a per-engine difference. |

Defects found on real data and fixed before these captures: repeated Docker-unavailable notes (one per Docker detector), a "0.0 GB of purgeable space" note, "Review 0.0 GB of Safe items" before the first scan, and "Last scan never".

Not yet captured: dark appearance, tray menu and notification (manual checks with the user).
