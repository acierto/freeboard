//! Area roots: the well-known locations, the scan folders and the top-level
//! home folders, measured during deep scans only (design D4). Quick scans
//! reuse the latest `Measurement`, so they never walk the disk.
//!
//! Each root is measured once, most specific first: nested roots, claimed
//! suggestion paths, Docker's virtual disk and user exclusions are skipped,
//! and hard-linked files are counted once across all roots. Only directory
//! entries and metadata are read; no file is opened and nothing is written.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::Claim;
use crate::platform::FileId;
use crate::scan::size::{self, AccountOpts, Denied};

/// A top-level home folder at least this large that is not scanned, or a
/// folder this large inside a scan folder that no detector claims and that is
/// not a project, is a blind spot. Smaller ones count with their area.
pub const BLIND_SPOT_MIN: u64 = 1_000_000_000;

/// Files that mark a folder as a project (subset of the projects detector's list).
const PROJECT_MARKERS: &[&str] = &[
    "package.json", "Cargo.toml", "pyproject.toml", "requirements.txt", "setup.py", "go.mod", "pom.xml", "build.gradle",
    "build.gradle.kts", "Gemfile", "composer.json", "mix.exs", "Makefile", "CMakeLists.txt", "Package.swift", "pubspec.yaml",
];

/// Personal folders: their contents are Your files, never "unknown".
const USER_FOLDERS: &[&str] = &["Desktop", "Documents", "Downloads", "Pictures", "Movies", "Music", "Videos", "Public"];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootBytes {
    pub path: String,
    pub area: String,
    /// Bytes in this root not claimed by a suggestion or a nested root.
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimRecord {
    pub paths: Vec<String>,
    pub bytes: u64,
}

/// A folder that may become a blind spot.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub path: String,
    /// None when Freeboard did not read it (it would trigger a macOS prompt).
    pub bytes: Option<u64>,
    /// The extension holding most of its bytes (unknown files only).
    #[serde(default)]
    pub ext: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Denial {
    pub path: String,
    pub kind: Denied,
    /// Inside the home folder or a scan folder (the person's own data).
    pub own: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VmDisk {
    pub path: String,
    pub area: String,
    pub bytes: u64,
}

/// The result of one deep-scan measurement, stored for quick scans.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measurement {
    pub at: i64,
    pub roots: Vec<RootBytes>,
    /// Suggestion paths that were skipped, with their bytes, so later quick
    /// scans can reconcile claims that came or went.
    pub claims: Vec<ClaimRecord>,
    pub unscanned: Vec<Candidate>,
    pub unknown: Vec<Candidate>,
    pub denied: Vec<Denial>,
    pub vm_disks: Vec<VmDisk>,
    /// macOS: whether Freeboard had Full Disk Access during the measurement.
    pub full_disk_access: Option<bool>,
    pub canceled: bool,
}

/// What the measurement needs from the scan.
pub struct MeasureEnv<'a> {
    pub home: PathBuf,
    pub platform: &'a str,
    pub scan_roots: Vec<PathBuf>,
    pub exclusions: Vec<PathBuf>,
    /// Repositories found by the deep scan's discovery walk.
    pub repos: &'a [PathBuf],
    /// macOS Full Disk Access (None elsewhere or when unknown).
    pub full_disk_access: Option<bool>,
    /// Also measure system locations outside the home folder (off in tests).
    pub outside_home: bool,
    pub cancel: &'a AtomicBool,
    pub progress: &'a (dyn Fn(u64) + Sync),
    pub now: i64,
    /// Size from which an unscanned or unknown folder is a blind spot
    /// (`UNSCANNED_MIN`; fixtures use smaller values).
    pub min_blind_spot: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Fixed,
    ScanRoot,
    /// A folder directly inside a scan folder; may hold unknown files.
    ScanChild { unknown_candidate: bool },
    /// A top-level home folder outside every rule; may be unscanned.
    HomeChild,
}

#[derive(Debug, Clone)]
struct Rule {
    path: PathBuf,
    area: &'static str,
    kind: Kind,
}

