//! Shared helpers for detectors: stable IDs, fingerprints, labels, errors.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::model::*;
use crate::platform;
use crate::runner::RunError;
use crate::scan::size::{measure, MeasureOpts, Measured};
use crate::scan::ScanContext;

pub const DAY: i64 = 86_400;

pub fn hash_hex(s: &str) -> String {
    let d = Sha256::digest(s.as_bytes());
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// Stable hash of detector ID and resource identity.
pub fn stable_id(detector: &str, identity: &str) -> String {
    hash_hex(&format!("{detector}\0{identity}"))[..20].to_string()
}

pub fn rfc3339(ts: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(ts, 0).map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

pub fn parse_time(s: &str) -> Option<i64> {
    let t = chrono::DateTime::parse_from_rfc3339(s.trim()).ok()?.timestamp();
    // Docker uses year 1 for "never".
    if t <= 0 {
        None
    } else {
        Some(t)
    }
}

pub fn days_since(now: i64, ts: i64) -> i64 {
    ((now - ts).max(0)) / DAY
}

pub fn ago(now: i64, ts: i64) -> String {
    match days_since(now, ts) {
        0 => "today".into(),
        1 => "1 day ago".into(),
        n => format!("{n} days ago"),
    }
}

/// Decimal GB/MB like the interface ("4.3 GB").
pub fn human(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.0} MB", b / 1e6)
    } else if b >= 1e3 {
        format!("{:.0} kB", b / 1e3)
    } else {
        format!("{bytes} B")
    }
}

/// Parse tool sizes such as "1.2GB", "34.6MB", "0B", "512 kB", "1GB".
pub fn parse_human(s: &str) -> Option<u64> {
    let s = s.trim();
    let s = s.split_whitespace().collect::<String>();
    let s = s.split('(').next()?.trim().to_string();
    let idx = s.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))?;
    let (num, unit) = s.split_at(idx);
    let n: f64 = num.replace(',', "").parse().ok()?;
    let mult = match unit.to_ascii_lowercase().as_str() {
        "b" => 1.0,
        "kb" | "k" => 1e3,
        "mb" | "m" => 1e6,
        "gb" | "g" => 1e9,
        "tb" | "t" => 1e12,
        "kib" => 1024.0,
        "mib" => 1024.0 * 1024.0,
        "gib" => 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((n * mult).round() as u64)
}

pub fn trash_word(ctx: &ScanContext) -> &'static str {
    if ctx.platform == "windows" {
        "Recycle Bin"
    } else {
        "Trash"
    }
}

pub fn move_label(ctx: &ScanContext) -> String {
    format!("Move to {}", trash_word(ctx))
}

/// Show a path with `~` for the home directory (Unix), like the prototype.
pub fn display_path(ctx: &ScanContext, p: &Path) -> String {
    let home = std::fs::canonicalize(&ctx.home).unwrap_or_else(|_| ctx.home.clone());
    for h in [&home, &ctx.home] {
        if let Ok(rest) = p.strip_prefix(h) {
            if ctx.platform == "windows" {
                return p.display().to_string();
            }
            return if rest.as_os_str().is_empty() { "~".into() } else { format!("~/{}", rest.display()) };
        }
    }
    p.display().to_string()
}

/// Environment overrides (CARGO_HOME, TEMP, ...) apply only when scanning the
/// real user's home, so fixture scans are never redirected to real data.
pub fn env_override(ctx: &ScanContext, var: &str) -> Option<PathBuf> {
    let real = platform::home_dir()?;
    if std::fs::canonicalize(&real).ok()? != std::fs::canonicalize(&ctx.home).ok()? {
        return None;
    }
    let v = std::env::var_os(var)?;
    let p = PathBuf::from(v);
    if p.is_absolute() {
        Some(p)
    } else {
        None
    }
}

pub fn err(detector: &str, kind: &str, message: impl Into<String>) -> DetectorError {
    DetectorError { detector: detector.into(), kind: kind.into(), message: message.into() }
}

/// Map a runner error to a detector error (missing tool, timeout, ...).
pub fn run_err(detector: &str, e: &RunError) -> DetectorError {
    match e {
        RunError::NotFound(p) => err(detector, "unavailable", format!("{p} is not installed.")),
        RunError::Timeout(c) => err(detector, "timeout", format!("{c} did not answer within 20 seconds.")),
        RunError::Forbidden(m) => err(detector, "error", m.clone()),
        RunError::OutputTooLarge => err(detector, "malformed", "The tool returned too much output."),
        RunError::Io(m) => {
            if m.to_lowercase().contains("permission") {
                err(detector, "permission", m.clone())
            } else {
                err(detector, "error", m.clone())
            }
        }
    }
}

