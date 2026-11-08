//! Allocated-size measurement without following links.
//!
//! * Size is the actual on-disk allocation (`st_blocks` on Unix,
//!   `GetCompressedFileSizeW` on Windows) via `platform::allocated_size`.
//! * Each hard-linked file is counted once (device + inode / volume + file ID).
//! * A hard-linked file whose links are not all inside the measured tree is
//!   "shared": removing the tree does not free it. This is how pnpm-managed
//!   `node_modules` (hard links into the content store) avoid inflated
//!   reclaim estimates.
//! * Links, junctions and reparse points are never followed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::platform::{self, FileId};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Measured {
    /// Allocated bytes, each hard-linked file counted once.
    pub bytes: u64,
    /// Bytes in files whose link count > 1 and whose links are not all inside
    /// the tree (shared with something outside). Not independently reclaimable.
    pub shared_bytes: u64,
    pub files: u64,
    /// Newest modification time seen (unix seconds).
    pub newest_mtime: Option<i64>,
    /// True when parts of the tree could not be read.
    pub incomplete: bool,
    /// The files' own sizes (st_size), each hard-linked file once. Used to
    /// tell whether contents changed: allocated bytes also change when the
    /// file system settles a freshly written file (an ext4 extent block
    /// appeared between scan and cleanup; Linux VM, 2026-11-07).
    pub content_bytes: u64,
}

impl Measured {
    /// Bytes that removing the tree would really free.
    pub fn reclaimable(&self) -> u64 {
        self.bytes.saturating_sub(self.shared_bytes)
    }
    /// True when the figure is not exact (shared data or unreadable parts).
    pub fn is_estimate(&self) -> bool {
        self.shared_bytes > 0 || self.incomplete
    }
}

#[derive(Debug, Clone, Default)]
pub struct MeasureOpts {
    /// Absolute paths to skip (user exclusions).
    pub exclusions: Vec<PathBuf>,
    /// Walk on the calling thread only (used for low-priority deep scans and
    /// for small trees).
    pub serial: bool,
}

/// What the walker's threads learn about one entry, so the per-file system
/// calls run in parallel rather than on the thread reading the results.
#[derive(Debug, Clone, Copy)]
enum Facts {
    /// A link, junction or reparse point: never followed, counts nothing.
    Link,
    /// A folder's own allocation.
    Dir(u64),
    File { size: u64, len: u64, mtime: i64, nlink: u64, id: Option<FileId> },
}

fn facts_of(p: &Path) -> Option<Facts> {
    let m = std::fs::symlink_metadata(p).ok()?;
    Some(if platform::is_link_meta(p, &m) {
        Facts::Link
    } else if m.is_dir() {
        Facts::Dir(platform::allocated_size(p, &m))
    } else {
        let (size, nlink, id) = if m.is_file() { platform::file_facts(p, &m) } else { (platform::allocated_size(p, &m), 1, None) };
        Facts::File { size, len: m.len(), mtime: platform::modified_secs(&m), nlink, id }
    })
}

#[derive(Default)]
struct Acc {
    m: Measured,
    /// id -> (links seen inside the tree, total link count, allocated bytes)
    multi: HashMap<FileId, (u64, u64, u64)>,
}

impl Acc {
    fn file(&mut self, path: &Path, meta: &std::fs::Metadata) {
        let (size, nlink, id) = if meta.is_file() { platform::file_facts(path, meta) } else { (platform::allocated_size(path, meta), 1, None) };
        self.add(Facts::File { size, len: meta.len(), mtime: platform::modified_secs(meta), nlink, id });
    }
    fn add(&mut self, f: Facts) {
        match f {
            Facts::Link => {}
            Facts::Dir(size) => self.m.bytes += size,
            Facts::File { size, len, mtime, nlink, id } => {
                self.m.newest_mtime = Some(self.m.newest_mtime.map_or(mtime, |n| n.max(mtime)));
                self.m.files += 1;
                if let (true, Some(id)) = (nlink > 1, id) {
                    // Hard-linked files are added once, in finish(), with their length.
                    let e = self.multi.entry(id).or_insert((0, nlink, size));
                    if e.0 == 0 {
                        self.m.content_bytes += len;
                    }
                    e.0 += 1;
                    return;
                }
                self.m.content_bytes += len;
                self.m.bytes += size;
            }
        }
    }
    fn finish(mut self) -> Measured {
        for (_, (seen, nlink, size)) in self.multi {
            self.m.bytes += size;
            if seen < nlink {
                self.m.shared_bytes += size;
            }
        }
        self.m
    }
}

