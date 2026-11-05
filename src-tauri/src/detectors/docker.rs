//! Docker detectors: build cache, dangling images, unused tagged images,
//! stopped containers and unattached volumes.
//!
//! All five share one read-only snapshot per scan. `docker version` is checked
//! first; if the CLI is missing or the daemon is down, every Docker detector
//! reports "unavailable" and the other detectors continue.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::Value;

use super::common::*;
use super::Probe;
use crate::model::*;
use crate::runner::Cmd;
use crate::scan::{Detector, ScanContext};

pub const STOPPED_DAYS: i64 = 14;

#[derive(Debug, Clone, Default)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image_id: String,
    pub status: String,
    pub running: bool,
    pub finished_at: Option<i64>,
    pub created: Option<i64>,
    pub volumes: Vec<String>,
    pub size_rw: u64,
}

impl Container {
    pub fn stopped(&self) -> bool {
        !self.running && matches!(self.status.as_str(), "exited" | "created" | "dead")
    }
    pub fn stopped_since(&self) -> Option<i64> {
        self.finished_at.or(self.created)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Image {
    pub id: String,
    pub tags: Vec<String>,
    pub size: u64,
    pub created: Option<i64>,
}

impl Image {
    pub fn dangling(&self) -> bool {
        self.tags.iter().all(|t| t == "<none>:<none>" || t.is_empty())
    }
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub containers: Vec<Container>,
    pub images: Vec<Image>,
    pub volumes: Vec<String>,
    pub volume_sizes: HashMap<String, u64>,
    pub volume_meta: HashMap<String, VolumeMeta>,
    pub build_cache_reclaimable: Option<u64>,
}

/// What Docker records about a volume besides its name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VolumeMeta {
    /// Created by Docker for a container (label `com.docker.volume.anonymous`).
    pub anonymous: bool,
    /// Docker Compose project (label `com.docker.compose.project`).
    pub project: Option<String>,
    /// Creation time from `docker volume inspect`.
    pub created: Option<i64>,
}

/// `docker volume ls` prints labels as `k=v,k2=v2`.
fn parse_labels(s: &str) -> HashMap<String, String> {
    s.split(',')
        .filter(|kv| !kv.is_empty())
        .map(|kv| match kv.split_once('=') {
            Some((k, v)) => (k.trim().to_string(), v.to_string()),
            None => (kv.trim().to_string(), String::new()),
        })
        .collect()
}

impl Snapshot {
    pub fn image_used(&self, id: &str) -> bool {
        self.containers.iter().any(|c| c.image_id == id)
    }
    pub fn volume_used(&self, name: &str) -> bool {
        self.containers.iter().any(|c| c.volumes.iter().any(|v| v == name))
    }
}

fn read(ctx: &ScanContext, det: &str, args: &[&str]) -> Result<String, DetectorError> {
    let out = ctx.runner.run_read(&Cmd::new("docker", args.iter().copied())).map_err(|e| run_err(det, &e))?;
    if !out.success() {
        let msg = out.stderr.lines().next().unwrap_or("docker failed").to_string();
        return Err(err(det, "error", msg));
    }
    Ok(out.stdout)
}

fn json_lines(det: &str, s: &str) -> Result<Vec<Value>, DetectorError> {
    s.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Value>(l).map_err(|_| err(det, "malformed", "Docker returned output Freeboard could not read.")))
        .collect()
}

fn json_array(det: &str, s: &str) -> Result<Vec<Value>, DetectorError> {
    match serde_json::from_str::<Value>(s) {
        Ok(Value::Array(a)) => Ok(a),
        _ => Err(err(det, "malformed", "Docker returned output Freeboard could not read.")),
    }
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string()
}

