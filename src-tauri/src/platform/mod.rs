//! Platform differences: allocated size, file identity, links/junctions,
//! process detection, battery, volumes and well-known locations.

use std::path::{Path, PathBuf};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
pub use windows::*;

pub fn platform_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(windows) {
        "windows"
    } else {
        "linux"
    }
}

/// Stable identity of a file system object: (device or volume, inode or file id).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId {
    pub dev: u64,
    pub ino: u64,
}

impl FileId {
    pub fn encode(&self) -> String {
        format!("{}:{}", self.dev, self.ino)
    }
}

/// True for symlinks and, on Windows, junctions and other reparse points.
/// Never follows the link.
pub fn is_link(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(m) => is_link_meta(path, &m),
        Err(_) => false,
    }
}

pub fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

/// XDG-aware cache home on Linux; platform cache dir elsewhere.
pub fn cache_home(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Caches")
    } else if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData").join("Local"))
    } else {
        match std::env::var_os("XDG_CACHE_HOME") {
            Some(p) if Path::new(&p).is_absolute() => PathBuf::from(p),
            _ => home.join(".cache"),
        }
    }
}

/// XDG-aware data home (Freeboard database and logs live here).
pub fn data_home(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support")
    } else if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData").join("Roaming"))
    } else {
        match std::env::var_os("XDG_DATA_HOME") {
            Some(p) if Path::new(&p).is_absolute() => PathBuf::from(p),
            _ => home.join(".local/share"),
        }
    }
}

/// The Downloads folder (XDG user dir on Linux when configured).
pub fn downloads_dir(home: &Path) -> PathBuf {
    if cfg!(target_os = "linux") {
        if let Ok(cfg) = std::fs::read_to_string(home.join(".config/user-dirs.dirs")) {
            for line in cfg.lines() {
                if let Some(rest) = line.strip_prefix("XDG_DOWNLOAD_DIR=") {
                    let v = rest.trim_matches('"').replace("$HOME", &home.to_string_lossy());
                    if Path::new(&v).is_absolute() {
                        return PathBuf::from(v);
                    }
                }
            }
        }
    }
    home.join("Downloads")
}

/// WSL filesystems are excluded from scanning in v1.
pub fn is_wsl_path(path: &Path) -> bool {
    let s = path.to_string_lossy().to_ascii_lowercase();
    s.starts_with(r"\\wsl$") || s.starts_with(r"\\wsl.localhost") || s.starts_with("//wsl$")
}

/// Battery state: Some(true) when running on battery.
pub fn on_battery() -> Option<bool> {
    on_battery_impl()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wsl_paths_detected() {
        assert!(is_wsl_path(Path::new(r"\\wsl$\Ubuntu\home")));
        assert!(is_wsl_path(Path::new(r"\\wsl.localhost\Ubuntu")));
        assert!(!is_wsl_path(Path::new("/home/user")));
    }

    #[test]
    fn allocated_size_counts_blocks_not_length() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sparse");
        let f = std::fs::File::create(&p).unwrap();
        // 64 MiB apparent length, nothing written.
        f.set_len(64 * 1024 * 1024).unwrap();
        drop(f);
        let alloc = allocated_size(&p, &std::fs::symlink_metadata(&p).unwrap());
        assert!(alloc < 1024 * 1024, "sparse file reported {alloc} allocated bytes");
    }

    #[cfg(unix)]
    #[test]
    fn hard_links_share_identity() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        std::fs::write(&a, b"x").unwrap();
        let b = dir.path().join("b");
        std::fs::hard_link(&a, &b).unwrap();
        let ia = file_id(&a, &std::fs::symlink_metadata(&a).unwrap()).unwrap();
        let ib = file_id(&b, &std::fs::symlink_metadata(&b).unwrap()).unwrap();
        assert_eq!(ia, ib);
        assert_eq!(link_count(&a, &std::fs::symlink_metadata(&a).unwrap()), 2);
    }
}
