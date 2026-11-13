# Detectors

Source: `docs/reference/FREEBOARD_SPEC.md`, "Scans and detectors". Code:
`src-tauri/src/detectors/`, `src-tauri/src/scan/`.

Each source detector ID is one `Detector` (`Detector::id()` equals the
`detector` field of its suggestions). Detectors only read: they receive a
runner that enforces the read-only command policy and no mutation API. One
detector failing never stops the others; its error is reported with a kind
(`unavailable`, `timeout`, `permission`, `malformed`, `error`).

## Scan kinds

| Kind | Detectors | Notes |
| --- | --- | --- |
| Quick | docker.\*, cache.\*, files.\* | Tool reports and known cache locations. |
| Deep | all, plus git.worktrees, postgres.worktree_dbs, project.dependencies, project.build_output | One discovery walk of the configured scan folders per scan, shared by the deep detectors. |

Discovery (`scan/walk.rs`) reads directory entries only. It never follows
symlinks, junctions or reparse points, never descends into `.git` or
generated folders (`node_modules`, `target`, `.venv`, `dist`, `build`,
`.next`), skips exclusions and WSL paths, pauses while on battery (deep scans,
default setting) and stops on cancel. Deep scans lower the scanning thread's
priority (`platform::lower_thread_priority`); callers run scans on a dedicated
thread.

Size (`scan/size.rs`) is allocated size (`st_blocks`, `GetCompressedFileSizeW`),
each hard-linked file counted once by device/inode or volume/file ID. A file
whose hard links are not all inside the measured tree is "shared" and not
counted as reclaimable; this is how pnpm-linked `node_modules` and pnpm's
store report only space removal would free. Shared or unreadable data marks
the size as an estimate (the interface shows "about").

## Detector summary

