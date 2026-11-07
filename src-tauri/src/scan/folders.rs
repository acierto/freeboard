//! Folder suggestions for people who have not chosen scan folders yet.
//!
//! Worktrees, project dependencies and build output are found only inside
//! the folders the user chooses (source: "Scan folders | Chosen during first
//! run"). Skipping that step silently hid them (found by the user,
//! 2026-11-05), so Freeboard offers the folders in the home folder that hold
//! Git repositories. It only reads directory listings two levels deep and
//! never follows links. The user still chooses: nothing is added on its own.

use std::path::{Path, PathBuf};

/// Home subfolders that are never offered (system data, media, app state).
const SKIP: [&str; 12] =
    ["Library", "Applications", "Pictures", "Music", "Movies", "Videos", "Public", "AppData", "snap", "OneDrive", "Dropbox", "iCloud Drive"];

/// macOS asks the person for permission the first time an app reads these
/// folders. Looking for repositories inside them just to offer them would
/// trigger those prompts without the person asking (found 2026-11-06), so
/// they are not looked into; the person can still add them with Add folder,
/// which grants access through the folder picker.
const MACOS_PROTECTED: [&str; 3] = ["Desktop", "Documents", "Downloads"];

fn skipped(name: &str) -> bool {
    SKIP.iter().any(|s| s.eq_ignore_ascii_case(name)) || (cfg!(target_os = "macos") && MACOS_PROTECTED.iter().any(|s| s.eq_ignore_ascii_case(name)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub path: PathBuf,
    /// Git repositories found at most two levels inside (including itself).
    pub repos: u32,
}

fn is_dir_no_link(p: &Path) -> bool {
    std::fs::symlink_metadata(p).map(|m| m.is_dir() && !crate::platform::is_link_meta(p, &m)).unwrap_or(false)
}

fn is_repo(p: &Path) -> bool {
    std::fs::symlink_metadata(p.join(".git")).is_ok()
}

fn hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// Git repositories in `dir`, itself or at most two levels inside.
pub fn count_repos(dir: &Path) -> u32 {
    if is_repo(dir) {
        return 1;
    }
    let mut n = 0;
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    for e in rd.flatten().take(2000) {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if hidden(&name) || name == "node_modules" || !is_dir_no_link(&p) {
            continue;
        }
        if is_repo(&p) {
            n += 1;
            continue;
        }
        let Ok(rd2) = std::fs::read_dir(&p) else { continue };
        for e2 in rd2.flatten().take(500) {
            let q = e2.path();
            let name2 = e2.file_name().to_string_lossy().into_owned();
            if !hidden(&name2) && name2 != "node_modules" && is_dir_no_link(&q) && is_repo(&q) {
                n += 1;
            }
        }
    }
    n
}

/// Up to `limit` folders directly in `home` that contain Git repositories,
/// most repositories first. Folders already covered by `chosen` are left out.
pub fn suggest(home: &Path, chosen: &[String], limit: usize) -> Vec<Candidate> {
    let covered = |p: &Path| chosen.iter().any(|c| p.starts_with(c) || Path::new(c).starts_with(p));
    let Ok(rd) = std::fs::read_dir(home) else { return vec![] };
    let mut out: Vec<Candidate> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let p = e.path();
            if hidden(&name) || skipped(&name) || !is_dir_no_link(&p) || covered(&p) {
                return None;
            }
            let repos = count_repos(&p);
            (repos > 0).then_some(Candidate { path: p, repos })
        })
        .collect();
    out.sort_by(|a, b| b.repos.cmp(&a.repos).then_with(|| a.path.cmp(&b.path)));
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(p: &Path) {
        std::fs::create_dir_all(p.join(".git")).unwrap();
    }

    #[test]
    fn offers_folders_with_repositories_most_first() {
        let t = tempfile::tempdir().unwrap();
        let h = t.path();
        repo(&h.join("proj/a"));
        repo(&h.join("proj/b"));
        repo(&h.join("proj/group/c"));
        repo(&h.join("work/x"));
        repo(&h.join("solo"));
        std::fs::create_dir_all(h.join("empty/sub")).unwrap();
        repo(&h.join("Library/Caches/r"));
        repo(&h.join(".hidden/r"));
        repo(&h.join("proj/a/node_modules/dep"));
        let s = suggest(h, &[], 5);
        let names: Vec<(String, u32)> =
            s.iter().map(|c| (c.path.file_name().unwrap().to_string_lossy().into_owned(), c.repos)).collect();
        assert_eq!(names, vec![("proj".into(), 3), ("solo".into(), 1), ("work".into(), 1)]);
        assert_eq!(suggest(h, &[], 1).len(), 1);
    }

    #[test]
    fn leaves_out_folders_already_chosen() {
        let t = tempfile::tempdir().unwrap();
        let h = t.path();
        repo(&h.join("proj/a"));
        repo(&h.join("work/x"));
        let chosen = vec![h.join("proj").to_string_lossy().into_owned()];
        let s = suggest(h, &chosen, 5);
        assert_eq!(s.len(), 1);
        assert!(s[0].path.ends_with("work"));
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_links() {
        let t = tempfile::tempdir().unwrap();
        let h = t.path().join("home");
        repo(&t.path().join("elsewhere/r"));
        std::fs::create_dir_all(&h).unwrap();
        std::os::unix::fs::symlink(t.path().join("elsewhere"), h.join("link")).unwrap();
        assert!(suggest(&h, &[], 5).is_empty());
    }
}
