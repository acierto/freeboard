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

const PROGRAMS: &[&str] = &["docker", "git", "pnpm", "pip", "pip3", "brew", "xcrun", "npm", "lsof", "ps"];

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
        "git" => starts(&a, &["worktree", "remove"]) && a.len() == 3,
        "pnpm" => a == ["store", "prune"],
        "pip" | "pip3" => a == ["cache", "purge"],
        "brew" => a == ["cleanup"],
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
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Runner {
    /// PATH used to resolve executables. Tests point this at fake tools.
    search_path: Option<OsString>,
    pub timeout: Duration,
    pub max_output: usize,
}

impl Default for Runner {
    fn default() -> Self {
        Runner { search_path: None, timeout: DEFAULT_TIMEOUT, max_output: MAX_OUTPUT }
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

    /// Run a cleanup action for a confirmed plan.
    pub fn run_action(&self, cmd: &Cmd, confirmed: &Confirmed) -> Result<CmdOutput, RunError> {
        check(cmd, Policy::Action, Some(confirmed))?;
        self.exec(cmd)
    }

    fn exec(&self, cmd: &Cmd) -> Result<CmdOutput, RunError> {
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
        let mut child = c.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                RunError::NotFound(cmd.program.clone())
            } else {
                RunError::Io(e.to_string())
            }
        })?;
        let max = self.max_output;
        let mut so = child.stdout.take().expect("piped");
        let mut se = child.stderr.take().expect("piped");
        let to = std::thread::spawn(move || read_bounded(&mut so, max));
        let te = std::thread::spawn(move || read_bounded(&mut se, 64 * 1024));
        let deadline = Instant::now() + self.timeout;
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
    }

    #[test]
    fn arbitrary_programs_and_paths_rejected() {
        for p in ["sh", "bash", "cmd", "powershell", "rm", "/bin/rm", "../docker", "C:\\x\\docker"] {
            assert!(check(&c(p, &["-c", "echo"]), Policy::ReadOnly, None).is_err(), "{p}");
            assert!(check(&c(p, &[]), Policy::Action, Some(&Confirmed::new("p"))).is_err(), "{p}");
        }
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
