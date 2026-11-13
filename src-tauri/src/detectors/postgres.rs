//! `postgres.worktree_dbs`: per-worktree copies of a development database on
//! a Postgres server on this computer (`<base>_wt<N>`, such as
//! `crystal_wt49`). Projects that give every worktree its own database clone
//! it from a template, so each copy holds gigabytes that removing the
//! worktree folder leaves behind (found 2026-11-13: seven copies held 29 GB).
//!
//! Local servers only: the libpq default (the local socket, or localhost on
//! Windows) and the localhost entries of the user's own `~/.pgpass`. Freeboard
//! reads only the host, port and user of those entries; psql reads the
//! password itself, and Freeboard never stores one. The scan runs one constant
//! read-only query (`QUERY`). Removing runs `dropdb` for one database, which
//! is Risky (no undo), never preselected and acknowledged by name. A database
//! with open connections is blocked. Template databases never match.
//!
//! `link_worktrees` runs after every scan and ties each database to the
//! worktree it was made for, when one by that name is still there, and shows
//! the database on that worktree's row.

use std::path::{Path, PathBuf};

use super::common::*;
use super::git;
use super::Probe;
use crate::model::*;
use crate::runner::{Cmd, Runner};
use crate::settings::PostgresServer;
use crate::scan::{Detector, ScanContext};

pub const ID: &str = "postgres.worktree_dbs";

pub struct WorktreeDatabases;

/// The one query the scan and the pre-cleanup check run: a server row, then
/// one row per database that accepts connections and is not a template.
/// Every column is text; the size is empty when the user may not connect.
pub const QUERY: &str = concat!(
    "SELECT 'S', current_setting('port'), coalesce((SELECT setting FROM pg_settings WHERE name = 'data_directory'), ''), '', '' ",
    "UNION ALL SELECT 'D', d.datname, d.oid::text, ",
    "CASE WHEN has_database_privilege(d.oid, 'CONNECT') THEN pg_database_size(d.oid)::text ELSE '' END, ",
    "coalesce(s.numbackends, 0)::text ",
    "FROM pg_database d LEFT JOIN pg_stat_database s ON s.datid = d.oid ",
    "WHERE NOT d.datistemplate AND d.datallowconn"
);

/// psql output: fields split by the unit separator, rows by NUL (`-0`), so a
/// database name can never pass for another row.
pub const FIELD_SEP: &str = "\u{1f}";

/// Hosts that are this computer.
const LOCAL_HOSTS: &[&str] = &["localhost", "127.0.0.1", "::1"];

/// How to reach one server: what psql and dropdb get as `-h`, `-p`, `-U`.
/// None leaves it to libpq's default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Conn {
    pub host: Option<String>,
    pub port: Option<String>,
    pub user: Option<String>,
}

impl Conn {
    pub fn args(&self) -> Vec<String> {
        let mut a = vec![];
        for (flag, v) in [("-h", &self.host), ("-p", &self.port), ("-U", &self.user)] {
            if let Some(v) = v {
                a.push(flag.to_string());
                a.push(v.clone());
            }
        }
        a
    }
}

