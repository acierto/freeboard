//! Disk accounting (spec "Disk accounting", "Blind spots", "Places checked
//! and growth"; design D4 and D5).
//!
//! The map starts from the used bytes of the home volume and assigns bytes to
//! areas from three sources, each byte once:
//!
//! 1. claims: the paths of every suggestion (and its parts), most specific
//!    first, plus suggestions without paths (tool-level items such as Docker
//!    images or `brew cleanup`), which are assumed to sit inside their area's
//!    roots and never add to them;
//! 2. area roots measured during deep scans (`roots::measure`), stored as a
//!    `Measurement` that quick scans reuse;
//! 3. platform volumes read live (macOS: the System, Preboot and other APFS
//!    volumes that share the container).
//!
//! Not explained is used bytes minus everything claimed. If the claims add up
//! to more than used bytes by over 1% (APFS clones and snapshots can be
//! counted twice) the map says it is incomplete and shows the difference.

pub mod roots;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{DetectorError, Finding, Risk, Target};
pub use roots::{Candidate, Denial, Measurement};

/// Map areas in the prototype's TILES order: (id, label, leave-alone label).
pub const AREAS: [(&str, &str, &str); 8] = [
    ("projects", "Projects", "Active projects and code"),
    ("docker", "Docker", "Images and data you are using"),
    ("ai", "AI models", "Models you used recently"),
    ("caches", "Caches", "Caches in active use"),
    ("files", "Your files", "Documents, photos and videos"),
    ("mobile", "Phones and emulators", "Current simulators"),
    ("apps", "Apps and tools", "Apps and tools you use"),
    ("system", "System", "System"),
];

/// The system area is named after the platform (spec "Disk accounting").
pub fn system_name(platform: &str) -> &'static str {
    match platform {
        "windows" => "Windows",
        "linux" => "Linux",
        _ => "macOS",
    }
}

pub fn area_label(id: &str, platform: &str) -> String {
    if id == "system" {
        return system_name(platform).into();
    }
    if id == "unknown" {
        return "Not explained".into();
    }
    AREAS.iter().find(|a| a.0 == id).map(|a| a.1.to_string()).unwrap_or_else(|| id.to_string())
}

