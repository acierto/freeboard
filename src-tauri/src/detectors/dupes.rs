//! `files.duplicates` (exact duplicate files) and `files.large_media` (big
//! videos not opened for 6 months), both in deep scans only.
//!
//! Both walk the scan folders (and, for videos, Downloads and the videos
//! folder) and never look inside hidden folders, app or library bundles,
//! repositories or generated folders, never follow links, and never read a
//! file's contents except to hash duplicate candidates.

use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use super::common::*;
use super::Probe;
use crate::guard::Guard;
use crate::model::*;
use crate::platform;
use crate::scan::{walk, Detector, ScanContext};

pub const DUPLICATES: &str = "files.duplicates";
pub const LARGE_MEDIA: &str = "files.large_media";
/// Smaller duplicates free too little to be worth a question.
pub const DUP_MIN: u64 = 1_000_000;
pub const MEDIA_MIN: u64 = 500_000_000;
pub const MEDIA_DAYS: i64 = 182;
pub const HEAD: usize = 64 * 1024;
pub const VIDEO_EXTS: [&str; 14] = ["mp4", "m4v", "mov", "mkv", "avi", "webm", "wmv", "flv", "mpg", "mpeg", "mts", "m2ts", "3gp", "vob"];

/// Folders that are packages, not plain folders: apps and media libraries.
fn is_bundle(name: &str) -> bool {
    let l = name.to_lowercase();
    [".app", ".bundle", ".framework", ".photoslibrary", ".tvlibrary", ".musiclibrary", ".imovielibrary", ".fcpbundle", ".lrdata", ".pkg", ".xcarchive"]
        .iter()
        .any(|e| l.ends_with(e))
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size: u64,
    pub mtime: i64,
    pub atime: i64,
    pub id: Option<String>,
}

