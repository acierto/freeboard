//! SQLite access, versioned migrations and retention rules.
//!
//! Tables follow the source "Local storage" table. Additional backend-only
//! tables hold plan snapshots, acknowledgments, scoped Trash identities and
//! skip notes.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection, OptionalExtension};

use crate::model::{Finding, Suggestion};
use crate::settings::Settings;

pub const SAMPLE_RETENTION_SECS: i64 = 30 * 24 * 3600;
pub const HISTORY_RETENTION_SECS: i64 = 365 * 24 * 3600;
/// Daily disk-map snapshots used for growth: 30 days.
pub const MAP_SNAPSHOT_RETENTION_SECS: i64 = 30 * 24 * 3600;

/// Ordered migrations. Never edit a shipped migration; append a new one.
pub const MIGRATIONS: &[&str] = &[
    // 1: source tables plus backend-only plan state.
    r#"
    CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE free_space_samples (
        id INTEGER PRIMARY KEY,
        volume TEXT NOT NULL,
        at INTEGER NOT NULL,
        free_bytes INTEGER NOT NULL,
        total_bytes INTEGER NOT NULL
    );
    CREATE INDEX free_space_samples_volume_at ON free_space_samples(volume, at);
    CREATE TABLE suggestions (
        id TEXT PRIMARY KEY,
        scan_id TEXT NOT NULL,
        ord INTEGER NOT NULL,
        json TEXT NOT NULL,
        target TEXT NOT NULL
    );
    CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE cleanup_plans (
        id TEXT PRIMARY KEY,
        created_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        scan_generation INTEGER NOT NULL,
        snapshot TEXT NOT NULL,
        digest TEXT NOT NULL,
        state TEXT NOT NULL
    );
    CREATE TABLE plan_acks (
        plan_id TEXT NOT NULL,
        item_id TEXT NOT NULL,
        at INTEGER NOT NULL,
        PRIMARY KEY (plan_id, item_id)
    );
    CREATE TABLE cleanup_runs (
        id TEXT PRIMARY KEY,
        plan_id TEXT NOT NULL,
        plan_json TEXT NOT NULL,
        started_at INTEGER NOT NULL,
        finished_at INTEGER,
        state TEXT NOT NULL,
        volume TEXT NOT NULL,
        free_before INTEGER NOT NULL,
        free_after INTEGER,
        expected_bytes INTEGER NOT NULL,
        trash_bytes INTEGER NOT NULL DEFAULT 0,
        involves_docker INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX cleanup_runs_started ON cleanup_runs(started_at);
    CREATE TABLE cleanup_items (
        run_id TEXT NOT NULL,
        item_id TEXT NOT NULL,
        ord INTEGER NOT NULL,
        snapshot TEXT NOT NULL,
        action TEXT NOT NULL,
        state TEXT NOT NULL,
        reason TEXT,
        expected_bytes INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (run_id, item_id)
    );
    CREATE TABLE trash_items (
        id INTEGER PRIMARY KEY,
        run_id TEXT NOT NULL,
        item_id TEXT NOT NULL,
        original_path TEXT NOT NULL,
        trashed_at INTEGER NOT NULL,
        bytes INTEGER NOT NULL,
        identity TEXT,
        state TEXT NOT NULL
    );
    "#,
    // 2: skipped items keep their reason across scans.
    r#"
    CREATE TABLE skip_notes (
        item_id TEXT PRIMARY KEY,
        reason TEXT NOT NULL,
        at INTEGER NOT NULL
    );
    "#,
    // 3: rebuild (2026-11-06). Parts, decisions, the pile, the disk map,
    // blind spots and Keep me tidy rules. Stored suggestions use the old
    // category model, so they are cleared (they are replaced on every scan
    // anyway) and plans built from them are invalidated.
    r#"
    CREATE TABLE suggestion_parts (
        suggestion_id TEXT NOT NULL,
        part_id TEXT NOT NULL,
        ord INTEGER NOT NULL,
        json TEXT NOT NULL,
        PRIMARY KEY (suggestion_id, part_id)
    );
    CREATE TABLE item_decisions (
        resource_id TEXT PRIMARY KEY,
        decision TEXT,
        deselected_parts TEXT NOT NULL DEFAULT '[]',
        at INTEGER NOT NULL
    );
    CREATE TABLE pile_state (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        json TEXT NOT NULL,
        at INTEGER NOT NULL
    );
    CREATE TABLE map_snapshots (
        volume TEXT NOT NULL,
        day INTEGER NOT NULL,
        json TEXT NOT NULL,
        PRIMARY KEY (volume, day)
    );
    CREATE TABLE blind_spot_decisions (
        spot_id TEXT PRIMARY KEY,
        decision TEXT NOT NULL,
        at INTEGER NOT NULL
    );
    CREATE TABLE tidy_rules (
        rule_id TEXT PRIMARY KEY,
        enabled INTEGER NOT NULL DEFAULT 0,
        at INTEGER NOT NULL
    );
    CREATE TABLE tidy_runs (
        id TEXT PRIMARY KEY,
        rule_id TEXT NOT NULL,
        run_id TEXT,
        at INTEGER NOT NULL,
        result TEXT NOT NULL,
        bytes_freed INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX tidy_runs_at ON tidy_runs(at);
    DELETE FROM suggestions;
    UPDATE cleanup_plans SET state = 'invalidated' WHERE state = 'open';
    "#,
];

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("database schema {0} is newer than this app supports")]
    TooNew(i64),
}