/// A user name psql and dropdb may get as `-U`.
pub fn safe_user(u: &str) -> bool {
    !u.is_empty()
        && u.len() <= 63
        && !u.starts_with('-')
        && u.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

fn safe_port(p: &str) -> bool {
    !p.is_empty() && p.len() <= 5 && p.chars().all(|c| c.is_ascii_digit())
}

/// `<base>_wt<N>`: letters, digits and `_` only, so it needs no quoting and
/// can't be read as an option; `N` is the worktree's number.
pub fn worktree_db(name: &str) -> Option<(&str, &str)> {
    if name.is_empty()
        || name.len() > 63
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    let (base, n) = name.rsplit_once("_wt")?;
    let base_ok = !base.is_empty() && !base.to_ascii_lowercase().ends_with("template");
    (base_ok && !n.is_empty() && n.len() <= 6 && n.chars().all(|c| c.is_ascii_digit()))
        .then_some((base, n))
}

/// Leading `-h <local host> -p <port> -U <user>` pairs, each at most once,
/// in that order. Returns how many arguments they take, or None.
fn conn_prefix(a: &[&str]) -> Option<usize> {
    let mut i = 0;
    for (flag, ok) in [
        (
            "-h",
            (|v: &str| LOCAL_HOSTS.contains(&v)) as fn(&str) -> bool,
        ),
        ("-p", safe_port),
        ("-U", safe_user),
    ] {
        if a.get(i) == Some(&flag) {
            if !a.get(i + 1).is_some_and(|v| ok(v)) {
                return None;
            }
            i += 2;
        }
    }
    Some(i)
}

/// The read-only form the runner allows: exactly the scan's psql command.
pub fn psql_read_ok(a: &[&str]) -> bool {
    let head = ["-X", "-w", "-A", "-t", "-q", "-0", "-F", FIELD_SEP];
    if a.len() < head.len() || a[..head.len()] != head {
        return false;
    }
    let rest = &a[head.len()..];
    let Some(n) = conn_prefix(rest) else {
        return false;
    };
    rest[n..] == ["-d", "postgres", "-c", QUERY]
}

/// The removal the runner allows: `dropdb [conn] -w <name>`, one worktree
/// database, never a template.
pub fn dropdb_ok(a: &[&str]) -> bool {
    let Some(n) = conn_prefix(a) else {
        return false;
    };
    a.len() == n + 2 && a[n] == "-w" && worktree_db(a[n + 1]).is_some()
}

fn psql_cmd(conn: &Conn) -> Cmd {
    let mut a: Vec<String> = ["-X", "-w", "-A", "-t", "-q", "-0", "-F", FIELD_SEP]
        .iter()
        .map(|s| s.to_string())
        .collect();
    a.extend(conn.args());
    a.extend(
        ["-d", "postgres", "-c", QUERY]
            .iter()
            .map(|s| s.to_string()),
    );
    Cmd::new("psql", a)
}

/// Where `~/.pgpass` is (`%APPDATA%\postgresql\pgpass.conf` on Windows).
pub fn pgpass_path(home: &Path, platform: &str) -> PathBuf {
    if platform == "windows" {
        home.join("AppData")
            .join("Roaming")
            .join("postgresql")
            .join("pgpass.conf")
    } else {
        home.join(".pgpass")
    }
}

/// The first four fields of each pgpass line (host, port, database, user),
/// with `\:` and `\\` unescaped. The password field is never kept.
fn pgpass_fields(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(n) = chars.next() {
                    out.last_mut().unwrap().push(n);
                }
            }
            ':' => {
                if out.len() == 4 {
                    break;
                }
                out.push(String::new());
            }
            c => out.last_mut().unwrap().push(c),
        }
    }
    out
}

/// Servers to try: libpq's default, then each localhost entry of the pgpass
/// file. Remote entries are ignored (local servers only).
pub fn connections(ctx: &ScanContext) -> Vec<Conn> {
    connections_for(&ctx.home, ctx.platform, &ctx.settings.postgres_servers)
}