fn excluded(p: &Path, ex: &[PathBuf]) -> bool {
    ex.iter().any(|x| p.starts_with(x))
}

thread_local! {
    static FORCE_SERIAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Run `f` with every `measure` on this thread walking serially. Quick scans
/// already run detectors in parallel; nested walker pools would only compete.
pub fn with_serial<T>(f: impl FnOnce() -> T) -> T {
    FORCE_SERIAL.with(|c| c.set(true));
    let r = f();
    FORCE_SERIAL.with(|c| c.set(false));
    r
}

pub fn measure(path: &Path, opts: &MeasureOpts) -> Measured {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return Measured { incomplete: true, ..Default::default() },
    };
    let mut acc = Acc::default();
    if platform::is_link_meta(path, &meta) {
        // A link is measured as the link itself; its target is never visited.
        return acc.finish();
    }
    if !meta.is_dir() {
        acc.file(path, &meta);
        return acc.finish();
    }
    if excluded(path, &opts.exclusions) {
        return acc.finish();
    }
    let ex = Arc::new(opts.exclusions.clone());
    let par = if opts.serial || FORCE_SERIAL.with(|c| c.get()) {
        jwalk::Parallelism::Serial
    } else {
        let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(2, 8);
        jwalk::Parallelism::RayonNewPool(n)
    };
    let walker = jwalk::WalkDirGeneric::<((), Option<Facts>)>::new(path)
        .follow_links(false)
        .skip_hidden(false)
        .sort(false)
        .parallelism(par)
        .process_read_dir(move |_depth, _dir, _state, children| {
            children.retain(|r| match r {
                Ok(e) => !excluded(&e.path(), &ex),
                Err(_) => true,
            });
            for e in children.iter_mut().flatten() {
                let f = facts_of(&e.path());
                // Never descend into links, junctions or reparse points.
                if matches!(f, Some(Facts::Link)) || e.file_type.is_symlink() {
                    e.read_children_path = None;
                }
                e.client_state = f;
            }
        });
    for entry in walker {
        match entry {
            Ok(e) => {
                if e.depth == 0 {
                    continue;
                }
                if e.read_children_error.is_some() {
                    acc.m.incomplete = true;
                }
                match e.client_state {
                    Some(f) => acc.add(f),
                    None => acc.m.incomplete = true,
                }
            }
            Err(_) => acc.m.incomplete = true,
        }
    }
    acc.finish()
}

// ----- disk accounting (diskmap) -------------------------------------------

/// Why a folder could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Denied {
    /// EPERM: a privacy protection (macOS TCC) refused it.
    Privacy,
    /// EACCES (or Windows access denied): file permissions refused it.
    Access,
    /// Any other read failure.
    Other,
}

fn denied_kind(e: &std::io::Error) -> Denied {
    match e.raw_os_error() {
        #[cfg(unix)]
        Some(libc::EPERM) => Denied::Privacy,
        #[cfg(unix)]
        Some(libc::EACCES) => Denied::Access,
        #[cfg(windows)]
        Some(5) => Denied::Access,
        _ if e.kind() == std::io::ErrorKind::PermissionDenied => Denied::Access,
        _ => Denied::Other,
    }
}