/// Allowed difference between the map's total and used bytes.
pub const TOLERANCE: f64 = 0.01;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskMapArea {
    pub id: String,
    pub label: String,
    pub bytes: u64,
    pub leave_alone_bytes: u64,
    // Additions beyond the source contract.
    /// What the leave-alone space is ("Models you used recently").
    pub leave_alone_label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskMap {
    pub used_bytes: u64,
    pub explained_bytes: u64,
    pub not_explained_bytes: u64,
    pub areas: Vec<DiskMapArea>,
    // Additions beyond the source contract.
    /// The areas add up to more than used bytes by over 1%.
    pub incomplete: bool,
    /// How much more than used bytes the areas add up to.
    pub over_bytes: u64,
    /// When folders were last measured (a deep scan); None before the first.
    pub measured_at: Option<String>,
    /// macOS: purgeable space and local snapshots, reported separately.
    pub purgeable_bytes: Option<u64>,
    pub snapshot_count: Option<u32>,
    /// macOS: the last measurement had Full Disk Access, so it also walked
    /// the system places; what stays untraced is only macOS's own.
    #[serde(default)]
    pub system_measured: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlindSpotKind {
    UnscannedFolder,
    NeedsPermission,
    UnknownFiles,
    ToolUnavailable,
}

/// What resolving a blind spot does. The interface sends the action it was
/// offered; the backend checks it matches the spot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlindSpotAction {
    /// Add the folder to the scan folders and run a deep scan of it.
    ScanFolder,
    /// Count the folder as the person's files and stop asking.
    MarkMine,
    /// Open the operating system's permission settings (macOS Full Disk Access).
    OpenSettings,
    /// Scan again (after starting a tool, or after access was granted).
    ScanAgain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpotState {
    Open,
    /// The action is running (a scan was started for it).
    Working,
    /// Resolved recently; shown with `done_text`.
    Done,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlindSpot {
    pub id: String,
    pub kind: BlindSpotKind,
    pub title: String,
    pub text: String,
    pub location: String,
    pub size_bytes: Option<u64>,
    pub action_label: String,
    // Additions beyond the source contract.
    pub action: BlindSpotAction,
    pub state: SpotState,
    pub done_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceState {
    Found,
    Clean,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceChecked {
    pub detector: String,
    pub label: String,
    pub state: PlaceState,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaGrowth {
    pub id: String,
    pub label: String,
    /// Bytes added over the period (positive: growing).
    pub bytes: i64,
}

/// One day's area totals (`map_snapshots.json`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaySnapshot {
    pub used_bytes: u64,
    pub not_explained_bytes: u64,
    pub areas: BTreeMap<String, u64>,
}

/// A user decision about a blind spot (`blind_spot_decisions`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotDecision {
    /// "mine", "scan", "asked" or "rescan".
    pub decision: String,
    pub at: i64,
    /// The spot as it was when decided, so a resolved spot can still be shown.
    #[serde(default)]
    pub kind: Option<BlindSpotKind>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub bytes: Option<u64>,
}

/// Resolved spots stay visible, marked done, for this long.
pub const DONE_FOR_SECS: i64 = 7 * 86_400;

// ----- claims -------------------------------------------------------------------

/// Bytes a suggestion (or one of its parts) accounts for at known paths.
#[derive(Debug, Clone, PartialEq)]
pub struct Claim {
    pub paths: Vec<PathBuf>,
    pub bytes: u64,
    pub area: String,
}

/// Claims from findings: path claims resolved most specific first (each byte
/// once), and the bytes of tool-level items per area.
pub fn claims_of(findings: &[Finding]) -> (Vec<Claim>, HashMap<String, u64>) {
    let mut claims = Vec::new();
    let mut native: HashMap<String, u64> = HashMap::new();
    let paths_of = |t: &Target| -> Vec<PathBuf> {
        match t {
            Target::Paths { paths } => paths.iter().map(PathBuf::from).collect(),
            _ => vec![],
        }
    };
    for f in findings {
        let area = f.suggestion.map_area.clone();
        if f.parts.is_empty() {
            let paths = paths_of(&f.target);
            let bytes = f.suggestion.full_size_bytes.max(f.suggestion.size_bytes);
            if paths.is_empty() {
                *native.entry(area).or_default() += bytes;
            } else {
                claims.push(Claim { paths, bytes, area });
            }
        } else {
            for p in &f.parts {
                let paths = paths_of(&p.target);
                let bytes = p.suggestion.size_bytes;
                if paths.is_empty() {
                    *native.entry(area.clone()).or_default() += bytes;
                } else {
                    claims.push(Claim { paths, bytes, area: area.clone() });
                }
            }
        }
    }
    (resolve_claims(claims), native)
}

fn max_depth(c: &Claim) -> usize {
    c.paths.iter().map(|p| p.components().count()).max().unwrap_or(0)
}

/// Most specific first: a claim nested inside another is subtracted from it,
/// so a `node_modules` inside a claimed worktree is counted once.
pub fn resolve_claims(mut claims: Vec<Claim>) -> Vec<Claim> {
    claims.sort_by(|a, b| max_depth(b).cmp(&max_depth(a)).then(a.paths.cmp(&b.paths)));
    let mut out: Vec<Claim> = Vec::new();
    for mut c in claims {
        if out.iter().any(|o| o.paths == c.paths) {
            continue;
        }
        let inner: u64 = out.iter().filter(|o| o.paths.iter().all(|p| c.paths.iter().any(|q| p.starts_with(q)))).map(|o| o.bytes).sum();
        c.bytes = c.bytes.saturating_sub(inner);
        out.push(c);
    }
    out
}

// ----- the map ------------------------------------------------------------------

pub struct BuildInput<'a> {
    pub used_bytes: u64,
    pub platform: &'a str,
    pub claims: &'a [Claim],
    pub native: &'a HashMap<String, u64>,
    pub measurement: Option<&'a Measurement>,
    /// Space the platform reports outside the measured volume (macOS: the
    /// other APFS volumes in the container).
    pub system_volume_bytes: u64,
    /// Blind spots the person marked as theirs: spot ID -> decision.
    pub decisions: &'a HashMap<String, SpotDecision>,
    pub purgeable_bytes: Option<u64>,
    pub snapshot_count: Option<u32>,
}

fn innermost_root<'a>(roots: &'a mut [roots::RootBytes], p: &Path) -> Option<&'a mut roots::RootBytes> {
    roots.iter_mut().filter(|r| p.starts_with(&r.path)).max_by_key(|r| r.path.len())
}

pub fn spot_id(kind: BlindSpotKind, identity: &str) -> String {
    let k = match kind {
        BlindSpotKind::UnscannedFolder => "unscanned",
        BlindSpotKind::NeedsPermission => "permission",
        BlindSpotKind::UnknownFiles => "unknown",
        BlindSpotKind::ToolUnavailable => "tool",
    };
    crate::detectors::common::stable_id(&format!("blind.{k}"), identity)
}

pub fn iso(t: i64) -> String {
    chrono::DateTime::from_timestamp(t, 0).map(|d| d.to_rfc3339()).unwrap_or_default()
}

/// Build the map. Pure: every input is passed in.
pub fn build(i: &BuildInput) -> DiskMap {
    let mut path_claimed: HashMap<String, u64> = HashMap::new();
    for c in i.claims {
        *path_claimed.entry(c.area.clone()).or_default() += c.bytes;
    }
    let mut rest: HashMap<String, u64> = HashMap::new();
    let mut vm: HashMap<String, u64> = HashMap::new();
    if let Some(m) = i.measurement {
        let mut roots = m.roots.clone();
        // Reconcile claims that changed since the measurement: a path no
        // longer suggested but still there is leave-alone space again; a newly
        // suggested path was counted in its root, so move it out.
        let now_claimed: Vec<Vec<String>> = i.claims.iter().map(|c| c.paths.iter().map(|p| p.to_string_lossy().into_owned()).collect()).collect();
        for old in &m.claims {
            if !now_claimed.contains(&old.paths) && old.paths.first().is_some_and(|p| Path::new(p).exists()) {
                if let Some(r) = innermost_root(&mut roots, Path::new(&old.paths[0])) {
                    r.bytes += old.bytes;
                }
            }
        }
        for (c, paths) in i.claims.iter().zip(&now_claimed) {
            if !m.claims.iter().any(|o| &o.paths == paths) {
                if let Some(first) = c.paths.first() {
                    if let Some(r) = innermost_root(&mut roots, first) {
                        r.bytes = r.bytes.saturating_sub(c.bytes);
                    }
                }
            }
        }
        for r in &roots {
            *rest.entry(r.area.clone()).or_default() += r.bytes;
        }
        for d in &m.vm_disks {
            *vm.entry(d.area.clone()).or_default() += d.bytes;
        }
        // Folders the person marked as theirs count under Your files.
        for c in m.unknown.iter().chain(&m.unscanned) {
            let mine = [BlindSpotKind::UnknownFiles, BlindSpotKind::UnscannedFolder]
                .iter()
                .any(|k| i.decisions.get(&spot_id(*k, &c.path)).is_some_and(|d| d.decision == "mine"));
            if mine {
                *rest.entry("files".into()).or_default() += c.bytes.unwrap_or(0);
            }
        }
    }
    *rest.entry("system".into()).or_default() += i.system_volume_bytes;
    let mut areas = Vec::new();
    for (id, _, leave) in AREAS {
        let p = path_claimed.get(id).copied().unwrap_or(0);
        let n = i.native.get(id).copied().unwrap_or(0);
        let r = rest.get(id).copied().unwrap_or(0) + vm.get(id).copied().unwrap_or(0);
        // Tool-level items live inside their area's roots (Docker's images in
        // its virtual disk, Homebrew's downloads in Library/Caches), so they
        // never add to them; when the roots are smaller, the items win.
        let bytes = p + r.max(n);
        let leave_label = if id == "system" { format!("{} itself", system_name(i.platform)) } else { leave.to_string() };
        areas.push(DiskMapArea { id: id.into(), label: area_label(id, i.platform), bytes, leave_alone_bytes: bytes - p - n.min(bytes - p), leave_alone_label: leave_label });
    }
    let explained: u64 = areas.iter().map(|a| a.bytes).sum();
    let not_explained = i.used_bytes.saturating_sub(explained);
    let over = explained.saturating_sub(i.used_bytes);
    DiskMap {
        used_bytes: i.used_bytes,
        explained_bytes: explained.min(i.used_bytes),
        not_explained_bytes: not_explained,
        areas,
        incomplete: (over as f64) > (i.used_bytes as f64) * TOLERANCE,
        over_bytes: over,
        measured_at: i.measurement.map(|m| iso(m.at)),
        purgeable_bytes: i.purgeable_bytes,
        snapshot_count: i.snapshot_count,
        system_measured: i.platform == "macos" && i.measurement.is_some_and(|m| m.full_disk_access == Some(true)),
    }
}

// ----- blind spots --------------------------------------------------------------

pub struct SpotInput<'a> {
    pub measurement: Option<&'a Measurement>,
    pub errors: &'a [DetectorError],
    pub decisions: &'a HashMap<String, SpotDecision>,
    pub scan_folders: &'a [PathBuf],
    pub exclusions: &'a [PathBuf],
    /// macOS Full Disk Access right now (None elsewhere or unknown).
    pub full_disk_access: Option<bool>,
    pub scanning: bool,
    pub now: i64,
    /// When the latest scan of any kind finished.
    pub last_scan_at: Option<i64>,
    pub platform: &'a str,
    pub home: &'a Path,
    /// Findings of the latest scan, to say what a resolved spot turned up.
    pub findings: &'a [Finding],
}

