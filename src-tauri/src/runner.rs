//! The only module that starts external commands (invariant 8).
//!
//! Commands are an executable name plus an argument array, run without a
//! shell, resolved against the search path, with a timeout and bounded output.
//! Two policies apply: read-only commands for scans (invariant 1) and action
//! commands for confirmed cleanups. Banned commands (invariant 7) are rejected
//! in both.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);
/// Cleanup actions the user confirmed (`git worktree remove`, `docker image
/// rm`, ...) may take minutes on a big worktree or image set. Stopping one
/// halfway can leave a half-deleted worktree, so they get far longer than
/// detector reads (found on a real Mac, 2026-11-07: many worktree removals
/// timed out at 20 seconds).
pub const ACTION_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// One worktree removal (spec "Removing many worktrees": 120 s each; a
/// timeout fails that worktree only and the run goes on).
pub const WORKTREE_TIMEOUT: Duration = Duration::from_secs(120);
pub const MAX_OUTPUT: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct Cmd {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
}

impl Cmd {
    pub fn new<I, S>(program: &str, args: I) -> Cmd
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Cmd { program: program.to_string(), args: args.into_iter().map(|s| s.as_ref().to_string()).collect(), cwd: None }
    }
    pub fn in_dir(mut self, dir: &Path) -> Cmd {
        self.cwd = Some(dir.to_path_buf());
        self
    }
    pub fn display(&self) -> String {
        std::iter::once(self.program.as_str()).chain(self.args.iter().map(|s| s.as_str())).collect::<Vec<_>>().join(" ")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CmdOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CmdOutput {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum RunError {
    #[error("{0} is not installed")]
    NotFound(String),
    #[error("command refused: {0}")]
    Forbidden(String),
    #[error("{0} timed out")]
    Timeout(String),
    #[error("output too large")]
    OutputTooLarge,
    #[error("{0}")]
    Io(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// Scans: read-only subcommands only.
    ReadOnly,
    /// Confirmed cleanup actions.
    Action,
    /// Opening an operating-system settings pane: only the exact constant
    /// commands in `SETTINGS_PANES`, never an argument from anywhere else.
    OpenSettings,
}

/// Operating-system settings panes Freeboard may open for a blind spot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPane {
    /// macOS Privacy & Security > Full Disk Access.
    FullDiskAccess,
}

/// The only commands `Policy::OpenSettings` allows: (pane, platform, program,
/// arguments). Arguments are constants; nothing is ever appended to them.
pub const SETTINGS_PANES: &[(SettingsPane, &str, &str, &[&str])] =
    &[(SettingsPane::FullDiskAccess, "macos", "open", &["x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles"])];

/// The command that opens `pane` on `platform`, if there is one.
pub fn settings_command(pane: SettingsPane, platform: &str) -> Option<Cmd> {
    SETTINGS_PANES.iter().find(|(p, os, _, _)| *p == pane && *os == platform).map(|(_, _, prog, args)| Cmd::new(prog, args.iter()))
}

/// Proof that a plan was confirmed. Only the cleanup engine constructs it.
#[derive(Debug, Clone)]
pub struct Confirmed {
    pub plan_id: String,
    _private: (),
}

impl Confirmed {
    pub(crate) fn new(plan_id: &str) -> Confirmed {
        Confirmed { plan_id: plan_id.to_string(), _private: () }
    }
}

/// Proof that the user acknowledged, in a confirmed plan, that one
/// worktree's uncommitted files are deleted. Only the cleanup engine makes
/// one, from the plan's recorded acknowledgments.
pub struct Acknowledged {
    pub item_id: String,
    _private: (),
}

impl Acknowledged {
    pub(crate) fn new(item_id: &str) -> Acknowledged {
        Acknowledged { item_id: item_id.to_string(), _private: () }
    }
}

const PROGRAMS: &[&str] = &["docker", "git", "pnpm", "pip", "pip3", "brew", "xcrun", "npm", "lsof", "ps", "tmutil", "ollama", "rustup", "mdls"];

fn is_force_flag(a: &str) -> bool {
    if a == "--force" || a.starts_with("--force=") || a == "-f" {
        return true;
    }
    // Clustered short flags such as -af.
    a.starts_with('-') && !a.starts_with("--") && a.len() > 2 && a[1..].chars().all(|c| c.is_ascii_alphabetic()) && a.contains('f')
}

/// Invariant 7: banned commands, regardless of policy or confirmation.
pub fn banned_reason(cmd: &Cmd) -> Option<String> {
    let a: Vec<&str> = cmd.args.iter().map(|s| s.as_str()).collect();
    let has = |w: &str| a.iter().any(|x| *x == w);
    match cmd.program.as_str() {
        "git" => {
            if has("clean") {
                return Some("git clean is banned".into());
            }
            if has("worktree") && has("remove") && a.iter().any(|x| is_force_flag(x)) {
                return Some("git worktree remove --force is banned".into());
            }
        }
        "docker" => {
            if has("system") && has("prune") {
                return Some("docker system prune is banned".into());
            }
            if has("volume") && has("prune") {
                return Some("docker volume prune is banned".into());
            }
        }
        "npm" => {
            if has("cache") && has("clean") {
                return Some("npm cache clean needs --force, which is not allowed".into());
            }
        }
        _ => {}
    }
    None
}

fn starts(a: &[&str], prefix: &[&str]) -> bool {
    a.len() >= prefix.len() && a[..prefix.len()] == *prefix
}

fn read_only_allowed(cmd: &Cmd) -> bool {
    let a: Vec<&str> = cmd.args.iter().map(|s| s.as_str()).collect();
    match cmd.program.as_str() {
        "docker" => [
            &["version"][..],
            &["system", "df"],
            &["image", "ls"],
            &["image", "inspect"],
            &["ps"],
            &["container", "ls"],
            &["container", "inspect"],
            &["volume", "ls"],
            &["volume", "inspect"],
            &["buildx", "du"],
            &["builder", "du"],
        ]
        .iter()
        .any(|p| starts(&a, p)),
        "git" => {
            // Allow `-C <dir>` prefix.
            let a = if starts(&a, &["-C"]) && a.len() >= 2 { &a[2..] } else { &a[..] };
            [
                &["worktree", "list"][..],
                &["status"],
                &["rev-parse"],
                &["for-each-ref"],
                &["log"],
                &["branch"],
                &["merge-base"],
                &["rev-list"],
                &["diff", "--name-only"],
                &["diff", "--quiet"],
                &["config", "--get"],
                &["symbolic-ref"],
                &["check-ignore"],
                &["ls-files"],
                &["--version"],
            ]
            .iter()
            .any(|p| starts(a, p))
        }
        "pnpm" => starts(&a, &["store", "path"]) || starts(&a, &["--version"]),
        "pip" | "pip3" => starts(&a, &["cache", "dir"]) || starts(&a, &["cache", "info"]) || starts(&a, &["--version"]),
        "brew" => starts(&a, &["--cache"]) || starts(&a, &["--version"]) || (starts(&a, &["cleanup"]) && (has_dry_run(&a))),
        "xcrun" => starts(&a, &["simctl", "list"]),
        "npm" => starts(&a, &["config", "get", "cache"]) || starts(&a, &["--version"]),
        "tmutil" => a.len() == 2 && a[0] == "listlocalsnapshots",
        "rustup" => a == ["toolchain", "list"] || a == ["--version"],
        // Spotlight attributes only: `-name kMDItem...` pairs, then absolute
        // paths. Never `-plist <file>`, which writes a file.
        "mdls" => {
            let names = a.iter().take_while(|x| x.starts_with('-')).count();
            let mut i = 0;
            let mut ok = names > 0;
            while i < a.len() && a[i].starts_with('-') {
                ok &= a[i] == "-name" && a.get(i + 1).is_some_and(|n| n.starts_with("kMDItem"));
                i += 2;
            }
            ok && i < a.len() && a[i..].iter().all(|p| p.starts_with('/'))
        }
        "lsof" => true,
        "ps" => true,
        _ => false,
    }
}

fn has_dry_run(a: &[&str]) -> bool {
    a.iter().any(|x| *x == "--dry-run" || *x == "-n")
}

fn action_allowed(cmd: &Cmd) -> bool {
    let a: Vec<&str> = cmd.args.iter().map(|s| s.as_str()).collect();
    match cmd.program.as_str() {
        "docker" => {
            starts(&a, &["builder", "prune"])
                || starts(&a, &["image", "prune"])
                || (starts(&a, &["image", "rm"]) && a.len() > 2)
                || (starts(&a, &["container", "rm"]) && a.len() > 2)
                || (starts(&a, &["volume", "rm"]) && a.len() > 2)
        }
        // A linked worktree's folder, its lock, and a merged branch (spec
        // "Git worktrees"). Branch names never start with "-".
        "git" => {
            ((starts(&a, &["worktree", "remove"]) || starts(&a, &["worktree", "unlock"])) && a.len() == 3)
                || ((starts(&a, &["branch", "-d"]) || starts(&a, &["branch", "-D"])) && a.len() == 3 && !a[2].starts_with('-'))
        }
        "pnpm" => a == ["store", "prune"],
        "pip" | "pip3" => a == ["cache", "purge"],
        "brew" => a == ["cleanup"],
        // One model by name; `ollama rm` takes several, but each part runs alone.
        // One toolchain by name per part.
        "rustup" => a.len() == 3 && a[..2] == ["toolchain", "uninstall"] && crate::detectors::tools::safe_toolchain(a[2]),
        "ollama" => a.len() == 2 && a[0] == "rm" && crate::detectors::ai::safe_model_name(a[1]),
        "xcrun" => {
            a == ["simctl", "delete", "unavailable"]
                // Bounded form: exact device UDIDs only.
                || (starts(&a, &["simctl", "delete"])
                    && a.len() > 2
                    && a[2..].iter().all(|u| u.len() == 36 && u.chars().all(|c| c.is_ascii_hexdigit() || c == '-')))
        }
        _ => false,
    }
}

/// Check a command against a policy. Force flags are allowed only for
/// `docker builder prune` and `docker image prune` with a confirmed plan.
pub fn check(cmd: &Cmd, policy: Policy, confirmed: Option<&Confirmed>) -> Result<(), RunError> {
    if policy == Policy::OpenSettings {
        let exact = cmd.cwd.is_none()
            && SETTINGS_PANES.iter().any(|(_, _, prog, args)| cmd.program == *prog && cmd.args.iter().map(|a| a.as_str()).eq(args.iter().copied()));
        return if exact { Ok(()) } else { Err(RunError::Forbidden(format!("{} is not an allowed settings pane", cmd.display()))) };
    }
    if cmd.program.contains('/') || cmd.program.contains('\\') || !PROGRAMS.contains(&cmd.program.as_str()) {
        return Err(RunError::Forbidden(format!("{} is not an allowed program", cmd.program)));
    }
    if let Some(r) = banned_reason(cmd) {
        return Err(RunError::Forbidden(r));
    }
    let a: Vec<&str> = cmd.args.iter().map(|s| s.as_str()).collect();
    let force_exception = cmd.program == "docker" && (starts(&a, &["builder", "prune"]) || starts(&a, &["image", "prune"]));
    let has_force = a.iter().any(|x| is_force_flag(x));
    match policy {
        Policy::ReadOnly => {
            if has_force || !read_only_allowed(cmd) {
                return Err(RunError::Forbidden(format!("{} is not a read-only command", cmd.display())));
            }
        }
        Policy::Action => {
            if confirmed.is_none() {
                return Err(RunError::Forbidden("no confirmed plan".into()));
            }
            if !action_allowed(cmd) {
                return Err(RunError::Forbidden(format!("{} is not an allowed cleanup action", cmd.display())));
            }
            if has_force && !force_exception {
                return Err(RunError::Forbidden("force flags are not allowed for this command".into()));
            }
        }
        Policy::OpenSettings => unreachable!("handled above"),
    }
    Ok(())
}

/// Processes Freeboard started through the runner and that are still
/// running, so the process check never mistakes them for another program
/// (the scan runs git in several worktrees at once; Linux CI, 2026-11-06).
static OWN_CHILDREN: std::sync::Mutex<Vec<u32>> = std::sync::Mutex::new(Vec::new());

pub fn own_children() -> Vec<u32> {
    OWN_CHILDREN.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

struct OwnChild(u32);
impl OwnChild {
    fn new(pid: u32) -> OwnChild {
        OWN_CHILDREN.lock().unwrap_or_else(|e| e.into_inner()).push(pid);
        OwnChild(pid)
    }
}
impl Drop for OwnChild {
    fn drop(&mut self) {
        let mut v = OWN_CHILDREN.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = v.iter().position(|p| *p == self.0) {
            v.swap_remove(i);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Runner {
    /// PATH used to resolve executables. Tests point this at fake tools.
    search_path: Option<OsString>,
    pub timeout: Duration,
    /// For `run_action` (see `ACTION_TIMEOUT`).
    pub action_timeout: Duration,
    pub max_output: usize,
}

impl Default for Runner {
    fn default() -> Self {
        Runner { search_path: None, timeout: DEFAULT_TIMEOUT, action_timeout: ACTION_TIMEOUT, max_output: MAX_OUTPUT }
    }
}

impl Runner {
    pub fn with_path(path: impl Into<OsString>) -> Runner {
        Runner { search_path: Some(path.into()), ..Default::default() }
    }

    pub fn with_timeout(mut self, t: Duration) -> Runner {
        self.timeout = t;
        self
    }

    pub fn with_action_timeout(mut self, t: Duration) -> Runner {
        self.action_timeout = t;
        self
    }

    fn path_var(&self) -> OsString {
        self.search_path.clone().or_else(|| std::env::var_os("PATH")).unwrap_or_default()
    }

    /// Resolve a program on the search path (never through a shell).
    pub fn resolve(&self, program: &str) -> Option<PathBuf> {
        let exts: Vec<String> = if cfg!(windows) {
            vec![".exe".into(), ".cmd".into(), ".bat".into(), String::new()]
        } else {
            vec![String::new()]
        };
        let mut dirs: Vec<PathBuf> = std::env::split_paths(&self.path_var()).collect();
        if self.search_path.is_none() && cfg!(target_os = "macos") {
            // GUI apps on macOS start with a minimal PATH.
            for d in ["/opt/homebrew/bin", "/usr/local/bin", "/Applications/Docker.app/Contents/Resources/bin"] {
                dirs.push(PathBuf::from(d));
            }
        }
        for d in dirs {
            for e in &exts {
                let p = d.join(format!("{program}{e}"));
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        None
    }

    pub fn available(&self, program: &str) -> bool {
        self.resolve(program).is_some()
    }

    /// Run a read-only command (scans).
    pub fn run_read(&self, cmd: &Cmd) -> Result<CmdOutput, RunError> {
        check(cmd, Policy::ReadOnly, None)?;
        self.exec(cmd)
    }

    /// Open an operating-system settings pane (blind spots). Only the constant
    /// commands in `SETTINGS_PANES` can run this way.
    pub fn open_settings(&self, pane: SettingsPane, platform: &str) -> Result<CmdOutput, RunError> {
        let cmd = settings_command(pane, platform).ok_or_else(|| RunError::Forbidden("no settings pane on this platform".into()))?;
        check(&cmd, Policy::OpenSettings, None)?;
        self.exec(&cmd)
    }

    /// Run a cleanup action for a confirmed plan.
    /// `git worktree remove --force <path>`: the one exception to the ban
    /// on forced removal (invariant 7, decision 2026-11-07). Exactly that
    /// form, for an acknowledged item in a confirmed plan; anything else goes
    /// through the normal checks.
    pub fn run_acknowledged_removal(&self, cmd: &Cmd, confirmed: &Confirmed, _ack: &Acknowledged) -> Result<CmdOutput, RunError> {
        let a: Vec<&str> = cmd.args.iter().map(|s| s.as_str()).collect();
        let exact = cmd.program == "git" && a.len() == 4 && a[..3] == ["worktree", "remove", "--force"] && !a[3].starts_with('-') && !a[3].is_empty();
        if !exact {
            return Err(RunError::Forbidden(format!("{} is not an allowed acknowledged removal", cmd.display())));
        }
        let _ = confirmed;
        self.exec_for(cmd, self.action_timeout)
    }

    pub fn run_action(&self, cmd: &Cmd, confirmed: &Confirmed) -> Result<CmdOutput, RunError> {
        check(cmd, Policy::Action, Some(confirmed))?;
        self.exec_for(cmd, self.action_timeout)
    }

    fn exec(&self, cmd: &Cmd) -> Result<CmdOutput, RunError> {
        self.exec_for(cmd, self.timeout)
    }

    fn exec_for(&self, cmd: &Cmd, timeout: Duration) -> Result<CmdOutput, RunError> {
        let exe = self.resolve(&cmd.program).ok_or_else(|| RunError::NotFound(cmd.program.clone()))?;
        let mut c = Command::new(exe);
        c.args(&cmd.args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(p) = &self.search_path {
            c.env("PATH", p);
        }
        // Keep tool output stable and non-interactive.
        c.env("LC_ALL", "C").env("GIT_TERMINAL_PROMPT", "0").env("GIT_OPTIONAL_LOCKS", "0");
        // Tools must not update themselves or send analytics when Freeboard
        // runs them (invariant 12: no network activity of Freeboard's own).
        c.env("HOMEBREW_NO_AUTO_UPDATE", "1")
            .env("HOMEBREW_NO_ANALYTICS", "1")
            .env("HOMEBREW_NO_ENV_HINTS", "1")
            .env("DOCKER_CLI_HINTS", "false")
            .env("PIP_DISABLE_PIP_VERSION_CHECK", "1")
            .env("NPM_CONFIG_UPDATE_NOTIFIER", "false")
            .env("NO_UPDATE_NOTIFIER", "1");
        if let Some(d) = &cmd.cwd {
            c.current_dir(d);
        }
        // ETXTBSY (26): on Linux an executable that was just written can be
        // briefly "busy" while another thread forks; retry a few times.
        let mut attempt = 0;
        let mut child = loop {
            match c.spawn() {
                Ok(ch) => break ch,
                Err(e) if e.raw_os_error() == Some(26) && attempt < 5 => {
                    attempt += 1;
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(RunError::NotFound(cmd.program.clone())),
                Err(e) => return Err(RunError::Io(e.to_string())),
            }
        };
        // Freeboard's own tool processes never count as "a program using"
        // a folder (platform::process_cwds); registered while they run.
        let _own = OwnChild::new(child.id());
        let max = self.max_output;
        let mut so = child.stdout.take().expect("piped");
        let mut se = child.stderr.take().expect("piped");
        let to = std::thread::spawn(move || read_bounded(&mut so, max));
        let te = std::thread::spawn(move || read_bounded(&mut se, 64 * 1024));
        let deadline = Instant::now() + timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(RunError::Timeout(cmd.display()));
                    }
                    std::thread::sleep(Duration::from_millis(15));
                }
                Err(e) => return Err(RunError::Io(e.to_string())),
            }
        };
        let (stdout, over) = to.join().unwrap_or_default();
        let (stderr, _) = te.join().unwrap_or_default();
        if over {
            return Err(RunError::OutputTooLarge);
        }
        Ok(CmdOutput { code: status.code(), stdout, stderr })
    }
}

fn read_bounded(r: &mut impl Read, max: usize) -> (String, bool) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    let mut over = false;
    loop {
        match r.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if buf.len() + n > max {
                    over = true;
                    // Keep draining so the child does not block on a full pipe.
                    continue;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
        }
    }
    (String::from_utf8_lossy(&buf).into_owned(), over)
}

/// Test support: write fake tool executables into a directory.
#[cfg(any(test, feature = "test-support"))]
pub mod fake {
    use std::path::Path;

    /// Create a fake executable `name` in `dir` that runs a POSIX sh script
    /// body (unix) or a batch body (windows). Tests use this only to simulate
    /// tools; production never runs a shell.
    pub fn tool(dir: &Path, name: &str, body: &str) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let p = dir.join(name);
            std::fs::write(&p, format!("#!/bin/sh\nexport PATH=\"$PATH:/usr/bin:/bin\"\n{body}\n")).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        #[cfg(windows)]
        {
            let p = dir.join(format!("{name}.cmd"));
            std::fs::write(&p, format!("@echo off\r\n{body}\r\n")).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(p: &str, a: &[&str]) -> Cmd {
        Cmd::new(p, a.iter().copied())
    }

    #[test]
    fn banned_commands_rejected_even_when_confirmed() {
        let ok = Confirmed::new("plan");
        for cmd in [
            c("git", &["clean", "-fdx"]),
            // Banned here; only run_acknowledged_removal accepts it.
            c("git", &["worktree", "remove", "--force", "/x"]),
            c("git", &["worktree", "remove", "-f", "/x"]),
            c("docker", &["system", "prune", "--all"]),
            c("docker", &["system", "prune"]),
            c("docker", &["volume", "prune"]),
            c("npm", &["cache", "clean", "--force"]),
        ] {
            assert!(matches!(check(&cmd, Policy::Action, Some(&ok)), Err(RunError::Forbidden(_))), "{cmd:?}");
            assert!(check(&cmd, Policy::ReadOnly, None).is_err());
        }
    }

    #[test]
    fn forced_worktree_removal_only_in_its_exact_form() {
        let r = Runner::default();
        let ok = Confirmed::new("plan");
        let ack = Acknowledged::new("item");
        for bad in [
            c("git", &["worktree", "remove", "-f", "/x"]),
            c("git", &["worktree", "remove", "--force", "--force", "/x"]),
            c("git", &["worktree", "remove", "--force"]),
            c("git", &["clean", "-fdx"]),
            c("git", &["branch", "-D", "x"]),
            c("rm", &["-rf", "/x"]),
        ] {
            assert!(matches!(r.run_acknowledged_removal(&bad, &ok, &ack), Err(RunError::Forbidden(_))), "{bad:?}");
        }
    }

    #[test]
    fn force_only_for_docker_builder_and_image_prune() {
        let ok = Confirmed::new("plan");
        assert!(check(&c("docker", &["builder", "prune", "--force"]), Policy::Action, Some(&ok)).is_ok());
        assert!(check(&c("docker", &["image", "prune", "--force"]), Policy::Action, Some(&ok)).is_ok());
        assert!(check(&c("docker", &["image", "rm", "--force", "abc"]), Policy::Action, Some(&ok)).is_err());
        assert!(check(&c("docker", &["container", "rm", "-f", "abc"]), Policy::Action, Some(&ok)).is_err());
        assert!(check(&c("docker", &["volume", "rm", "-f", "v"]), Policy::Action, Some(&ok)).is_err());
        assert!(check(&c("docker", &["image", "rm", "-af", "abc"]), Policy::Action, Some(&ok)).is_err());
        // Without a confirmed plan, even the exceptions are refused.
        assert!(check(&c("docker", &["builder", "prune", "--force"]), Policy::Action, None).is_err());
        assert!(check(&c("docker", &["builder", "prune", "--force"]), Policy::ReadOnly, None).is_err());
    }

    #[test]
    fn read_only_policy_rejects_mutations() {
        assert!(check(&c("docker", &["image", "rm", "x"]), Policy::ReadOnly, None).is_err());
        assert!(check(&c("git", &["worktree", "remove", "/x"]), Policy::ReadOnly, None).is_err());
        assert!(check(&c("pnpm", &["store", "prune"]), Policy::ReadOnly, None).is_err());
        assert!(check(&c("brew", &["cleanup"]), Policy::ReadOnly, None).is_err());
        assert!(check(&c("brew", &["cleanup", "--dry-run"]), Policy::ReadOnly, None).is_ok());
        assert!(check(&c("docker", &["system", "df", "--format", "{{json .}}"]), Policy::ReadOnly, None).is_ok());
        assert!(check(&c("git", &["-C", "/r", "worktree", "list", "--porcelain"]), Policy::ReadOnly, None).is_ok());
        assert!(check(&c("tmutil", &["listlocalsnapshots", "/"]), Policy::ReadOnly, None).is_ok());
        assert!(check(&c("tmutil", &["deletelocalsnapshots", "/"]), Policy::ReadOnly, None).is_err());
        assert!(check(&c("tmutil", &["thinlocalsnapshots", "/"]), Policy::ReadOnly, None).is_err());
        assert!(check(&c("tmutil", &["deletelocalsnapshots", "/"]), Policy::Action, Some(&Confirmed::new("p"))).is_err());
    }

    #[test]
    fn arbitrary_programs_and_paths_rejected() {
        for p in ["sh", "bash", "cmd", "powershell", "rm", "/bin/rm", "../docker", "C:\\x\\docker"] {
            assert!(check(&c(p, &["-c", "echo"]), Policy::ReadOnly, None).is_err(), "{p}");
            assert!(check(&c(p, &[]), Policy::Action, Some(&Confirmed::new("p"))).is_err(), "{p}");
        }
    }

    #[test]
    fn settings_policy_allows_only_the_constant_panes() {
        let fda = settings_command(SettingsPane::FullDiskAccess, "macos").unwrap();
        assert_eq!(fda.program, "open");
        assert!(check(&fda, Policy::OpenSettings, None).is_ok());
        // No pane on other platforms.
        assert!(settings_command(SettingsPane::FullDiskAccess, "linux").is_none());
        assert!(settings_command(SettingsPane::FullDiskAccess, "windows").is_none());
        // Anything else is refused: other arguments, extra arguments, paths,
        // other programs, a working folder.
        for bad in [
            c("open", &["/Users/me/Documents"]),
            c("open", &["-a", "Terminal"]),
            c("open", &["x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles", "/etc"]),
            c("open", &["https://example.com"]),
            c("open", &[]),
            c("docker", &["system", "df"]),
            c("/usr/bin/open", &["x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles"]),
            c("sh", &["-c", "open x-apple.systempreferences:"]),
        ] {
            assert!(check(&bad, Policy::OpenSettings, None).is_err(), "{bad:?}");
        }
        assert!(check(&fda.clone().in_dir(Path::new("/tmp")), Policy::OpenSettings, None).is_err());
        // `open` is never allowed by the scan or cleanup policies.
        assert!(check(&fda, Policy::ReadOnly, None).is_err());
        assert!(check(&fda, Policy::Action, Some(&Confirmed::new("p"))).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn open_settings_runs_the_constant_command_only() {
        let d = tempfile::tempdir().unwrap();
        let log = d.path().join("log");
        fake::tool(d.path(), "open", &format!(r#"for a in "$@"; do echo "[$a]" >> '{}'; done"#, log.display()));
        let r = Runner::with_path(d.path());
        assert!(r.open_settings(SettingsPane::FullDiskAccess, "linux").is_err());
        r.open_settings(SettingsPane::FullDiskAccess, "macos").unwrap();
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "[x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles]\n");
    }

    #[cfg(unix)]
    #[test]
    fn no_shell_interpolation() {
        let d = tempfile::tempdir().unwrap();
        fake::tool(d.path(), "git", r#"for a in "$@"; do echo "[$a]"; done"#);
        let r = Runner::with_path(d.path());
        let out = r.run_read(&c("git", &["log", "$(touch /tmp/pwned); `id`"])).unwrap();
        assert_eq!(out.stdout, "[log]\n[$(touch /tmp/pwned); `id`]\n");
    }

    #[cfg(unix)]
    #[test]
    fn timeout_kills_and_reports() {
        let d = tempfile::tempdir().unwrap();
        fake::tool(d.path(), "docker", "sleep 5");
        let r = Runner::with_path(d.path()).with_timeout(Duration::from_millis(200));
        let t = Instant::now();
        assert!(matches!(r.run_read(&c("docker", &["version"])), Err(RunError::Timeout(_))));
        assert!(t.elapsed() < Duration::from_secs(3));
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_actions_get_their_own_longer_timeout() {
        let d = tempfile::tempdir().unwrap();
        fake::tool(d.path(), "git", "sleep 1");
        // A read stops at the short limit; the same command as a confirmed
        // action outlives it.
        let r = Runner::with_path(d.path()).with_timeout(Duration::from_millis(200)).with_action_timeout(Duration::from_secs(5));
        assert!(matches!(r.run_read(&c("git", &["status"])), Err(RunError::Timeout(_))));
        let ok = r.run_action(&c("git", &["worktree", "remove", "/tmp/wt"]), &Confirmed::new("plan-test"));
        assert!(ok.is_ok(), "{ok:?}");
        assert_eq!(Runner::default().action_timeout, ACTION_TIMEOUT);
        assert!(ACTION_TIMEOUT >= Duration::from_secs(10 * 60));
    }

    #[test]
    fn default_timeout_is_twenty_seconds() {
        assert_eq!(Runner::default().timeout, Duration::from_secs(20));
    }

    #[test]
    fn missing_tool_is_not_found() {
        let d = tempfile::tempdir().unwrap();
        let r = Runner::with_path(d.path());
        assert!(matches!(r.run_read(&c("docker", &["version"])), Err(RunError::NotFound(_))));
    }

    #[cfg(unix)]
    #[test]
    fn output_is_bounded() {
        let d = tempfile::tempdir().unwrap();
        fake::tool(d.path(), "docker", "head -c 200000 /dev/zero");
        let mut r = Runner::with_path(d.path());
        r.max_output = 1000;
        assert_eq!(r.run_read(&c("docker", &["version"])), Err(RunError::OutputTooLarge));
    }
}
