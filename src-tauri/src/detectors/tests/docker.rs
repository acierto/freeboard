use super::*;
use crate::detectors::docker::*;
use crate::detectors::{probe, Probe};

fn docker_dets() -> Vec<Box<dyn Detector>> {
    vec![Box::new(BuildCache), Box::new(DanglingImages), Box::new(UnusedImages), Box::new(StoppedContainers), Box::new(Volumes)]
}

#[test]
fn classifies_fixture_state() {
    let fx = Fx::new();
    fx.docker(true);
    let o = fx.scan(ScanKind::Quick, &docker_dets());
    assert!(o.errors.is_empty(), "{:?}", o.errors);

    let bc = by_detector(&o, BUILD_CACHE);
    assert_eq!(bc.len(), 1);
    assert_eq!(bc[0].size_bytes, 14_800_000_000);
    assert_eq!(bc[0].risk, Risk::Safe);
    assert!(bc[0].preselected());

    let dang = find(&o, DANGLING);
    assert_eq!(dang.suggestion.risk, Risk::Safe);
    let Target::Native { args, resources, .. } = &dang.target else { panic!() };
    // dd3 is referenced by the stopped "recent" container, so it is excluded.
    assert_eq!(resources, &vec!["sha256:dd1".to_string(), "sha256:dd2".to_string()]);
    assert_eq!(args[..2], ["image".to_string(), "rm".to_string()]);
    assert_eq!(dang.suggestion.size_bytes, 6_200_000_000);

    // Images referenced by stopped containers are used.
    let unused = by_detector(&o, UNUSED);
    assert_eq!(unused.len(), 1);
    assert_eq!(unused[0].title, "postgres:14 image");
    assert_eq!(unused[0].risk, Risk::Review);
    assert!(!unused[0].preselected());
    assert_eq!(unused[0].action_label, "docker image rm postgres:14");

    // Only the container stopped for 14+ days.
    let st = find(&o, STOPPED);
    let Target::Native { resources, .. } = &st.target else { panic!() };
    assert_eq!(resources.len(), 1);
    assert!(resources[0].starts_with("c2"));
    assert_eq!(st.suggestion.risk, Risk::Review);
    assert_eq!(st.suggestion.size_bytes, 1_100_000_000);

    // Volumes mounted by any container (even stopped) are attached.
    let vols = by_detector(&o, VOLUMES);
    assert_eq!(vols.len(), 2, "one named volume and one group of unnamed volumes");
    assert_eq!(vols[0].title, "Volume billing_pgdata");
    assert_eq!(vols[0].size_bytes, 7_600_000_000);
}

#[test]
fn inv10_volumes_never_preselected_and_need_ack() {
    let fx = Fx::new();
    fx.docker(true);
    let o = fx.scan(ScanKind::Quick, &docker_dets());
    for v in by_detector(&o, VOLUMES) {
        assert_eq!(v.risk, Risk::Risky);
        assert!(v.needs_confirmation);
        assert!(!v.recoverable);
        assert!(!v.preselected());
        assert!(v.ack_name.is_some());
        assert_eq!(v.recovery, "Not recoverable.");
    }
}

#[test]
fn daemon_down_marks_all_docker_detectors_unavailable_and_others_continue() {
    let fx = Fx::new();
    fx.docker(false);
    fx.write("Downloads/old.dmg", b"x");
    age(&fx.home.join("Downloads"), 200);
    let o = fx.scan_all(ScanKind::Quick);
    let docker_errs: Vec<_> = o.errors.iter().filter(|e| e.detector.starts_with("docker.")).collect();
    assert_eq!(docker_errs.len(), 5, "{:?}", o.errors);
    assert!(docker_errs.iter().all(|e| e.kind == "unavailable" && e.message.contains("not running")));
    assert!(o.findings.iter().all(|f| !f.suggestion.detector.starts_with("docker.")));
    assert_eq!(by_detector(&o, "files.installers").len(), 1, "unrelated detectors still produce results");
}

#[test]
fn missing_docker_is_unavailable() {
    let fx = Fx::new();
    let o = fx.scan(ScanKind::Quick, &docker_dets());
    assert_eq!(o.errors.len(), 5);
    assert!(o.errors.iter().all(|e| e.kind == "unavailable" && e.message == "Docker is not installed."));
}