/// Well-known locations inside the home folder, by platform: (relative path, area).
fn home_rules(platform: &str) -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<(&str, &str)> = vec![
        // Caches
        (".cache", "caches"),
        (".npm", "caches"),
        (".pnpm-store", "caches"),
        (".gradle", "caches"),
        (".m2", "caches"),
        (".cargo/registry", "caches"),
        (".cargo/git", "caches"),
        ("go/pkg/mod", "caches"),
        (".bun/install/cache", "caches"),
        (".yarn", "caches"),
        // AI models
        (".ollama", "ai"),
        (".cache/huggingface", "ai"),
        (".cache/torch", "ai"),
        (".cache/lm-studio", "ai"),
        (".lmstudio", "ai"),
        // Phones and emulators
        (".android", "mobile"),
        // Apps and tools
        (".nvm", "apps"),
        (".rustup", "apps"),
        (".cargo", "apps"),
        (".pyenv", "apps"),
        (".sdkman", "apps"),
        (".volta", "apps"),
        (".bun", "apps"),
        (".deno", "apps"),
        (".vscode", "apps"),
        (".local", "apps"),
        (".config", "apps"),
        ("go", "apps"),
        ("Applications", "apps"),
        // Docker
        (".docker", "docker"),
        // Your files
        ("Desktop", "files"),
        ("Documents", "files"),
        ("Downloads", "files"),
        ("Pictures", "files"),
        ("Movies", "files"),
        ("Music", "files"),
        ("Videos", "files"),
        ("Public", "files"),
        ("Dropbox", "files"),
        ("OneDrive", "files"),
    ];
    match platform {
        // Without Full Disk Access only these Library folders are walked, not
        // all of ~/Library: Apple keeps media and personal data there
        // (AppleMediaServices, com.apple.MediaPlayer, com.apple.iTunesCloud,
        // ...). Reading those asked for the Apple Music media library on a
        // real Mac, 2026-11-07. With Full Disk Access all of Library is
        // measured (see `measure`).
        "macos" => v.extend([
            ("Library/Application Support", "apps"),
            ("Library/Developer", "apps"),
            ("Library/Logs", "apps"),
            ("Library/Python", "apps"),
            ("Library/Jupyter", "apps"),
            ("Library/Google", "apps"),
            ("Library/Java", "apps"),
            ("Library/Fonts", "apps"),
            ("Library/Caches", "caches"),
            ("Library/pnpm", "caches"),
            ("Library/Developer/Xcode/DerivedData", "caches"),
            ("Library/Developer/CoreSimulator", "mobile"),
            ("Library/Developer/Xcode/iOS DeviceSupport", "mobile"),
            ("Library/Application Support/MobileSync", "mobile"),
            ("Library/Android", "mobile"),
            ("Library/Containers/com.docker.docker", "docker"),
            (".Trash", "files"),
        ]),
        "linux" => v.extend([
            (".local/share/pnpm", "caches"),
            (".local/share/containers", "docker"),
            (".local/share/Trash", "files"),
            ("Android", "mobile"),
            ("snap", "apps"),
            (".var", "apps"),
        ]),
        "windows" => v.extend([
            ("AppData", "apps"),
            ("AppData/Local/Temp", "caches"),
            ("AppData/Local/npm-cache", "caches"),
            ("AppData/Local/pip/cache", "caches"),
            ("AppData/Local/Android", "mobile"),
            ("AppData/Local/Docker", "docker"),
        ]),
        _ => {}
    }
    v
}

/// macOS locations that are privacy-protected. Reading them without Full
/// Disk Access fails or, worse, makes macOS ask the person for permission
/// they did not request, so they are left alone unless access was granted.
fn private_locations(platform: &str) -> Vec<(&'static str, &'static str)> {
    if platform != "macos" {
        return vec![];
    }
    vec![
        ("Library/Containers", "apps"),
        ("Library/Group Containers", "apps"),
        ("Library/Mail", "files"),
        ("Library/Messages", "files"),
        ("Library/Safari", "apps"),
        ("Library/Mobile Documents", "files"),
        ("Library/CloudStorage", "files"),
        ("Pictures/Photos Library.photoslibrary", "files"),
        // Also prompt (found on a real Mac, 2026-11-06, where measuring the
        // home folder asked for the Apple Music library): Contacts and
        // Calendars data.
        ("Library/Application Support/AddressBook", "files"),
        ("Library/Calendars", "files"),
    ]
}

/// macOS folders that ask for permission when read; read only when they are
/// scan folders or Freeboard has Full Disk Access. Music, Movies and Pictures
/// hold the Apple Music, TV and Photos libraries, which prompt for media
/// library access (found on a real Mac, 2026-11-06).
pub fn prompting_folders(platform: &str) -> &'static [&'static str] {
    if platform == "macos" {
        &["Desktop", "Documents", "Downloads", "Music", "Movies", "Pictures"]
    } else {
        &[]
    }
}