pub fn gb_text(b: u64) -> String {
    let g = b as f64 / crate::model::GB as f64;
    if b > 0 && g < 0.05 {
        "<0.1 GB".into()
    } else {
        format!("{g:.1} GB")
    }
}

pub fn display(home: &Path, platform: &str, p: &str) -> String {
    let path = Path::new(p);
    if platform != "windows" {
        let canon = std::fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf());
        for h in [home, canon.as_path()] {
            if let Ok(rest) = path.strip_prefix(h) {
                return if rest.as_os_str().is_empty() { "~".into() } else { format!("~/{}", rest.display()) };
            }
        }
        // macOS: the Data volume path of a firmlinked location.
        if let Some(rest) = p.strip_prefix("/System/Volumes/Data") {
            return rest.to_string();
        }
    }
    p.to_string()
}

fn family_label(family: &str) -> &'static str {
    match family {
        "docker" => "Docker",
        "git" => "Git",
        "project" => "Project tools",
        "cache" => "A package manager",
        "mobile" => "Xcode",
        "files" => "File checks",
        _ => "A tool",
    }
}

fn how_to_start(family: &str, platform: &str) -> &'static str {
    match (family, platform) {
        ("docker", "linux") => "Start the Docker service, then scan again.",
        ("docker", _) => "Start Docker Desktop, then scan again.",
        ("mobile", _) => "Open Xcode once so its tools are set up, then scan again.",
        _ => "Install it or start it, then scan again.",
    }
}