/// Regular files of at least `min` bytes under `roots`. Returns the files and
/// whether the walk was complete.
pub fn walk_files(ctx: &ScanContext, roots: &[PathBuf], min: u64) -> (Vec<FileEntry>, bool) {
    let ex = walk::exclusions(ctx);
    let mut out = vec![];
    let mut complete = true;
    let mut seen = 0u64;
    let mut stack: Vec<PathBuf> = roots.iter().filter(|r| r.is_dir() && !platform::is_link(r)).cloned().collect();
    let mut visited = HashSet::new();
    while let Some(dir) = stack.pop() {
        if !visited.insert(dir.clone()) {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else {
            complete = false;
            continue;
        };
        let entries: Vec<_> = rd.flatten().collect();
        // A repository is left alone as a whole (spec: never include files
        // in repositories).
        if entries.iter().any(|e| e.file_name() == ".git") {
            continue;
        }
        for e in entries {
            seen += 1;
            if seen % 4096 == 0 && !walk::wait_if_paused(ctx) {
                return (out, false);
            }
            let name = e.file_name().to_string_lossy().to_string();
            let Ok(ft) = e.file_type() else {
                complete = false;
                continue;
            };
            if ft.is_symlink() || name.starts_with('.') {
                continue;
            }
            let p = e.path();
            if cfg!(windows) && platform::is_link(&p) {
                continue;
            }
            if ft.is_dir() {
                if is_bundle(&name) || walk::GENERATED.contains(&name.as_str()) || ex.iter().any(|x| p.starts_with(x)) {
                    continue;
                }
                stack.push(p);
            } else if ft.is_file() {
                let Ok(m) = e.metadata() else { continue };
                if m.len() < min {
                    continue;
                }
                out.push(FileEntry {
                    size: m.len(),
                    mtime: platform::modified_secs(&m),
                    atime: platform::accessed_secs(&m),
                    id: platform::file_id(&p, &m).map(|i| i.encode()),
                    path: p,
                });
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    (out, complete)
}

// ----- duplicates ------------------------------------------------------------

pub struct Duplicates;

/// Open for reading without moving the file's access time where the system
/// allows it (Linux `O_NOATIME`, for files the person owns), so hashing a
/// video never makes it look recently opened to `files.large_media`. macOS
/// has no such flag; there a hashed file can only look more recently used,
/// which suggests less, never more.
fn open_quietly(p: &Path) -> Option<std::fs::File> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        if let Ok(f) = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOATIME).open(p) {
            return Some(f);
        }
    }
    std::fs::File::open(p).ok()
}

fn hash_file(p: &Path, limit: Option<usize>) -> Option<String> {
    let mut f = open_quietly(p)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut left = limit.unwrap_or(usize::MAX);
    while left > 0 {
        let want = buf.len().min(left);
        let n = f.read(&mut buf[..want]).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        left -= n;
    }
    Some(h.finalize().to_hex().to_string())
}

/// Sets of files with identical contents: same size, then the same hash of
/// the first 64 KiB, then the same full `blake3`. Hard links to one file are
/// one file, not duplicates. Each set is sorted with the copy to keep first.
pub fn duplicate_sets(files: &[FileEntry]) -> Vec<(String, Vec<FileEntry>)> {
    let mut by_size: BTreeMap<u64, Vec<&FileEntry>> = BTreeMap::new();
    for f in files {
        by_size.entry(f.size).or_default().push(f);
    }
    let mut sets = vec![];
    for (_, same) in by_size {
        let mut ids = HashSet::new();
        let same: Vec<&FileEntry> = same
            .into_iter()
            .filter(|f| match &f.id {
                None => true,
                Some(i) => ids.insert(i.clone()),
            })
            .collect();
        if same.len() < 2 {
            continue;
        }
        let mut by_head: BTreeMap<String, Vec<&FileEntry>> = BTreeMap::new();
        for f in same {
            if let Some(h) = hash_file(&f.path, Some(HEAD)) {
                by_head.entry(h).or_default().push(f);
            }
        }
        for (_, cand) in by_head.into_iter().filter(|(_, v)| v.len() > 1) {
            let mut by_full: BTreeMap<String, Vec<FileEntry>> = BTreeMap::new();
            for f in cand {
                if let Some(h) = hash_file(&f.path, None) {
                    by_full.entry(h).or_default().push(f.clone());
                }
            }
            for (h, mut v) in by_full.into_iter().filter(|(_, v)| v.len() > 1) {
                // Keep the oldest copy; on a tie, the shortest path.
                v.sort_by(|a, b| a.mtime.cmp(&b.mtime).then(a.path.as_os_str().len().cmp(&b.path.as_os_str().len())).then(a.path.cmp(&b.path)));
                sets.push((h, v));
            }
        }
    }
    sets
}

const KEEP: &str = ";m:";

impl Detector for Duplicates {
    fn id(&self) -> &'static str {
        DUPLICATES
    }
    fn deep_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let roots = walk::roots(ctx);
        let (files, _) = walk_files(ctx, &roots, DUP_MIN);
        let sets = duplicate_sets(&files);
        let ex = walk::exclusions(ctx);
        let mut parts = vec![];
        for (hash, set) in sets {
            let keep = &set[0];
            let extras: Vec<PathBuf> = set[1..].iter().map(|f| f.path.clone()).collect();
            let states: Vec<PathState> = extras.iter().filter_map(|p| path_state(p, &ex)).collect();
            if states.len() != extras.len() {
                continue;
            }
            let name = keep.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let n = extras.len();
            let mut s = base(DUPLICATES, Group::YourFiles, &format!("{hash}\0{}", keep.path.display()));
            s.title = format!("{name}: {} extra cop{}", n, if n == 1 { "y" } else { "ies" });
            s.location = display_path(ctx, &extras[0]);
            s.size_bytes = states.iter().map(|s| s.m.reclaimable()).sum();
            s.full_size_bytes = s.size_bytes;
            s.size_is_estimate = states.iter().any(|s| s.m.is_estimate());
            s.last_used_label = format!("Kept: {}", display_path(ctx, &keep.path));
            s.risk = Risk::Review;
            s.action = Action::Trash;
            s.action_label = move_label(ctx);
            s.reason = DUP_REASON.into();
            s.recovery = format!("Restore them from the {}.", trash_word(ctx));
            s.recoverable = true;
            // The copy kept and the shared hash ride along after ";m:", which
            // the generic path check ignores and `probe` reads.
            s.fingerprint = format!("{}{KEEP}{hash}\0{}", fingerprint_paths(&states), keep.path.display());
            if s.size_bytes > 0 {
                parts.push(Finding::new(s, Target::Paths { paths: extras.iter().map(|p| p.to_string_lossy().to_string()).collect() }));
            }
        }
        Ok(dup_item(ctx, parts).into_iter().collect())
    }
}

const DUP_REASON: &str = "Files with exactly the same contents. One copy of each is kept: the oldest.";