/// Check `docker version`: the CLI must exist and the daemon must answer.
pub fn check_version(ctx: &ScanContext, det: &str) -> Result<(), DetectorError> {
    let out = ctx.runner.run_read(&Cmd::new("docker", ["version", "--format", "{{json .}}"])).map_err(|e| match e {
        crate::runner::RunError::NotFound(_) => err(det, "unavailable", "Docker is not installed."),
        e => run_err(det, &e),
    })?;
    let v: Value = serde_json::from_str(out.stdout.trim())
        .map_err(|_| err(det, "unavailable", "Docker is installed but not running."))?;
    if v.get("Server").is_none_or(|s| s.is_null()) {
        return Err(err(det, "unavailable", "Docker is installed but not running."));
    }
    Ok(())
}

/// Read the full Docker state (read-only commands only).
pub fn load(ctx: &ScanContext, det: &str) -> Result<Snapshot, DetectorError> {
    check_version(ctx, det)?;
    let mut snap = Snapshot::default();
    // Independent read-only queries run concurrently; parsing below is unchanged.
    // Docker 29 and later leave untagged (dangling) images out of a plain
    // `image ls`, so they are listed with their own filter and merged.
    // (`--all` would add intermediate build layers, which are not dangling.)
    let (ps_out, images_out, dangling_out, volumes_out, df_out, dfv_out) = std::thread::scope(|sc| {
        let ps = sc.spawn(|| read(ctx, det, &["ps", "-a", "--no-trunc", "--format", "{{json .}}"]));
        let im = sc.spawn(|| read(ctx, det, &["image", "ls", "--no-trunc", "--format", "{{json .}}"]));
        let da = sc.spawn(|| {
            read(ctx, det, &["image", "ls", "--no-trunc", "--filter", "dangling=true", "--format", "{{json .}}"])
        });
        let vo = sc.spawn(|| read(ctx, det, &["volume", "ls", "--format", "{{json .}}"]));
        let df = sc.spawn(|| read(ctx, det, &["system", "df", "--format", "{{json .}}"]));
        let dv = sc.spawn(|| read(ctx, det, &["system", "df", "-v", "--format", "{{json .}}"]));
        let j = |h: std::thread::ScopedJoinHandle<'_, Result<String, DetectorError>>| {
            h.join().unwrap_or_else(|_| Err(err(det, "error", "A Docker query stopped unexpectedly.")))
        };
        (j(ps), j(im), j(da), j(vo), j(df), j(dv))
    });

    // Containers, including stopped ones.
    let ids: Vec<String> = json_lines(det, &ps_out?)?
        .iter()
        .map(|v| s(v, "ID"))
        .filter(|i| !i.is_empty())
        .collect();
    if !ids.is_empty() {
        let mut args = vec!["container", "inspect", "--size"];
        args.extend(ids.iter().map(|s| s.as_str()));
        for c in json_array(det, &read(ctx, det, &args)?)? {
            let state = c.get("State").cloned().unwrap_or(Value::Null);
            let volumes = c
                .get("Mounts")
                .and_then(|m| m.as_array())
                .map(|a| {
                    a.iter()
                        .filter(|m| s(m, "Type") == "volume")
                        .map(|m| s(m, "Name"))
                        .filter(|n| !n.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            let id = s(&c, "Id");
            if id.is_empty() {
                return Err(err(det, "malformed", "A Docker container had no ID."));
            }
            snap.containers.push(Container {
                id,
                name: s(&c, "Name").trim_start_matches('/').to_string(),
                image_id: s(&c, "Image"),
                status: s(&state, "Status"),
                running: state.get("Running").and_then(|r| r.as_bool()).unwrap_or(true),
                finished_at: parse_time(&s(&state, "FinishedAt")),
                created: parse_time(&s(&c, "Created")),
                volumes,
                size_rw: c.get("SizeRw").and_then(|x| x.as_u64()).unwrap_or(0),
            });
        }
    }

    // Top-level images.
    let mut image_lines = json_lines(det, &images_out?)?;
    image_lines.extend(json_lines(det, &dangling_out?)?);
    let img_ids: Vec<String> = image_lines
        .iter()
        .map(|v| s(v, "ID"))
        .filter(|i| !i.is_empty())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    if !img_ids.is_empty() {
        let mut args = vec!["image", "inspect"];
        args.extend(img_ids.iter().map(|s| s.as_str()));
        for i in json_array(det, &read(ctx, det, &args)?)? {
            let id = s(&i, "Id");
            if id.is_empty() {
                return Err(err(det, "malformed", "A Docker image had no ID."));
            }
            snap.images.push(Image {
                id,
                tags: i
                    .get("RepoTags")
                    .and_then(|t| t.as_array())
                    .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                size: i.get("Size").and_then(|x| x.as_u64()).unwrap_or(0),
                created: parse_time(&s(&i, "Created")),
            });
        }
    }

    // Volumes, with labels (anonymous, Compose project).
    for v in json_lines(det, &volumes_out?)? {
        let name = s(&v, "Name");
        if name.is_empty() {
            continue;
        }
        let labels = parse_labels(&s(&v, "Labels"));
        let meta = VolumeMeta {
            anonymous: labels.contains_key("com.docker.volume.anonymous"),
            project: labels.get("com.docker.compose.project").filter(|p| !p.is_empty()).cloned(),
            created: None,
        };
        snap.volume_meta.insert(name.clone(), meta);
        snap.volumes.push(name);
    }
    // Creation times (read-only `docker volume inspect`); optional: without
    // them the rows say Docker does not record use.
    let names: Vec<&str> = snap.volumes.iter().map(|n| n.as_str()).filter(|n| safe_arg(n)).collect();
    for chunk in names.chunks(200) {
        let mut args = vec!["volume", "inspect", "--format", "{{json .}}"];
        args.extend(chunk.iter().copied());
        if let Ok(out) = read(ctx, det, &args) {
            for v in json_lines(det, &out).unwrap_or_default() {
                if let Some(m) = snap.volume_meta.get_mut(&s(&v, "Name")) {
                    m.created = parse_time(&s(&v, "CreatedAt"));
                }
            }
        }
    }

    // Sizes: build cache and per-volume usage (estimates from `docker system df`).
    if let Ok(out) = df_out {
        for row in json_lines(det, &out).unwrap_or_default() {
            if s(&row, "Type") == "Build Cache" {
                snap.build_cache_reclaimable = parse_human(&s(&row, "Reclaimable"));
            }
        }
    }
    if let Ok(out) = dfv_out {
        if let Ok(v) = serde_json::from_str::<Value>(out.trim()) {
            for vol in v.get("Volumes").and_then(|x| x.as_array()).cloned().unwrap_or_default() {
                if let Some(sz) = parse_human(&s(&vol, "Size")) {
                    snap.volume_sizes.insert(s(&vol, "Name"), sz);
                }
            }
        }
    }
    snap.containers.sort_by(|a, b| a.id.cmp(&b.id));
    snap.images.sort_by(|a, b| a.id.cmp(&b.id));
    snap.volumes.sort();
    Ok(snap)
}

type Slot = Mutex<Option<(usize, i64, Result<Arc<Snapshot>, DetectorError>)>>;
fn cache() -> &'static Slot {
    static C: OnceLock<Slot> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

/// One snapshot per scan, shared by the five Docker detectors. The lock is
/// held while loading so detectors running in parallel wait for the first
/// load instead of each querying Docker again.
fn shared(ctx: &ScanContext, det: &str) -> Result<Arc<Snapshot>, DetectorError> {
    let key = ctx.cancel as *const _ as usize;
    let mut c = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some((k, now, r)) = c.as_ref() {
        if *k == key && *now == ctx.now {
            return r.clone().map_err(|mut e| {
                e.detector = det.into();
                e
            });
        }
    }
    let r = load(ctx, det).map(Arc::new);
    *c = Some((key, ctx.now, r.clone()));
    r
}

fn short(id: &str) -> &str {
    let id = id.strip_prefix("sha256:").unwrap_or(id);
    &id[..id.len().min(12)]
}

fn list_label(prefix: &str, ids: &[String]) -> String {
    let shown: Vec<&str> = ids.iter().take(3).map(|i| short(i)).collect();
    let more = if ids.len() > 3 { format!(" and {} more", ids.len() - 3) } else { String::new() };
    format!("{prefix} {}{more}", shown.join(" "))
}

fn safe_arg(a: &str) -> bool {
    !a.is_empty() && !a.starts_with('-')
}

// ----- detectors -------------------------------------------------------------

pub struct BuildCache;
pub struct DanglingImages;
pub struct UnusedImages;
pub struct StoppedContainers;
pub struct Volumes;

pub const BUILD_CACHE: &str = "docker.build_cache";
pub const DANGLING: &str = "docker.dangling_images";
pub const UNUSED: &str = "docker.unused_images";
pub const STOPPED: &str = "docker.stopped_containers";
pub const VOLUMES: &str = "docker.volumes";

/// Placeholder replaced at execution time with the age of the scan, so the
/// prune only touches cache that was already unused when the plan was made.
pub const UNTIL_PLACEHOLDER: &str = "until={scan_age}";

fn desktop_note(ctx: &ScanContext) -> &'static str {
    if ctx.platform == "linux" {
        ""
    } else {
        " Docker Desktop may keep the space in its virtual disk for a while."
    }
}

pub fn build_cache_findings(ctx: &ScanContext, snap: &Snapshot) -> Vec<Finding> {
    let Some(bytes) = snap.build_cache_reclaimable.filter(|b| *b > 0) else { return vec![] };
    let mut s = base(BUILD_CACHE, Category::Docker, "build-cache");
    s.title = "Docker build cache".into();
    s.location = "Unused build cache".into();
    s.size_bytes = bytes;
    s.size_is_estimate = true;
    s.last_used_label = "Not used by any current build".into();
    s.risk = Risk::Safe;
    s.action = Action::NativeCommand;
    s.action_label = "docker builder prune".into();
    s.reason = format!("Build layers that no recent build has reused.{}", desktop_note(ctx));
    s.recovery = "Docker rebuilds the layers on the next build.".into();
    s.recoverable = true;
    s.fingerprint = fingerprint_of(&["build-cache", &bytes.to_string()]);
    vec![Finding {
        suggestion: s,
        target: Target::Native {
            program: "docker".into(),
            args: vec!["builder".into(), "prune".into(), "--force".into(), "--filter".into(), UNTIL_PLACEHOLDER.into()],
            resources: vec![format!("scanned_at={}", ctx.now), format!("bytes={bytes}")],
            cwd: None,
        },
    }]
}

pub fn dangling_findings(ctx: &ScanContext, snap: &Snapshot) -> Vec<Finding> {
    let imgs: Vec<&Image> = snap.images.iter().filter(|i| i.dangling() && !snap.image_used(&i.id) && safe_arg(&i.id)).collect();
    if imgs.is_empty() {
        return vec![];
    }
    let ids: Vec<String> = imgs.iter().map(|i| i.id.clone()).collect();
    let oldest = imgs.iter().filter_map(|i| i.created).max();
    let mut s = base(DANGLING, Category::Docker, "dangling-images");
    s.title = "Untagged Docker images".into();
    s.location = format!("{} dangling image{}", ids.len(), if ids.len() == 1 { "" } else { "s" });
    s.size_bytes = imgs.iter().map(|i| i.size).sum();
    s.size_is_estimate = true;
    s.last_used_at = oldest.and_then(rfc3339);
    s.last_used_label = oldest.map(|t| format!("Newest created {}", ago(ctx.now, t))).unwrap_or_else(|| "Creation time unknown".into());
    s.risk = Risk::Safe;
    s.action = Action::NativeCommand;
    s.action_label = list_label("docker image rm", &ids);
    s.reason = format!("Old image layers with no tag and no container using them.{}", desktop_note(ctx));
    s.recovery = "Rebuild the image from its Dockerfile.".into();
    s.recoverable = true;
    let mut fp: Vec<&str> = vec!["dangling"];
    fp.extend(ids.iter().map(|s| s.as_str()));
    s.fingerprint = fingerprint_of(&fp);
    let mut args = vec!["image".to_string(), "rm".to_string()];
    args.extend(ids.iter().cloned());
    vec![Finding { suggestion: s, target: Target::Native { program: "docker".into(), args, resources: ids, cwd: None } }]
}

pub fn unused_findings(ctx: &ScanContext, snap: &Snapshot) -> Vec<Finding> {
    let mut out = vec![];
    for i in snap.images.iter().filter(|i| !i.dangling() && !snap.image_used(&i.id)) {
        let tags: Vec<String> = i.tags.iter().filter(|t| *t != "<none>:<none>").cloned().collect();
        if tags.iter().any(|t| !safe_arg(t)) {
            continue;
        }
        let name = tags.join(", ");
        let mut s = base(UNUSED, Category::Docker, &i.id);
        s.title = format!("{name} image");
        s.location = "Not used by any container".into();
        s.size_bytes = i.size;
        s.size_is_estimate = true;
        s.last_used_at = i.created.and_then(rfc3339);
        s.last_used_label = i.created.map(|t| format!("Created {}", ago(ctx.now, t))).unwrap_or_else(|| "Creation time unknown".into());
        s.risk = Risk::Review;
        s.action = Action::NativeCommand;
        s.action_label = format!("docker image rm {}", tags.join(" "));
        s.reason = format!("A tagged image that no container uses.{}", desktop_note(ctx));
        s.recovery = format!("Run docker pull {}, or build it again.", tags[0]);
        s.recoverable = true;
        let mut fp = vec![i.id.as_str()];
        fp.extend(tags.iter().map(|t| t.as_str()));
        s.fingerprint = fingerprint_of(&fp);
        let mut args = vec!["image".to_string(), "rm".to_string()];
        args.extend(tags.iter().cloned());
        out.push(Finding {
            suggestion: s,
            target: Target::Native { program: "docker".into(), args, resources: vec![i.id.clone()], cwd: None },
        });
    }
    out
}

pub fn stopped_findings(ctx: &ScanContext, snap: &Snapshot) -> Vec<Finding> {
    let old: Vec<&Container> = snap
        .containers
        .iter()
        .filter(|c| c.stopped() && safe_arg(&c.id) && c.stopped_since().is_some_and(|t| ctx.now - t >= STOPPED_DAYS * DAY))
        .collect();
    if old.is_empty() {
        return vec![];
    }
    let ids: Vec<String> = old.iter().map(|c| c.id.clone()).collect();
    let newest = old.iter().filter_map(|c| c.stopped_since()).max();
    let mut s = base(STOPPED, Category::Docker, "stopped-containers");
    s.title = "Stopped containers".into();
    s.location = format!("{} container{}", ids.len(), if ids.len() == 1 { "" } else { "s" });
    s.size_bytes = old.iter().map(|c| c.size_rw).sum();
    s.last_used_at = newest.and_then(rfc3339);
    s.last_used_label = format!("Stopped over {STOPPED_DAYS} days ago");
    s.risk = Risk::Review;
    s.action = Action::NativeCommand;
    s.action_label = list_label("docker container rm", &ids);
    s.reason = "Their writable layers still take up space.".into();
    s.recovery = "Create them again. Files written inside them are lost.".into();
    s.recoverable = true;
    let mut fp: Vec<String> = vec!["stopped".into()];
    fp.extend(old.iter().map(|c| format!("{}|{:?}|{}", c.id, c.finished_at, c.status)));
    s.fingerprint = fingerprint_of(&fp.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let mut args = vec!["container".to_string(), "rm".to_string()];
    args.extend(ids.iter().cloned());
    vec![Finding { suggestion: s, target: Target::Native { program: "docker".into(), args, resources: ids, cwd: None } }]
}

pub fn volume_findings(ctx: &ScanContext, snap: &Snapshot) -> Vec<Finding> {
    let mut out = vec![];
    let unused: Vec<&String> = snap.volumes.iter().filter(|v| !snap.volume_used(v)).collect();
    let meta = |v: &str| snap.volume_meta.get(v).cloned().unwrap_or_default();
    // Named volumes: one row each, with the Compose project and creation date.
    for v in unused.iter().filter(|v| !meta(v).anonymous || !safe_arg(v)) {
        let m = meta(v);
        let mut s = base(VOLUMES, Category::Docker, v);
        s.title = format!("Volume {v}");
        s.location = match &m.project {
            Some(p) => format!("Compose project {p}, not attached to a container"),
            None => "Not attached to a container".into(),
        };
        s.size_bytes = snap.volume_sizes.get(v.as_str()).copied().unwrap_or(0);
        s.size_is_estimate = true;
        s.last_used_label = match m.created {
            Some(t) => format!("Created {}. Docker does not record later use", ago(ctx.now, t)),
            None => "Docker does not record when a volume was last used".into(),
        };
        s.risk = Risk::Risky;
        s.action = Action::NativeCommand;
        s.action_label = format!("docker volume rm {v}");
        s.reason = "Not attached to a container, but it may hold the only copy of this data.".into();
        s.recovery = "Not recoverable.".into();
        s.recoverable = false;
        s.needs_confirmation = true;
        s.ack_name = Some(format!("{v} volume"));
        if !safe_arg(v) {
            s.blocked_reason = Some("This volume name can't be passed to Docker safely.".into());
        }
        s.fingerprint = fingerprint_of(&["volume", v]);
        out.push(Finding {
            suggestion: s,
            target: Target::Native {
                program: "docker".into(),
                args: vec!["volume".into(), "rm".into(), v.to_string()],
                resources: vec![v.to_string()],
                cwd: None,
            },
        });
    }
    // Unnamed volumes: Docker created them for containers that are gone.
    // Dozens of hash-named rows told the user nothing (2026-11-05), so they
    // form one group with one acknowledgment. Still Risky, never preselected.
    let anon: Vec<String> = unused.iter().filter(|v| meta(v).anonymous && safe_arg(v)).map(|v| v.to_string()).collect();
    if !anon.is_empty() {
        let n = anon.len();
        let mut s = base(VOLUMES, Category::Docker, "unnamed-volumes");
        s.title = if n == 1 { "1 unnamed volume".into() } else { format!("{n} unnamed volumes") };
        s.location = "Created by Docker for containers that no longer exist".into();
        s.size_bytes = anon.iter().map(|v| snap.volume_sizes.get(v).copied().unwrap_or(0)).sum();
        s.size_is_estimate = true;
        let created: Vec<i64> = anon.iter().filter_map(|v| meta(v).created).collect();
        s.last_used_label = match (created.iter().min(), created.iter().max()) {
            (Some(&old), Some(&new)) if days_since(ctx.now, old) != days_since(ctx.now, new) => {
                format!("Created {} to {} days ago", days_since(ctx.now, new), days_since(ctx.now, old))
            }
            (Some(&old), _) => format!("Created {}", ago(ctx.now, old)),
            _ => "Docker does not record when a volume was last used".into(),
        };
        s.risk = Risk::Risky;
        s.action = Action::NativeCommand;
        s.action_label = if n == 1 { format!("docker volume rm {}", anon[0]) } else { format!("docker volume rm (the {n} volumes)") };
        s.reason = "No container uses them. Docker doesn't name or describe them, so it can't tell what is inside; one may still hold the only copy of some data.".into();
        s.recovery = "Not recoverable.".into();
        s.recoverable = false;
        s.needs_confirmation = true;
        s.ack_name = Some(if n == 1 { "unnamed Docker volume".into() } else { format!("{n} unnamed Docker volumes") });
        let mut fp: Vec<&str> = vec!["unnamed-volumes"];
        fp.extend(anon.iter().map(|v| v.as_str()));
        s.fingerprint = fingerprint_of(&fp);
        let mut args = vec!["volume".to_string(), "rm".to_string()];
        args.extend(anon.iter().cloned());
        out.push(Finding { suggestion: s, target: Target::Native { program: "docker".into(), args, resources: anon, cwd: None } });
    }
    out
}

macro_rules! docker_detector {
    ($t:ty, $id:expr, $f:ident) => {
        impl Detector for $t {
            fn id(&self) -> &'static str {
                $id
            }
            fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
                let snap = shared(ctx, $id)?;
                Ok($f(ctx, &snap))
            }
        }
    };
}
docker_detector!(BuildCache, BUILD_CACHE, build_cache_findings);
docker_detector!(DanglingImages, DANGLING, dangling_findings);
docker_detector!(UnusedImages, UNUSED, unused_findings);
docker_detector!(StoppedContainers, STOPPED, stopped_findings);
docker_detector!(Volumes, VOLUMES, volume_findings);