fn found_under(findings: &[Finding], path: &str) -> usize {
    findings
        .iter()
        .filter(|f| {
            let ps: Vec<&String> = if f.parts.is_empty() { f.paths().iter().collect() } else { f.parts.iter().flat_map(|p| p.paths()).collect() };
            ps.iter().any(|p| Path::new(p).starts_with(path))
        })
        .count()
}

fn found_text(n: usize) -> String {
    match n {
        0 => "I found nothing to remove there.".into(),
        1 => "I found 1 thing you could remove.".into(),
        n => format!("I found {n} things you could remove."),
    }
}

/// Current blind spots, open ones first, then recently resolved ones.
pub fn blind_spots(i: &SpotInput) -> Vec<BlindSpot> {
    let mut open: Vec<BlindSpot> = Vec::new();
    let show = |p: &str| display(i.home, i.platform, p);
    let decision = |id: &str| i.decisions.get(id);
    let recent = |d: &SpotDecision| i.now - d.at <= DONE_FOR_SECS;
    if let Some(m) = i.measurement {
        for c in &m.unscanned {
            let p = Path::new(&c.path);
            let id = spot_id(BlindSpotKind::UnscannedFolder, &c.path);
            if i.exclusions.iter().any(|x| p.starts_with(x)) {
                continue;
            }
            if i.scan_folders.iter().any(|f| p.starts_with(f)) {
                // Added to the scan folders but not measured since: it stays
                // open until a deep scan has looked inside.
                if decision(&id).is_some_and(|d| d.decision == "scan" && d.at > m.at) {
                    let loc = show(&c.path);
                    open.push(BlindSpot {
                        id,
                        kind: BlindSpotKind::UnscannedFolder,
                        title: "A folder I was not asked to scan".into(),
                        text: format!("{loc} is in your scan folders now. I look inside during the next deep scan."),
                        location: loc,
                        size_bytes: c.bytes,
                        action_label: "Scan this folder".into(),
                        action: BlindSpotAction::ScanAgain,
                        state: SpotState::Open,
                        done_text: None,
                    });
                }
                continue;
            }
            if decision(&id).is_some_and(|d| d.decision == "mine") {
                continue;
            }
            let loc = show(&c.path);
            let text = match c.bytes {
                Some(b) => format!("{loc} holds {} that no detector looked at.", gb_text(b)),
                None => format!("I was not asked to look inside {loc}, so its size is unknown. macOS asks you first when I do."),
            };
            open.push(BlindSpot {
                id,
                kind: BlindSpotKind::UnscannedFolder,
                title: "A folder I was not asked to scan".into(),
                text,
                location: loc,
                size_bytes: c.bytes,
                action_label: "Scan this folder".into(),
                action: BlindSpotAction::ScanFolder,
                state: SpotState::Open,
                done_text: None,
            });
        }
        for c in &m.unknown {
            let id = spot_id(BlindSpotKind::UnknownFiles, &c.path);
            if decision(&id).is_some_and(|d| d.decision == "mine") || i.exclusions.iter().any(|x| Path::new(&c.path).starts_with(x)) {
                continue;
            }
            let loc = show(&c.path);
            let size = gb_text(c.bytes.unwrap_or(0));
            let text = match &c.ext {
                Some(e) => format!("{loc} has {size} of .{e} files from an unknown tool."),
                None => format!("{loc} has {size} of files from an unknown tool."),
            };
            open.push(BlindSpot {
                id,
                kind: BlindSpotKind::UnknownFiles,
                title: "Files I do not recognize".into(),
                text,
                location: loc,
                size_bytes: c.bytes,
                action_label: "They are mine, keep them".into(),
                action: BlindSpotAction::MarkMine,
                state: SpotState::Open,
                done_text: None,
            });
        }
        // Privacy protections (macOS): one spot with the Full Disk Access flow.
        let private: Vec<&Denial> = m.denied.iter().filter(|d| d.own && d.kind == crate::scan::size::Denied::Privacy).collect();
        if !private.is_empty() {
            let id = spot_id(BlindSpotKind::NeedsPermission, "privacy");
            let loc = show(&private[0].path);
            let more = private.len() - 1;
            // Whole sentences per count, so each language can translate them.
            let places = match more {
                0 => loc.clone(),
                1 => format!("{loc} and 1 more place"),
                _ => format!("{loc} and {more} more places"),
            };
            let granted = i.full_disk_access == Some(true) && m.full_disk_access != Some(true);
            let asked = decision(&id).is_some_and(|d| d.decision == "asked");
            let (text, label, action) = if granted {
                (format!("I can read {places} now. Scan again to see what is inside."), "Scan again", BlindSpotAction::ScanAgain)
            } else if i.platform == "macos" {
                let base = if more == 0 {
                    format!("{places} needs Full Disk Access, so I do not know how much space they use.")
                } else {
                    format!("{places} need Full Disk Access, so I do not know how much space they use.")
                };
                let extra = if asked { " I opened System Settings. Turn on Freeboard under Full Disk Access, then reopen Freeboard." } else { "" };
                (base + extra, "Allow access", BlindSpotAction::OpenSettings)
            } else {
                (format!("{places} cannot be read with your account's permissions."), "Scan again", BlindSpotAction::ScanAgain)
            };
            open.push(BlindSpot {
                id,
                kind: BlindSpotKind::NeedsPermission,
                title: "A place I am not allowed to read".into(),
                text,
                location: loc,
                size_bytes: None,
                action_label: label.into(),
                action,
                state: SpotState::Open,
                done_text: None,
            });
        }
        // File permissions inside the person's own folders.
        let access: Vec<&Denial> = m.denied.iter().filter(|d| d.own && d.kind != crate::scan::size::Denied::Privacy).collect();
        if !access.is_empty() {
            let id = spot_id(BlindSpotKind::NeedsPermission, "access");
            let loc = show(&access[0].path);
            let more = access.len() - 1;
            let places = match more {
                0 => loc.clone(),
                1 => format!("{loc} and 1 more folder"),
                _ => format!("{loc} and {more} more folders"),
            };
            open.push(BlindSpot {
                id,
                kind: BlindSpotKind::NeedsPermission,
                title: "A place I am not allowed to read".into(),
                text: if more == 0 {
                    format!("The permissions on {places} do not let your account read it, so the size is unknown. Change them, then scan again.")
                } else {
                    format!("The permissions on {places} do not let your account read them, so the size is unknown. Change them, then scan again.")
                },
                location: loc,
                size_bytes: None,
                action_label: "Scan again".into(),
                action: BlindSpotAction::ScanAgain,
                state: SpotState::Open,
                done_text: None,
            });
        }
    }
    // Tools that were not available, one spot per tool family.
    let mut families: Vec<(&str, &DetectorError)> = Vec::new();
    for e in i.errors {
        let fam = e.detector.split('.').next().unwrap_or("");
        if !families.iter().any(|(f, _)| *f == fam) {
            families.push((fam, e));
        }
    }
    for (fam, e) in families {
        let id = spot_id(BlindSpotKind::ToolUnavailable, fam);
        let label = family_label(fam);
        open.push(BlindSpot {
            id,
            kind: BlindSpotKind::ToolUnavailable,
            title: "A tool I could not use".into(),
            text: format!("{} {}", e.message.trim_end_matches('.').to_string() + ".", how_to_start(fam, i.platform)),
            location: label.into(),
            size_bytes: None,
            action_label: "Scan again".into(),
            action: BlindSpotAction::ScanAgain,
            state: SpotState::Open,
            done_text: None,
        });
    }
    // A spot whose action is running.
    if i.scanning {
        for s in open.iter_mut() {
            if decision(&s.id).is_some_and(|d| matches!(d.decision.as_str(), "scan" | "rescan") && recent(d)) {
                s.state = SpotState::Working;
                s.done_text = Some("Scanning it now. This can take a few minutes.".into());
            }
        }
    }
    // Recently resolved spots, shown done.
    let mut done: Vec<BlindSpot> = Vec::new();
    let mut decided: Vec<(&String, &SpotDecision)> = i.decisions.iter().collect();
    decided.sort_by(|a, b| b.1.at.cmp(&a.1.at).then(a.0.cmp(b.0)));
    for (id, d) in decided {
        if open.iter().any(|s| &s.id == id) || !recent(d) || d.title.is_empty() {
            continue;
        }
        let Some(kind) = d.kind else { continue };
        // Done only once a scan after the decision really saw the change.
        let measured_at = i.measurement.map(|m| m.at).unwrap_or(i64::MIN);
        let seen_after = match (kind, d.decision.as_str()) {
            (_, "mine") => true,
            (BlindSpotKind::ToolUnavailable, _) => i.last_scan_at.is_some_and(|t| t >= d.at),
            _ => measured_at >= d.at,
        };
        if !seen_after {
            continue;
        }
        let text = match (kind, d.decision.as_str()) {
            (_, "mine") => "Marked as yours. I will not ask again.".to_string(),
            (BlindSpotKind::UnscannedFolder, _) => format!("Scanned. {}", found_text(found_under(i.findings, &d.path))),
            (BlindSpotKind::NeedsPermission, _) => "Now readable. Its space is on the map.".into(),
            (BlindSpotKind::ToolUnavailable, _) => "Available again. I looked inside.".into(),
            (BlindSpotKind::UnknownFiles, _) => "Resolved.".into(),
        };
        done.push(BlindSpot {
            id: id.clone(),
            kind,
            title: d.title.clone(),
            text: String::new(),
            location: d.location.clone(),
            size_bytes: d.bytes,
            action_label: String::new(),
            action: match kind {
                BlindSpotKind::UnscannedFolder => BlindSpotAction::ScanFolder,
                BlindSpotKind::UnknownFiles => BlindSpotAction::MarkMine,
                BlindSpotKind::NeedsPermission => BlindSpotAction::OpenSettings,
                BlindSpotKind::ToolUnavailable => BlindSpotAction::ScanAgain,
            },
            state: SpotState::Done,
            done_text: Some(text),
        });
    }
    open.extend(done);
    open
}