| Detector | Finds | Risk | Action | Platforms | Tests |
| --- | --- | --- | --- | --- | --- |
| `docker.build_cache` | Reclaimable build cache from `docker system df` | Safe | `docker builder prune --force --filter until=<scan age>` | All | `tests::docker::classifies_fixture_state`, `build_cache_growth_is_detected` |
| `docker.dangling_images` | Untagged images no container (running or stopped) uses | Safe | `docker image rm <exact IDs>` | All | `classifies_fixture_state`, `cleanup::tests::aggregate_commands_stay_within_the_plan` |
| `docker.unused_images` | Tagged images no container uses | Review | `docker image rm <tags>` | All | `classifies_fixture_state`, `probe_detects_container_using_image_and_volume` |
| `docker.stopped_containers` | Containers stopped 14+ days | Review | `docker container rm <ids>` | All | `classifies_fixture_state` |
| `docker.volumes` | Volumes no container mounts | Risky, individual acknowledgment | `docker volume rm <name>` | All | `inv10_volumes_never_preselected_and_need_ack` |
| `docker.disk_slack` | Docker Desktop's disk image (`Docker.raw`, `docker_data.vhdx`) on disk minus the total of `docker system df`, when 1 GB or more. macOS reads it only with Full Disk Access (otherwise unavailable, "permission"). Information only: no action, `Target::None`, shown as blocked with how Docker Desktop can shrink it | Review | None | macOS, Windows | `tests::docker::disk_slack_is_information_only`, `disk_slack_reads_docker_usage_and_needs_the_daemon` |
| `git.worktrees` | Linked worktrees | Review or Risky; removal disabled | `git worktree remove <path>` (not run, see cleanup.md) | All | `tests::git::worktree_state_matrix` |
| `postgres.worktree_dbs` | Per-worktree copies of a database (`<base>_wt<N>`, never a template) on a Postgres server on this computer: libpq's default, the servers in Settings → Postgres databases, and the localhost entries of `~/.pgpass` (host, port and user only; psql reads the password). One constant read-only query. Tied after the scan to the worktree named `wt<N>`, whose row then shows the database that stays behind. Blocked while it has connections. Silent when psql is missing or no server runs; reported as unavailable when a server refuses Freeboard | Risky, individual acknowledgment | `dropdb [-h -p -U] -w <name>` | All | `tests::postgres::lists_worktree_databases_and_links_them_to_their_worktree`, `probe_rechecks_the_same_database`, `runner_allows_only_the_constant_query_and_one_worktree_database` |
| `project.dependencies` | `node_modules`, `.venv` | Safe / Review | Delete / Trash | All | `tests::projects::ownership_evidence_table` |
| `project.build_output` | `target`, `dist`, `build`, `.next` | Safe | Delete | All | `ownership_evidence_table` |
| `cache.pnpm` | Store files no project links to | Safe | `pnpm store prune` | All | `tests::caches::pnpm_reports_only_unreferenced_packages` |
| `cache.pip` | pip download cache | Safe | `pip3 cache purge` (or `pip`) | All | `pip_cache_and_missing_tool`, `pip_timeout_reports_timeout` |
| `cache.npm` | `_cacache` | Safe | Delete the folder (never `npm cache clean`) | All | `npm_cache_is_deleted_as_a_folder_never_with_force` |
| `cache.homebrew` | What `brew cleanup` would remove | Safe | `brew cleanup` | macOS, Linux | `homebrew_dry_run_and_growth`, `homebrew_excluded_on_windows` |
| `cache.cargo` | `registry/cache`, `registry/src`, `git/db`, `git/checkouts` | Review | Trash | All | `cargo_and_gradle_are_review_trash` |
| `cache.gradle` | `~/.gradle/caches` | Review; blocked while a daemon runs or when unknown | Trash | All | `gradle_blocked_while_daemon_runs_or_unknown`, `gradle_probe_rechecks_daemon` |
| `cache.jetbrains` | Cache (and on macOS log) folders of JetBrains IDE versions older than the newest version of the same product, judged from caches and settings folders; one part per version | Safe | Delete | All | `tests::app_caches::jetbrains_offers_only_versions_older_than_the_newest`, `jetbrains_with_one_version_per_product_offers_nothing` |
| `cache.electron_apps` | `Cache`, `Code Cache`, `GPUCache`, `Service Worker/CacheStorage` of Slack, Discord and Teams; one part per closed app. An open app (process name on macOS and Linux, Chromium's `lockfile` on Windows) is its own blocked item; unknown blocks | Safe | Delete | All | `electron_caches_only_while_the_app_is_closed`, `electron_unknown_process_state_blocks` |
| `cache.playwright` | `ms-playwright/<browser>-<revision>` folders that no project's `playwright-core/browsers.json` asks for and that are not the newest revision of their browser; deep scans only; unreadable `browsers.json` marks it unavailable | Safe | Delete | All | `playwright_keeps_what_projects_need_and_the_newest`, `playwright_unreadable_needs_mark_it_unavailable` |
| `cache.xcode` | DerivedData | Safe | Delete | macOS | `xcode_and_simulators_are_macos_only` |
| `mobile.simulators` | Unavailable simulators, one part per simulator (name and the runtime it needs) | Review | `xcrun simctl delete <UDID>` per part | macOS | `xcode_and_simulators_are_macos_only` |
| `mobile.ios_backups` | iPhone and iPad backups whose `Info.plist` "Last Backup Date" (or folder change time) is 180+ days old, one part per backup named after the device. macOS reads them only with Full Disk Access (otherwise unavailable, "permission") | Review | Trash | macOS, Windows | `tests::mobile::ios_backups_older_than_180_days_split_by_backup` |
| `mobile.android_avd` | `*.avd` folders (and their `.ini`) in `ANDROID_AVD_HOME`, `ANDROID_USER_HOME/avd` or `~/.android/avd` with no change for 90 days, one part per emulator; never while its `*.lock` files exist | Review | Trash | All | `android_emulators_not_started_for_90_days_and_never_while_running` |
| `ai.ollama` | Models whose manifest and layers were not read or changed for 60 days, from `~/.ollama/models/manifests` (or `OLLAMA_MODELS`); one part per model; size counts only layers no other model uses. Blocked when `ollama` is not installed; a malformed manifest marks it unavailable | Review | `ollama rm <model>`, one model per part | All | `tests::ai::ollama_offers_idle_models_per_model_counting_only_their_own_layers`, `ollama_malformed_manifest_marks_it_unavailable_and_missing_cli_blocks`, `ollama_rm_is_the_only_allowed_action` |
| `ai.huggingface` | `models--*`, `datasets--*`, `spaces--*` in the hub cache (`HF_HUB_CACHE`, `HF_HOME/hub`, `~/.cache/huggingface/hub`); one part per repository; space shared with the hub's `blobs` folder is not counted | Review | Trash | All | `huggingface_repositories_are_parts_and_go_to_the_trash` |
| `ai.lm_studio` | `<publisher>/<model>` folders in `~/.lmstudio/models` and `~/.cache/lm-studio/models`; one part per model | Review | Trash | All | `lm_studio_models_split_by_model_in_both_locations` |
| `tools.node_versions` | nvm versions (`$NVM_DIR/versions/node/v*`) that are not the newest, do not match `alias/default`, and match no project's `.nvmrc`, `.node-version`, `engines.node` or `volta.node` (semver ranges, partial versions, `lts/<name>`); deep scans only; one part per version. A request it cannot read keeps every version (unavailable, "malformed") | Review | Trash (`nvm` is a shell function, so the version's folder goes, as `nvm uninstall` does) | All | `tests::tools::node_versions_keep_default_newest_and_every_requested_version`, `node_versions_unreadable_request_keeps_everything` |
| `tools.rust_toolchains` | `rustup toolchain list` entries that are not default, active or an override, and that no project's `rust-toolchain.toml` or `rust-toolchain` pins; linked toolchains are skipped; deep scans only; one part per toolchain | Review | `rustup toolchain uninstall <name>` per part | All | `rust_toolchains_keep_default_and_pinned`, `rustup_actions_are_bounded` |
| `apps.unused` | Top-level `.app` bundles (never links) in `/Applications` and `~/Applications` whose Spotlight `kMDItemLastUsedDate` is 365+ days old; never without that date, never `com.apple.*`, never while running (unknown counts as running); one part per app, each with its own acknowledgment | Review | Trash only, through `Guard::check_unused_app` | macOS (Windows and Linux keep no readable last-opened date) | `tests::apps::unused_apps_need_last_opened_data_and_a_year_without_use`, `unused_apps_skip_links_and_running_apps`, `mdls_is_read_only_and_bounded`, `guard::tests::unused_app_exception_is_narrow` |
| `files.installers` | `.dmg .pkg .exe .msi .AppImage .deb .rpm .iso` in Downloads not opened or modified for 90 days; on macOS only when Downloads is a scan folder or with Full Disk Access (otherwise "permission") | Review | Trash | All | `tests::files::downloads_only_old_installers` |
| `files.duplicates` | Files of 1 MB or more in the scan folders with exactly the same contents: same size, then the same `blake3` of the first 64 KiB, then the same full `blake3`. Hard links to one file are one file. Never in hidden folders, app or library bundles, repositories or generated folders. One part per set; the oldest copy is kept. Before cleanup the kept copy and every extra are hashed again in full; if the kept copy changed or is gone, nothing goes. Deep scans only | Review | Trash | All | `tests::dupes::duplicates_match_exact_contents_and_keep_the_oldest`, `several_duplicate_sets_are_parts` |
| `files.large_media` | Video files (`mp4 m4v mov mkv avi webm wmv flv mpg mpeg mts m2ts 3gp vob`) of 500 MB or more not opened or modified for 182 days, in the scan folders, Downloads and the videos folder (macOS Movies only with Full Disk Access); same exclusions as duplicates; one part per file; deep scans only | Review | Trash | All | `large_videos_only_and_not_opened_for_six_months` |
| `system.snapshots` | Local snapshots (`tmutil listlocalsnapshots /`) more than an hour older than the newest backup from `tmutil latestbackup`, one part per snapshot. Without a confirmed backup (tmutil prints an error and exits 0 when there is no destination) it is unavailable and suggests nothing. tmutil has no size per snapshot: APFS purgeable space is shared out and every size says "about" | Review | `tmutil deletelocalsnapshots <date>` per part, never a mount point | macOS | `tests::system::snapshots_older_than_a_confirmed_backup_one_part_each`, `unconfirmed_backup_reports_unavailable_and_suggests_nothing`, `tmutil_deletes_one_snapshot_by_date_only` |
| `system.stale_temp` | Top-level entries of the person's `$TMPDIR` unchanged for 14 days that no process holds open (`lsof -Fn +D`), owned by the person, not `com.apple.*`; unknown `lsof` state is unavailable | Review | Trash | macOS | `stale_temp_leaves_files_apps_still_use` |
| `files.windows_temp` | `%TEMP%` entries unchanged for 14 days | Review | Recycle Bin | Windows | `windows_temp_old_entries_go_to_recycle_bin` |