/// Options for `account`: one measurement among many that must count each
/// file once between them.
pub struct AccountOpts<'a> {
    /// Exact paths to skip (exclusions, nested area roots and claimed paths),
    /// compared as encountered, so thousands of them stay cheap.
    pub skip: &'a std::collections::HashSet<PathBuf>,
    /// Hard-linked files already counted by an earlier measurement.
    pub seen: &'a std::sync::Mutex<std::collections::HashSet<FileId>>,
    pub cancel: &'a std::sync::atomic::AtomicBool,
    /// Do not cross into other file systems (mount points).
    pub same_device: bool,
    /// Record which file extension holds the most bytes.
    pub track_extensions: bool,
    /// Called with the number of entries looked at so far.
    pub progress: &'a (dyn Fn(u64) + Sync),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Accounted {
    /// Allocated bytes, each hard-linked file counted once across every
    /// measurement that shares `AccountOpts::seen`.
    pub bytes: u64,
    pub entries: u64,
    /// Folders that could not be read (at most `MAX_DENIED`), with the reason.
    pub denied: Vec<(PathBuf, Denied)>,
    pub incomplete: bool,
    pub canceled: bool,
    /// The extension holding the most bytes, and those bytes.
    pub top_extension: Option<(String, u64)>,
}

pub const MAX_DENIED: usize = 64;

/// Per-entry facts computed on the walker's threads: allocated bytes, link
/// count and identity. `None` when the entry could not be read.
type EntryInfo = Option<(u64, u64, Option<FileId>)>;

fn device_of(meta: &std::fs::Metadata, path: &Path) -> Option<u64> {
    platform::file_id(path, meta).map(|id| id.dev)
}