/// Where a blind spot's folder is (backend only; never sent to the interface).
pub fn spot_path(m: Option<&Measurement>, id: &str) -> Option<String> {
    let m = m?;
    m.unscanned
        .iter()
        .map(|c| (BlindSpotKind::UnscannedFolder, c))
        .chain(m.unknown.iter().map(|c| (BlindSpotKind::UnknownFiles, c)))
        .find(|(k, c)| spot_id(*k, &c.path) == id)
        .map(|(_, c)| c.path.clone())
}

// ----- places checked -------------------------------------------------------------

/// Readable names for each detector (prototype `placesList`).
pub fn detector_label(id: &str) -> String {
    match id {
        "docker.build_cache" => "Docker build cache",
        "docker.dangling_images" => "Untagged Docker images",
        "docker.unused_images" => "Unused Docker images",
        "docker.stopped_containers" => "Stopped Docker containers",
        "docker.volumes" => "Docker volumes",
        "docker.disk_slack" => "Empty space in Docker's disk",
        "git.worktrees" => "Git worktrees",
        "project.dependencies" => "Project dependencies",
        "project.build_output" => "Build output",
        "cache.pnpm" => "pnpm store",
        "cache.pip" => "pip cache",
        "cache.npm" => "npm cache",
        "cache.homebrew" => "Homebrew cache",
        "cache.cargo" => "Cargo registry",
        "cache.gradle" => "Gradle caches",
        "cache.jetbrains" => "JetBrains IDE caches",
        "cache.electron_apps" => "Slack, Discord and Teams caches",
        "cache.playwright" => "Playwright browsers",
        "cache.xcode" => "Xcode DerivedData",
        "mobile.simulators" => "iOS simulators",
        "mobile.ios_backups" => "iPhone and iPad backups",
        "mobile.android_avd" => "Android emulators",
        "ai.ollama" => "Ollama models",
        "ai.huggingface" => "Hugging Face downloads",
        "ai.lm_studio" => "LM Studio models",
        "files.installers" => "Installers in Downloads",
        "files.windows_temp" => "Windows temp files",
        other => other,
    }
    .to_string()
}

