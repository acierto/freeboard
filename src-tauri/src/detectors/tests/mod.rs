//! Fixture-only detector tests. Everything runs in temporary directories with
//! fake tool executables (`runner::fake::tool`). Real Git is used only to
//! build throwaway repositories inside the temporary directory.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::model::*;
use crate::runner::{fake, Runner};
use crate::scan::{run_scan, Detector, ScanContext, ScanOutcome};
use crate::settings::Settings;

#[cfg(unix)]
mod ai;
#[cfg(unix)]
mod app_caches;
#[cfg(unix)]
pub(crate) mod apps;
#[cfg(unix)]
mod caches;
#[cfg(unix)]
mod docker;
#[cfg(unix)]
mod dupes;
mod files;
#[cfg(unix)]
mod git;
#[cfg(unix)]
mod mobile;
#[cfg(unix)]
mod projects;
#[cfg(unix)]
mod scan;
#[cfg(unix)]
mod system;
#[cfg(unix)]
mod tools;

/// Test clock: 2026-11-04T00:00:00Z.
pub const NOW: i64 = 1_793_750_400;
pub const DAY: i64 = 86_400;

pub struct Fx {
    pub _tmp: tempfile::TempDir,
    pub root: PathBuf,
    pub home: PathBuf,
    pub bin: PathBuf,
    pub runner: Runner,
    pub settings: Settings,
    pub platform: &'static str,
    /// Simulated macOS Full Disk Access (None: a fixture home is readable).
    pub full_disk_access: Option<bool>,
}

fn real_dir_of(program: &str) -> Option<PathBuf> {
    Runner::default().resolve(program).and_then(|p| p.parent().map(Path::to_path_buf))
}

impl Fx {
    pub fn new() -> Fx {
        let t = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(t.path()).unwrap();
        let home = root.join("home");
        let bin = root.join("bin");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        // Fake bin first; real git available for building fixture repos and
        // for read-only git queries against them. Nothing else is real.
        let mut dirs = vec![bin.clone()];
        let gitbin = root.join("gitbin");
        std::fs::create_dir_all(&gitbin).unwrap();
        if let Some(g) = Runner::default().resolve("git") {
            #[cfg(unix)]
            std::os::unix::fs::symlink(&g, gitbin.join("git")).unwrap();
            let _ = g;
        }
        dirs.push(gitbin);
        let path = std::env::join_paths(dirs).unwrap();
        // Default fakes: no processes in any folder; no Gradle daemon.
        // `lsof -d cwd -Fpn`: one process, working in /, so nothing is in use.
        fake::tool(&bin, "lsof", "printf 'p1\\nfcwd\\nn/\\n'; exit 0");
        fake::tool(&bin, "ps", "exit 0");
        let mut settings = Settings::default();
        settings.scan_folders = vec![home.join("projects").to_string_lossy().to_string()];
        Fx { _tmp: t, root, home, bin, runner: Runner::with_path(path), settings, platform: crate::platform::platform_name(), full_disk_access: None }
    }

    pub fn tool(&self, name: &str, body: &str) {
        fake::tool(&self.bin, name, body);
    }

    pub fn with_ctx<R>(&self, kind: ScanKind, f: impl FnOnce(&ScanContext) -> R) -> R {
        let cancel = AtomicBool::new(false);
        let pause = || false;
        let progress = |_: &str, _: Option<&str>, _: u64| {};
        let ctx = ScanContext {
            runner: &self.runner,
            home: self.home.clone(),
            settings: self.settings.clone(),
            now: NOW,
            kind,
            cancel: &cancel,
            should_pause: &pause,
            progress: &progress,
            platform: self.platform,
            full_disk_access: self.full_disk_access,
        };
        f(&ctx)
    }

    pub fn scan(&self, kind: ScanKind, detectors: &[Box<dyn Detector>]) -> ScanOutcome {
        let out = self.with_ctx(kind, |ctx| run_scan(ctx, detectors));
        self.record_text(&out);
        out
    }

