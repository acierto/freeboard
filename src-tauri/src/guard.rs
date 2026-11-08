//! Protected-path checks and the guarded filesystem boundary.
//!
//! Every mutation of the filesystem goes through this module. It enforces
//! invariants 5 (protected paths, canonical form) and 6 (never follow links;
//! removing a link removes only the link).

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::platform::{self, FileId};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum GuardError {
    #[error("path is not absolute")]
    NotAbsolute,
    #[error("path contains a parent-directory component")]
    Traversal,
    #[error("path does not exist")]
    Missing,
    #[error("path is protected: {0}")]
    Protected(String),
    #[error("path resolves somewhere else than expected (link swap)")]
    Moved,
    #[error("item identity changed since the scan")]
    IdentityChanged,
    #[error("refusing to follow a link inside the tree: {0}")]
    LinkInTree(String),
    #[error("a .git directory or file was found inside the tree: {0}")]
    GitInTree(String),
    #[error("WSL filesystems are excluded")]
    Wsl,
    #[error("filesystem error: {0}")]
    Io(String),
}

/// A path that passed the protected-path checks.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckedPath {
    /// The path as given (lexical).
    pub path: PathBuf,
    /// Canonical parent joined with the final component. The final component
    /// is never resolved, so a link is checked as the link itself.
    pub canonical: PathBuf,
    pub is_link: bool,
    pub id: Option<FileId>,
    /// Passed `check_unused_app`: may only be moved to the Trash.
    pub app_bundle: bool,
}

#[derive(Debug, Clone)]
pub struct Guard {
    home: PathBuf,
    /// Paths that may never be removed, nor any of their ancestors.
    exact: Vec<PathBuf>,
    /// Subtrees whose contents may never be touched.
    subtree: Vec<PathBuf>,
    /// The folders whose top-level app bundles `check_unused_app` allows
    /// (macOS `/Applications` and `~/Applications`; empty elsewhere).
    app_dirs: Vec<PathBuf>,
}