/// Update places checked with the detectors that ran in this scan. Detectors
/// that did not run keep their last result. A detector that failed is
/// unavailable, never clean.
pub fn merge_places(prev: Vec<PlaceChecked>, ran: &[&str], findings: &[Finding], errors: &[DetectorError]) -> Vec<PlaceChecked> {
    let mut out: Vec<PlaceChecked> = prev.into_iter().filter(|p| !ran.contains(&p.detector.as_str())).collect();
    for id in ran {
        let failed = errors.iter().any(|e| e.detector == *id || e.detector.starts_with(&format!("{id}.")));
        let bytes: u64 = findings.iter().filter(|f| f.suggestion.detector == *id || f.suggestion.detector.starts_with(&format!("{id}."))).map(|f| f.suggestion.full_size_bytes.max(f.suggestion.size_bytes)).sum();
        let found = findings.iter().any(|f| f.suggestion.detector == *id || f.suggestion.detector.starts_with(&format!("{id}.")));
        let state = if failed {
            PlaceState::Unavailable
        } else if found {
            PlaceState::Found
        } else {
            PlaceState::Clean
        };
        out.push(PlaceChecked { detector: id.to_string(), label: detector_label(id), state, bytes: if failed { 0 } else { bytes } });
    }
    let order: HashMap<&str, usize> = crate::detectors::all().iter().enumerate().map(|(n, d)| (d.id(), n)).collect::<Vec<_>>().into_iter().collect();
    out.sort_by_key(|p| order.get(p.detector.as_str()).copied().unwrap_or(usize::MAX));
    out
}