    /// With FREEBOARD_I18N_SAMPLES=<file>, append every text a person would
    /// read in this scan's findings (with risk notes) to that file. The
    /// samples are the corpus for the translation tests
    /// (src/i18n/catalog/samples.txt, tests/ui/i18n.test.ts). Text holding
    /// this fixture's temporary paths is left out.
    fn record_text(&self, out: &ScanOutcome) {
        let Ok(file) = std::env::var("FREEBOARD_I18N_SAMPLES") else { return };
        let root = self.root.to_string_lossy().to_string();
        let mut lines: Vec<String> = Vec::new();
        for f in &out.findings {
            let mut s = f.suggestion.clone();
            crate::risk_notes::annotate(&mut s);
            let mut texts = vec![s.title, s.location, s.last_used_label, s.action_label, s.reason, s.recovery];
            texts.extend(s.blocked_reason);
            texts.extend(s.skip_reason);
            texts.extend(s.risk_why);
            texts.extend(s.safer.into_iter().map(|x| x.label));
            texts.extend(s.parts.into_iter().map(|p| p.label));
            lines.extend(texts);
        }
        lines.extend(out.errors.iter().map(|e| e.message.clone()));
        let body: String = lines
            .into_iter()
            .filter(|t| !t.trim().is_empty() && !t.contains(&root) && !t.contains("/tmp") && !t.contains("/var/folders"))
            .map(|t| t.replace('\n', " ") + "\n")
            .collect();
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(file).unwrap();
        let _ = f.write_all(body.as_bytes());
    }

    pub fn scan_all(&self, kind: ScanKind) -> ScanOutcome {
        self.scan(kind, &crate::detectors::all())
    }

    pub fn write(&self, rel: &str, data: &[u8]) -> PathBuf {
        let p = self.home.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, data).unwrap();
        p
    }

    pub fn mkdir(&self, rel: &str) -> PathBuf {
        let p = self.home.join(rel);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// Fake docker reading state files copied from tests/fixtures/docker.
    pub fn docker(&self, up: bool) -> PathBuf {
        let s = self.root.join("docker-state");
        std::fs::create_dir_all(&s).unwrap();
        let files: [(&str, &str); 10] = [
            ("version_up.json", include_str!("../../../../tests/fixtures/docker/version_up.json")),
            ("version_down.json", include_str!("../../../../tests/fixtures/docker/version_down.json")),
            ("ps.jsonl", include_str!("../../../../tests/fixtures/docker/ps.jsonl")),
            ("containers.json", include_str!("../../../../tests/fixtures/docker/containers.json")),
            ("images.jsonl", include_str!("../../../../tests/fixtures/docker/images.jsonl")),
            ("images.json", include_str!("../../../../tests/fixtures/docker/images.json")),
            ("volumes.jsonl", include_str!("../../../../tests/fixtures/docker/volumes.jsonl")),
            ("df.jsonl", include_str!("../../../../tests/fixtures/docker/df.jsonl")),
            ("df_v.json", include_str!("../../../../tests/fixtures/docker/df_v.json")),
            ("volume_inspect.jsonl", include_str!("../../../../tests/fixtures/docker/volume_inspect.jsonl")),
        ];
        for (n, c) in files {
            std::fs::write(s.join(n), c).unwrap();
        }
        // One file per image so `image inspect` returns only the IDs asked for.
        std::fs::create_dir_all(s.join("img")).unwrap();
        let imgs: Vec<serde_json::Value> = serde_json::from_str(files[5].1).unwrap();
        for i in imgs {
            let id = i["Id"].as_str().unwrap().replace(':', "_");
            std::fs::write(s.join("img").join(format!("{id}.json")), i.to_string()).unwrap();
        }
        if !up {
            std::fs::write(s.join("down"), "").unwrap();
        }
        let body = format!(
            r#"S='{s}'
[ -f "$S/sleep_all" ] && sleep "$(cat "$S/sleep_all")"
case "$1" in
version) if [ -f "$S/down" ]; then cat "$S/version_down.json"; exit 1; else cat "$S/version_up.json"; fi;;
ps) cat "$S/ps.jsonl";;
container) case "$2" in
  inspect) cat "$S/containers.json";;
  rm) shift 2; echo "container rm $*" >> "$S/actions"; [ -f "$S/fail_rm" ] && {{ echo "Error: failed" >&2; exit 1; }};;
  *) exit 2;; esac;;
image) case "$2" in
  # Like Docker 29: untagged images appear only with the dangling filter.
  ls) case "$*" in *dangling=true*) grep '"Tag":"<none>"' "$S/images.jsonl";; *) grep -v '"Tag":"<none>"' "$S/images.jsonl";; esac;;
  inspect) shift 2; printf '['; sep=''; for i in "$@"; do f="$S/img/$(printf %s "$i" | tr ':' '_').json"; [ -f "$f" ] && {{ printf '%s' "$sep"; cat "$f"; sep=','; }}; done; printf ']';;
  rm) shift 2; [ -f "$S/rm_sleep" ] && sleep "$(cat "$S/rm_sleep")"; echo "image rm $*" >> "$S/actions"; [ -f "$S/fail_rm" ] && {{ echo "Error: conflict" >&2; exit 1; }};;
  *) echo "image $*" >> "$S/actions";; esac;;