fn canon_or_self(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn norm_case(p: &Path) -> String {
    let s = p.to_string_lossy().to_string();
    if cfg!(any(windows, target_os = "macos")) {
        // Default macOS and Windows volumes are case-insensitive.
        s.to_lowercase()
    } else {
        s
    }
}

fn starts_with_ci(p: &Path, base: &Path) -> bool {
    let (p, b) = (norm_case(p), norm_case(base));
    let sep = std::path::MAIN_SEPARATOR;
    p == b || p.starts_with(&format!("{}{}", b.trim_end_matches(sep), sep))
}

impl Guard {
    /// Guard for the running system. `extra_protected` are additional exact
    /// paths (configured scan folders, Downloads, data dirs).
    pub fn for_system(home: &Path, extra_protected: &[PathBuf]) -> Guard {
        let (mut exact, mut subtree) = platform::system_protected();
        exact.push(home.to_path_buf());
        for d in ["Desktop", "Documents", "Downloads", "Library", "Pictures", "Music", "Movies", "Videos", ".ssh", ".config"] {
            exact.push(home.join(d));
        }
        subtree.push(home.join("Applications"));
        subtree.push(home.join(".ssh"));
        subtree.push(home.join(".gnupg"));
        exact.extend(extra_protected.iter().cloned());
        let g = Guard::new(home, exact, subtree);
        if cfg!(target_os = "macos") {
            g.with_app_dirs(&[PathBuf::from("/Applications"), home.join("Applications")])
        } else {
            g
        }
    }

    /// Allow the unused-app exception for top-level bundles in these folders.
    pub fn with_app_dirs(mut self, dirs: &[PathBuf]) -> Guard {
        self.app_dirs = dirs.iter().map(|d| canon_or_self(d)).collect();
        self
    }

    pub fn new(home: &Path, exact: Vec<PathBuf>, subtree: Vec<PathBuf>) -> Guard {
        let canon = |v: Vec<PathBuf>| {
            let mut out = Vec::new();
            for p in v {
                out.push(canon_or_self(&p));
                out.push(p);
            }
            out
        };
        Guard { home: canon_or_self(home), exact: canon(exact), subtree: canon(subtree), app_dirs: Vec::new() }
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    /// Canonicalize a path without resolving its final component.
    pub fn canonical_of(path: &Path) -> Result<PathBuf, GuardError> {
        if !path.is_absolute() {
            return Err(GuardError::NotAbsolute);
        }
        if path.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(GuardError::Traversal);
        }
        // Windows does not parse verbatim paths (`\\?\C:\...`): `a/../b` stays
        // one component. Refuse `..` or `.` written with either separator.
        if path.components().any(|c| match c {
            Component::Normal(n) => {
                let n = n.to_string_lossy();
                n.split(['/', '\\']).any(|part| part == ".." || part == ".")
            }
            _ => false,
        }) {
            return Err(GuardError::Traversal);
        }
        let name = path.file_name().ok_or_else(|| GuardError::Protected("filesystem root".into()))?;
        let parent = path.parent().ok_or_else(|| GuardError::Protected("filesystem root".into()))?;
        let cp = fs::canonicalize(parent).map_err(|_| GuardError::Missing)?;
        Ok(cp.join(name))
    }

    /// Reason a canonical path is protected, or None.
    pub fn protected_reason(&self, canonical: &Path) -> Option<String> {
        if canonical.parent().is_none() {
            return Some("filesystem root".into());
        }
        if platform::is_wsl_path(canonical) {
            return Some("WSL filesystem".into());
        }
        for c in canonical.components() {
            if let Component::Normal(n) = c {
                if n == ".git" {
                    return Some(".git directory or file".into());
                }
                let s = n.to_string_lossy();
                if s.ends_with(".app") || s.ends_with(".App") {
                    return Some("application bundle".into());
                }
            }
        }
        // Equal to, or an ancestor of, home or another protected location.
        for p in self.exact.iter().chain(std::iter::once(&self.home)) {
            if starts_with_ci(p, canonical) {
                return Some(format!("protected location {}", p.display()));
            }
        }
        for s in &self.subtree {
            if starts_with_ci(canonical, s) || starts_with_ci(s, canonical) {
                return Some(format!("system or application directory {}", s.display()));
            }
        }
        // Repository roots: a directory holding a .git entry (dir, or the
        // .git file of a linked worktree or submodule).
        if fs::symlink_metadata(canonical.join(".git")).is_ok() {
            return Some("repository root".into());
        }
        None
    }

    /// Check that `path` exists and may be acted on.
    pub fn check(&self, path: &Path) -> Result<CheckedPath, GuardError> {
        if platform::is_wsl_path(path) {
            return Err(GuardError::Wsl);
        }
        let canonical = Self::canonical_of(path)?;
        let meta = fs::symlink_metadata(&canonical).map_err(|_| GuardError::Missing)?;
        if let Some(r) = self.protected_reason(&canonical) {
            return Err(GuardError::Protected(r));
        }
        let is_link = platform::is_link_meta(&canonical, &meta);
        let id = platform::file_id(&canonical, &meta);
        Ok(CheckedPath { path: path.to_path_buf(), canonical, is_link, id, app_bundle: false })
    }

    /// Check a linked Git worktree before `git worktree remove`.
    ///
    /// Decision 2026-11-04 (task 4.1): linked worktree roots are the one
    /// exception to the repository-root rule. Everything else still applies:
    /// the path must not be a link, must not be or contain another protected
    /// location, and must be a *linked* worktree of `main` — its `.git` must be
    /// a file whose `gitdir:` points into `<main>/.git/worktrees/`. The main
    /// worktree (a `.git` directory) and every other repository root stay
    /// protected.
    pub fn check_linked_worktree(&self, path: &Path, main: &Path, expected_canonical: &Path) -> Result<CheckedPath, GuardError> {
        if platform::is_wsl_path(path) {
            return Err(GuardError::Wsl);
        }
        let canonical = Self::canonical_of(path)?;
        if norm_case(&canonical) != norm_case(expected_canonical) {
            return Err(GuardError::Moved);
        }
        let meta = fs::symlink_metadata(&canonical).map_err(|_| GuardError::Missing)?;
        if platform::is_link_meta(&canonical, &meta) || !meta.is_dir() {
            return Err(GuardError::Protected("not a worktree directory".into()));
        }
        let main = fs::canonicalize(main).map_err(|_| GuardError::Missing)?;
        if norm_case(&canonical) == norm_case(&main) {
            return Err(GuardError::Protected("main worktree".into()));
        }
        // The worktree's .git must be a plain file pointing at main's worktrees dir.
        let dotgit = canonical.join(".git");
        let gm = fs::symlink_metadata(&dotgit).map_err(|_| GuardError::Protected("not a linked worktree".into()))?;
        if !gm.is_file() || platform::is_link_meta(&dotgit, &gm) {
            return Err(GuardError::Protected("repository root".into()));
        }
        let content = fs::read_to_string(&dotgit).map_err(|e| GuardError::Io(e.to_string()))?;
        let gitdir = content
            .lines()
            .find_map(|l| l.strip_prefix("gitdir:"))
            .map(|s| PathBuf::from(s.trim()))
            .ok_or_else(|| GuardError::Protected("not a linked worktree".into()))?;
        let gitdir = if gitdir.is_absolute() { gitdir } else { canonical.join(gitdir) };
        let gitdir = fs::canonicalize(&gitdir).map_err(|_| GuardError::Protected("not a linked worktree".into()))?;
        if !starts_with_ci(&gitdir, &main.join(".git").join("worktrees")) {
            return Err(GuardError::Protected("worktree belongs to a different repository".into()));
        }
        // Every other protection still applies: evaluate the rules on the
        // path as if its .git file were absent.
        if let Some(r) = self.protected_reason_except_repo_root(&canonical) {
            return Err(GuardError::Protected(r));
        }
        let id = platform::file_id(&canonical, &meta);
        Ok(CheckedPath { path: path.to_path_buf(), canonical, is_link: false, id, app_bundle: false })
    }

    /// Check an unused app before moving it to the Trash (decision
    /// 2026-11-06, `apps.unused`): the one exception to protected application
    /// directories. Allowed only for a real `.app` bundle folder directly
    /// inside one of the guard's app folders (macOS `/Applications` or
    /// `~/Applications`), never a link, never a system app's bundle, never
    /// anything inside a bundle, and never the app folder itself. The result
    /// may only be moved to the Trash (`remove` refuses it).
    pub fn check_unused_app(&self, path: &Path) -> Result<CheckedPath, GuardError> {
        if self.app_dirs.is_empty() {
            return Err(GuardError::Protected("application bundle".into()));
        }
        let canonical = Self::canonical_of(path)?;
        let meta = fs::symlink_metadata(&canonical).map_err(|_| GuardError::Missing)?;
        if platform::is_link_meta(&canonical, &meta) || !meta.is_dir() {
            return Err(GuardError::Protected("not an application bundle".into()));
        }
        let name = canonical.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let parent = canonical.parent().ok_or_else(|| GuardError::Protected("filesystem root".into()))?;
        if !name.to_lowercase().ends_with(".app") || name.len() <= 4 || !self.app_dirs.iter().any(|d| norm_case(d) == norm_case(parent)) {
            return Err(GuardError::Protected("application bundle".into()));
        }
        if canonical.components().any(|c| matches!(c, Component::Normal(n) if n == ".git")) {
            return Err(GuardError::Protected(".git directory or file".into()));
        }
        if starts_with_ci(&canonical, Path::new("/System")) {
            return Err(GuardError::Protected("system application".into()));
        }
        let id = platform::file_id(&canonical, &meta);
        Ok(CheckedPath { path: path.to_path_buf(), canonical, is_link: false, id, app_bundle: true })
    }

    /// Check a path again right before acting, by the rule it first passed.
    pub fn recheck(&self, checked: &CheckedPath) -> Result<CheckedPath, GuardError> {
        if checked.app_bundle {
            self.check_unused_app(&checked.path)
        } else {
            self.check(&checked.path)
        }
    }

    fn protected_reason_except_repo_root(&self, canonical: &Path) -> Option<String> {
        if canonical.parent().is_none() {
            return Some("filesystem root".into());
        }
        for c in canonical.components() {
            if let Component::Normal(n) = c {
                if n == ".git" {
                    return Some(".git directory or file".into());
                }
                let s = n.to_string_lossy();
                if s.ends_with(".app") || s.ends_with(".App") {
                    return Some("application bundle".into());
                }
            }
        }
        for p in self.exact.iter().chain(std::iter::once(&self.home)) {
            if starts_with_ci(p, canonical) {
                return Some(format!("protected location {}", p.display()));
            }
        }
        for s in &self.subtree {
            if starts_with_ci(canonical, s) || starts_with_ci(s, canonical) {
                return Some(format!("system or application directory {}", s.display()));
            }
        }
        None
    }

    /// Check against the snapshot taken at scan time: same canonical path, same
    /// identity, same link-ness. Detects path-for-symlink swaps.
    pub fn check_expected(&self, path: &Path, expected_canonical: &Path, expected_id: Option<&str>) -> Result<CheckedPath, GuardError> {
        let c = self.check(path)?;
        if norm_case(&c.canonical) != norm_case(expected_canonical) {
            return Err(GuardError::Moved);
        }
        if let Some(exp) = expected_id {
            if c.id.map(|i| i.encode()).as_deref() != Some(exp) {
                return Err(GuardError::IdentityChanged);
            }
        }
        Ok(c)
    }

    /// Permanently remove a checked path without following links. A link is
    /// removed as a link. Directories are removed bottom-up; any `.git` entry
    /// inside the tree aborts before anything is deleted.
    pub fn remove(&self, checked: &CheckedPath) -> Result<u64, GuardError> {
        if checked.app_bundle {
            return Err(GuardError::Protected("an application is only ever moved to the Trash".into()));
        }
        // Re-check right before acting.
        let now = self.check(&checked.path)?;
        if now.canonical != checked.canonical || now.id != checked.id {
            return Err(GuardError::Moved);
        }
        if now.is_link {
            remove_link(&now.canonical)?;
            return Ok(0);
        }
        let meta = fs::symlink_metadata(&now.canonical).map_err(|e| GuardError::Io(e.to_string()))?;
        if !meta.is_dir() {
            let size = platform::allocated_size(&now.canonical, &meta);
            fs::remove_file(&now.canonical).map_err(|e| GuardError::Io(e.to_string()))?;
            return Ok(size);
        }
        // Pre-flight: refuse trees containing .git anywhere.
        preflight_tree(&now.canonical)?;
        let mut freed = 0;
        remove_tree(&now.canonical, &mut freed)?;
        Ok(freed)
    }

    /// Move a checked path to the platform Trash. Returns Err when no usable
    /// Trash exists; callers must then stop the item (never fall back silently).
    pub fn move_to_trash(&self, checked: &CheckedPath) -> Result<(), GuardError> {
        let now = self.recheck(checked)?;
        if now.canonical != checked.canonical || now.id != checked.id {
            return Err(GuardError::Moved);
        }
        // Only directories can contain a .git entry; files and links are moved as they are.
        let is_dir = fs::symlink_metadata(&now.canonical).map(|m| m.is_dir()).unwrap_or(false);
        if !now.is_link && is_dir {
            preflight_tree(&now.canonical)?;
        }
        trash::delete(&now.canonical).map_err(|e| GuardError::Io(format!("Trash unavailable: {e}")))
    }
}

fn remove_link(p: &Path) -> Result<(), GuardError> {
    // On Windows a directory junction/symlink is removed with remove_dir,
    // which deletes the link and never its target.
    match fs::remove_file(p) {
        Ok(()) => Ok(()),
        Err(e) => {
            if cfg!(windows) {
                fs::remove_dir(p).map_err(|e| GuardError::Io(e.to_string()))
            } else {
                Err(GuardError::Io(e.to_string()))
            }
        }
    }
}

fn preflight_tree(root: &Path) -> Result<(), GuardError> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rd = fs::read_dir(&dir).map_err(|e| GuardError::Io(e.to_string()))?;
        for e in rd {
            let e = e.map_err(|e| GuardError::Io(e.to_string()))?;
            let p = e.path();
            if e.file_name() == ".git" {
                return Err(GuardError::GitInTree(p.display().to_string()));
            }
            let m = fs::symlink_metadata(&p).map_err(|e| GuardError::Io(e.to_string()))?;
            if !platform::is_link_meta(&p, &m) && m.is_dir() {
                stack.push(p);
            }
        }
    }
    Ok(())
}