// ----- growth -----------------------------------------------------------------

/// Area changes between the newest day and the newest day at least `days`
/// old (or the oldest day there is, if it is at least a day old). Growing
/// areas only, fastest first.
pub fn growth(days: &[(i64, DaySnapshot)], window: u32, platform: &str) -> Vec<AreaGrowth> {
    let Some((today, latest)) = days.iter().max_by_key(|(d, _)| *d) else { return vec![] };
    let target = today - window as i64;
    let base = days.iter().filter(|(d, _)| *d <= target).max_by_key(|(d, _)| *d).or_else(|| days.iter().filter(|(d, _)| d < today).min_by_key(|(d, _)| *d));
    let Some((_, old)) = base else { return vec![] };
    let mut out: Vec<AreaGrowth> = latest
        .areas
        .iter()
        .map(|(id, b)| AreaGrowth { id: id.clone(), label: area_label(id, platform), bytes: *b as i64 - old.areas.get(id).copied().unwrap_or(0) as i64 })
        .filter(|g| g.bytes >= 50_000_000)
        .collect();
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.id.cmp(&b.id)));
    out
}

// ----- the disk report ----------------------------------------------------------

fn gb1(b: u64) -> f64 {
    (b as f64 / crate::model::GB as f64 * 10.0).round() / 10.0
}

fn risk_label(r: Risk) -> &'static str {
    match r {
        Risk::Safe => "Safe",
        Risk::Review => "Review",
        Risk::Risky => "Risky",
    }
}

pub struct ReportInput<'a> {
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub platform: &'a str,
    pub map: &'a DiskMap,
    pub spots: &'a [BlindSpot],
    pub suggestions: &'a [crate::model::Suggestion],
}

