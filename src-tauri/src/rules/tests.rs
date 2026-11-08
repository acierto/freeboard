//! Keep me tidy rules: fixtures and fake tools only.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use super::*;
use crate::app_core::tests::{core_with, FakeVolumes};
use crate::app_core::{AppCore, CoreConfig};
use crate::detectors::common::{base, rfc3339};
use crate::store::Store;

const GB: u64 = 1_000_000_000;

fn item(det: &str, id: &str, risk: Risk, days_old: i64, now: i64) -> Finding {
    let mut s = base(det, Group::Caches, id);
    s.id = id.into();
    s.risk = risk;
    s.action = Action::Delete;
    s.size_bytes = 2 * GB;
    s.last_used_at = rfc3339(now - days_old * DAY);
    Finding::new(s, Target::Paths { paths: vec![format!("/nowhere/{id}")] })
}

#[test]
fn fresh_install_has_both_rules_off_and_nothing_runs() {
    let d = tempfile::tempdir().unwrap();
    let (core, _, _, sink) = core_with(5, d.path());
    let rules = core.get_tidy_rules().unwrap();
    assert_eq!(rules.len(), 2);
    assert!(rules.iter().all(|r| !r.enabled && r.last_run_at.is_none()));
    core.run_tidy_rules();
    assert!(sink.events(EV_TIDY_RULE_RAN).is_empty());
}

#[test]
fn turning_a_rule_on_says_exactly_what_it_removes() {
    let d = tempfile::tempdir().unwrap();
    let (core, _, _, _) = core_with(5, d.path());
    let rules = core.set_tidy_rule(TidyRuleId::PackageCaches, true).unwrap();
    let r = rules.iter().find(|r| r.id == PACKAGE_CACHES).unwrap();
    assert!(r.enabled);
    assert_eq!(r.removes, ["pnpm store", "pip cache", "npm cache", "Homebrew cache"]);
    let docker = rules.iter().find(|r| r.id == DOCKER_BUILD_CACHE).unwrap();
    assert!(!docker.enabled);
    assert_eq!(docker.removes, ["Docker build cache"]);
    // Rule IDs are a closed set.
    assert!(serde_json::from_str::<TidyRuleId>("\"anything_else\"").is_err());
}

#[test]
fn kept_review_and_recent_items_are_never_chosen() {
    let now = 1_800_000_000;
    let rule = rule(PACKAGE_CACHES).unwrap();
    let mut kept = item("cache.pip", "pip", Risk::Safe, 90, now);
    kept.suggestion.decision = Some(ItemDecision::Keep);
    let findings = vec![
        kept,
        // A Review cargo cache, even of a detector the rule does not name.
        item("cache.cargo", "cargo", Risk::Review, 90, now),
        item("cache.npm", "npm", Risk::Safe, 45, now),
        // Changed last week: not over 30 days old.
        item("cache.pnpm", "pnpm", Risk::Safe, 7, now),
        // Safe, but not a package cache.
        item("cache.xcode", "xcode", Risk::Safe, 90, now),
    ];
    assert_eq!(eligible(&rule, &findings, now), ["npm"]);
    // A Review item of a named detector is out too.
    let mut review_npm = item("cache.npm", "npm2", Risk::Review, 90, now);
    review_npm.suggestion.reason = "x".into();
    assert!(eligible(&rule, &[review_npm], now).is_empty());
}

#[test]
fn rules_are_due_by_status_and_interval() {
    let (p, d) = (rule(PACKAGE_CACHES).unwrap(), rule(DOCKER_BUILD_CACHE).unwrap());
    let now = 1_800_000_000;
    assert!(!due(&p, DiskStatus::Healthy, None, now));
    assert!(due(&p, DiskStatus::Low, None, now));
    assert!(due(&p, DiskStatus::Critical, Some(now - DAY), now));
    assert!(!due(&p, DiskStatus::Low, Some(now - DAY + 60), now));
    assert!(due(&d, DiskStatus::Healthy, None, now));
    assert!(!due(&d, DiskStatus::Healthy, Some(now - 6 * DAY), now));
    assert!(due(&d, DiskStatus::Healthy, Some(now - WEEK), now));
}

