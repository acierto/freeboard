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

/// Allocated size, link count and identity of a file in one go (all from
/// `meta` here; see the Windows version for why this exists).
pub fn file_facts(path: &Path, meta: &Metadata) -> (u64, u64, Option<FileId>) {
    (allocated_size(path, meta), meta.nlink(), file_id(path, meta))
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

/// Processes whose current working directory is inside `dir` (best effort),
/// read fresh. Returns None when the check cannot be performed; callers must
/// treat that as "unknown" and raise risk (invariant 11).
pub fn processes_in_dir(dir: &Path, runner: &crate::runner::Runner) -> Option<Vec<u32>> {
    let canon = std::fs::canonicalize(dir).ok()?;
    Some(process_cwds(runner)?.iter().filter(|(_, cwd)| cwd.starts_with(&canon)).map(|(p, _)| *p).collect())
}

/// Every process's working folder, read fresh: /proc on Linux, one
/// `lsof -d cwd -Fpn` call elsewhere. (An earlier `lsof +D <dir>` per folder
/// walked each folder's whole tree, took 20+ s on large worktrees, timed out
/// and blocked them as "status unknown"; found on a real Mac, 2026-11-06.)
///
/// Freeboard itself and the tools it is running (and their children) are
/// left out: the scan runs `git status` in several worktrees at once, and
/// those made clean worktrees look "in use by a program" (Linux CI, 2026-11-06).
pub fn process_cwds(runner: &crate::runner::Runner) -> Option<Vec<(u32, PathBuf)>> {
    #[cfg(target_os = "linux")]
    let list = {
        let _ = runner;
        let mut v = Vec::new();
        for e in std::fs::read_dir("/proc").ok()?.flatten() {
            let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else { continue };
            if let Ok(cwd) = std::fs::read_link(e.path().join("cwd")) {
                let ppid = std::fs::read_to_string(e.path().join("stat")).ok().and_then(|s| parse_stat_ppid(&s)).unwrap_or(0);
                v.push((pid, ppid, cwd));
            }
        }
        v
    };
    #[cfg(not(target_os = "linux"))]
    let list = {
        let out = runner.run_read(&crate::runner::Cmd::new("lsof", ["-R", "-d", "cwd", "-Fpn"])).ok()?;
        // lsof exits 1 when some processes could not be read; the rest is valid.
        if out.code != Some(0) && out.code != Some(1) {
            return None;
        }
        let list = parse_lsof_cwds(&out.stdout);
        // lsof always sees at least itself and this process; empty means it failed.
        if list.is_empty() {
            return None;
        }
        list
    };
    Some(without_own(list, std::process::id(), &crate::runner::own_children()))
}

/// Drop `me`, and every process in `tools` or descended from one of them.
pub fn without_own(list: Vec<(u32, u32, PathBuf)>, me: u32, tools: &[u32]) -> Vec<(u32, PathBuf)> {
    let parent: std::collections::HashMap<u32, u32> = list.iter().map(|(p, pp, _)| (*p, *pp)).collect();
    let mine = |mut p: u32| {
        if p == me {
            return true;
        }
        // Bounded walk up the parent chain (a cycle or a missing parent ends it).
        for _ in 0..64 {
            if tools.contains(&p) {
                return true;
            }
            match parent.get(&p) {
                Some(&pp) if pp != 0 && pp != p => p = pp,
                _ => return false,
            }
        }
        false
    };
    list.into_iter().filter(|(p, _, _)| !mine(*p)).map(|(p, _, c)| (p, c)).collect()
}

/// The parent pid from `/proc/<pid>/stat` ("pid (comm) state ppid ...";
/// comm may itself contain spaces and parentheses).
pub fn parse_stat_ppid(stat: &str) -> Option<u32> {
    stat.rsplit_once(')')?.1.split_whitespace().nth(1)?.parse().ok()
}

/// Parse `lsof -R -Fpn` output: `p<pid>` starts a process, `R<ppid>` is its
/// parent, `n<path>` is its cwd.
pub fn parse_lsof_cwds(out: &str) -> Vec<(u32, u32, PathBuf)> {
    let mut v = Vec::new();
    let mut pid: Option<u32> = None;
    let mut ppid = 0;
    for l in out.lines() {
        if let Some(p) = l.strip_prefix('p') {
            pid = p.parse().ok();
            ppid = 0;
        } else if let Some(r) = l.strip_prefix('R') {
            ppid = r.parse().unwrap_or(0);
        } else if let (Some(n), Some(p)) = (l.strip_prefix('n'), pid) {
            v.push((p, ppid, PathBuf::from(n)));
        }
    }
    v
}

#[cfg(test)]
mod cwd_tests {
    use super::*;

    #[test]
    fn parses_lsof_cwd_output() {
        let out = "p423\nR1\nfcwd\nn/\np446\nR423\nfcwd\nn/Users/me/work/web-app/.worktrees/pay\np9\nR1\nfcwd\nn/Users/me/work/web-app-old\n";
        let v = parse_lsof_cwds(out);
        assert_eq!(v.len(), 3);
        assert_eq!(v[1], (446, 423, PathBuf::from("/Users/me/work/web-app/.worktrees/pay")));
        // Path prefixes match by component: web-app-old is not inside web-app.
        let inside: Vec<u32> = v.iter().filter(|(_, _, c)| c.starts_with("/Users/me/work/web-app")).map(|(p, _, _)| *p).collect();
        assert_eq!(inside, vec![446]);
    }

    #[test]
    fn freeboard_and_its_own_children_are_never_counted() {
        // 423 is Freeboard; 446 (its git) and 447 (git's helper) are its own.
        // 448 is another child of 423 that Freeboard did not start as a tool
        // (a test's stand-in for another program): it still counts.
        let list = vec![(1, 0, PathBuf::from("/")), (423, 1, PathBuf::from("/")), (446, 423, PathBuf::from("/w/a")), (447, 446, PathBuf::from("/w/b")), (448, 423, PathBuf::from("/w/d")), (500, 1, PathBuf::from("/w/c"))];
        let kept: Vec<u32> = without_own(list, 423, &[446]).into_iter().map(|(p, _)| p).collect();
        assert_eq!(kept, vec![1, 448, 500]);
        assert_eq!(parse_stat_ppid("446 (git (worker) x) S 423 446 ..."), Some(423));
    }

    #[test]
    fn the_running_test_is_left_out_of_the_live_list() {
        let r = crate::runner::Runner::default();
        if let Some(l) = process_cwds(&r) {
            assert!(!l.iter().any(|(p, _)| *p == std::process::id()));
        }
    }
}

pub(super) fn process_alive_impl(pid: u32) -> Option<bool> {
    let Ok(pid) = libc::pid_t::try_from(pid) else { return Some(false) };
    if pid <= 0 {
        return None;
    }
    // SAFETY: signal 0 only checks that the process exists.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return Some(true);
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::ESRCH) => Some(false),
        Some(libc::EPERM) => Some(true),
        _ => None,
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