#[test]
fn inv11_timeout_is_unavailable_not_safe() {
    let mut fx = Fx::new();
    let s = fx.docker(true);
    std::fs::write(s.join("sleep_all"), "3").unwrap();
    fx.runner = fx.runner.clone().with_timeout(Duration::from_millis(300));
    let o = fx.scan(ScanKind::Quick, &docker_dets());
    assert!(o.findings.is_empty());
    assert!(o.errors.iter().all(|e| e.kind == "timeout"), "{:?}", o.errors);
}

#[test]
fn malformed_json_is_reported() {
    let fx = Fx::new();
    let s = fx.docker(true);
    std::fs::write(s.join("containers.json"), "{not json").unwrap();
    let o = fx.scan(ScanKind::Quick, &docker_dets());
    assert!(o.findings.is_empty());
    assert!(o.errors.iter().all(|e| e.kind == "malformed"), "{:?}", o.errors);
}

#[test]
fn scans_run_only_read_only_docker_commands() {
    let fx = Fx::new();
    let s = fx.docker(true);
    fx.scan(ScanKind::Deep, &docker_dets());
    assert!(!s.join("actions").exists(), "a scan must never run a mutating docker command");
}

#[test]
fn probe_detects_container_using_image_and_volume() {
    let fx = Fx::new();
    let s = fx.docker(true);
    let o = fx.scan(ScanKind::Quick, &docker_dets());
    let img = find(&o, UNUSED).clone();
    let vol = find(&o, VOLUMES).clone();
    let st = find(&o, STOPPED).clone();
    fx.with_ctx(ScanKind::Quick, |ctx| {
        assert_eq!(probe(&img, ctx), Probe::Unchanged);
        assert_eq!(probe(&vol, ctx), Probe::Unchanged);
        assert_eq!(probe(&st, ctx), Probe::Unchanged);
    });
    // A new container now uses postgres:14 and mounts billing_pgdata; the
    // stopped container was started.
    let c = std::fs::read_to_string(s.join("containers.json")).unwrap();
    let c = c.replacen(
        "[\n",
        "[\n  {\"Id\":\"c9\",\"Image\":\"sha256:ccc3\",\"State\":{\"Status\":\"created\",\"Running\":false},\"Mounts\":[{\"Type\":\"volume\",\"Name\":\"billing_pgdata\"}]},\n",
        1,
    );
    let c = c.replace("\"Status\":\"exited\",\"Running\":false,\"FinishedAt\":\"2026-09-01", "\"Status\":\"running\",\"Running\":true,\"FinishedAt\":\"2026-09-01");
    std::fs::write(s.join("containers.json"), c).unwrap();
    fx.with_ctx(ScanKind::Quick, |ctx| {
        assert_eq!(probe(&img, ctx), Probe::Changed("A container now uses this image.".into()));
        assert_eq!(probe(&vol, ctx), Probe::Changed("A container now uses this volume.".into()));
        assert!(matches!(probe(&st, ctx), Probe::Changed(r) if r.contains("started")));
    });
    // Daemon gone: unknown, never "unchanged".
    std::fs::write(s.join("down"), "").unwrap();
    fx.with_ctx(ScanKind::Quick, |ctx| assert!(matches!(probe(&img, ctx), Probe::Unknown(_))));
}

#[test]
fn build_cache_growth_is_detected() {
    let fx = Fx::new();
    let s = fx.docker(true);
    let o = fx.scan(ScanKind::Quick, &docker_dets());
    let bc = find(&o, BUILD_CACHE).clone();
    let Target::Native { args, .. } = &bc.target else { panic!() };
    assert!(args.contains(&UNTIL_PLACEHOLDER.to_string()), "prune is bounded to cache unused at scan time");
    let df = std::fs::read_to_string(s.join("df.jsonl")).unwrap().replace("\"Reclaimable\":\"14.8GB\"", "\"Reclaimable\":\"30GB\"");
    std::fs::write(s.join("df.jsonl"), df).unwrap();
    fx.with_ctx(ScanKind::Quick, |ctx| assert!(matches!(probe(&bc, ctx), Probe::Changed(_))));
}