Failure modes per detector (missing tool, timeout, malformed output, daemon
down, permission) are covered by `missing_docker_is_unavailable`,
`daemon_down_marks_all_docker_detectors_unavailable_and_others_continue`,
`inv11_timeout_is_unavailable_not_safe`, `malformed_json_is_reported`,
`git_missing_is_unavailable`, `pip_timeout_reports_timeout`,
`xcode_and_simulators_are_macos_only` (malformed simctl output) and
`tests::scan::one_failing_detector_does_not_stop_others`.

## CLI formats relied on

Validated on 2026-11-04 against the installed tools on macOS (arm64) with
read-only commands only.

| Tool | Version checked | Commands and format |
| --- | --- | --- |
| Docker CLI | 29.8.2 | `docker version --format '{{json .}}'` (`Server` is `null` when the daemon is down; captured live). `ps -a --no-trunc`, `image ls --no-trunc`, `volume ls` and `system df` with `--format '{{json .}}'` (one JSON object per line); `container inspect --size` and `image inspect` (JSON arrays: `Id`, `Image`, `State.Status/Running/FinishedAt`, `Mounts[].Type/Name`, `SizeRw`, `RepoTags`, `Created`, `Size`); `system df -v --format '{{json .}}'` (`Volumes[].Name/Size`). The daemon was not running during development, so the list/inspect shapes are taken from the Docker CLI's documented JSON templates and kept as fixtures in `tests/fixtures/docker/`. Validate them against a running daemon before release. |
| Git | 2.38.1 | `worktree list --porcelain`, `status --porcelain=v2 --branch --untracked-files=normal`, `rev-parse --absolute-git-dir`, `rev-parse --path-format=absolute --git-common-dir`, `merge-base --is-ancestor`, `symbolic-ref --short refs/remotes/origin/HEAD`, `check-ignore -q`, `log -1 --format=%ct`. Exercised against real temporary repositories in tests. |
| Homebrew | installed | `brew cleanup --dry-run`: lines `Would remove: <path> (<details>, <size>)` and `==> This operation would free approximately <size> of disk space.` (captured live). |
| xcrun simctl | Xcode installed | `simctl list devices unavailable --json` (`devices.<runtime>[].udid/isAvailable/dataPath/dataPathSize/logPathSize`; captured live, fixture in `tests/fixtures/simctl/`). |
| pnpm | installed | `pnpm store path` prints the absolute store path. |
| pip | installed | `pip3 cache dir` prints the absolute cache path. |
| npm | installed | `npm config get cache` prints the cache root; `_cacache` lives inside it. |
| mdls | macOS 27 | `mdls -name kMDItemCFBundleIdentifier -name kMDItemLastUsedDate <apps...>` prints two `key = value` lines per file in that order; `(null)` when unknown; dates as `2026-10-17 20:17:56 +0000`. A missing file prints an error and exits 1, so the batch falls back to one call per app. Checked live 2026-11-08. |
| tmutil | macOS 27 | `tmutil listlocalsnapshots /` prints `Snapshots for disk /:` then `com.apple.TimeMachine.<YYYY-MM-DD-HHMMSS>.local` lines; `tmutil latestbackup` prints the backup's path, or with no destination `Failed to mount backup destination, error: ...` and still exits 0 (checked live 2026-11-08; this Mac has no snapshots or destination, so the backup path shape is from Apple's documentation). |
| nvm | 0.40 (`~/.nvm`) | `versions/node/v<major>.<minor>.<patch>/` folders and `alias/default` holding a version or alias (`24.18.0` here); checked live 2026-11-08. |
| rustup | 1.29.2 | `rustup toolchain list` prints one toolchain per line, flags in parentheses (`stable-aarch64-apple-darwin (active, default)`); checked live 2026-11-08. |
| psql / dropdb | 16 (Homebrew) | `psql -X -w -A -t -q -0 -F <US> [-h -p -U] -d postgres -c <QUERY>`: a server row (`S`, port, data folder or empty without the right to read it) and one row per database (`D`, name, oid, size or empty without CONNECT, open connections), fields split by U+001F, rows by NUL. A missing server prints `connection refused` or `No such file or directory` on stderr. Not checked against a live server: fixtures and a fake psql only. |
| ps / lsof | macOS | `ps -A -o args=` (Gradle daemon check); `lsof -a -d cwd -Fp +D <dir>` (processes in a folder; exit 1 means none). Linux reads `/proc/*/cwd`. |

## Limitations

- Docker sizes are Docker's own estimates (shared layers make per-image
  sizes overlap); they are shown with "about". Docker Desktop may keep freed
  space in its virtual disk, so cleanup reports the measured change.
- Volume "last used" is not recorded by Docker; it is shown as unknown.
- Process detection is unavailable on Windows (`processes_in_dir` returns
  unknown). Projects there are raised from Safe to Review (moved to the
  Recycle Bin instead of deleted) and worktrees are Risky. Gradle caches are
  blocked on Windows because the daemon check is unavailable.
- Project inactivity reads source file modification times, skipping
  generated folders and `.git`. Projects with more than 200,000 source
  entries are treated as unknown and not suggested.
- `dist`, `build` and `.next` need Git ignore evidence; folders outside a Git
  repository are not suggested.
- Installer age uses the newest of modification and access time. Volumes
  mounted with `noatime` can make a recently opened installer look old;
  Spotlight's "last opened" date is not read.
- `%TEMP%` entries are grouped into one item; open-file detection is not
  available, so items are Review and go to the Recycle Bin.
- Linked worktrees found through a `.git` file are resolved to their main
  repository; worktrees under exclusions or WSL paths are skipped.
- The quick and deep scan performance targets have not been measured yet.
