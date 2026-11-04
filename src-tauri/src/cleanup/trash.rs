//! Scoped Trash handling.
//!
//! Items Freeboard moves to the Trash are recorded with their original path,
//! time and file identity. Emptying touches only those records: each one is
//! matched in the Trash by identity (macOS, Linux) or original path and
//! deletion time (Windows Recycle Bin). Unrelated Trash content is never
//! touched. An item that can't be found is reported as gone (restored or
//! emptied outside Freeboard), never guessed.

use std::path::{Path, PathBuf};

use crate::guard::{CheckedPath, Guard};
use crate::platform;

#[derive(Debug, Clone, PartialEq)]
pub struct TrashRecord {
    pub id: i64,
    pub run_id: String,
    pub item_id: String,
    pub original_path: String,
    pub trashed_at: i64,
    pub bytes: u64,
    pub identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PurgeOutcome {
    /// Permanently deleted from the Trash.
    Purged,
    /// No longer in the Trash (restored or emptied outside Freeboard).
    Gone,
}

pub trait TrashBackend: Send + Sync {
    /// Move a checked path to the Trash. Err when no usable Trash exists.
    fn trash(&self, guard: &Guard, checked: &CheckedPath) -> Result<(), String>;
    /// Permanently delete exactly this record's item from the Trash.
    fn purge(&self, guard: &Guard, rec: &TrashRecord) -> Result<PurgeOutcome, String>;
}

/// The platform Trash (macOS Trash, freedesktop Trash, Windows Recycle Bin).
pub struct NativeTrash;

impl TrashBackend for NativeTrash {
    fn trash(&self, guard: &Guard, checked: &CheckedPath) -> Result<(), String> {
        guard.move_to_trash(checked).map_err(|e| e.to_string())
    }

    fn purge(&self, guard: &Guard, rec: &TrashRecord) -> Result<PurgeOutcome, String> {
        purge_native(guard, rec)
    }
}

/// Find the top-level entry in `dirs` whose file identity matches.
pub fn find_by_identity(dirs: &[PathBuf], identity: &str) -> Option<PathBuf> {
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if let Ok(m) = std::fs::symlink_metadata(&p) {
                if platform::file_id(&p, &m).map(|i| i.encode()).as_deref() == Some(identity) {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Remove a located Trash entry through the guard (never follows links).
pub fn remove_entry(guard: &Guard, p: &Path) -> Result<(), String> {
    let c = guard.check(p).map_err(|e| e.to_string())?;
    guard.remove(&c).map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn purge_native(guard: &Guard, rec: &TrashRecord) -> Result<PurgeOutcome, String> {
    let identity = rec.identity.as_deref().ok_or("Freeboard can't identify this item in the Trash.")?;
    let mut dirs = vec![guard.home().join(".Trash")];
    let orig = Path::new(&rec.original_path);
    if let Ok(rest) = orig.strip_prefix("/Volumes") {
        if let Some(vol) = rest.components().next() {
            // SAFETY: getuid has no preconditions.
            let uid = unsafe { libc::getuid() };
            dirs.push(Path::new("/Volumes").join(vol).join(".Trashes").join(uid.to_string()));
        }
    }
    match find_by_identity(&dirs, identity) {
        Some(p) => remove_entry(guard, &p).map(|_| PurgeOutcome::Purged),
        None => Ok(PurgeOutcome::Gone),
    }
}

#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
fn purge_native(guard: &Guard, rec: &TrashRecord) -> Result<PurgeOutcome, String> {
    let _ = guard;
    let items = trash::os_limited::list().map_err(|e| e.to_string())?;
    let orig = PathBuf::from(&rec.original_path);
    let mut best: Option<(i64, trash::TrashItem)> = None;
    for it in items {
        if it.original_path() != orig {
            continue;
        }
        let dt = (it.time_deleted - rec.trashed_at).abs();
        if dt > 600 {
            continue;
        }
        #[cfg(unix)]
        {
            // freedesktop: the item lives at <trash>/files/<name>; its
            // identity must match what Freeboard moved.
            let info = PathBuf::from(&it.id);
            let files = info.parent().and_then(|p| p.parent()).map(|t| t.join("files").join(&it.name));
            let matches = match (files, rec.identity.as_deref()) {
                (Some(f), Some(id)) => std::fs::symlink_metadata(&f)
                    .ok()
                    .and_then(|m| platform::file_id(&f, &m))
                    .is_some_and(|fid| fid.encode() == id),
                _ => false,
            };
            if !matches {
                continue;
            }
        }
        if best.as_ref().is_none_or(|(d, _)| dt < *d) {
            best = Some((dt, it));
        }
    }
    match best {
        Some((_, it)) => trash::os_limited::purge_all([it]).map(|_| PurgeOutcome::Purged).map_err(|e| e.to_string()),
        None => Ok(PurgeOutcome::Gone),
    }
}

/// Test backend: a plain directory acts as the Trash. Items are moved with a
/// rename and matched by identity, like the macOS implementation.
#[cfg(test)]
pub struct DirTrash {
    pub dir: PathBuf,
    pub available: bool,
}

#[cfg(test)]
impl TrashBackend for DirTrash {
    fn trash(&self, guard: &Guard, checked: &CheckedPath) -> Result<(), String> {
        if !self.available {
            return Err("this volume has no Trash".into());
        }
        let now = guard.check(&checked.path).map_err(|e| e.to_string())?;
        if now.canonical != checked.canonical || now.id != checked.id {
            return Err("changed".into());
        }
        let name = checked.canonical.file_name().unwrap();
        let mut dest = self.dir.join(name);
        let mut n = 1;
        while dest.exists() {
            dest = self.dir.join(format!("{} {n}", name.to_string_lossy()));
            n += 1;
        }
        std::fs::rename(&checked.canonical, dest).map_err(|e| e.to_string())
    }
    fn purge(&self, _guard: &Guard, rec: &TrashRecord) -> Result<PurgeOutcome, String> {
        let id = rec.identity.as_deref().ok_or("no identity")?;
        match find_by_identity(std::slice::from_ref(&self.dir), id) {
            Some(p) => {
                let m = std::fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
                if m.is_dir() {
                    std::fs::remove_dir_all(&p).map_err(|e| e.to_string())?;
                } else {
                    std::fs::remove_file(&p).map_err(|e| e.to_string())?;
                }
                Ok(PurgeOutcome::Purged)
            }
            None => Ok(PurgeOutcome::Gone),
        }
    }
}
