//! Phones and emulators: old iPhone and iPad backups and Android emulators
//! not started for 90 days, one part per backup or emulator. Unavailable iOS
//! simulators are in `caches.rs` (`mobile.simulators`).

use std::path::{Path, PathBuf};

use super::common::*;
use super::Probe;
use crate::guard::Guard;
use crate::model::*;
use crate::scan::{Detector, ScanContext};

pub const IOS_BACKUPS: &str = "mobile.ios_backups";
pub const ANDROID_AVD: &str = "mobile.android_avd";
/// A device still in use backs up far more often than this.
pub const BACKUP_DAYS: i64 = 180;
pub const AVD_DAYS: i64 = 90;

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

// ----- iPhone and iPad backups -----------------------------------------------

pub struct IosBackups;

pub fn backup_roots(ctx: &ScanContext) -> Vec<PathBuf> {
    match ctx.platform {
        "macos" => vec![ctx.home.join("Library/Application Support/MobileSync/Backup")],
        "windows" => vec![
            ctx.home.join("AppData").join("Roaming").join("Apple Computer").join("MobileSync").join("Backup"),
            // iTunes from the Microsoft Store.
            ctx.home.join("Apple").join("MobileSync").join("Backup"),
        ],
        _ => vec![],
    }
}

/// The value after `<key>name</key>` in an XML property list.
fn plist_value(xml: &str, key: &str) -> Option<String> {
    let rest = &xml[xml.find(&format!("<key>{key}</key>"))? + key.len() + 11..];
    let open = rest.find('<')?;
    let rest = &rest[open..];
    let start = rest.find('>')? + 1;
    let end = rest[start..].find('<')? + start;
    let v = rest[start..end].trim();
    (!v.is_empty()).then(|| v.replace("&lt;", "<").replace("&gt;", ">").replace("&apos;", "'").replace("&quot;", "\"").replace("&amp;", "&"))
}

/// Device name and last backup time from a backup's `Info.plist` (XML).
pub fn backup_info(xml: &str) -> (Option<String>, Option<i64>) {
    let name = plist_value(xml, "Device Name");
    let when = plist_value(xml, "Last Backup Date").and_then(|d| parse_time(&d));
    (name, when)
}

impl Detector for IosBackups {
    fn id(&self) -> &'static str {
        IOS_BACKUPS
    }
    fn platforms(&self) -> &'static [&'static str] {
        &["macos", "windows"]
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        if !may_read_protected(ctx) {
            return Err(err(IOS_BACKUPS, "permission", "Device backups are protected by macOS and need Full Disk Access."));
        }
        let mut entries = vec![];
        let mut shown = None;
        for root in backup_roots(ctx) {
            let Ok(rd) = std::fs::read_dir(&root) else { continue };
            shown.get_or_insert(root.clone());
            for e in rd.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())) {
                let dir = e.path();
                let (name, when) = std::fs::read_to_string(dir.join("Info.plist")).map(|x| backup_info(&x)).unwrap_or((None, None));
                // Without the backup's own date, the folder's change time.
                let when = when.or_else(|| std::fs::metadata(&dir).ok().map(|m| crate::platform::modified_secs(&m)));
                let Some(when) = when else { continue };
                if days_since(ctx.now, when) < BACKUP_DAYS {
                    continue;
                }
                let label = name.unwrap_or_else(|| format!("Backup {}", &name_of(&dir)[..name_of(&dir).len().min(8)]));
                entries.push((label, vec![dir], Some((Some(when), format!("Last backed up {}", ago(ctx.now, when))))));
            }
        }
        let Some(root) = shown else { return Ok(vec![]) };
        let tw = trash_word(ctx);
        let item = FolderItem {
            detector: IOS_BACKUPS,
            group: Group::PhonesEmulators,
            identity: "ios-backups",
            title: "Old iPhone and iPad backups".into(),
            location: display_path(ctx, &root),
            risk: Risk::Review,
            action: Action::Trash,
            reason: format!("Backups not updated for {BACKUP_DAYS} days or more, often of a phone or tablet you no longer use. A backup may be the only copy of that device's data."),
            recovery: format!("Restore them from the {tw}."),
        };
        Ok(split_finding(ctx, item, entries).into_iter().collect())
    }
}