/// libpq's default, the servers set in Settings, then the pgpass file's
/// localhost lines.
pub fn connections_for(home: &Path, platform: &str, servers: &[PostgresServer]) -> Vec<Conn> {
    let mut out = vec![Conn::default()];
    for sv in servers {
        let c = Conn {
            host: Some(sv.host.clone()),
            port: Some(sv.port.to_string()),
            user: (!sv.user.is_empty()).then(|| sv.user.clone()),
        };
        if !out.contains(&c) {
            out.push(c);
        }
    }
    let text = std::fs::read_to_string(pgpass_path(home, platform)).unwrap_or_default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f = pgpass_fields(line);
        if f.len() < 4 {
            continue;
        }
        let host = match f[0].as_str() {
            "*" => None,
            h if LOCAL_HOSTS.contains(&h) => Some(h.to_string()),
            _ => continue,
        };
        let port = match f[1].as_str() {
            "*" => None,
            p if safe_port(p) => Some(p.to_string()),
            _ => continue,
        };
        let user = match f[3].as_str() {
            "*" => None,
            u if safe_user(u) => Some(u.to_string()),
            _ => continue,
        };
        let c = Conn { host, port, user };
        if !out.contains(&c) {
            out.push(c);
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct Db {
    pub name: String,
    pub oid: String,
    /// None when the user may not connect to it.
    pub size: Option<u64>,
    pub connections: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Server {
    pub conn: Conn,
    pub port: String,
    pub data_dir: String,
    pub dbs: Vec<Db>,
}

impl Server {
    /// Two connections reach the same server when port, data folder and
    /// databases (with their oids) all agree.
    fn key(&self) -> String {
        let mut d: Vec<String> = self
            .dbs
            .iter()
            .map(|d| format!("{}={}", d.oid, d.name))
            .collect();
        d.sort();
        format!("{}|{}|{}", self.port, self.data_dir, d.join(","))
    }
    /// Postgres in a Docker container keeps its data in Docker's disk.
    fn map_area(&self) -> &'static str {
        if self.data_dir == "/var/lib/postgresql/data" || self.data_dir.ends_with("/docker") {
            "docker"
        } else {
            "apps"
        }
    }
}

pub fn parse(out: &str) -> Option<(String, String, Vec<Db>)> {
    let mut server = None;
    let mut dbs = vec![];
    for row in out.split('\0') {
        let row = row.trim_matches(|c| c == '\n' || c == '\r');
        let f: Vec<&str> = row.split(FIELD_SEP).collect();
        if f.len() != 5 {
            continue;
        }
        match f[0] {
            "S" => server = Some((f[1].to_string(), f[2].to_string())),
            "D" => dbs.push(Db {
                name: f[1].to_string(),
                oid: f[2].to_string(),
                size: f[3].parse().ok(),
                connections: f[4].parse().unwrap_or(u32::MAX),
            }),
            _ => {}
        }
    }
    server.map(|(port, dir)| (port, dir, dbs))
}

enum Read {
    Server(Server),
    /// No server answered there (refused, no socket): nothing to report.
    Absent,
    /// A server answered but would not let Freeboard in.
    Refused(String),
}

fn read(runner: &Runner, conn: &Conn) -> Read {
    let out = match runner.run_read(&psql_cmd(conn)) {
        Ok(o) => o,
        Err(e) => return Read::Refused(e.to_string()),
    };
    if out.success() {
        return match parse(&out.stdout) {
            Some((port, data_dir, dbs)) => Read::Server(Server {
                conn: conn.clone(),
                port,
                data_dir,
                dbs,
            }),
            None => Read::Refused("psql returned an unexpected answer.".into()),
        };
    }
    let msg = out
        .stderr
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string();
    let m = msg.to_ascii_lowercase();
    if m.contains("connection refused")
        || m.contains("no such file or directory")
        || m.contains("could not translate host name")
    {
        Read::Absent
    } else {
        Read::Refused(
            msg.trim_start_matches("psql: ")
                .trim_start_matches("error: ")
                .to_string(),
        )
    }
}

fn finding(srv: &Server, db: &Db, base_name: &str, n: &str, can_drop: bool) -> Finding {
    let mut s = base(
        ID,
        Group::Projects,
        &format!("{}|{}|{}", srv.port, srv.data_dir, db.name),
    );
    s.map_area = srv.map_area().into();
    s.title = format!("Database {}", db.name);
    s.location = format!("Postgres on this computer, port {}", srv.port);
    s.size_bytes = db.size.unwrap_or(0);
    s.last_used_label = "Postgres does not record when a database was last used".into();
    s.risk = Risk::Risky;
    s.action = Action::NativeCommand;
    let mut args = srv.conn.args();
    let shown = args.join(" ");
    args.push("-w".into());
    args.push(db.name.clone());
    s.action_label = if shown.is_empty() {
        format!("dropdb {}", db.name)
    } else {
        format!("dropdb {shown} {}", db.name)
    };
    s.reason = format!("A copy of the {base_name} database made for worktree {n}. Freeboard found no worktree named wt{n} in your scan folders.");
    s.recovery = "Not recoverable. If your project makes these copies from a template, it can make a fresh one, without the changes made since.".into();
    s.recoverable = false;
    s.needs_confirmation = true;
    s.ack_name = Some(format!("{} database", db.name));
    s.blocked_reason = if db.connections > 0 {
        Some(if db.connections == 1 {
            "1 connection is open to it. Stop what uses it, then scan again.".into()
        } else {
            format!(
                "{} connections are open to it. Stop what uses it, then scan again.",
                db.connections
            )
        })
    } else if db.size.is_none() {
        Some("Postgres does not let you connect to it, so Freeboard can't remove it.".into())
    } else if !can_drop {
        Some("dropdb is not installed, so Freeboard can't remove it.".into())
    } else {
        None
    };
    s.fingerprint = fingerprint_of(&["pgdb", &srv.key(), &db.name, &db.oid]);
    Finding::new(
        s,
        Target::Native {
            program: "dropdb".into(),
            args,
            resources: vec![db.name.clone(), format!("oid={}", db.oid)],
            cwd: None,
        },
    )
}

impl Detector for WorktreeDatabases {
    fn id(&self) -> &'static str {
        ID
    }
    /// Deep scans, like worktrees, so every database can be tied to one.
    fn deep_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        // Most computers have no Postgres: nothing to say then.
        if !ctx.runner.available("psql") {
            return Ok(vec![]);
        }
        let can_drop = ctx.runner.available("dropdb");
        let mut seen: Vec<String> = vec![];
        let mut out = vec![];
        let mut refused: Option<String> = None;
        let mut answered = false;
        for conn in connections(ctx) {
            match read(ctx.runner, &conn) {
                Read::Server(srv) => {
                    answered = true;
                    let key = srv.key();
                    if seen.contains(&key) {
                        continue;
                    }
                    seen.push(key);
                    for db in &srv.dbs {
                        if let Some((b, n)) = worktree_db(&db.name) {
                            out.push(finding(&srv, db, b, n, can_drop));
                        }
                    }
                }
                Read::Absent => {}
                Read::Refused(m) => {
                    refused.get_or_insert(m);
                }
            }
        }
        if !answered {
            if let Some(m) = refused {
                return Err(err(
                    ID,
                    "unavailable",
                    format!("Postgres on this computer did not let Freeboard in ({m}). Put its password in ~/.pgpass, or check it in Settings, Postgres databases."),
                ));
            }
        }
        Ok(out)
    }
}

