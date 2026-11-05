# Manual checks still open (macOS)

Checks that need a person at the Mac. Everything else in scope (macOS and Linux) has automated or recorded evidence; see `docs/performance.md`, `visual/native/*/REVIEW.md` and `linux-evidence/`.

Build first: `npx tauri build --bundles app,dmg`. The app is `src-tauri/target/release/bundle/macos/Freeboard.app`.

## 1. Notification click opens Suggestions (task 2.3)

1. Quit Freeboard (tray → Quit).
2. Make it go Low: `sqlite3 ~/Library/Application\ Support/app.freeboard.desktop/freeboard.db "update settings set value='300000000000' where key='lowThresholdBytes'; update meta set value='{\"home\":\"healthy\"}' where key='monitor_statuses';"`
3. Start Freeboard. If macOS asks whether Freeboard may send notifications, click **Allow** (then quit and repeat step 2–3, because the first one may be dropped).
4. Close the window, then click the "Disk space is low" notification. Expected: Freeboard comes to the front on **Suggestions**.
5. Restore: quit, then `... set value='20000000000' where key='lowThresholdBytes'`.

Last attempt (2026-11-05): the notification was issued (`notification_pending_view = suggestions` recorded) but the click was not yet confirmed by a person.

## 2. VoiceOver pass (task 5.2)

Turn on VoiceOver (Cmd+F5). With VO+arrow keys and Tab, go through Overview, Suggestions (open a group, select an item), Review plan, History and Settings. Expected: every button and checkbox has a spoken name; risk and status are spoken as words (Safe, Review, Low), not only colors; focus never gets lost after a dialog closes. Note anything unclear.

## 3. Dark mode native screenshots (task 5.3)

System Settings → Appearance → Dark. Open each screen and compare with the light captures in `visual/native/macos/`. Switch back afterwards.

## 4. Real Trash emptying (task 4.13, optional)

Needs Full Disk Access for Freeboard (System Settings → Privacy & Security → Full Disk Access), because macOS hides `~/.Trash` from apps without it. Without it Freeboard reports "Trash not readable" and removes nothing, which is the intended safe behavior.

With access: run `cargo test --no-default-features native_trash_round_trip -- --ignored` in `src-tauri/`. It moves two disposable files to the Trash and must remove only its own.

Two disposable files from an earlier run (`freeboard-disposable-*`, `freeboard-unrelated-*`) are in the Trash and can be deleted by hand.