fn dup_item(ctx: &ScanContext, mut parts: Vec<Finding>) -> Option<Finding> {
    if parts.len() <= 1 {
        return parts.pop();
    }
    parts.sort_by_key(|f| std::cmp::Reverse(f.suggestion.size_bytes));
    let mut s = base(DUPLICATES, Group::YourFiles, "duplicates");
    s.title = format!("Duplicate copies of {} files", parts.len());
    s.location = "Your scan folders".into();
    s.size_bytes = parts.iter().map(|f| f.suggestion.size_bytes).sum();
    s.full_size_bytes = s.size_bytes;
    s.size_is_estimate = parts.iter().any(|f| f.suggestion.size_is_estimate);
    s.last_used_label = "One copy of each is kept".into();
    s.risk = Risk::Review;
    s.action = Action::Trash;
    s.action_label = move_label(ctx);
    s.reason = DUP_REASON.into();
    s.recovery = format!("Restore them from the {}.", trash_word(ctx));
    s.recoverable = true;
    s.parts = parts
        .iter()
        .map(|f| SuggestionPart {
            id: f.suggestion.id.clone(),
            label: f.suggestion.title.clone(),
            location: f.suggestion.location.clone(),
            size_bytes: f.suggestion.size_bytes,
            selected: true,
        })
        .collect();
    s.fingerprint = parts.iter().map(|f| f.suggestion.fingerprint.split(KEEP).next().unwrap_or("")).collect::<Vec<_>>().join("|");
    let paths = parts.iter().flat_map(|f| f.paths().to_vec()).collect();
    Some(Finding { suggestion: s, target: Target::Paths { paths }, parts })
}

// ----- large videos ----------------------------------------------------------

pub struct LargeMedia;

pub fn is_video(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| VIDEO_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Scan folders, Downloads, and the videos folder (on macOS only with Full
/// Disk Access, because reading Movies can make macOS ask about the TV app's
/// library).
fn media_roots(ctx: &ScanContext) -> Vec<PathBuf> {
    let mut r = walk::roots(ctx);
    let dl = platform::downloads_dir(&ctx.home);
    if !would_prompt(ctx, &dl) {
        r.push(dl);
    }
    match ctx.platform {
        "macos" if may_read_protected(ctx) => r.push(ctx.home.join("Movies")),
        "macos" => {}
        _ => r.push(ctx.home.join("Videos")),
    }
    // A folder inside another root is walked once.
    r.sort();
    r.dedup();
    let all = r.clone();
    r.retain(|p| !all.iter().any(|q| q != p && p.starts_with(q)));
    r
}

impl Detector for LargeMedia {
    fn id(&self) -> &'static str {
        LARGE_MEDIA
    }
    fn deep_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let (files, _) = walk_files(ctx, &media_roots(ctx), MEDIA_MIN);
        let entries = files
            .into_iter()
            .filter(|f| is_video(&f.path))
            .filter(|f| days_since(ctx.now, f.mtime.max(f.atime)) >= MEDIA_DAYS)
            .map(|f| {
                let t = f.mtime.max(f.atime);
                let label = f.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                (label, vec![f.path], Some((Some(t), format!("Not opened in {}+ days", days_since(ctx.now, t)))))
            })
            .collect();
        let item = FolderItem {
            detector: LARGE_MEDIA,
            group: Group::YourFiles,
            identity: "large-videos",
            title: "Large videos not opened for 6 months".into(),
            location: "Your folders".into(),
            risk: Risk::Review,
            action: Action::Trash,
            reason: "Video files of 500 MB or more that you have not opened for 6 months. They may be the only copy.".into(),
            recovery: format!("Restore them from the {}.", trash_word(ctx)),
        };
        Ok(split_finding(ctx, item, entries).into_iter().collect())
    }
}

// ----- pre-cleanup checks ----------------------------------------------------

/// Duplicates: path checks, then the kept copy still exists with the same
/// contents and every extra copy still matches it, read again in full.
/// Videos: path checks and still not opened.
pub fn probe(f: &Finding, ctx: &ScanContext, guard: &Guard) -> Probe {
    let r = super::files::probe_paths(f, ctx, guard);
    if r != Probe::Unchanged {
        return r;
    }
    if f.suggestion.detector == LARGE_MEDIA {
        for p in f.paths() {
            if let Ok(m) = std::fs::metadata(p) {
                if days_since(ctx.now, platform::modified_secs(&m).max(platform::accessed_secs(&m))) < MEDIA_DAYS {
                    return Probe::Changed("The video was opened after the scan.".into());
                }
            }
        }
        return Probe::Unchanged;
    }
    let Some((hash, keep)) = f.suggestion.fingerprint.split_once(KEEP).and_then(|(_, k)| k.split_once('\0')) else {
        return Probe::Unknown("Freeboard doesn't know which copy to keep.".into());
    };
    match hash_file(Path::new(keep), None) {
        Some(h) if h == hash => {}
        Some(_) => return Probe::Changed("The copy being kept changed after the scan.".into()),
        None => return Probe::Changed("The copy being kept is gone, so the others stay.".into()),
    }
    for p in f.paths() {
        if hash_file(Path::new(p), None).as_deref() != Some(hash) {
            return Probe::Changed("A copy is no longer the same as the one being kept.".into());
        }
    }
    Probe::Unchanged
}