#[test]
fn a_rule_plan_refuses_anything_its_rule_does_not_allow() {
    let d = tempfile::tempdir().unwrap();
    let (core, _, _, _) = core_with(5, d.path());
    let now = core.now();
    let review = item("cache.npm", "review-npm", Risk::Review, 90, now);
    let other = item("cache.xcode", "xcode", Risk::Safe, 90, now);
    let mut kept = item("cache.pip", "kept-pip", Risk::Safe, 90, now);
    kept.suggestion.decision = Some(ItemDecision::Keep);
    core.store.replace_findings("scan", &[review, other, kept]).unwrap();
    for id in ["review-npm", "xcode", "kept-pip"] {
        let r = core.cleanup.create_rule_plan(PACKAGE_CACHES, &[id.to_string()], now);
        assert!(matches!(r, Err(crate::cleanup::CleanupError::NotEligible(_))), "{id}: {r:?}");
    }
    assert!(core.cleanup.create_rule_plan("made_up_rule", &["xcode".to_string()], now).is_err());
}

/// A core over a fixture home with a fake Docker and the Docker detectors.
fn docker_core(fx: &crate::detectors::tests::Fx, clock: Arc<AtomicI64>) -> (Arc<AppCore>, Arc<crate::model::MemorySink>) {
    let sink = Arc::new(crate::model::MemorySink::default());
    let c = clock.clone();
    let core = AppCore::new(CoreConfig {
        store: Arc::new(Store::open_in_memory().unwrap()),
        runner: fx.runner.clone(),
        home: fx.home.clone(),
        volumes: Arc::new(FakeVolumes(Arc::new(AtomicI64::new((100 * GB) as i64)))),
        events: sink.clone(),
        detectors: vec![Box::new(crate::detectors::docker::BuildCache)],
        clock: Arc::new(move || c.load(Ordering::Relaxed)),
        platform: fx.platform,
        extra_protected: vec![],
        measure_outside_home: false,
    });
    (core, sink)
}

#[test]
#[cfg(unix)]
fn weekly_docker_rule_prunes_once_a_week_and_shows_in_the_logbook() {
    crate::detectors::docker::reset_snapshot_cache();
    let fx = crate::detectors::tests::Fx::new();
    let state = fx.docker(true);
    let clock = Arc::new(AtomicI64::new(1_800_000_000));
    let (core, sink) = docker_core(&fx, clock.clone());
    let prunes = || std::fs::read_to_string(state.join("actions")).unwrap_or_default().matches("builder prune").count();
    // Off: a scan prunes nothing.
    core.run_scan_blocking("s0", ScanKind::Quick);
    assert_eq!(prunes(), 0);
    core.set_tidy_rule(TidyRuleId::DockerBuildCache, true).unwrap();
    core.run_scan_blocking("s1", ScanKind::Quick);
    assert_eq!(prunes(), 1, "one prune once the rule is on");
    let ran = sink.events(EV_TIDY_RULE_RAN);
    assert_eq!(ran.len(), 1);
    assert_eq!(ran[0]["ruleId"], DOCKER_BUILD_CACHE);
    assert_eq!(ran[0]["result"], "done");
    let page = core.list_logbook(0).unwrap();
    assert_eq!(page.runs[0].rule.as_deref(), Some(DOCKER_BUILD_CACHE));
    // A day later: not due.
    clock.fetch_add(DAY, Ordering::Relaxed);
    core.run_scan_blocking("s2", ScanKind::Quick);
    assert_eq!(prunes(), 1);
    // A week after the first run: due again.
    clock.fetch_add(WEEK - DAY, Ordering::Relaxed);
    core.run_scan_blocking("s3", ScanKind::Quick);
    assert_eq!(prunes(), 2);
    let rules = core.get_tidy_rules().unwrap();
    assert_eq!(rules.iter().find(|r| r.id == DOCKER_BUILD_CACHE).unwrap().last_result.as_deref(), Some("done"));
}