// ----- Android emulators -----------------------------------------------------

pub struct AndroidAvd;

pub fn avd_home(ctx: &ScanContext) -> PathBuf {
    if let Some(p) = env_override(ctx, "ANDROID_AVD_HOME") {
        return p;
    }
    if let Some(p) = env_override(ctx, "ANDROID_USER_HOME") {
        return p.join("avd");
    }
    ctx.home.join(".android").join("avd")
}

/// The emulator holds `*.lock` files in the AVD folder while it runs.
pub fn avd_running(dir: &Path) -> Option<bool> {
    let rd = std::fs::read_dir(dir).ok()?;
    Some(rd.flatten().any(|e| e.file_name().to_string_lossy().ends_with(".lock")))
}

/// Newest change inside an AVD (the emulator writes its disk images and
/// settings while it runs).
fn avd_last_started(dir: &Path) -> Option<i64> {
    crate::scan::size::measure(dir, &Default::default()).newest_mtime
}

impl Detector for AndroidAvd {
    fn id(&self) -> &'static str {
        ANDROID_AVD
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let home = avd_home(ctx);
        let Ok(rd) = std::fs::read_dir(&home) else { return Ok(vec![]) };
        let mut entries = vec![];
        let mut dirs: Vec<PathBuf> =
            rd.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) && name_of(&e.path()).ends_with(".avd")).map(|e| e.path()).collect();
        dirs.sort();
        for dir in dirs {
            if avd_running(&dir) != Some(false) {
                continue;
            }
            let Some(last) = avd_last_started(&dir) else { continue };
            if days_since(ctx.now, last) < AVD_DAYS {
                continue;
            }
            let name = name_of(&dir).trim_end_matches(".avd").to_string();
            let mut paths = vec![dir.clone()];
            let ini = home.join(format!("{name}.ini"));
            if ini.is_file() {
                paths.push(ini);
            }
            entries.push((name.replace('_', " "), paths, Some((Some(last), format!("Not started in {}+ days", days_since(ctx.now, last))))));
        }
        let tw = trash_word(ctx);
        let item = FolderItem {
            detector: ANDROID_AVD,
            group: Group::PhonesEmulators,
            identity: "android-avds",
            title: "Android emulators not started for 90+ days".into(),
            location: display_path(ctx, &home),
            risk: Risk::Review,
            action: Action::Trash,
            reason: "Emulators with their disk images and saved state. Apps installed in them are removed too.".into(),
            recovery: format!("Restore them from the {tw}, or create the emulator again in Android Studio."),
        };
        Ok(split_finding(ctx, item, entries).into_iter().collect())
    }
}

// ----- pre-cleanup checks ----------------------------------------------------

pub fn probe(f: &Finding, ctx: &ScanContext, guard: &Guard) -> Probe {
    let r = super::files::probe_paths(f, ctx, guard);
    if r != Probe::Unchanged || f.suggestion.detector != ANDROID_AVD {
        return r;
    }
    for p in f.paths().iter().map(Path::new).filter(|p| p.is_dir()) {
        match avd_running(p) {
            Some(false) => {}
            Some(true) => return Probe::Changed("This emulator is running. Close it, then scan again.".into()),
            None => return Probe::Unknown("Freeboard couldn't check whether this emulator is running.".into()),
        }
    }
    Probe::Unchanged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_backup_info() {
        let xml = "<plist><dict><key>Device Name</key>\n\t<string>Ann&apos;s iPhone</string><key>Last Backup Date</key>\n\t<date>2025-01-02T03:04:05Z</date></dict></plist>";
        let (n, t) = backup_info(xml);
        assert_eq!(n.as_deref(), Some("Ann's iPhone"));
        assert_eq!(t, parse_time("2025-01-02T03:04:05Z"));
        assert_eq!(backup_info("<plist/>"), (None, None));
    }
}