/// Measure a tree for the disk map: on-disk size, never following links,
/// skipping `opts.skip`, counting hard links once across measurements, and
/// recording unreadable folders. Reads directory entries and metadata only;
/// no file is opened.
pub fn account(path: &Path, opts: &AccountOpts) -> Accounted {
    use std::sync::atomic::Ordering;
    let mut out = Accounted::default();
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) => {
            out.incomplete = true;
            if e.kind() != std::io::ErrorKind::NotFound {
                out.denied.push((path.to_path_buf(), denied_kind(&e)));
            }
            return out;
        }
    };
    if platform::is_link_meta(path, &meta) || opts.skip.contains(path) {
        return out;
    }
    let mut multi: HashMap<FileId, u64> = HashMap::new();
    let mut ext: HashMap<String, u64> = HashMap::new();
    let mut add_file = |p: &Path, size: u64, nlink: u64, id: Option<FileId>, out: &mut Accounted| {
        if opts.track_extensions {
            if let Some(e) = p.extension().and_then(|e| e.to_str()) {
                *ext.entry(e.to_ascii_lowercase()).or_default() += size;
            }
        }
        match (nlink > 1, id) {
            (true, Some(id)) => {
                multi.insert(id, size);
            }
            _ => out.bytes += size,
        }
    };
    if !meta.is_dir() {
        add_file(path, platform::allocated_size(path, &meta), platform::link_count(path, &meta), platform::file_id(path, &meta), &mut out);
    } else {
        out.bytes += platform::allocated_size(path, &meta);
        let root_dev = if opts.same_device { device_of(&meta, path) } else { None };
        let skip = Arc::new(opts.skip.clone());
        let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(2, 8);
        let walker = jwalk::WalkDirGeneric::<((), EntryInfo)>::new(path)
            .follow_links(false)
            .skip_hidden(false)
            .sort(false)
            .parallelism(jwalk::Parallelism::RayonNewPool(n))
            .process_read_dir(move |_depth, _dir, _state, children| {
                children.retain(|r| match r {
                    Ok(e) => !skip.contains(&e.path()),
                    Err(_) => true,
                });
                for e in children.iter_mut().flatten() {
                    let p = e.path();
                    match std::fs::symlink_metadata(&p) {
                        Ok(m) => {
                            if platform::is_link_meta(&p, &m) {
                                // A link is never followed and counts nothing.
                                e.read_children_path = None;
                                e.client_state = Some((0, 1, None));
                            } else if m.is_dir() {
                                if root_dev.is_some() && device_of(&m, &p) != root_dev {
                                    // Another file system mounted here.
                                    e.read_children_path = None;
                                    e.client_state = Some((0, 1, None));
                                } else {
                                    e.client_state = Some((platform::allocated_size(&p, &m), 1, None));
                                }
                            } else {
                                e.client_state = Some((platform::allocated_size(&p, &m), platform::link_count(&p, &m), platform::file_id(&p, &m)));
                            }
                        }
                        Err(_) => e.client_state = None,
                    }
                }
            });
        for entry in walker {
            if opts.cancel.load(Ordering::Relaxed) {
                out.canceled = true;
                break;
            }
            let e = match entry {
                Ok(e) => e,
                Err(err) => {
                    out.incomplete = true;
                    if let (Some(p), Some(io)) = (err.path(), err.io_error()) {
                        if out.denied.len() < MAX_DENIED {
                            out.denied.push((p.to_path_buf(), denied_kind(io)));
                        }
                    }
                    continue;
                }
            };
            out.entries += 1;
            if out.entries % 16_384 == 0 {
                (opts.progress)(out.entries);
            }
            if let Some(err) = &e.read_children_error {
                out.incomplete = true;
                if out.denied.len() < MAX_DENIED {
                    let kind = err.io_error().map(denied_kind).unwrap_or(Denied::Other);
                    out.denied.push((e.path(), kind));
                }
            }
            if e.depth == 0 {
                continue;
            }
            match e.client_state {
                Some((size, nlink, id)) => {
                    if e.file_type.is_dir() {
                        out.bytes += size;
                    } else {
                        add_file(&e.path(), size, nlink, id, &mut out);
                    }
                }
                None => out.incomplete = true,
            }
        }
    }
    let mut seen = opts.seen.lock().unwrap_or_else(|e| e.into_inner());
    for (id, size) in multi {
        if seen.insert(id) {
            out.bytes += size;
        }
    }
    drop(seen);
    out.top_extension = ext.into_iter().max_by_key(|(_, b)| *b);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn acct(p: &Path, skip: &[PathBuf], seen: &std::sync::Mutex<std::collections::HashSet<FileId>>) -> Accounted {
        let skip: std::collections::HashSet<PathBuf> = skip.iter().cloned().collect();
        let cancel = std::sync::atomic::AtomicBool::new(false);
        account(p, &AccountOpts { skip: &skip, seen, cancel: &cancel, same_device: true, track_extensions: true, progress: &|_| {} })
    }

    #[test]
    fn account_counts_hard_links_once_across_measurements() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a");
        let b = t.path().join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("blob.dat"), vec![7u8; 512 * 1024]).unwrap();
        fs::hard_link(a.join("blob.dat"), b.join("same.dat")).unwrap();
        fs::hard_link(a.join("blob.dat"), a.join("again.dat")).unwrap();
        let one = platform::allocated_size(&a.join("blob.dat"), &fs::symlink_metadata(a.join("blob.dat")).unwrap());
        let seen = Default::default();
        let ma = acct(&a, &[], &seen);
        let mb = acct(&b, &[], &seen);
        assert!(ma.bytes >= one && ma.bytes < one + 64 * 1024, "{ma:?}");
        assert!(mb.bytes < 64 * 1024, "the second tree must not count the shared file again: {mb:?}");
        assert_eq!(ma.top_extension.as_ref().map(|(e, _)| e.as_str()), Some("dat"));
    }

    #[test]
    fn account_skips_exact_paths_and_links() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        fs::create_dir_all(r.join("keep")).unwrap();
        fs::create_dir_all(r.join("claimed")).unwrap();
        fs::write(r.join("keep/a"), vec![1u8; 200_000]).unwrap();
        fs::write(r.join("claimed/b"), vec![1u8; 2_000_000]).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(r.join("claimed"), r.join("keep/link")).unwrap();
        let m = acct(r, &[r.join("claimed")], &Default::default());
        assert!(m.bytes < 1_000_000, "{m:?}");
        assert!(!m.incomplete);
    }

    #[cfg(unix)]
    #[test]
    fn account_records_unreadable_folders() {
        use std::os::unix::fs::PermissionsExt;
        let t = tempfile::tempdir().unwrap();
        let locked = t.path().join("locked");
        fs::create_dir_all(&locked).unwrap();
        fs::write(locked.join("x"), vec![1u8; 100_000]).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let m = acct(t.path(), &[], &Default::default());
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        // Root can read anything; the rule only holds for ordinary users.
        if unsafe { libc::geteuid() } != 0 {
            assert!(m.incomplete);
            assert_eq!(m.denied.len(), 1, "{m:?}");
            assert_eq!(m.denied[0].1, Denied::Access);
            assert!(m.denied[0].0.ends_with("locked"));
        }
    }

    #[test]
    fn counts_hard_links_once_and_marks_shared() {
        let t = tempfile::tempdir().unwrap();
        let store = t.path().join("store");
        let nm = t.path().join("proj/node_modules");
        fs::create_dir_all(&store).unwrap();
        fs::create_dir_all(&nm).unwrap();
        let data = vec![7u8; 256 * 1024];
        fs::write(store.join("blob"), &data).unwrap();
        // Two links inside node_modules to one store file (pnpm-style).
        fs::hard_link(store.join("blob"), nm.join("a.js")).unwrap();
        fs::hard_link(store.join("blob"), nm.join("b.js")).unwrap();
        fs::write(nm.join("own.js"), &data).unwrap();
        let m = measure(&nm, &MeasureOpts::default());
        let one = platform::allocated_size(&nm.join("own.js"), &fs::symlink_metadata(nm.join("own.js")).unwrap());
        assert_eq!(m.files, 3);
        // blob counted once; it is shared with the store so not reclaimable.
        assert!(m.bytes >= 2 * one && m.bytes < 3 * one + 64 * 1024, "{m:?}");
        assert_eq!(m.shared_bytes, one);
        assert!(m.reclaimable() < 2 * one);
        assert!(m.is_estimate());
        // Measuring the parent that holds every link: nothing is shared.
        let whole = measure(t.path(), &MeasureOpts::default());
        assert_eq!(whole.shared_bytes, 0);
        // Content bytes are the files' own sizes, each linked file once.
        assert_eq!(m.content_bytes, 2 * data.len() as u64);
    }

    #[test]
    fn content_bytes_follow_contents_not_allocation() {
        let t = tempfile::tempdir().unwrap();
        platform::make_sparse(&t.path().join("sparse"), 256 * 1024 * 1024);
        fs::write(t.path().join("a"), b"hello").unwrap();
        let m = measure(t.path(), &MeasureOpts::default());
        assert_eq!(m.content_bytes, 256 * 1024 * 1024 + 5, "a sparse file's length counts, not its blocks");
    }

    #[test]
    fn sparse_file_uses_allocation() {
        let t = tempfile::tempdir().unwrap();
        platform::make_sparse(&t.path().join("sparse"), 256 * 1024 * 1024);
        let m = measure(t.path(), &MeasureOpts::default());
        assert!(m.bytes < 4 * 1024 * 1024, "{m:?}");
    }

    #[cfg(unix)]
    #[test]
    fn never_follows_links() {
        let t = tempfile::tempdir().unwrap();
        let outside = t.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("big"), vec![1u8; 1024 * 1024]).unwrap();
        let tree = t.path().join("tree");
        fs::create_dir_all(&tree).unwrap();
        std::os::unix::fs::symlink(&outside, tree.join("link")).unwrap();
        let m = measure(&tree, &MeasureOpts::default());
        assert!(m.bytes < 64 * 1024, "{m:?}");
        assert_eq!(m.files, 0);
        // A link measured directly is just a link.
        assert_eq!(measure(&tree.join("link"), &MeasureOpts::default()).bytes, 0);
    }

    #[test]
    fn exclusions_are_skipped() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(t.path().join("keep")).unwrap();
        fs::create_dir_all(t.path().join("skip")).unwrap();
        fs::write(t.path().join("keep/a"), vec![1u8; 100_000]).unwrap();
        fs::write(t.path().join("skip/b"), vec![1u8; 100_000]).unwrap();
        let m = measure(t.path(), &MeasureOpts { exclusions: vec![t.path().join("skip")], serial: true });
        assert_eq!(m.files, 1);
    }

    #[test]
    fn missing_path_is_incomplete() {
        let m = measure(Path::new("/definitely/not/here/freeboard"), &MeasureOpts::default());
        assert!(m.incomplete);
    }
}