fn remove_tree(dir: &Path, freed: &mut u64) -> Result<(), GuardError> {
    let rd = fs::read_dir(dir).map_err(|e| GuardError::Io(e.to_string()))?;
    for e in rd {
        let e = e.map_err(|e| GuardError::Io(e.to_string()))?;
        let p = e.path();
        let m = fs::symlink_metadata(&p).map_err(|e| GuardError::Io(e.to_string()))?;
        if platform::is_link_meta(&p, &m) {
            remove_link(&p)?;
        } else if m.is_dir() {
            remove_tree(&p, freed)?;
        } else {
            if platform::link_count(&p, &m) <= 1 {
                *freed += platform::allocated_size(&p, &m);
            }
            fs::remove_file(&p).map_err(|e| GuardError::Io(e.to_string()))?;
        }
    }
    fs::remove_dir(dir).map_err(|e| GuardError::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fx {
        _t: tempfile::TempDir,
        root: PathBuf,
        guard: Guard,
    }

    fn fx() -> Fx {
        let t = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(t.path()).unwrap();
        let home = root.join("home");
        let sys = root.join("System");
        fs::create_dir_all(home.join("projects/app/node_modules/pkg")).unwrap();
        fs::write(home.join("projects/app/node_modules/pkg/index.js"), b"x").unwrap();
        fs::write(home.join("projects/app/package.json"), b"{}").unwrap();
        fs::create_dir_all(sys.join("Library")).unwrap();
        fs::write(sys.join("Library/kernel"), b"k").unwrap();
        let guard = Guard::new(&home, vec![root.clone(), home.join("projects")], vec![sys]);
        Fx { _t: t, root, guard }
    }

    #[test]
    fn rejects_relative_and_traversal() {
        let f = fx();
        assert_eq!(f.guard.check(Path::new("relative/x")).unwrap_err(), GuardError::NotAbsolute);
        let p = f.root.join("home/projects/app/node_modules/../../..");
        // Windows: joining onto a verbatim root (\\?\C:\...) resolves `..`
        // already; the result is the protected folder itself.
        assert!(f.guard.check(&p).is_err());
        // A raw path string keeps `..` with either separator.
        let sep = std::path::MAIN_SEPARATOR;
        for tail in ["home/projects/app/node_modules/../../..".to_string(), format!("home{sep}projects{sep}app{sep}..{sep}..")] {
            let raw = PathBuf::from(format!("{}{sep}{tail}", f.root.display()));
            assert_eq!(f.guard.check(&raw).unwrap_err(), GuardError::Traversal, "{raw:?}");
        }
    }

    #[test]
    fn rejects_root_home_and_ancestors() {
        let f = fx();
        assert!(matches!(f.guard.check(&f.root.join("home")), Err(GuardError::Protected(_))));
        assert!(matches!(f.guard.check(&f.root.join("home/projects")), Err(GuardError::Protected(_))));
        let fsroot = if cfg!(windows) { PathBuf::from("C:\\") } else { PathBuf::from("/") };
        assert!(f.guard.check(&fsroot).is_err());
    }

    #[test]
    fn rejects_system_subtree() {
        let f = fx();
        assert!(matches!(f.guard.check(&f.root.join("System/Library/kernel")), Err(GuardError::Protected(_))));
    }

    #[test]
    fn rejects_repository_root_and_git() {
        let f = fx();
        let repo = f.root.join("home/projects/app");
        fs::create_dir_all(repo.join(".git/objects")).unwrap();
        assert!(matches!(f.guard.check(&repo), Err(GuardError::Protected(r)) if r == "repository root"));
        assert!(matches!(f.guard.check(&repo.join(".git")), Err(GuardError::Protected(_))));
        assert!(matches!(f.guard.check(&repo.join(".git/objects")), Err(GuardError::Protected(_))));
        // Linked worktree root (.git file) is a repository root too.
        let wt = f.root.join("home/projects/wt");
        fs::create_dir_all(&wt).unwrap();
        fs::write(wt.join(".git"), b"gitdir: /elsewhere").unwrap();
        assert!(matches!(f.guard.check(&wt), Err(GuardError::Protected(_))));
    }

    #[test]
    #[cfg(unix)]
    fn unused_app_exception_is_narrow() {
        let f = fx();
        let apps = f.root.join("home/Applications");
        let app = apps.join("Old Tool.app");
        fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        fs::write(app.join("Contents/MacOS/tool"), b"x").unwrap();
        // Without app folders (Linux, Windows, or a guard built without them)
        // nothing passes.
        assert!(f.guard.check_unused_app(&app).is_err());
        let g = f.guard.clone().with_app_dirs(std::slice::from_ref(&apps));
        // Positive: a top-level bundle in an app folder, Trash only.
        let c = g.check_unused_app(&app).unwrap();
        assert!(c.app_bundle);
        assert!(matches!(g.remove(&c), Err(GuardError::Protected(_))), "never deleted permanently");
        assert!(app.exists());
        // The normal check still protects every bundle.
        assert!(matches!(g.check(&app), Err(GuardError::Protected(r)) if r == "application bundle"));
        // Negative: inside a bundle, the app folder itself, a link, a bundle
        // elsewhere, a nested bundle, a file named .app, a system path.
        assert!(g.check_unused_app(&app.join("Contents")).is_err());
        assert!(g.check_unused_app(&apps).is_err());
        let link = apps.join("Linked.app");
        std::os::unix::fs::symlink(&app, &link).unwrap();
        assert!(g.check_unused_app(&link).is_err());
        let elsewhere = f.root.join("home/projects/Other.app");
        fs::create_dir_all(&elsewhere).unwrap();
        assert!(g.check_unused_app(&elsewhere).is_err());
        let nested = app.join("Contents/Helpers/Helper.app");
        fs::create_dir_all(&nested).unwrap();
        assert!(g.check_unused_app(&nested).is_err());
        fs::write(apps.join("Note.app"), b"not a bundle").unwrap();
        assert!(g.check_unused_app(&apps.join("Note.app")).is_err());
        assert!(g.check_unused_app(&apps.join(".app")).is_err());
        assert!(g.check_unused_app(&apps.join("Missing.app")).is_err());
        let g_sys = f.guard.clone().with_app_dirs(&[PathBuf::from("/System/Applications")]);
        assert!(g_sys.check_unused_app(Path::new("/System/Applications/Calculator.app")).is_err());
        // A bundle swapped for a link after the check fails the recheck.
        let c = g.check_unused_app(&app).unwrap();
        fs::rename(&app, apps.join("moved")).unwrap();
        std::os::unix::fs::symlink(apps.join("moved"), &app).unwrap();
        assert!(g.recheck(&c).is_err());
    }

    #[test]
    fn linked_worktree_exception_is_narrow() {
        let f = fx();
        let main = f.root.join("home/projects/app");
        fs::create_dir_all(main.join(".git/worktrees/feat")).unwrap();
        let wt = main.join(".worktrees/feat");
        fs::create_dir_all(&wt).unwrap();
        fs::write(wt.join(".git"), format!("gitdir: {}\n", main.join(".git/worktrees/feat").display())).unwrap();
        // The generic check still protects it as a repository root.
        assert!(matches!(f.guard.check(&wt), Err(GuardError::Protected(_))));
        // The dedicated worktree check allows it.
        assert!(f.guard.check_linked_worktree(&wt, &main, &wt).is_ok());
        // Main worktree, .git, wrong expected path, and foreign repos are refused.
        assert!(f.guard.check_linked_worktree(&main, &main, &main).is_err());
        assert!(f.guard.check_linked_worktree(&main.join(".git"), &main, &main.join(".git")).is_err());
        assert_eq!(f.guard.check_linked_worktree(&wt, &main, &main.join("other")).unwrap_err(), GuardError::Moved);
        let other = f.root.join("home/projects/other");
        fs::create_dir_all(other.join(".git")).unwrap();
        assert!(f.guard.check_linked_worktree(&wt, &other, &wt).is_err());
        // A worktree whose .git is a directory (a full repository) is refused.
        let full = main.join(".worktrees/full");
        fs::create_dir_all(full.join(".git")).unwrap();
        assert!(f.guard.check_linked_worktree(&full, &main, &full).is_err());
        // A worktree inside a protected subtree stays protected.
        let sys = f.root.join("System/wt");
        fs::create_dir_all(&sys).unwrap();
        fs::write(sys.join(".git"), format!("gitdir: {}\n", main.join(".git/worktrees/feat").display())).unwrap();
        assert!(f.guard.check_linked_worktree(&sys, &main, &sys).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn linked_worktree_symlink_is_refused() {
        let f = fx();
        let main = f.root.join("home/projects/app");
        fs::create_dir_all(main.join(".git/worktrees/feat")).unwrap();
        let real = f.root.join("home/projects/realwt");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join(".git"), format!("gitdir: {}\n", main.join(".git/worktrees/feat").display())).unwrap();
        let link = main.join("linkwt");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(f.guard.check_linked_worktree(&link, &main, &link).is_err());
    }

    #[test]
    fn single_file_passes_trash_preflight() {
        // Regression: the .git preflight ran read_dir on files, so moving any
        // single file (e.g. an installer) to the Trash failed.
        let f = fx();
        let file = f.root.join("home/projects/app/node_modules/pkg/index.js");
        let c = f.guard.check(&file).unwrap();
        let is_dir = fs::symlink_metadata(&c.canonical).unwrap().is_dir();
        assert!(!is_dir);
        // The preflight itself must not be applied to files.
        assert!(preflight_tree(&c.canonical).is_err(), "read_dir on a file fails, so callers must skip it");
    }

    #[test]
    fn allows_and_removes_generated_folder() {
        let f = fx();
        let nm = f.root.join("home/projects/app/node_modules");
        let c = f.guard.check(&nm).unwrap();
        f.guard.remove(&c).unwrap();
        assert!(!nm.exists());
        assert!(f.root.join("home/projects/app/package.json").exists());
    }

    #[test]
    fn refuses_tree_with_nested_git() {
        let f = fx();
        let nm = f.root.join("home/projects/app/node_modules");
        fs::create_dir_all(nm.join("dep/.git")).unwrap();
        let c = f.guard.check(&nm).unwrap();
        assert!(matches!(f.guard.remove(&c), Err(GuardError::GitInTree(_))));
        assert!(nm.join("pkg/index.js").exists(), "nothing deleted on refusal");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_trap_removes_only_link() {
        let f = fx();
        let victim = f.root.join("victim");
        fs::create_dir_all(&victim).unwrap();
        fs::write(victim.join("precious"), b"data").unwrap();
        let nm = f.root.join("home/projects/app/node_modules");
        std::os::unix::fs::symlink(&victim, nm.join("trap")).unwrap();
        let c = f.guard.check(&nm).unwrap();
        f.guard.remove(&c).unwrap();
        assert!(victim.join("precious").exists(), "link target untouched");
    }

    #[cfg(unix)]
    #[test]
    fn link_to_protected_target_is_checked_as_link() {
        let f = fx();
        let link = f.root.join("home/projects/app/build");
        std::os::unix::fs::symlink(f.root.join("home"), &link).unwrap();
        let c = f.guard.check(&link).unwrap();
        assert!(c.is_link);
        f.guard.remove(&c).unwrap();
        assert!(f.root.join("home/projects/app/package.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn ancestor_symlink_swap_is_detected() {
        let f = fx();
        let nm = f.root.join("home/projects/app/node_modules");
        let snap = f.guard.check(&nm).unwrap();
        // Swap the project directory for a link pointing at another tree.
        let other = f.root.join("other/app/node_modules");
        fs::create_dir_all(&other).unwrap();
        fs::rename(f.root.join("home/projects/app"), f.root.join("moved")).unwrap();
        std::os::unix::fs::symlink(f.root.join("other/app"), f.root.join("home/projects/app")).unwrap();
        let r = f.guard.check_expected(&nm, &snap.canonical, snap.id.map(|i| i.encode()).as_deref());
        assert_eq!(r.unwrap_err(), GuardError::Moved);
        assert!(other.exists());
    }

    #[test]
    fn identity_change_is_detected() {
        let f = fx();
        let nm = f.root.join("home/projects/app/node_modules");
        let snap = f.guard.check(&nm).unwrap();
        fs::remove_dir_all(&nm).unwrap();
        fs::create_dir_all(&nm).unwrap();
        // Recreate with a filler to avoid inode reuse.
        let _keep = fs::File::create(f.root.join("home/projects/app/filler")).unwrap();
        let r = f.guard.check_expected(&nm, &snap.canonical, snap.id.map(|i| i.encode()).as_deref());
        if snap.id.is_some() && f.guard.check(&nm).unwrap().id != snap.id {
            assert_eq!(r.unwrap_err(), GuardError::IdentityChanged);
        }
    }

    #[test]
    fn real_system_guard_protects_home_and_root() {
        let home = platform::home_dir().unwrap();
        let g = Guard::for_system(&home, &[]);
        assert!(g.check(&home).is_err());
        if cfg!(unix) {
            assert!(g.check(Path::new("/")).is_err());
            assert!(g.check(Path::new("/usr/bin")).is_err());
            assert!(g.check(Path::new("/etc")).is_err());
        }
    }
}