/// Apple's own data folders inside ~/Library/Application Support and
/// ~/Library/Caches. Some hold media and personal data guarded by macOS.
fn is_apple_private(name: &str) -> bool {
    name.starts_with("com.apple.")
        || matches!(name, "AddressBook" | "CallHistoryDB" | "CallHistoryTransactions" | "Knowledge" | "CloudDocs" | "FileProvider" | "iCloud" | "Apple" | "AppleMediaServices")
}

/// Docker Desktop's virtual disk, and WSL's, claimed by a single `stat`.
fn vm_disk_paths(home: &Path, platform: &str) -> Vec<(PathBuf, &'static str)> {
    match platform {
        "macos" => vec![(home.join("Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw"), "docker")],
        "linux" => vec![(home.join(".docker/desktop/vms/0/data/Docker.raw"), "docker")],
        "windows" => {
            let local = home.join("AppData/Local");
            let mut v = vec![
                (local.join("Docker/wsl/disk/docker_data.vhdx"), "docker"),
                (local.join("Docker/wsl/data/ext4.vhdx"), "docker"),
                (local.join("Docker/wsl/main/ext4.vhdx"), "docker"),
            ];
            // WSL distributions keep their file systems in virtual disks (spec
            // platform notes: show them in the map, do not scan inside).
            if let Ok(rd) = std::fs::read_dir(local.join("Packages")) {
                for e in rd.flatten() {
                    let disk = e.path().join("LocalState/ext4.vhdx");
                    if disk.is_file() {
                        v.push((disk, "apps"));
                    }
                }
            }
            v
        }
        _ => vec![],
    }
}

/// System locations outside home on the home volume: (absolute path, area).
fn outside_rules(home: &Path, platform: &str, full_disk_access: bool) -> Vec<(PathBuf, &'static str)> {
    let mut v = Vec::new();
    let mut add_children = |root: &Path, map: &dyn Fn(&str) -> Option<&'static str>| {
        if let Ok(rd) = std::fs::read_dir(root) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if let Some(area) = map(&name) {
                    v.push((e.path(), area));
                }
            }
        }
    };
    match platform {
        "macos" => {
            // Firmlinked locations live on the Data volume; the System volume
            // itself is read with statvfs (diskmap::platform_volumes).
            //
            // Only locations that never trigger a privacy prompt are walked.
            // Walking the rest of the Data volume (/Library, /private, ...)
            // as an ordinary app runs into macOS privacy guards: found on a
            // real Mac, 2026-11-06, where it asked for the Apple Music media
            // library. That space stays in Not explained, said honestly.
            let data = Path::new("/System/Volumes/Data");
            for (rel, area) in [("Applications", "apps"), ("opt", "apps"), ("usr/local", "apps")] {
                v.push((data.join(rel), area));
            }
            v.push((data.join("Users/Shared"), "files"));
            // Simulator runtimes installed by Xcode.
            v.push((data.join("Library/Developer/CoreSimulator"), "mobile"));
            // With Full Disk Access nothing here can ask the person, so the
            // system-wide places are measured too (found on a real Mac,
            // 2026-11-07: access granted, yet 115 GB stayed "not traced").
            // What only macOS itself may open (Spotlight's index, other
            // accounts) is still left out and said so.
            if full_disk_access {
                v.push((data.join("Library"), "apps"));
                for rel in ["private/var/vm", "private/var/db", "private/var/log", "private/var/logs"] {
                    v.push((data.join(rel), "system"));
                }
                // The person's own temporary and cache folders under
                // /private/var/folders. Not its X folder: apps keep code-sign
                // copies of themselves there (Chrome hundreds), APFS clones
                // sharing blocks with the app, which a walk would count again
                // for every copy.
                for p in user_dirs() {
                    v.push((p, "caches"));
                }
            }
        }
        "linux" => {
            let home_top = home.components().nth(1).map(|c| c.as_os_str().to_string_lossy().into_owned());
            add_children(Path::new("/"), &|n| match n {
                "opt" | "snap" => Some("apps"),
                "proc" | "sys" | "dev" | "run" | "mnt" | "media" | "lost+found" => None,
                n if Some(n) == home_top.as_deref() => None,
                _ => Some("system"),
            });
            v.push((PathBuf::from("/var/lib/docker"), "docker"));
            v.push((PathBuf::from("/var/lib/flatpak"), "apps"));
            v.push((PathBuf::from("/var/lib/snapd"), "apps"));
        }
        "windows" => {
            if let Some(drive) = home.ancestors().last() {
                add_children(drive, &|n| match n.to_ascii_lowercase().as_str() {
                    "program files" | "program files (x86)" | "programdata" => Some("apps"),
                    "users" => None,
                    "$recycle.bin" => Some("files"),
                    _ => Some("system"),
                });
            }
        }
        _ => {}
    }
    v
}

