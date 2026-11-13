//! Worktree databases on a local Postgres, with a fake `psql` (nothing real
//! is ever connected to) and a throwaway repository whose worktree `wt49`
//! has a database `crystal_wt49`.

use super::*;
use crate::detectors::postgres::*;
use crate::detectors::{git, probe, Probe};
use crate::runner::{check, Cmd, Policy};

const SEP: &str = "\u{1f}";

/// Rows as the fake server answers them: (name, oid, size, connections).
fn answer(rows: &[(&str, &str, &str, &str)]) -> Vec<u8> {
    let mut out = format!("S{SEP}5432{SEP}/var/lib/postgresql/data{SEP}{SEP}");
    for (n, oid, size, conns) in rows {
        out.push('\0');
        out.push_str(&format!("D{SEP}{n}{SEP}{oid}{SEP}{size}{SEP}{conns}"));
    }
    out.push('\n');
    out.into_bytes()
}

const ROWS: &[(&str, &str, &str, &str)] = &[
    ("app", "16384", "5620000000", "12"),
    ("crystal_template", "16390", "3100000000", "0"),
    ("crystal_wt49", "16401", "3400000000", "0"),
    ("crystal_wt8", "16402", "2900000000", "1"),
    ("axle_template_validation", "16403", "270000000", "0"),
];

/// A fake psql: the server behind 127.0.0.1 as `bol_deploy` (a Postgres in
/// Docker) answers from `answer.bin`; the default socket has no server. Every
/// call's arguments go to `psql.log`.
fn setup(fx: &Fx) -> PathBuf {
    let data = fx.root.join("pg");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join("answer.bin"), answer(ROWS)).unwrap();
    let d = data.to_string_lossy();
    fx.tool(
        "psql",
        &format!(
            "echo \"$@\" >> '{d}/psql.log'\ncase \"$*\" in\n  *'-h 127.0.0.1 -p 5432 -U bol_deploy'*) cat '{d}/answer.bin' ;;\n  *) echo 'psql: error: connection to server on socket \"/tmp/.s.PGSQL.5432\" failed: No such file or directory' >&2; exit 2 ;;\nesac"
        ),
    );
    fx.tool("dropdb", "exit 0");
    std::fs::write(
        fx.home.join(".pgpass"),
        "# local Docker Postgres\n127.0.0.1:5432:*:bol_deploy:s3cr\\:et\ndb.example.com:5432:*:prod:hunter2\n",
    )
    .unwrap();
    data
}

/// `projects/app` with linked worktrees `wt49` and `feature`.
fn repo(fx: &Fx) {
    let app = fx.mkdir("projects/app");
    git(&app, &["init", "-q", "-b", "main"]);
    std::fs::write(app.join("README.md"), "hi\n").unwrap();
    git(&app, &["add", "."]);
    git(&app, &["commit", "-q", "-m", "init"]);
    for name in ["wt49", "feature"] {
        let p = app.join(".worktrees").join(name);
        git(
            &app,
            &["worktree", "add", "-q", "-b", name, &p.to_string_lossy()],
        );
    }
}

fn dets() -> Vec<Box<dyn Detector>> {
    vec![Box::new(git::Worktrees), Box::new(WorktreeDatabases)]
}

fn db<'a>(o: &'a ScanOutcome, name: &str) -> &'a Finding {
    o.findings
        .iter()
        .find(|f| f.suggestion.title == format!("Database {name}"))
        .unwrap_or_else(|| panic!("no {name}"))
}

