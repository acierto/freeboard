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

#[derive(Default)]
struct Acc {
    m: Measured,
    /// id -> (links seen inside the tree, total link count, allocated bytes)
    multi: HashMap<FileId, (u64, u64, u64)>,
}

impl Acc {
    fn file(&mut self, path: &Path, meta: &std::fs::Metadata) {
        let size = platform::allocated_size(path, meta);
        let mt = platform::modified_secs(meta);
        self.m.newest_mtime = Some(self.m.newest_mtime.map_or(mt, |n| n.max(mt)));
        self.m.files += 1;
        let nlink = if meta.is_file() { platform::link_count(path, meta) } else { 1 };
        if nlink > 1 {
            if let Some(id) = platform::file_id(path, meta) {
                let e = self.multi.entry(id).or_insert((0, nlink, size));
                e.0 += 1;
                return;
            }
        }
        self.m.bytes += size;
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
    let par = if opts.serial {
        jwalk::Parallelism::Serial
    } else {
        let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(2, 8);
        jwalk::Parallelism::RayonNewPool(n)
    };
    let walker = jwalk::WalkDir::new(path)
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
                // Never descend into links, junctions or reparse points.
                if e.read_children_path.is_some() && (e.file_type.is_symlink() || platform::is_link(&e.path())) {
                    e.read_children_path = None;
                }
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
                let p = e.path();
                match std::fs::symlink_metadata(&p) {
                    Ok(m) => {
                        if platform::is_link_meta(&p, &m) || m.is_dir() {
                            // Directory and link entries: count their own
                            // allocation only (small), never their target.
                            if !platform::is_link_meta(&p, &m) {
                                acc.m.bytes += platform::allocated_size(&p, &m);
                            }
                        } else {
                            acc.file(&p, &m);
                        }
                    }
                    Err(_) => acc.m.incomplete = true,
                }
            }
            Err(_) => acc.m.incomplete = true,
        }
    }
    acc.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
    }

    #[test]
    fn sparse_file_uses_allocation() {
        let t = tempfile::tempdir().unwrap();
        let f = fs::File::create(t.path().join("sparse")).unwrap();
        f.set_len(256 * 1024 * 1024).unwrap();
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
