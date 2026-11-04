use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use super::FileId;

/// Actual on-disk size: `st_blocks * 512`.
pub fn allocated_size(_path: &Path, meta: &Metadata) -> u64 {
    meta.blocks().saturating_mul(512)
}

pub fn file_id(_path: &Path, meta: &Metadata) -> Option<FileId> {
    Some(FileId { dev: meta.dev(), ino: meta.ino() })
}

pub fn link_count(_path: &Path, meta: &Metadata) -> u64 {
    meta.nlink()
}

pub fn is_link_meta(_path: &Path, meta: &Metadata) -> bool {
    meta.file_type().is_symlink()
}

pub fn modified_secs(meta: &Metadata) -> i64 {
    meta.mtime()
}

pub fn accessed_secs(meta: &Metadata) -> i64 {
    meta.atime()
}

/// Filesystem roots that must never be removed, plus system subtrees whose
/// contents must never be touched. Returned as (exact_or_ancestor, subtree).
pub fn system_protected() -> (Vec<PathBuf>, Vec<PathBuf>) {
    let exact: Vec<PathBuf> = [
        "/", "/var", "/private", "/private/var", "/private/tmp", "/tmp", "/Users", "/home", "/Volumes",
        "/mnt", "/media", "/run",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    let subtree: Vec<PathBuf> = if cfg!(target_os = "macos") {
        [
            "/System", "/usr", "/bin", "/sbin", "/etc", "/private/etc", "/Library", "/Applications", "/cores",
            "/dev", "/opt", "/private/var/db", "/private/var/root",
        ]
        .iter()
        .map(PathBuf::from)
        .collect()
    } else {
        [
            "/usr", "/bin", "/sbin", "/lib", "/lib32", "/lib64", "/etc", "/boot", "/proc", "/sys", "/dev", "/snap",
            "/opt", "/srv", "/root", "/var/lib", "/var/log",
        ]
        .iter()
        .map(PathBuf::from)
        .collect()
    };
    (exact, subtree)
}

/// Processes whose current working directory is inside `dir` (best effort).
/// Returns None when the check cannot be performed; callers must treat that
/// as "unknown" and raise risk (invariant 11).
pub fn processes_in_dir(dir: &Path, runner: &crate::runner::Runner) -> Option<Vec<u32>> {
    #[cfg(target_os = "linux")]
    {
        let _ = runner;
        let canon = std::fs::canonicalize(dir).ok()?;
        let mut pids = Vec::new();
        let entries = std::fs::read_dir("/proc").ok()?;
        for e in entries.flatten() {
            let name = e.file_name();
            let Some(pid) = name.to_str().and_then(|s| s.parse::<u32>().ok()) else { continue };
            if let Ok(cwd) = std::fs::read_link(e.path().join("cwd")) {
                if cwd.starts_with(&canon) {
                    pids.push(pid);
                }
            }
        }
        Some(pids)
    }
    #[cfg(not(target_os = "linux"))]
    {
        // `lsof -a -d cwd -Fp +D <dir>` lists processes whose cwd is under dir.
        let canon = std::fs::canonicalize(dir).ok()?;
        let out = runner
            .run_read(&crate::runner::Cmd::new(
                "lsof",
                ["-a", "-d", "cwd", "-Fp", "+D", &canon.to_string_lossy()],
            ))
            .ok()?;
        // lsof exits 1 when nothing matches.
        if out.code != Some(0) && out.code != Some(1) {
            return None;
        }
        Some(
            out.stdout
                .lines()
                .filter_map(|l| l.strip_prefix('p'))
                .filter_map(|p| p.parse().ok())
                .collect(),
        )
    }
}

pub(super) fn on_battery_impl() -> Option<bool> {
    #[cfg(target_os = "linux")]
    {
        let dir = std::fs::read_dir("/sys/class/power_supply").ok()?;
        let mut saw_ac = false;
        for e in dir.flatten() {
            let p = e.path();
            let kind = std::fs::read_to_string(p.join("type")).unwrap_or_default();
            if kind.trim() == "Mains" {
                saw_ac = true;
                if std::fs::read_to_string(p.join("online")).unwrap_or_default().trim() == "1" {
                    return Some(false);
                }
            }
        }
        if saw_ac {
            Some(true)
        } else {
            None
        }
    }
    #[cfg(target_os = "macos")]
    {
        // `pmset -g batt` prints "Now drawing from 'Battery Power'".
        let out = std::process::Command::new("/usr/bin/pmset").args(["-g", "batt"]).output().ok()?;
        let s = String::from_utf8_lossy(&out.stdout);
        if s.contains("'Battery Power'") {
            Some(true)
        } else if s.contains("'AC Power'") {
            Some(false)
        } else {
            None
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// Lower the current thread's scheduling priority for deep scans.
pub fn lower_thread_priority() {
    #[cfg(target_os = "macos")]
    // SAFETY: adjusts only the calling thread's QoS class.
    unsafe {
        let _ = libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0);
    }
    #[cfg(target_os = "linux")]
    // SAFETY: on Linux PRIO_PROCESS with a thread id adjusts only that thread.
    unsafe {
        let tid = libc::syscall(libc::SYS_gettid) as libc::id_t;
        let _ = libc::setpriority(libc::PRIO_PROCESS, tid, 10);
    }
}