#[test]
fn lists_worktree_databases_and_links_them_to_their_worktree() {
    let fx = Fx::new();
    let data = setup(&fx);
    repo(&fx);
    let o = fx.scan(ScanKind::Deep, &dets());
    assert!(o.errors.is_empty(), "{:?}", o.errors);

    // Only <base>_wt<N>: never the template, the main database or others.
    let names: Vec<&str> = by_detector(&o, ID)
        .iter()
        .map(|s| s.title.as_str())
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");

    let wt49 = db(&o, "crystal_wt49");
    let s = &wt49.suggestion;
    assert_eq!(s.size_bytes, 3_400_000_000);
    assert_eq!(s.risk, Risk::Risky);
    assert!(s.needs_confirmation && !s.preselected() && s.selectable());
    assert_eq!(
        s.map_area, "docker",
        "Postgres in Docker keeps its data in Docker's disk"
    );
    assert_eq!(
        s.action_label,
        "dropdb -h 127.0.0.1 -p 5432 -U bol_deploy crystal_wt49"
    );
    assert!(
        s.reason
            .contains("worktree wt49 in app, which is still there"),
        "{}",
        s.reason
    );
    let Target::Native { program, args, .. } = &wt49.target else {
        panic!()
    };
    assert_eq!(program, "dropdb");
    assert_eq!(
        args,
        &[
            "-h",
            "127.0.0.1",
            "-p",
            "5432",
            "-U",
            "bol_deploy",
            "-w",
            "crystal_wt49"
        ]
    );

    // The worktree's row shows the database that stays behind.
    let row = o
        .findings
        .iter()
        .find(|f| f.suggestion.detector == git::ID && f.suggestion.title.ends_with(": wt49"))
        .unwrap();
    let w = row.suggestion.worktree.as_ref().unwrap();
    assert_eq!(
        w.database.as_ref().map(|d| (d.name.as_str(), d.size_bytes)),
        Some(("crystal_wt49", 3_400_000_000))
    );
    let other = o
        .findings
        .iter()
        .find(|f| f.suggestion.title.ends_with(": feature"))
        .unwrap();
    assert_eq!(other.suggestion.worktree.as_ref().unwrap().database, None);

    // In use: blocked. No worktree by its name: said so.
    let wt8 = &db(&o, "crystal_wt8").suggestion;
    assert!(wt8
        .blocked_reason
        .as_deref()
        .unwrap()
        .starts_with("1 connection is open"));
    assert!(
        wt8.reason.contains("found no worktree named wt8"),
        "{}",
        wt8.reason
    );

    // Local servers only, the password never on a command line, one constant query.
    let log = std::fs::read_to_string(data.join("psql.log")).unwrap();
    assert!(
        !log.contains("db.example.com") && !log.contains("s3cr") && !log.contains("hunter2"),
        "{log}"
    );
    assert_eq!(
        log.lines().count(),
        2,
        "the default socket and the one local pgpass entry: {log}"
    );

    // Risk notes offer a copy with the same connection.
    let mut s = s.clone();
    crate::risk_notes::annotate(&mut s);
    assert!(s.safer[0].command.ends_with("pg_dump -Fc -f ~/Backups/crystal_wt49.dump -h 127.0.0.1 -p 5432 -U bol_deploy crystal_wt49"));
}

#[test]
fn probe_rechecks_the_same_database() {
    let fx = Fx::new();
    let data = setup(&fx);
    let o = fx.scan(ScanKind::Deep, &dets());
    let f = db(&o, "crystal_wt49").clone();
    let p = || fx.with_ctx(ScanKind::Quick, |ctx| probe(&f, ctx));
    assert_eq!(p(), Probe::Unchanged);

    let write = |rows: &[(&str, &str, &str, &str)]| {
        std::fs::write(data.join("answer.bin"), answer(rows)).unwrap()
    };
    write(&[("crystal_wt49", "16401", "3400000000", "2")]);
    assert!(matches!(p(), Probe::Changed(m) if m.contains("connected")));
    write(&[("crystal_wt49", "20000", "100000", "0")]);
    assert!(matches!(p(), Probe::Changed(m) if m.contains("made again")));
    write(&[("app", "16384", "5620000000", "0")]);
    assert!(matches!(p(), Probe::Gone(_)));
}

#[test]
fn no_postgres_says_nothing_and_a_refusal_is_reported() {
    let fx = Fx::new();
    let o = fx.scan(ScanKind::Deep, &dets());
    assert!(
        o.errors.is_empty() && by_detector(&o, ID).is_empty(),
        "no psql: silent"
    );

    fx.tool("psql", "echo 'psql: error: connection to server on socket \"/tmp/.s.PGSQL.5432\" failed: FATAL:  role \"me\" does not exist' >&2; exit 2");
    let o = fx.scan(ScanKind::Deep, &dets());
    let e = o
        .errors
        .iter()
        .find(|e| e.detector == ID)
        .expect("a refusal is reported");
    assert_eq!(e.kind, "unavailable");
    assert!(
        e.message.contains("role \"me\" does not exist") && e.message.contains("~/.pgpass"),
        "{}",
        e.message
    );
}

#[test]
fn runner_allows_only_the_constant_query_and_one_worktree_database() {
    let ro = |args: &[&str]| check(&Cmd::new("psql", args.iter()), Policy::ReadOnly, None).is_ok();
    let mut q = vec![
        "-X", "-w", "-A", "-t", "-q", "-0", "-F", SEP, "-d", "postgres", "-c", QUERY,
    ];
    assert!(ro(&q));
    q[11] = "DROP DATABASE crystal_wt49";
    assert!(!ro(&q));
    assert!(!ro(&["-c", "SELECT 1"]));

    let confirmed = crate::runner::Confirmed::new("plan");
    let act = |p: &str, args: &[&str]| {
        check(&Cmd::new(p, args.iter()), Policy::Action, Some(&confirmed)).is_ok()
    };
    assert!(act(
        "dropdb",
        &[
            "-h",
            "127.0.0.1",
            "-p",
            "5432",
            "-U",
            "bol_deploy",
            "-w",
            "crystal_wt49"
        ]
    ));
    assert!(!act("dropdb", &["-w", "crystal_template"]));
    assert!(!act("dropdb", &["-w", "app"]));
    assert!(!act("dropdb", &["-f", "-w", "crystal_wt49"]));
    assert!(!act(
        "dropdb",
        &["-h", "db.example.com", "-w", "crystal_wt49"]
    ));
    assert!(!act("psql", &["-c", "DROP DATABASE crystal_wt49"]));
    assert!(
        check(
            &Cmd::new("dropdb", ["-w", "crystal_wt49"].iter()),
            Policy::ReadOnly,
            None
        )
        .is_err(),
        "never during a scan"
    );
}