/// A suggestion with conservative defaults; detectors fill in the rest.
pub fn base(detector: &str, group: Group, identity: &str) -> Suggestion {
    Suggestion {
        id: stable_id(detector, identity),
        detector: detector.into(),
        group,
        map_area: group.default_area().into(),
        title: String::new(),
        location: String::new(),
        size_bytes: 0,
        full_size_bytes: 0,
        last_used_at: None,
        last_used_label: String::new(),
        risk: Risk::Risky,
        action: Action::None,
        action_label: "None".into(),
        reason: String::new(),
        recovery: String::new(),
        recoverable: false,
        blocked_reason: None,
        needs_confirmation: false,
        parts: Vec::new(),
        fingerprint: String::new(),
        decision: None,
        size_is_estimate: false,
        skip_reason: None,
        risk_why: Vec::new(),
        safer: Vec::new(),
        ack_name: None,
        worktree: None,
    }
}

// ---------------------------------------------------------------------------
// Path fingerprints.
//
// Format: "p:<location hash>;c:<content hash>". The location part covers the
// canonical path and file identity of each path (detects moves and
// path-for-symlink swaps); the content part covers newest mtime, file count
// and allocated size (detects modified files).

#[derive(Debug, Clone, PartialEq)]
pub struct PathState {
    pub canonical: PathBuf,
    pub identity: String,
    pub is_link: bool,
    pub m: Measured,
}

pub fn path_state(p: &Path, exclusions: &[PathBuf]) -> Option<PathState> {
    let parent = std::fs::canonicalize(p.parent()?).ok()?;
    let canonical = parent.join(p.file_name()?);
    let meta = std::fs::symlink_metadata(&canonical).ok()?;
    let is_link = platform::is_link_meta(&canonical, &meta);
    let identity = platform::file_id(&canonical, &meta).map(|i| i.encode()).unwrap_or_default();
    let m = if is_link {
        Measured::default()
    } else {
        measure(&canonical, &MeasureOpts { exclusions: exclusions.to_vec(), serial: false })
    };
    Some(PathState { canonical, identity, is_link, m })
}

pub fn fingerprint_paths(states: &[PathState]) -> String {
    fingerprint_paths_with(states, |m| m.content_bytes)
}

/// The fingerprint as versions before 2026-11-07 computed it (allocated
/// bytes). Items scanned by such a version are still checked against it, so
/// updating Freeboard never makes every unchanged folder look changed (found
/// on a real Mac, 2026-11-07: 81 items skipped right after the update).
pub fn fingerprint_paths_legacy(states: &[PathState]) -> String {
    fingerprint_paths_with(states, |m| m.bytes)
}

fn fingerprint_paths_with(states: &[PathState], size: impl Fn(&crate::scan::size::Measured) -> u64) -> String {
    let mut loc = String::new();
    let mut con = String::new();
    for s in states {
        loc.push_str(&format!("{}|{}|{}\n", s.canonical.display(), s.identity, s.is_link));
        // Content only: file sizes, not the space the file system allocates
        // (see Measured::content_bytes).
        con.push_str(&format!("{:?}|{}|{}\n", s.m.newest_mtime, s.m.files, size(&s.m)));
    }
    format!("p:{};c:{}", &hash_hex(&loc)[..16], &hash_hex(&con)[..16])
}

pub fn fingerprint_of(parts: &[&str]) -> String {
    format!("r:{}", &hash_hex(&parts.join("\n"))[..24])
}

/// Newest mtime of source files in a project, skipping generated folders,
/// `.git` and links. Returns None if the tree is too large or unreadable
/// (unknown activity is never treated as inactive).
pub fn newest_source_mtime(dir: &Path, cap: u64) -> Option<i64> {
    let mut newest: Option<i64> = None;
    let mut stack = vec![dir.to_path_buf()];
    let mut seen = 0u64;
    while let Some(d) = stack.pop() {
        let rd = std::fs::read_dir(&d).ok()?;
        for e in rd {
            let e = e.ok()?;
            seen += 1;
            if seen > cap {
                return None;
            }
            let name = e.file_name();
            let ft = e.file_type().ok()?;
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                let n = name.to_string_lossy();
                if n == ".git" || crate::scan::walk::GENERATED.contains(&n.as_ref()) {
                    continue;
                }
                if cfg!(windows) && platform::is_link(&e.path()) {
                    continue;
                }
                stack.push(e.path());
            } else {
                let m = std::fs::symlink_metadata(e.path()).ok()?;
                let t = platform::modified_secs(&m);
                newest = Some(newest.map_or(t, |n| n.max(t)));
            }
        }
    }
    Some(newest.unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tool_sizes() {
        assert_eq!(parse_human("1.2GB"), Some(1_200_000_000));
        assert_eq!(parse_human("34.6MB"), Some(34_600_000));
        assert_eq!(parse_human("1GB"), Some(1_000_000_000));
        assert_eq!(parse_human("0B"), Some(0));
        assert_eq!(parse_human("800MB (66%)"), Some(800_000_000));
        assert_eq!(parse_human("512 kB"), Some(512_000));
        assert_eq!(parse_human("garbage"), None);
    }

    #[test]
    fn stable_ids_are_stable_and_distinct() {
        assert_eq!(stable_id("a", "x"), stable_id("a", "x"));
        assert_ne!(stable_id("a", "x"), stable_id("b", "x"));
    }

    #[test]
    fn human_sizes() {
        assert_eq!(human(4_300_000_000), "4.3 GB");
        assert_eq!(human(250_000_000), "250 MB");
    }
}