// ----- pre-cleanup checks ----------------------------------------------------

/// Re-read Docker state and confirm the resources are unchanged and unused.
pub fn probe(f: &Finding, ctx: &ScanContext) -> Probe {
    let det = f.suggestion.detector.as_str();
    let snap = match load(ctx, det) {
        Ok(s) => s,
        Err(e) => return Probe::Unknown(format!("Docker could not be checked: {}", e.message)),
    };
    let Target::Native { resources, .. } = &f.target else {
        return Probe::Unknown("Unexpected cleanup target.".into());
    };
    match det {
        BUILD_CACHE => {
            let before: u64 = resources.iter().find_map(|r| r.strip_prefix("bytes=")).and_then(|b| b.parse().ok()).unwrap_or(0);
            match snap.build_cache_reclaimable {
                None => Probe::Unknown("Docker did not report its build cache.".into()),
                Some(now) if now > before + before / 10 + 100_000_000 => {
                    Probe::Changed("More build cache became unused after the scan. Scan again to include it.".into())
                }
                Some(_) => Probe::Unchanged,
            }
        }
        DANGLING => {
            for id in resources {
                match snap.images.iter().find(|i| &i.id == id) {
                    None => return Probe::Changed("An image in this group no longer exists.".into()),
                    Some(i) if !i.dangling() => return Probe::Changed("An image in this group was tagged after the scan.".into()),
                    Some(i) if snap.image_used(&i.id) => return Probe::Changed("A container now uses an image in this group.".into()),
                    _ => {}
                }
            }
            Probe::Unchanged
        }
        UNUSED => {
            let id = &resources[0];
            match snap.images.iter().find(|i| &i.id == id) {
                None => Probe::Changed("The image no longer exists.".into()),
                Some(i) if snap.image_used(&i.id) => Probe::Changed("A container now uses this image.".into()),
                Some(i) => {
                    let mut fp = vec![i.id.as_str()];
                    fp.extend(i.tags.iter().filter(|t| *t != "<none>:<none>").map(|t| t.as_str()));
                    if fingerprint_of(&fp) != f.suggestion.fingerprint {
                        Probe::Changed("The image's tags changed after the scan.".into())
                    } else {
                        Probe::Unchanged
                    }
                }
            }
        }
        STOPPED => {
            let mut fp: Vec<String> = vec!["stopped".into()];
            for id in resources {
                match snap.containers.iter().find(|c| &c.id == id) {
                    None => return Probe::Changed("A container in this group no longer exists.".into()),
                    Some(c) if !c.stopped() => return Probe::Changed("A container in this group was started after the scan.".into()),
                    Some(c) => fp.push(format!("{}|{:?}|{}", c.id, c.finished_at, c.status)),
                }
            }
            if fingerprint_of(&fp.iter().map(|s| s.as_str()).collect::<Vec<_>>()) != f.suggestion.fingerprint {
                Probe::Changed("A container in this group ran after the scan.".into())
            } else {
                Probe::Unchanged
            }
        }
        VOLUMES => {
            // One volume, or the unnamed group: every volume must still exist
            // and still be unused, or nothing is removed.
            let group = resources.len() > 1;
            for name in resources {
                if !snap.volumes.contains(name) {
                    return Probe::Changed(if group { "A volume in this group no longer exists." } else { "The volume no longer exists." }.into());
                }
                if snap.volume_used(name) {
                    return Probe::Changed(if group { "A container now uses a volume in this group." } else { "A container now uses this volume." }.into());
                }
            }
            Probe::Unchanged
        }
        _ => Probe::Unknown("Unknown Docker detector.".into()),
    }
}
