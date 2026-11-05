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

/// Identity of an item moved to the Trash: device and inode (or volume and
/// file ID), plus its kind, modification time to the nanosecond and, for
/// files, its length. A move to the Trash is a rename, which keeps all of
/// these. Inode numbers alone are not enough: Linux reuses them immediately,
/// so an unrelated item created after the original was deleted could carry
/// the same device and inode.
pub fn trash_identity(path: &Path) -> Option<String> {
    let m = std::fs::symlink_metadata(path).ok()?;
    let id = platform::file_id(path, &m)?;
    let kind = if platform::is_link_meta(path, &m) {
        "l"
    } else if m.is_dir() {
        "d"
    } else {
        "f"
    };
    let mtime = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?;
    let len = if kind == "f" { m.len() } else { 0 };
    Some(format!("{}:{kind}:{}.{:09}:{len}", id.encode(), mtime.as_secs(), mtime.subsec_nanos()))
}

/// Message shown when the Trash cannot be read (macOS needs Full Disk Access).
pub const TRASH_NOT_READABLE: &str = "Freeboard can't look inside the Trash. On macOS, allow Full Disk Access for Freeboard in System Settings → Privacy & Security, then try again. Nothing was deleted";

/// Find the top-level entry in `dirs` whose file identity matches.
/// A Trash directory that exists but cannot be read is an error, never
/// "not found": reporting an item as gone when Freeboard simply could not
/// look would hide space that is still held.
pub fn find_by_identity(dirs: &[PathBuf], identity: &str) -> Result<Option<PathBuf>, String> {
    for d in dirs {
        let rd = match std::fs::read_dir(d) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(TRASH_NOT_READABLE.into()),
        };
        for e in rd {
            let p = e.map_err(|_| TRASH_NOT_READABLE.to_string())?.path();
            if trash_identity(&p).as_deref() == Some(identity) {
                return Ok(Some(p));
            }
        }
    }
    Ok(None)
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
    match find_by_identity(&dirs, identity)? {
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
                (Some(f), Some(id)) => trash_identity(&f).as_deref() == Some(id),
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
        match find_by_identity(std::slice::from_ref(&self.dir), id)? {
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

#[cfg(test)]
mod identity_tests {
    use super::trash_identity;

    #[cfg(unix)]
    #[test]
    fn unreadable_trash_is_an_error_not_gone() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let t = d.path().join("Trash");
        std::fs::create_dir(&t).unwrap();
        std::fs::write(t.join("item"), b"x").unwrap();
        let id = super::trash_identity(&t.join("item")).unwrap();
        std::fs::set_permissions(&t, std::fs::Permissions::from_mode(0o000)).unwrap();
        let r = super::find_by_identity(std::slice::from_ref(&t), &id);
        std::fs::set_permissions(&t, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Root can read anything; the check only means something as a normal user.
        if unsafe { libc::geteuid() } != 0 {
            assert_eq!(r, Err(super::TRASH_NOT_READABLE.to_string()));
        }
        // A missing Trash directory is simply "not found".
        assert_eq!(super::find_by_identity(&[d.path().join("missing")], &id), Ok(None));
    }

    #[test]
    fn rename_keeps_identity() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a");
        std::fs::create_dir(&a).unwrap();
        std::fs::write(a.join("x"), b"x").unwrap();
        let before = trash_identity(&a).unwrap();
        let b = d.path().join("trash-b");
        std::fs::rename(&a, &b).unwrap();
        assert_eq!(trash_identity(&b).unwrap(), before);
    }

    #[test]
    fn same_inode_with_different_content_is_a_different_item() {
        // Simulates inode reuse: the device and inode stay equal, but the
        // modification time (and here also the length) differ.
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("f");
        std::fs::write(&p, b"original").unwrap();
        let f = std::fs::File::options().write(true).open(&p).unwrap();
        f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000)).unwrap();
        let original = trash_identity(&p).unwrap();
        f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_001)).unwrap();
        assert_ne!(trash_identity(&p).unwrap(), original, "mtime is part of the identity");
        std::fs::write(&p, b"a different item").unwrap();
        f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000)).unwrap();
        assert_ne!(trash_identity(&p).unwrap(), original, "length is part of the identity");
    }
}