pub struct Store {
    conn: Mutex<Connection>,
}

pub fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

impl Store {
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Store, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Store, StoreError> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        migrate(&conn, MIGRATIONS.len())?;
        Ok(Store { conn: Mutex::new(conn) })
    }

    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn schema_version(&self) -> i64 {
        self.conn().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap_or(0)
    }

    // ----- settings -------------------------------------------------------

    pub fn load_settings(&self) -> Result<Settings, StoreError> {
        let c = self.conn();
        let mut st = c.prepare("SELECT key, value FROM settings")?;
        let rows = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Settings::from_rows(&rows))
    }

    pub fn save_settings(&self, s: &Settings) -> Result<(), StoreError> {
        let mut c = self.conn();
        let tx = c.transaction()?;
        for (k, v) in s.to_rows() {
            tx.execute(
                "INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![k, v],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn has_settings(&self) -> bool {
        self.conn().query_row("SELECT COUNT(*) FROM settings", [], |r| r.get::<_, i64>(0)).unwrap_or(0) > 0
    }

    // ----- meta -----------------------------------------------------------

    pub fn meta_get(&self, key: &str) -> Option<String> {
        self.conn()
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    pub fn meta_set(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.conn().execute(
            "INSERT INTO meta(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn scan_generation(&self) -> i64 {
        self.meta_get("scan_generation").and_then(|v| v.parse().ok()).unwrap_or(0)
    }

    // ----- samples --------------------------------------------------------

    pub fn insert_sample(&self, volume: &str, at: i64, free: u64, total: u64) -> Result<(), StoreError> {
        self.conn().execute(
            "INSERT INTO free_space_samples(volume, at, free_bytes, total_bytes) VALUES (?1, ?2, ?3, ?4)",
            params![volume, at, free as i64, total as i64],
        )?;
        Ok(())
    }

    /// Samples for a volume since `since`, oldest first: (at, free, total).
    pub fn samples(&self, volume: &str, since: i64) -> Result<Vec<(i64, u64, u64)>, StoreError> {
        let c = self.conn();
        let mut st = c.prepare(
            "SELECT at, free_bytes, total_bytes FROM free_space_samples WHERE volume = ?1 AND at >= ?2 ORDER BY at",
        )?;
        let rows = st
            .query_map(params![volume, since], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)? as u64, r.get::<_, i64>(2)? as u64)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn last_sample_at(&self, volume: &str) -> Option<i64> {
        self.conn()
            .query_row("SELECT MAX(at) FROM free_space_samples WHERE volume = ?1", [volume], |r| r.get::<_, Option<i64>>(0))
            .ok()
            .flatten()
    }

    // ----- suggestions ----------------------------------------------------

    /// Replace the latest scan's results (retention: replaced on each scan)
    /// and bump the scan generation, which invalidates open plans.
    pub fn replace_findings(&self, scan_id: &str, findings: &[Finding]) -> Result<i64, StoreError> {
        let mut c = self.conn();
        let tx = c.transaction()?;
        tx.execute("DELETE FROM suggestions", [])?;
        tx.execute("DELETE FROM suggestion_parts", [])?;
        for (i, f) in findings.iter().enumerate() {
            tx.execute(
                "INSERT INTO suggestions(id, scan_id, ord, json, target) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![f.suggestion.id, scan_id, i as i64, serde_json::to_string(&f.suggestion)?, serde_json::to_string(&f.target)?],
            )?;
        }
        let gen: i64 = tx
            .query_row("SELECT value FROM meta WHERE key = 'scan_generation'", [], |r| r.get::<_, String>(0))
            .optional()?
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
            + 1;
        tx.execute(
            "INSERT INTO meta(key, value) VALUES ('scan_generation', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [gen.to_string()],
        )?;
        tx.execute("UPDATE cleanup_plans SET state = 'invalidated' WHERE state = 'open'", [])?;
        tx.commit()?;
        Ok(gen)
    }

    pub fn findings(&self) -> Result<Vec<Finding>, StoreError> {
        let c = self.conn();
        let mut st = c.prepare("SELECT json, target FROM suggestions ORDER BY ord")?;
        let rows = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(st);
        let notes = skip_notes(&c)?;
        let mut out = Vec::with_capacity(rows.len());
        for (j, t) in rows {
            let mut suggestion: Suggestion = serde_json::from_str(&j)?;
            if let Some(r) = notes.get(&suggestion.id) {
                suggestion.skip_reason = Some(r.clone());
            }
            out.push(Finding { suggestion, target: serde_json::from_str(&t)? });
        }
        Ok(out)
    }

    pub fn finding(&self, id: &str) -> Result<Option<Finding>, StoreError> {
        Ok(self.findings()?.into_iter().find(|f| f.suggestion.id == id))
    }

    pub fn set_skip_note(&self, item_id: &str, reason: &str) -> Result<(), StoreError> {
        self.conn().execute(
            "INSERT INTO skip_notes(item_id, reason, at) VALUES (?1, ?2, ?3) ON CONFLICT(item_id) DO UPDATE SET reason = excluded.reason, at = excluded.at",
            params![item_id, reason, now_secs()],
        )?;
        Ok(())
    }

    pub fn clear_skip_note(&self, item_id: &str) -> Result<(), StoreError> {
        self.conn().execute("DELETE FROM skip_notes WHERE item_id = ?1", [item_id])?;
        Ok(())
    }

    // ----- Keep me tidy rules ---------------------------------------------

    /// Whether a Keep me tidy rule is on. Rules are off unless the user
    /// turned them on (spec "Default settings": Keep me tidy rules Off).
    pub fn tidy_rule_enabled(&self, rule_id: &str) -> bool {
        self.conn()
            .query_row("SELECT enabled FROM tidy_rules WHERE rule_id = ?1", [rule_id], |r| r.get::<_, i64>(0))
            .optional()
            .ok()
            .flatten()
            .unwrap_or(0)
            != 0
    }

    // ----- retention ------------------------------------------------------

    /// Apply source retention rules relative to `now`.
    pub fn prune(&self, now: i64) -> Result<(), StoreError> {
        let mut c = self.conn();
        let tx = c.transaction()?;
        tx.execute("DELETE FROM free_space_samples WHERE at < ?1", [now - SAMPLE_RETENTION_SECS])?;
        let cutoff = now - HISTORY_RETENTION_SECS;
        tx.execute(
            "DELETE FROM cleanup_items WHERE run_id IN (SELECT id FROM cleanup_runs WHERE started_at < ?1)",
            [cutoff],
        )?;
        // Keep Trash records that still hold space; drop settled ones with their run.
        tx.execute(
            "DELETE FROM trash_items WHERE state != 'held' AND run_id IN (SELECT id FROM cleanup_runs WHERE started_at < ?1)",
            [cutoff],
        )?;
        tx.execute("DELETE FROM cleanup_runs WHERE started_at < ?1", [cutoff])?;
        tx.execute("DELETE FROM plan_acks WHERE plan_id IN (SELECT id FROM cleanup_plans WHERE created_at < ?1)", [cutoff])?;
        tx.execute("DELETE FROM cleanup_plans WHERE created_at < ?1", [cutoff])?;
        tx.execute("DELETE FROM skip_notes WHERE at < ?1", [cutoff])?;
        tx.execute("DELETE FROM tidy_runs WHERE at < ?1", [cutoff])?;
        tx.execute("DELETE FROM map_snapshots WHERE day < ?1", [(now - MAP_SNAPSHOT_RETENTION_SECS).div_euclid(86400)])?;
        tx.commit()?;
        Ok(())
    }
}

fn skip_notes(c: &Connection) -> Result<std::collections::HashMap<String, String>, StoreError> {
    let mut st = c.prepare("SELECT item_id, reason FROM skip_notes")?;
    let m = st
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<Result<std::collections::HashMap<_, _>, _>>()?;
    Ok(m)
}

/// Apply migrations up to `target` (exposed for upgrade tests).
pub fn migrate(conn: &Connection, target: usize) -> Result<(), StoreError> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if current as usize > MIGRATIONS.len() {
        return Err(StoreError::TooNew(current));
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().take(target).skip(current as usize) {
        conn.execute_batch(&format!("BEGIN; {sql}; PRAGMA user_version = {}; COMMIT;", i + 1))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    pub fn finding(id: &str) -> Finding {
        Finding {
            suggestion: Suggestion {
                id: id.into(),
                detector: "test".into(),
                category: Category::Builds,
                title: id.into(),
                location: "/x".into(),
                size_bytes: 1,
                last_used_at: None,
                last_used_label: String::new(),
                risk: Risk::Safe,
                action: Action::Delete,
                action_label: "Delete the folder".into(),
                reason: String::new(),
                recovery: String::new(),
                recoverable: true,
                blocked_reason: None,
                needs_confirmation: false,
                fingerprint: "f".into(),
                size_is_estimate: false,
                skip_reason: None,
                ack_name: None,
            },
            target: Target::Paths { paths: vec!["/x".into()] },
        }
    }

    #[test]
    fn fresh_database_creation() {
        let s = Store::open_in_memory().unwrap();
        assert_eq!(s.schema_version(), MIGRATIONS.len() as i64);
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("nested/freeboard.db");
        let s = Store::open(&p).unwrap();
        assert!(p.exists());
        assert_eq!(s.schema_version(), MIGRATIONS.len() as i64);
    }

    #[test]
    fn upgrade_from_v1_fixture_keeps_data() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("v1.db");
        {
            let c = Connection::open(&p).unwrap();
            migrate(&c, 1).unwrap();
            c.execute("INSERT INTO settings(key, value) VALUES ('notifications', 'false')", []).unwrap();
            c.execute(
                "INSERT INTO cleanup_runs(id, plan_id, plan_json, started_at, state, volume, free_before, expected_bytes) VALUES ('r1','p1','{}',?1,'finished','home',1,1)",
                [now_secs()],
            )
            .unwrap();
            let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
            assert_eq!(v, 1);
        }
        let s = Store::open(&p).unwrap();
        assert_eq!(s.schema_version(), MIGRATIONS.len() as i64);
        assert!(!s.load_settings().unwrap().notifications);
        let n: i64 = s.conn().query_row("SELECT COUNT(*) FROM cleanup_runs", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
        s.set_skip_note("x", "changed").unwrap();
    }

    #[test]
    fn upgrade_from_v2_to_rebuild_keeps_settings_samples_and_history() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("v2.db");
        let at = now_secs();
        {
            let c = Connection::open(&p).unwrap();
            migrate(&c, 2).unwrap();
            c.execute("INSERT INTO settings(key, value) VALUES ('notifications', 'false')", []).unwrap();
            c.execute("INSERT INTO free_space_samples(volume, at, free_bytes, total_bytes) VALUES ('home', ?1, 5, 10)", [at]).unwrap();
            c.execute(
                "INSERT INTO cleanup_runs(id, plan_id, plan_json, started_at, state, volume, free_before, expected_bytes) VALUES ('r1','p1','{}',?1,'finished','home',1,1)",
                [at],
            )
            .unwrap();
            c.execute(
                "INSERT INTO cleanup_items(run_id, item_id, ord, snapshot, action, state, expected_bytes, updated_at) VALUES ('r1','i',0,'{}','delete','done',1,?1)",
                [at],
            )
            .unwrap();
            c.execute("INSERT INTO suggestions(id, scan_id, ord, json, target) VALUES ('old','s',0,'{\"category\":\"builds\"}','{}')", []).unwrap();
            c.execute(
                "INSERT INTO cleanup_plans(id, created_at, expires_at, scan_generation, snapshot, digest, state) VALUES ('p',?1,?1,1,'[]','d','open')",
                [at],
            )
            .unwrap();
        }
        let s = Store::open(&p).unwrap();
        assert_eq!(s.schema_version(), MIGRATIONS.len() as i64);
        assert!(!s.load_settings().unwrap().notifications, "settings kept");
        assert_eq!(s.samples("home", 0).unwrap(), vec![(at, 5, 10)], "samples kept");
        let c = s.conn();
        let count = |t: &str| c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get::<_, i64>(0)).unwrap();
        assert_eq!(count("cleanup_runs"), 1, "history kept");
        assert_eq!(count("cleanup_items"), 1, "history items kept");
        assert_eq!(count("suggestions"), 0, "old-model suggestions are cleared for the next scan");
        let st: String = c.query_row("SELECT state FROM cleanup_plans WHERE id='p'", [], |r| r.get(0)).unwrap();
        assert_eq!(st, "invalidated", "plans from old suggestions cannot run");
        for t in ["suggestion_parts", "item_decisions", "pile_state", "map_snapshots", "blind_spot_decisions", "tidy_rules", "tidy_runs"] {
            assert_eq!(count(t), 0, "{t} exists and starts empty");
        }
    }

    #[test]
    fn rebuild_tables_follow_their_retention_rules() {
        let s = Store::open_in_memory().unwrap();
        let now = 1_800_000_000;
        let day = |secs_ago: i64| (now - secs_ago).div_euclid(86400);
        {
            let c = s.conn();
            // map_snapshots: daily for 30 days.
            c.execute("INSERT INTO map_snapshots(volume, day, json) VALUES ('home', ?1, '{}')", [day(31 * 86400)]).unwrap();
            c.execute("INSERT INTO map_snapshots(volume, day, json) VALUES ('home', ?1, '{}')", [day(29 * 86400)]).unwrap();
            // tidy_runs: 1 year. tidy_rules: permanent.
            c.execute("INSERT INTO tidy_runs(id, rule_id, at, result) VALUES ('old','docker',?1,'done')", [now - 366 * 86400]).unwrap();
            c.execute("INSERT INTO tidy_runs(id, rule_id, at, result) VALUES ('new','docker',?1,'done')", [now - 364 * 86400]).unwrap();
            c.execute("INSERT INTO tidy_rules(rule_id, enabled, at) VALUES ('docker', 1, ?1)", [now - 900 * 86400]).unwrap();
            // Decisions last until changed, however old.
            c.execute("INSERT INTO item_decisions(resource_id, decision, at) VALUES ('x','keep',?1)", [now - 900 * 86400]).unwrap();
            c.execute("INSERT INTO blind_spot_decisions(spot_id, decision, at) VALUES ('s','mine',?1)", [now - 900 * 86400]).unwrap();
            c.execute("INSERT INTO pile_state(id, json, at) VALUES (1, '{}', ?1)", [now - 900 * 86400]).unwrap();
        }
        s.prune(now).unwrap();
        let c = s.conn();
        let ids = |q: &str| -> Vec<String> { c.prepare(q).unwrap().query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap() };
        assert_eq!(ids("SELECT CAST(day AS TEXT) FROM map_snapshots"), vec![day(29 * 86400).to_string()]);
        assert_eq!(ids("SELECT id FROM tidy_runs"), vec!["new".to_string()]);
        for t in ["tidy_rules", "item_decisions", "blind_spot_decisions", "pile_state"] {
            let n: i64 = c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0)).unwrap();
            assert_eq!(n, 1, "{t} is not pruned by age");
        }
    }

    #[test]
    fn tidy_rules_are_off_on_a_fresh_install() {
        let s = Store::open_in_memory().unwrap();
        assert!(!s.tidy_rule_enabled("package_caches"));
        assert!(!s.tidy_rule_enabled("docker_build_cache"));
        let n: i64 = s.conn().query_row("SELECT COUNT(*) FROM tidy_rules WHERE enabled != 0", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn each_scan_replaces_suggestion_parts() {
        let s = Store::open_in_memory().unwrap();
        s.conn().execute("INSERT INTO suggestion_parts(suggestion_id, part_id, ord, json) VALUES ('a','p',0,'{}')", []).unwrap();
        s.replace_findings("s1", &[finding("a")]).unwrap();
        let n: i64 = s.conn().query_row("SELECT COUNT(*) FROM suggestion_parts", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn refuses_newer_schema() {
        let c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "user_version", 99).unwrap();
        assert!(matches!(migrate(&c, MIGRATIONS.len()), Err(StoreError::TooNew(99))));
    }

    #[test]
    fn settings_round_trip() {
        let s = Store::open_in_memory().unwrap();
        assert_eq!(s.load_settings().unwrap(), Settings::default());
        let mut changed = Settings::default();
        changed.low_threshold_bytes = 40 * GB;
        changed.notifications = false;
        s.save_settings(&changed).unwrap();
        assert_eq!(s.load_settings().unwrap(), changed);
    }

    #[test]
    fn retention_prunes_samples_and_history() {
        let s = Store::open_in_memory().unwrap();
        let now = 1_800_000_000;
        s.insert_sample("home", now - 31 * 86400, 1, 2).unwrap();
        s.insert_sample("home", now - 29 * 86400, 1, 2).unwrap();
        s.save_settings(&Settings::default()).unwrap();
        {
            let c = s.conn();
            for (id, at) in [("old", now - 366 * 86400), ("new", now - 364 * 86400)] {
                c.execute(
                    "INSERT INTO cleanup_runs(id, plan_id, plan_json, started_at, state, volume, free_before, expected_bytes) VALUES (?1,'p','{}',?2,'finished','home',1,1)",
                    params![id, at],
                )
                .unwrap();
                c.execute(
                    "INSERT INTO cleanup_items(run_id, item_id, ord, snapshot, action, state, expected_bytes, updated_at) VALUES (?1,'i',0,'{}','delete','done',1,?2)",
                    params![id, at],
                )
                .unwrap();
            }
            c.execute(
                "INSERT INTO trash_items(run_id, item_id, original_path, trashed_at, bytes, state) VALUES ('old','i','/x',0,5,'held')",
                [],
            )
            .unwrap();
        }
        s.prune(now).unwrap();
        assert_eq!(s.samples("home", 0).unwrap().len(), 1);
        let c = s.conn();
        let runs: Vec<String> = c
            .prepare("SELECT id FROM cleanup_runs")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(runs, vec!["new".to_string()]);
        let items: i64 = c.query_row("SELECT COUNT(*) FROM cleanup_items", [], |r| r.get(0)).unwrap();
        assert_eq!(items, 1);
        let held: i64 = c.query_row("SELECT COUNT(*) FROM trash_items", [], |r| r.get(0)).unwrap();
        assert_eq!(held, 1, "held Trash records survive so space is never forgotten");
        let settings: i64 = c.query_row("SELECT COUNT(*) FROM settings", [], |r| r.get(0)).unwrap();
        assert!(settings > 0, "settings are permanent");
    }

    #[test]
    fn each_scan_replaces_suggestions_and_invalidates_plans() {
        let s = Store::open_in_memory().unwrap();
        let g1 = s.replace_findings("s1", &[finding("a"), finding("b")]).unwrap();
        s.conn()
            .execute(
                "INSERT INTO cleanup_plans(id, created_at, expires_at, scan_generation, snapshot, digest, state) VALUES ('p',0,0,?1,'[]','d','open')",
                [g1],
            )
            .unwrap();
        let g2 = s.replace_findings("s2", &[finding("c")]).unwrap();
        assert_eq!(g2, g1 + 1);
        let ids: Vec<String> = s.findings().unwrap().into_iter().map(|f| f.suggestion.id).collect();
        assert_eq!(ids, vec!["c"]);
        let st: String = s.conn().query_row("SELECT state FROM cleanup_plans WHERE id='p'", [], |r| r.get(0)).unwrap();
        assert_eq!(st, "invalidated");
    }

    #[test]
    fn skip_notes_survive_rescan() {
        let s = Store::open_in_memory().unwrap();
        s.replace_findings("s1", &[finding("a")]).unwrap();
        s.set_skip_note("a", "A new commit appeared after the scan.").unwrap();
        s.replace_findings("s2", &[finding("a")]).unwrap();
        assert_eq!(s.findings().unwrap()[0].suggestion.skip_reason.as_deref(), Some("A new commit appeared after the scan."));
    }
}