/// macOS: the person's own temporary and cache folders (`$TMPDIR` and
/// `DARWIN_USER_CACHE_DIR`), on the Data volume.
#[cfg(target_os = "macos")]
fn user_dirs() -> Vec<PathBuf> {
    let read = |name: libc::c_int| -> Option<PathBuf> {
        let mut buf = vec![0u8; 1024];
        // SAFETY: the buffer is valid for its length; confstr writes a
        // NUL-terminated string of at most that length.
        let n = unsafe { libc::confstr(name, buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
        if n == 0 || n > buf.len() {
            return None;
        }
        let s = String::from_utf8_lossy(&buf[..n - 1]).trim_end_matches('/').to_string();
        let p = std::fs::canonicalize(&s).ok()?;
        // /private/var/... lives on the Data volume.
        Some(Path::new("/System/Volumes/Data").join(p.strip_prefix("/").ok()?))
    };
    [libc::_CS_DARWIN_USER_TEMP_DIR, libc::_CS_DARWIN_USER_CACHE_DIR].into_iter().filter_map(read).collect()
}

#[cfg(not(target_os = "macos"))]
fn user_dirs() -> Vec<PathBuf> {
    Vec::new()
}

fn depth(p: &Path) -> usize {
    p.components().count()
}

fn is_link(p: &Path) -> bool {
    crate::platform::is_link(p)
}

fn exists_dir_or_file(p: &Path) -> bool {
    std::fs::symlink_metadata(p).map(|m| !crate::platform::is_link_meta(p, &m)).unwrap_or(false)
}

fn device(p: &Path) -> Option<u64> {
    let m = std::fs::symlink_metadata(p).ok()?;
    crate::platform::file_id(p, &m).map(|i| i.dev)
}

fn has_marker(dir: &Path) -> bool {
    dir.join(".git").exists() || PROJECT_MARKERS.iter().any(|m| dir.join(m).exists())
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// A folder's size at the last measurement: the measured roots inside it,
/// plus suggested items inside it (measured separately), or the folder's own
/// blind-spot size. `None` when the measurement did not reach it.
pub fn folder_bytes(m: &Measurement, folder: &Path) -> Option<u64> {
    let inside = |p: &str| Path::new(p).starts_with(folder);
    let roots: Vec<&RootBytes> = m.roots.iter().filter(|r| inside(&r.path)).collect();
    if roots.is_empty() {
        return m.unscanned.iter().chain(&m.unknown).find(|c| Path::new(&c.path) == folder).and_then(|c| c.bytes);
    }
    let claimed: u64 = m.claims.iter().filter(|c| c.paths.first().is_some_and(|p| inside(p))).map(|c| c.bytes).sum();
    Some(roots.iter().map(|r| r.bytes).sum::<u64>() + claimed)
}

/// Measure every area root for the disk map (deep scans only).
pub fn measure(env: &MeasureEnv, claims: &[Claim]) -> Measurement {
    let home = std::fs::canonicalize(&env.home).unwrap_or_else(|_| env.home.clone());
    let home = crate::platform::plain_path(&home);
    let home_dev = device(&home);
    let fda = env.full_disk_access;
    let mut out = Measurement { at: env.now, full_disk_access: fda, ..Default::default() };
    let excluded = |p: &Path| env.exclusions.iter().any(|x| p.starts_with(x));
    let in_scan_root = |p: &Path| env.scan_roots.iter().any(|r| p.starts_with(r));

    // Global skip set: exclusions, claimed paths, virtual disks and private places.
    let mut skip: HashSet<PathBuf> = env.exclusions.iter().cloned().collect();
    let claim_paths: Vec<&PathBuf> = claims.iter().flat_map(|c| c.paths.iter()).collect();
    skip.extend(claim_paths.iter().map(|p| (*p).clone()));
    for c in claims {
        out.claims.push(ClaimRecord { paths: c.paths.iter().map(|p| s(p)).collect(), bytes: c.bytes });
    }
    let mut rules: Vec<Rule> = Vec::new();
    let push = |rules: &mut Vec<Rule>, path: PathBuf, area: &'static str, kind: Kind| {
        if !rules.iter().any(|r| r.path == path) {
            rules.push(Rule { path, area, kind });
        }
    };

    // Docker Desktop's and WSL's virtual disks: a single stat each. On macOS
    // Docker's disk sits among other apps' data, so it is read only with Full
    // Disk Access (otherwise macOS may ask the person).
    for (p, area) in vm_disk_paths(&home, env.platform) {
        if env.platform == "macos" && fda != Some(true) {
            break;
        }
        if let Ok(m) = std::fs::symlink_metadata(&p) {
            if m.is_file() {
                out.vm_disks.push(VmDisk { path: s(&p), area: area.into(), bytes: crate::platform::allocated_size(&p, &m) });
                skip.insert(p);
            }
        }
    }

    // Private macOS places: measured with Full Disk Access, otherwise left alone.
    let mut private_skipped: Vec<PathBuf> = Vec::new();
    for (rel, area) in private_locations(env.platform) {
        let p = home.join(rel);
        if !exists_dir_or_file(&p) || excluded(&p) {
            continue;
        }
        if fda == Some(true) {
            push(&mut rules, p, area, Kind::Fixed);
        } else {
            out.denied.push(Denial { path: s(&p), kind: Denied::Privacy, own: true });
            private_skipped.push(p.clone());
            skip.insert(p);
        }
    }
    for rel in prompting_folders(env.platform) {
        let p = home.join(rel);
        if !exists_dir_or_file(&p) || excluded(&p) || in_scan_root(&p) || fda == Some(true) {
            continue;
        }
        // Not read: reading it would make macOS ask. Its size is unknown.
        out.unscanned.push(Candidate { path: s(&p), bytes: None, ext: None });
        private_skipped.push(p.clone());
        skip.insert(p);
    }

    // Apple's own data inside the Library folders that are walked: left alone
    // without Full Disk Access, since macOS may ask the person about it.
    if env.platform == "macos" {
        if fda == Some(true) {
            let lib = home.join("Library");
            if exists_dir_or_file(&lib) && !excluded(&lib) {
                push(&mut rules, lib, "apps", Kind::Fixed);
            }
        } else {
            for parent in ["Library/Application Support", "Library/Caches"] {
                if let Ok(rd) = std::fs::read_dir(home.join(parent)) {
                    for e in rd.flatten() {
                        if is_apple_private(&e.file_name().to_string_lossy()) {
                            skip.insert(e.path());
                        }
                    }
                }
            }
        }
    }

    // Well-known locations in the home folder.
    for (rel, area) in home_rules(env.platform) {
        let p = home.join(rel);
        // Nothing inside a place left alone above is read either.
        if exists_dir_or_file(&p) && !excluded(&p) && !skip.contains(&p) && !private_skipped.iter().any(|x| p.starts_with(x)) {
            push(&mut rules, p, area, Kind::Fixed);
        }
    }

    // Scan folders and the folders directly inside them.
    for root in &env.scan_roots {
        if !exists_dir_or_file(root) || excluded(root) || claim_paths.iter().any(|c| root.starts_with(c)) {
            continue;
        }
        let personal = root.ancestors().any(|a| a.parent() == Some(home.as_path()) && a.file_name().is_some_and(|n| USER_FOLDERS.iter().any(|u| n == *u)));
        let area = if personal || !env.repos.iter().any(|r| r.starts_with(root)) { "files" } else { "projects" };
        // A scan folder that is also a well-known location keeps its area.
        let area = rules.iter().find(|r| &r.path == root).map(|r| r.area).unwrap_or(area);
        rules.retain(|r| &r.path != root);
        push(&mut rules, root.clone(), area, Kind::ScanRoot);
        if let Ok(rd) = std::fs::read_dir(root) {
            for e in rd.flatten() {
                let p = e.path();
                let Ok(ft) = e.file_type() else { continue };
                if !ft.is_dir() || ft.is_symlink() || is_link(&p) || skip.contains(&p) || excluded(&p) || rules.iter().any(|r| r.path == p) {
                    continue;
                }
                // Unknown files are looked for in project trees only; in any
                // other scan folder the rest is the person's own files.
                let holds_repo = env.repos.iter().any(|r| r.starts_with(&p));
                let unknown_candidate = area == "projects" && !holds_repo && !has_marker(&p);
                push(&mut rules, p, area, Kind::ScanChild { unknown_candidate });
            }
        }
    }

    // The rest of ~/Library stays unread without Full Disk Access (see
    // `home_rules`); it is not picked up as an ordinary top-level folder.
    if env.platform == "macos" && fda != Some(true) {
        skip.insert(home.join("Library"));
    }

    // Everything else at the top of the home folder.
    if let Ok(rd) = std::fs::read_dir(&home) {
        for e in rd.flatten() {
            let p = e.path();
            if skip.contains(&p) || excluded(&p) || rules.iter().any(|r| r.path == p) {
                continue;
            }
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() || is_link(&p) {
                continue;
            }
            if ft.is_dir() {
                push(&mut rules, p, "files", Kind::HomeChild);
            } else {
                push(&mut rules, p, "files", Kind::Fixed);
            }
        }
    }

    if env.outside_home {
        for (p, area) in outside_rules(&home, env.platform, fda == Some(true)) {
            if exists_dir_or_file(&p) && !excluded(&p) && !p.starts_with(&home) && !home.starts_with(&p) {
                push(&mut rules, p, area, Kind::Fixed);
            }
        }
    }

    // Only the home volume counts toward its used space, and a root inside
    // a claimed path is already counted by that claim.
    rules.retain(|r| (home_dev.is_none() || device(&r.path) == home_dev) && !claim_paths.iter().any(|c| r.path.starts_with(c)));
    // Most specific first.
    rules.sort_by(|a, b| depth(&b.path).cmp(&depth(&a.path)).then(a.path.cmp(&b.path)));
    let all: HashSet<PathBuf> = rules.iter().map(|r| r.path.clone()).collect();
    let seen: Mutex<HashSet<FileId>> = Mutex::new(HashSet::new());
    let mut entries_before = 0u64;
    for r in &rules {
        if env.cancel.load(std::sync::atomic::Ordering::Relaxed) {
            out.canceled = true;
            break;
        }
        let mut rskip = skip.clone();
        rskip.extend(all.iter().filter(|p| **p != r.path && p.starts_with(&r.path)).cloned());
        let base = entries_before;
        let progress = |n: u64| (env.progress)(base + n);
        let m = size::account(
            &r.path,
            &AccountOpts {
                skip: &rskip,
                seen: &seen,
                cancel: env.cancel,
                same_device: true,
                track_extensions: matches!(r.kind, Kind::ScanChild { unknown_candidate: true }),
                progress: &progress,
            },
        );
        entries_before += m.entries;
        (env.progress)(entries_before);
        if m.canceled {
            out.canceled = true;
        }
        for (p, kind) in &m.denied {
            if out.denied.len() < size::MAX_DENIED {
                let own = p.starts_with(&home) || in_scan_root(p);
                out.denied.push(Denial { path: s(p), kind: *kind, own });
            }
        }
        match r.kind {
            Kind::ScanChild { unknown_candidate: true } if m.bytes >= env.min_blind_spot => {
                out.unknown.push(Candidate { path: s(&r.path), bytes: Some(m.bytes), ext: m.top_extension.filter(|(_, b)| *b * 2 > m.bytes).map(|(e, _)| e) });
            }
            Kind::HomeChild if m.bytes >= env.min_blind_spot => {
                out.unscanned.push(Candidate { path: s(&r.path), bytes: Some(m.bytes), ext: None });
            }
            _ => out.roots.push(RootBytes { path: s(&r.path), area: r.area.into(), bytes: m.bytes }),
        }
    }
    out.unscanned.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.path.cmp(&b.path)));
    out.unknown.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.path.cmp(&b.path)));
    out
}

#[cfg(all(test, target_os = "macos"))]
mod fda_tests {
    use super::*;

    #[test]
    fn full_disk_access_adds_system_places_but_never_code_sign_copies() {
        let home = std::env::var("HOME").map(PathBuf::from).unwrap();
        let without: Vec<PathBuf> = outside_rules(&home, "macos", false).into_iter().map(|(p, _)| p).collect();
        let with = outside_rules(&home, "macos", true);
        let data = Path::new("/System/Volumes/Data");
        assert!(!without.contains(&data.join("Library")) && !without.iter().any(|p| p.starts_with(data.join("private"))));
        assert!(with.iter().any(|(p, a)| p == &data.join("Library") && *a == "apps"));
        assert!(with.iter().any(|(p, a)| p == &data.join("private/var/vm") && *a == "system"));
        let own: Vec<&PathBuf> = with.iter().filter(|(p, _)| p.starts_with(data.join("private/var/folders"))).map(|(p, _)| p).collect();
        assert_eq!(own.len(), 2, "{own:?}");
        assert!(own.iter().all(|p| p.ends_with("T") || p.ends_with("C")), "{own:?}");
    }
}