/// What Settings → Postgres databases shows for one server after Check.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCheck {
    /// `-h`, `-p`, `-U` as given; empty for libpq's default.
    pub host: String,
    pub port: String,
    pub user: String,
    /// "ok", "absent" (no server there), "refused" or "no_psql".
    pub state: String,
    /// Why it refused, in psql's words.
    pub message: Option<String>,
    /// Worktree databases found there, and their size.
    pub databases: u32,
    pub bytes: u64,
}

/// Settings → Check: try every server the scan would, read-only, now.
pub fn check(runner: &Runner, home: &Path, platform: &str, servers: &[PostgresServer]) -> Vec<ServerCheck> {
    let conns = connections_for(home, platform, servers);
    let none = !runner.available("psql");
    conns
        .into_iter()
        .map(|c| {
            let mut r = ServerCheck {
                host: c.host.clone().unwrap_or_default(),
                port: c.port.clone().unwrap_or_default(),
                user: c.user.clone().unwrap_or_default(),
                state: "no_psql".into(),
                message: None,
                databases: 0,
                bytes: 0,
            };
            if none {
                return r;
            }
            match read(runner, &c) {
                Read::Server(srv) => {
                    r.state = "ok".into();
                    for d in srv.dbs.iter().filter(|d| worktree_db(&d.name).is_some()) {
                        r.databases += 1;
                        r.bytes += d.size.unwrap_or(0);
                    }
                    if r.port.is_empty() {
                        r.port = srv.port;
                    }
                }
                Read::Absent => r.state = "absent".into(),
                Read::Refused(m) => {
                    r.state = "refused".into();
                    r.message = Some(m);
                }
            }
            r
        })
        .collect()
}

