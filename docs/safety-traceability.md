# Safety invariant traceability

Source: `docs/reference/FREEBOARD_SPEC.md`, "Safety invariants". Every
invariant maps to at least one automated test in the Rust core
(`src-tauri`, `cargo test --no-default-features`). All tests use temporary
fixtures, fake tool executables and a directory-backed Trash; none runs real
cleanup against the developer machine.

Check this table against the test suite with:

```sh
scripts/safety-traceability.sh
```

The script fails if any listed test is missing or fails, or if an invariant
has no test.

| # | Invariant | Tests |
| --- | --- | --- |
| 1 | Scans are read-only | `detectors::tests::scan::inv01_scans_are_read_only`, `detectors::tests::docker::scans_run_only_read_only_docker_commands`, `runner::tests::read_only_policy_rejects_mutations` |
| 2 | Nothing runs without a confirmed plan | `cleanup::tests::inv02_nothing_runs_without_a_saved_plan`, `cleanup::tests::inv02_altered_plan_is_rejected`, `cleanup::tests::plan_expires_after_thirty_minutes`, `cleanup::tests::new_scan_invalidates_plan`, `cleanup::tests::split_plans_still_expire_and_are_invalidated_by_a_new_scan`, `cleanup::tests::a_split_item_is_never_cleaned_through_its_combined_target`, `runner::tests::force_only_for_docker_builder_and_image_prune` |
| 3 | The backend owns paths and commands | `cleanup::tests::inv03_unknown_ids_and_paths_are_rejected`, `app_core::tests::inv03_ids_only_rejects_paths_and_commands`, `app_core::tests::inv03_settings_patch_cannot_carry_paths_or_commands`, `contract_tests::inv03_app_commands_take_ids_not_paths`, `contract_tests::inv03_capabilities_grant_no_filesystem_shell_or_network_plugins` |
| 4 | Every item is checked again immediately before cleanup | `cleanup::tests::inv04_modified_file_is_skipped_and_explained`, `cleanup::tests::inv04_symlink_swap_is_skipped`, `cleanup::tests::inv04_container_starts_using_image_or_volume`, `cleanup::tests::inv04_removed_manifest_is_skipped`, `cleanup::tests::a_changed_part_is_skipped_while_the_others_run`, `detectors::tests::git::probe_detects_new_commit`, `detectors::tests::docker::probe_detects_container_using_image_and_volume` |
| 5 | Protected paths are never touched | `cleanup::tests::inv05_protected_paths_are_refused_at_execution`, `guard::tests::rejects_root_home_and_ancestors`, `guard::tests::rejects_repository_root_and_git`, `guard::tests::rejects_system_subtree`, `guard::tests::rejects_relative_and_traversal`, `guard::tests::real_system_guard_protects_home_and_root`, `guard::tests::unused_app_exception_is_narrow` (the macOS unused-apps exception: Trash only, a top-level bundle in an app folder; the end-to-end runs, macOS only, are cleanup::tests::unused_app_goes_to_the_trash_only_after_its_acknowledgment and unused_app_is_never_deleted_permanently) |
| 6 | Symlinks and junctions are never followed | `detectors::tests::scan::inv06_scans_never_follow_links`, `cleanup::tests::inv06_delete_removes_links_not_targets`, `guard::tests::symlink_trap_removes_only_link`, `guard::tests::ancestor_symlink_swap_is_detected`, `scan::size::tests::never_follows_links` |
| 7 | Dangerous commands are banned | `cleanup::tests::inv07_banned_and_forced_commands_are_refused`, `runner::tests::banned_commands_rejected_even_when_confirmed`, `runner::tests::force_only_for_docker_builder_and_image_prune` |
| 8 | Commands run without a shell | `cleanup::tests::inv08_arguments_are_passed_without_a_shell`, `runner::tests::no_shell_interpolation`, `runner::tests::arbitrary_programs_and_paths_rejected` |
| 9 | Git work is never preselected | `detectors::tests::git::inv09_git_work_never_preselected_only_clean_review_selectable`, `detectors::tests::git::worktree_state_matrix`, `cleanup::tests::inv09_unpushed_worktree_cannot_be_planned` |
| 10 | Docker volumes are never preselected | `detectors::tests::docker::inv10_volumes_never_preselected_and_need_ack`, `cleanup::tests::inv10_volume_requires_individual_ack`, `cleanup::tests::a_volume_beside_a_split_item_still_needs_its_own_acknowledgment` |
| 11 | Unknown means less safe | `detectors::tests::docker::inv11_timeout_is_unavailable_not_safe`, `detectors::tests::git::inv11_unknown_process_state_is_risky`, `detectors::tests::projects::inv11_unknown_process_state_raises_risk`, `cleanup::tests::inv11_unknown_status_skips` |
| 12 | All data stays local | `contract_tests::inv12_no_network_client_in_dependencies`, `contract_tests::inv12_csp_forbids_remote_content`, `contract_tests::inv12_frontend_has_no_remote_urls` |
| 13 | Every action leaves a record | `cleanup::tests::inv13_failures_are_independent_and_recorded`, `cleanup::tests::history_shows_mixed_results_and_paginates`, `cleanup::tests::interrupted_run_is_recorded_and_never_replayed`, `cleanup::tests::only_selected_parts_are_removed_and_the_rest_stay_listed` |

Platform notes:

- Fixture tests that need fake executables are `#[cfg(unix)]` (they use
  POSIX shell scripts as stand-ins for `docker`, `pnpm`, `lsof`, ...). On
  Windows the guard, runner, store and file-detector tests run; Windows
  evidence for the cleanup tests requires porting the fakes to `.cmd`
  scripts and is tracked as incomplete platform evidence.
- `cleanup::tests::native_trash_round_trip_with_disposable_file` is
  `#[ignore]`d. It moves one dedicated disposable file to the real platform
  Trash and empties exactly that item; run it manually per platform with
  `cargo test --no-default-features -- --ignored native_trash`.