/// The copyable disk report (prototype `reportText`): sizes, folder names and
/// risk levels. Built only from the map, the blind spots and the suggestions'
/// titles and locations; no file is ever read for it.
pub fn report(i: &ReportInput) -> serde_json::Value {
    let un = i.map.not_explained_bytes;
    let used = i.map.used_bytes;
    let pct = if used == 0 { 100.0 } else { (used - un.min(used)) as f64 / used as f64 * 100.0 };
    let mut areas: Vec<serde_json::Value> = i.map.areas.iter().map(|a| serde_json::json!({ "name": a.label, "gb": gb1(a.bytes) })).collect();
    areas.push(serde_json::json!({ "name": "Not explained", "gb": gb1(un) }));
    let suggestions: Vec<serde_json::Value> =
        i.suggestions.iter().map(|s| serde_json::json!({ "name": s.title, "gb": gb1(s.size_bytes), "risk": risk_label(s.risk), "where": s.location })).collect();
    let not_explained: Vec<serde_json::Value> = i
        .spots
        .iter()
        .filter(|s| s.state != SpotState::Done)
        .map(|s| serde_json::json!({ "what": s.text, "gb": s.size_bytes.map(gb1) }))
        .collect();
    let mut v = serde_json::json!({
        "disk_gb": gb1(i.total_bytes),
        "free_gb": gb1(i.free_bytes),
        "explained_pct": (pct * 10.0).round() / 10.0,
        "areas": areas,
        "suggestions": suggestions,
        "not_explained": not_explained,
    });
    let o = v.as_object_mut().unwrap();
    o.insert("platform".into(), i.platform.into());
    if i.map.incomplete {
        o.insert("incomplete_over_gb".into(), gb1(i.map.over_bytes).into());
    }
    if let Some(p) = i.map.purgeable_bytes {
        o.insert("purgeable_gb".into(), gb1(p).into());
    }
    if let Some(n) = i.map.snapshot_count {
        o.insert("local_snapshots".into(), n.into());
    }
    if i.map.measured_at.is_none() {
        o.insert("folders_measured".into(), false.into());
    }
    v
}

// ----- platform -----------------------------------------------------------------

/// macOS: whether Freeboard has Full Disk Access. Opening the system TCC
/// database read-only succeeds only with it (nothing is read, and macOS shows
/// no prompt for this check). None on other platforms.
pub fn full_disk_access(platform: &str) -> Option<bool> {
    if platform != "macos" || !cfg!(target_os = "macos") {
        return None;
    }
    let p = Path::new("/Library/Application Support/com.apple.TCC/TCC.db");
    if !p.exists() {
        return None;
    }
    Some(std::fs::File::open(p).is_ok())
}

/// macOS: bytes used by the APFS volumes that share the home volume's
/// container but are not measured by walking (System, Preboot, VM, Update,
/// Recovery and other unmounted volumes). Read with statvfs only. 0 elsewhere.
pub fn platform_volume_bytes(platform: &str) -> u64 {
    #[cfg(target_os = "macos")]
    if platform == "macos" {
        return macos_volumes::other_volume_bytes();
    }
    let _ = platform;
    0
}

#[cfg(target_os = "macos")]
mod macos_volumes {
    /// One volume's own size figures from `statfs`. (`statvfs` on APFS
    /// reports the whole container's blocks and free space for every volume,
    /// so its "used" is the container's: found on a real Mac, 2026-11-07,
    /// where four system volumes were each counted as 921 GB.)
    fn statfs(p: &str) -> Option<libc::statfs> {
        let c = std::ffi::CString::new(p).ok()?;
        // SAFETY: valid NUL-terminated path and a zeroed out-parameter.
        unsafe {
            let mut st: libc::statfs = std::mem::zeroed();
            (libc::statfs(c.as_ptr(), &mut st) == 0).then_some(st)
        }
    }

    const DATA: &str = "/System/Volumes/Data";

    /// Space the APFS container uses for everything but the Data volume
    /// (System, Preboot, VM, Update, Recovery and other volumes): the
    /// container's used space minus the Data volume's own used space.
    pub fn other_volume_bytes() -> u64 {
        let Some(data) = statfs(DATA) else { return 0 };
        let bsize = data.f_bsize as u64;
        let total = data.f_blocks * bsize;
        // Free space is shared by every volume in the container.
        let container_free = data.f_bavail * bsize;
        let container_used = total.saturating_sub(container_free);
        let data_used = (data.f_blocks.saturating_sub(data.f_bfree)) * bsize;
        container_used.saturating_sub(data_used)
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn other_volumes_are_less_than_the_container() {
            // On a real Mac: never more than the container uses in all.
            let Some(d) = super::statfs(super::DATA) else { return };
            let total = d.f_blocks * d.f_bsize as u64;
            let others = super::other_volume_bytes();
            assert!(others < total, "{others} >= container {total}");
        }
    }
}