/// The worktree a database was made for: one whose folder (or branch) is
/// named `wt<N>`, or ends in `-wt<N>`, `_wt<N>` or `/wt<N>`.
fn names_slot(name: &str, n: &str) -> bool {
    let norm: String = name
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let token = format!("wt{n}");
    norm == token || norm.ends_with(&format!("_{token}"))
}

fn folder_of(w: &WorktreeInfo) -> &str {
    Path::new(&w.rel_path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(&w.rel_path)
}

/// After a scan: tie each worktree database to its worktree, both ways. The
/// worktree's row shows the database that stays behind when the folder goes;
/// the database says which worktree still uses it.
pub fn link_worktrees(findings: &mut [Finding]) {
    let dbs: Vec<(usize, String, u64)> = findings
        .iter()
        .enumerate()
        .filter(|(_, f)| f.suggestion.detector == ID)
        .filter_map(|(i, f)| {
            f.suggestion
                .title
                .strip_prefix("Database ")
                .map(|n| (i, n.to_string(), f.suggestion.size_bytes))
        })
        .collect();
    for (di, name, size) in dbs {
        let Some((base_name, n)) = worktree_db(&name) else {
            continue;
        };
        let wt = findings.iter().position(|f| {
            f.suggestion.detector == git::ID
                && f.suggestion.worktree.as_ref().is_some_and(|w| {
                    names_slot(folder_of(w), n)
                        || w.branch.as_deref().is_some_and(|b| names_slot(b, n))
                })
        });
        let Some(wi) = wt else { continue };
        let (folder, repo) = {
            let w = findings[wi].suggestion.worktree.as_mut().unwrap();
            w.database = Some(WtDatabase {
                name: name.clone(),
                size_bytes: size,
            });
            (folder_of(w).to_string(), w.repo.clone())
        };
        findings[di].suggestion.reason =
            format!("A copy of the {base_name} database made for worktree {folder} in {repo}, which is still there. Removing it breaks that worktree's local setup.");
    }
}

/// Pre-cleanup check: the same database (same oid) is still there and nothing
/// is connected to it.
pub fn probe(f: &Finding, ctx: &ScanContext) -> Probe {
    let Target::Native {
        program,
        args,
        resources,
        ..
    } = &f.target
    else {
        return Probe::Unknown("Unexpected cleanup target.".into());
    };
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    if program != "dropdb" || !dropdb_ok(&a) || resources.len() != 2 {
        return Probe::Unknown("Unexpected cleanup target.".into());
    }
    let name = a[a.len() - 1];
    let oid = resources[1].trim_start_matches("oid=");
    let mut conn = Conn::default();
    let mut i = 0;
    while i < a.len() - 2 {
        let v = Some(a[i + 1].to_string());
        match a[i] {
            "-h" => conn.host = v,
            "-p" => conn.port = v,
            "-U" => conn.user = v,
            _ => {}
        }
        i += 2;
    }
    let srv = match read(ctx.runner, &conn) {
        Read::Server(s) => s,
        Read::Absent => return Probe::Unknown("Postgres is not running.".into()),
        Read::Refused(m) => return Probe::Unknown(format!("Postgres could not be checked: {m}")),
    };
    match srv.dbs.iter().find(|d| d.name == name) {
        None => Probe::Gone("The database no longer exists.".into()),
        Some(d) if d.oid != oid => {
            Probe::Changed("The database was made again after the scan.".into())
        }
        Some(d) if d.connections > 0 => {
            Probe::Changed("A program connected to the database after the scan.".into())
        }
        Some(_) => Probe::Unchanged,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_worktree_databases_only() {
        assert_eq!(worktree_db("crystal_wt49"), Some(("crystal", "49")));
        assert_eq!(worktree_db("crystal_wt1"), Some(("crystal", "1")));
        for n in [
            "crystal_template",
            "crystal_template_wt1",
            "app",
            "postgres",
            "_wt3",
            "crystal_wt",
            "crystal_wtx",
            "a-b_wt1",
            "x_wt1;drop",
        ] {
            assert_eq!(worktree_db(n), None, "{n}");
        }
    }

    #[test]
    fn pgpass_keeps_local_hosts_and_never_the_password() {
        assert_eq!(
            pgpass_fields("127.0.0.1:5432:*:bol_deploy:se\\:cret"),
            vec!["127.0.0.1", "5432", "*", "bol_deploy"]
        );
        assert_eq!(pgpass_fields("h\\:x:1:d:u"), vec!["h:x", "1", "d", "u"]);
    }

    #[test]
    fn slot_names() {
        assert!(names_slot("wt49", "49"));
        assert!(names_slot("app-wt49", "49"));
        assert!(names_slot("crystal_wt49", "49"));
        assert!(names_slot("feature/WT49", "49"));
        assert!(!names_slot("wt491", "49"));
        assert!(!names_slot("newt49", "49"));
    }

    #[test]
    fn runner_forms() {
        let q = |extra: &[&str]| {
            let mut a = vec!["-X", "-w", "-A", "-t", "-q", "-0", "-F", FIELD_SEP];
            a.extend_from_slice(extra);
            a.extend_from_slice(&["-d", "postgres", "-c", QUERY]);
            a.iter().map(|s| s.to_string()).collect::<Vec<_>>()
        };
        let ok = |v: Vec<String>| psql_read_ok(&v.iter().map(String::as_str).collect::<Vec<_>>());
        assert!(ok(q(&[])));
        assert!(ok(q(&[
            "-h",
            "127.0.0.1",
            "-p",
            "5432",
            "-U",
            "bol_deploy"
        ])));
        assert!(!ok(q(&["-h", "db.example.com"])));
        assert!(!ok(q(&["-U", "-c"])));
        let mut other = q(&[]);
        *other.last_mut().unwrap() = "DROP DATABASE crystal_wt1".into();
        assert!(!ok(other));
        assert!(dropdb_ok(&[
            "-h",
            "localhost",
            "-U",
            "me",
            "-w",
            "crystal_wt3"
        ]));
        assert!(dropdb_ok(&["-w", "crystal_wt3"]));
        assert!(!dropdb_ok(&["-w", "crystal_template"]));
        assert!(!dropdb_ok(&["-w", "app"]));
        assert!(!dropdb_ok(&["crystal_wt3"]));
        assert!(!dropdb_ok(&["-w", "crystal_wt3", "app"]));
        assert!(!dropdb_ok(&["--force", "-w", "crystal_wt3"]));
        assert!(!dropdb_ok(&["-h", "10.0.0.5", "-w", "crystal_wt3"]));
    }

    #[test]
    fn parses_rows() {
        let s = format!("S{0}5432{0}/var/lib/postgresql/data{0}{0}\0D{0}crystal_wt1{0}16384{0}3100000000{0}0\0D{0}app{0}16385{0}{0}2\n", FIELD_SEP);
        let (port, dir, dbs) = parse(&s).unwrap();
        assert_eq!(
            (port.as_str(), dir.as_str()),
            ("5432", "/var/lib/postgresql/data")
        );
        assert_eq!(dbs.len(), 2);
        assert_eq!(dbs[0].size, Some(3_100_000_000));
        assert_eq!(dbs[1].size, None);
        assert_eq!(dbs[1].connections, 2);
    }
}