#[test]
fn servers_set_in_settings_are_scanned_and_checked() {
    use crate::settings::PostgresServer;
    let mut fx = Fx::new();
    let data = setup(&fx);
    // A second server, set in Settings: port 5433 as app, with its own copy.
    std::fs::write(data.join("other.bin"), answer(&[("shop_wt2", "17000", "1200000000", "0")])).unwrap();
    let d = data.to_string_lossy();
    fx.tool(
        "psql",
        &format!(
            "echo \"$@\" >> '{d}/psql.log'\ncase \"$*\" in\n  *'-h 127.0.0.1 -p 5432 -U bol_deploy'*) cat '{d}/answer.bin' ;;\n  *'-h localhost -p 5433 -U app'*) cat '{d}/other.bin' ;;\n  *'-h localhost -p 5434'*) echo 'psql: error: connection to server at \"localhost\" (::1), port 5434 failed: FATAL:  password authentication failed for user \"me\"' >&2; exit 2 ;;\n  *) echo 'psql: error: connection to server on socket \"/tmp/.s.PGSQL.5432\" failed: No such file or directory' >&2; exit 2 ;;\nesac"
        ),
    );
    fx.settings.postgres_servers = vec![
        PostgresServer { host: "localhost".into(), port: 5433, user: "app".into() },
        PostgresServer { host: "localhost".into(), port: 5434, user: String::new() },
    ];
    let o = fx.scan(ScanKind::Deep, &dets());
    assert!(o.errors.is_empty(), "one server refusing does not hide the others: {:?}", o.errors);
    let shop = db(&o, "shop_wt2");
    assert_eq!(shop.suggestion.action_label, "dropdb -h localhost -p 5433 -U app shop_wt2");
    assert!(by_detector(&o, ID).len() == 3, "two from the pgpass server, one from Settings");

    // Check: the default (no server), Settings' two, then the pgpass line.
    let checks = crate::detectors::postgres::check(&fx.runner, &fx.home, fx.platform, &fx.settings.postgres_servers);
    let states: Vec<(&str, &str, &str)> = checks.iter().map(|c| (c.host.as_str(), c.port.as_str(), c.state.as_str())).collect();
    assert_eq!(states, vec![("", "", "absent"), ("localhost", "5433", "ok"), ("localhost", "5434", "refused"), ("127.0.0.1", "5432", "ok")]);
    assert_eq!((checks[1].databases, checks[1].bytes), (1, 1_200_000_000));
    assert_eq!(checks[3].databases, 2);
    assert!(checks[2].message.as_deref().unwrap().contains("password authentication failed"));

    // Without psql: every server says so.
    let empty = Runner::with_path(fx.root.join("nothing"));
    assert!(crate::detectors::postgres::check(&empty, &fx.home, fx.platform, &fx.settings.postgres_servers).iter().all(|c| c.state == "no_psql"));
}

#[test]
fn settings_accept_only_local_servers() {
    use crate::settings::{PostgresServer, Settings, SettingsPatch};
    let s = Settings::default();
    let with = |v: Vec<PostgresServer>| s.apply(SettingsPatch { postgres_servers: Some(v), ..Default::default() });
    let srv = |h: &str, p: u16, u: &str| PostgresServer { host: h.into(), port: p, user: u.into() };
    assert!(with(vec![srv("localhost", 5432, ""), srv("127.0.0.1", 5433, "app"), srv("::1", 5434, "dev.user")]).is_ok());
    assert!(with(vec![srv("db.example.com", 5432, "")]).is_err());
    assert!(with(vec![srv("localhost", 0, "")]).is_err());
    assert!(with(vec![srv("localhost", 5432, "-c")]).is_err());
    assert!(with(vec![srv("localhost", 5432, "a b")]).is_err());
    assert!(with(vec![srv("localhost", 5432, "x"), srv("localhost", 5432, "x")]).is_err(), "no duplicates");
    assert!(with((0..11).map(|i| srv("localhost", 6000 + i, "")).collect()).is_err(), "at most ten");
}