volume) case "$2" in
  ls) cat "$S/volumes.jsonl";;
  # Like Docker: one JSON line per requested volume that exists.
  inspect) shift 4; for v in "$@"; do grep -F "\"Name\":\"$v\"" "$S/volume_inspect.jsonl"; done;;
  rm) shift 2; printf 'volume rm' >> "$S/actions"; for a in "$@"; do printf ' [%s]' "$a" >> "$S/actions"; done; echo >> "$S/actions";;
  *) echo "volume $*" >> "$S/actions";; esac;;
system) if [ "$3" = "-v" ]; then cat "$S/df_v.json"; else cat "$S/df.jsonl"; fi;;
builder) echo "builder $*" >> "$S/actions";;
*) exit 2;;
esac
exit 0"#,
            s = s.display()
        );
        self.tool("docker", &body);
        s
    }
}

/// Run real git for building fixtures (never via the app's runner).
pub fn git(dir: &Path, args: &[&str]) -> String {
    git_at(dir, args, None)
}

/// `git` with commits dated `days` before `NOW`, for idle worktrees.
pub fn git_old(dir: &Path, args: &[&str], days: i64) -> String {
    git_at(dir, args, Some(NOW - days * DAY))
}

fn git_at(dir: &Path, args: &[&str], when: Option<i64>) -> String {
    let mut cmd = std::process::Command::new(Runner::default().resolve("git").expect("git is required for fixture tests"));
    if let Some(t) = when {
        cmd.env("GIT_AUTHOR_DATE", format!("@{t} +0000")).env("GIT_COMMITTER_DATE", format!("@{t} +0000"));
    }
    let out = cmd
        .args(["-c", "init.defaultBranch=main", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null"])
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Set modification and access time of every file under `p`.
pub fn age(p: &Path, days: i64) {
    let t = UNIX_EPOCH + Duration::from_secs((NOW - days * DAY) as u64);
    let set = |f: &Path| {
        if let Ok(file) = std::fs::File::options().write(true).open(f) {
            let _ = file.set_times(std::fs::FileTimes::new().set_modified(t).set_accessed(t));
        }
    };
    // A file can vanish while walking (Git's short-lived lock files under
    // .git, seen on macOS CI): skip it.
    let Ok(m) = std::fs::symlink_metadata(p) else { return };
    if m.is_file() {
        set(p);
    } else if m.is_dir() {
        let Ok(rd) = std::fs::read_dir(p) else { return };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            age(&e.path(), days);
        }
        // Directory times too (after children, which update them).
        if let Ok(d) = crate::platform::open_dir_for_times(p) {
            let _ = d.set_times(std::fs::FileTimes::new().set_modified(t).set_accessed(t));
        }
    }
}

pub fn now_system() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(NOW as u64)
}

/// Snapshot of a tree without following links: path -> description
/// (type, length, mtime, content hash or link target).
pub fn tree(root: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let m = std::fs::symlink_metadata(&p).unwrap();
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().to_string();
            let mt = m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_nanos()).unwrap_or(0);
            let desc = if m.file_type().is_symlink() {
                format!("link->{}", std::fs::read_link(&p).unwrap().display())
            } else if m.is_dir() {
                stack.push(p.clone());
                format!("dir|{mt}")
            } else {
                // Streamed: fixtures hold sparse files of hundreds of MB.
                let mut h = blake3::Hasher::new();
                if let Ok(f) = std::fs::File::open(&p) {
                    let _ = h.update_reader(f);
                }
                format!("file|{}|{mt}|{}", m.len(), &h.finalize().to_hex()[..16])
            };
            out.insert(rel, desc);
        }
    }
    out
}

pub fn by_detector<'a>(o: &'a ScanOutcome, det: &str) -> Vec<&'a Suggestion> {
    o.findings.iter().filter(|f| f.suggestion.detector == det).map(|f| &f.suggestion).collect()
}

pub fn find<'a>(o: &'a ScanOutcome, det: &str) -> &'a Finding {
    o.findings.iter().find(|f| f.suggestion.detector == det).unwrap_or_else(|| panic!("no {det} finding in {:#?}", o))
}

/// A Node project: package.json + lockfile + node_modules, aged `days`.
pub fn node_project(fx: &Fx, rel: &str, days: i64) -> PathBuf {
    let p = fx.mkdir(rel);
    std::fs::write(p.join("package.json"), b"{\"name\":\"x\"}").unwrap();
    std::fs::write(p.join("package-lock.json"), b"{}").unwrap();
    std::fs::create_dir_all(p.join("src")).unwrap();
    std::fs::write(p.join("src/index.js"), b"console.log(1)").unwrap();
    std::fs::create_dir_all(p.join("node_modules/dep")).unwrap();
    std::fs::write(p.join("node_modules/dep/index.js"), vec![b'x'; 100_000]).unwrap();
    age(&p, days);
    p
}