#[test]
fn named_volume_shows_compose_project_and_creation_date() {
    let fx = Fx::new();
    fx.docker(true);
    let o = fx.scan(ScanKind::Quick, &docker_dets());
    let v = by_detector(&o, VOLUMES).into_iter().find(|v| v.title == "Volume billing_pgdata").unwrap();
    assert_eq!(v.location, "Compose project billing, not attached to a container");
    assert_eq!(v.last_used_label, "Created 63 days ago. Docker does not record later use");
    assert_eq!(v.ack_name.as_deref(), Some("billing_pgdata volume"));
}

#[test]
fn unnamed_volumes_form_one_risky_group_with_one_acknowledgment() {
    let fx = Fx::new();
    fx.docker(true);
    let o = fx.scan(ScanKind::Quick, &docker_dets());
    let found: Vec<_> = o.findings.iter().filter(|f| f.suggestion.detector == VOLUMES && f.suggestion.title.contains("unnamed")).collect();
    assert_eq!(found.len(), 1);
    let g = &found[0];
    let s = &g.suggestion;
    assert_eq!(s.title, "2 unnamed volumes");
    assert_eq!(s.size_bytes, 500_000_000);
    assert_eq!(s.last_used_label, "Created 14 to 33 days ago");
    assert_eq!((s.risk, s.needs_confirmation, s.recoverable, s.preselected()), (Risk::Risky, true, false, false));
    assert_eq!(s.ack_name.as_deref(), Some("2 unnamed Docker volumes"));
    let Target::Native { args, resources, .. } = &g.target else { panic!() };
    assert_eq!(&args[..2], &["volume".to_string(), "rm".to_string()]);
    assert_eq!(resources.len(), 2);
    assert!(!args.iter().any(|a| a.starts_with('-')), "never a force flag: {args:?}");
}

#[test]
fn unnamed_group_is_skipped_when_any_volume_changes() {
    let fx = Fx::new();
    fx.docker(true);
    let o = fx.scan(ScanKind::Quick, &docker_dets());
    let g = o.findings.iter().find(|f| f.suggestion.title == "2 unnamed volumes").unwrap().clone();
    fx.with_ctx(ScanKind::Quick, |ctx| assert_eq!(probe(&g, ctx), Probe::Unchanged));
    // One volume disappears: the whole group is skipped.
    let vols = fx.root.join("docker-state/volumes.jsonl");
    let kept: String = std::fs::read_to_string(&vols).unwrap().lines().filter(|l| !l.contains("bbbbbbbb")).map(|l| format!("{l}\n")).collect();
    std::fs::write(&vols, kept).unwrap();
    fx.with_ctx(ScanKind::Quick, |ctx| assert_eq!(probe(&g, ctx), Probe::Changed("A volume in this group no longer exists.".into())));
}

#[test]
fn unused_images_are_one_item_with_a_part_per_image() {
    // Decided by the user 2026-11-06: one entry, Choose which for the images.
    let fx = Fx::new();
    fx.docker(true);
    let o = fx.scan(ScanKind::Quick, &docker_dets());
    let one = find(&o, UNUSED).clone();
    let mut other = one.clone();
    other.suggestion.id = "other-image".into();
    other.suggestion.title = "redis:7 image".into();
    other.suggestion.size_bytes = one.suggestion.size_bytes * 3;
    fx.with_ctx(ScanKind::Quick, |ctx| {
        let merged = merge_unused(ctx, vec![one.clone(), other.clone()]);
        assert_eq!(merged.len(), 1);
        let s = &merged[0].suggestion;
        assert_eq!(s.title, "Unused Docker images");
        assert_eq!(s.location, "2 images no container uses");
        assert_eq!(s.risk, Risk::Review);
        assert!(!s.preselected(), "never added automatically");
        let labels: Vec<_> = s.parts.iter().map(|p| p.label.as_str()).collect();
        assert_eq!(labels, vec!["redis:7", "postgres:14"], "largest first");
        assert_eq!(s.size_bytes, one.suggestion.size_bytes * 4);
        assert_eq!(merged[0].parts.len(), 2, "each image keeps its own finding for its pre-cleanup check");
        assert_eq!(probe(&merged[0].parts[1], ctx), Probe::Unchanged);
        // A single unused image stays a plain item.
        assert!(merge_unused(ctx, vec![one.clone()])[0].suggestion.parts.is_empty());
    });
}
