//! SQL for plans, acknowledgments, runs, items and Trash records.
//! Every per-item transition is written before and after the action, so an
//! interruption leaves an honest record and nothing is ever replayed.

use rusqlite::{params, Connection, OptionalExtension};

use super::trash::TrashRecord;
use crate::detectors::common::rfc3339;
use crate::model::*;

type R<T> = Result<T, rusqlite::Error>;

#[derive(Debug, Clone)]
pub struct PlanRow {
    pub id: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub generation: i64,
    pub snapshot: String,
    pub digest: String,
    pub state: String,
}

pub fn insert_plan(c: &Connection, p: &PlanRow) -> R<()> {
    c.execute(
        "INSERT INTO cleanup_plans(id, created_at, expires_at, scan_generation, snapshot, digest, state) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![p.id, p.created_at, p.expires_at, p.generation, p.snapshot, p.digest, p.state],
    )?;
    Ok(())
}

pub fn load_plan(c: &Connection, id: &str) -> R<Option<PlanRow>> {
    c.query_row(
        "SELECT id, created_at, expires_at, scan_generation, snapshot, digest, state FROM cleanup_plans WHERE id = ?1",
        [id],
        |r| {
            Ok(PlanRow {
                id: r.get(0)?,
                created_at: r.get(1)?,
                expires_at: r.get(2)?,
                generation: r.get(3)?,
                snapshot: r.get(4)?,
                digest: r.get(5)?,
                state: r.get(6)?,
            })
        },
    )
    .optional()
}

pub fn set_plan_state(c: &Connection, id: &str, state: &str) -> R<()> {
    c.execute("UPDATE cleanup_plans SET state = ?2 WHERE id = ?1", params![id, state])?;
    Ok(())
}

pub fn acks(c: &Connection, plan: &str) -> R<Vec<String>> {
    let mut st = c.prepare("SELECT item_id FROM plan_acks WHERE plan_id = ?1 ORDER BY at")?;
    let v = st.query_map([plan], |r| r.get(0))?.collect::<R<Vec<String>>>()?;
    Ok(v)
}

pub fn add_ack(c: &Connection, plan: &str, item: &str, at: i64) -> R<()> {
    c.execute("INSERT OR IGNORE INTO plan_acks(plan_id, item_id, at) VALUES (?1,?2,?3)", params![plan, item, at])?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn insert_run(
    c: &mut Connection,
    run_id: &str,
    plan_id: &str,
    plan_json: &str,
    started: i64,
    free_before: u64,
    expected: u64,
    docker: bool,
    items: &[(Suggestion, String)],
) -> R<()> {
    let tx = c.transaction()?;
    tx.execute(
        "INSERT INTO cleanup_runs(id, plan_id, plan_json, started_at, state, volume, free_before, expected_bytes, involves_docker) VALUES (?1,?2,?3,?4,'running','home',?5,?6,?7)",
        params![run_id, plan_id, plan_json, started, free_before as i64, expected as i64, docker as i64],
    )?;
    for (i, (s, action)) in items.iter().enumerate() {
        tx.execute(
            "INSERT INTO cleanup_items(run_id, item_id, ord, snapshot, action, state, reason, expected_bytes, updated_at) VALUES (?1,?2,?3,?4,?5,'waiting',NULL,?6,?7)",
            params![run_id, s.id, i as i64, serde_json::to_string(s).unwrap_or_default(), action, s.size_bytes as i64, started],
        )?;
    }
    tx.execute("UPDATE cleanup_plans SET state = 'running' WHERE id = ?1", [plan_id])?;
    tx.commit()
}

pub fn set_item(c: &Connection, run: &str, item: &str, state: ItemState, reason: Option<&str>, at: i64) -> R<()> {
    c.execute(
        "UPDATE cleanup_items SET state = ?3, reason = ?4, updated_at = ?5 WHERE run_id = ?1 AND item_id = ?2",
        params![run, item, state.as_str(), reason, at],
    )?;
    Ok(())
}

pub fn finish_run(c: &Connection, run: &str, plan: &str, state: &str, free_after: u64, at: i64) -> R<()> {
    let trash: i64 = c.query_row("SELECT COALESCE(SUM(bytes),0) FROM trash_items WHERE run_id = ?1", [run], |r| r.get(0))?;
    c.execute(
        "UPDATE cleanup_runs SET state = ?2, free_after = ?3, finished_at = ?4, trash_bytes = ?5 WHERE id = ?1",
        params![run, state, free_after as i64, at, trash],
    )?;
    c.execute("UPDATE cleanup_plans SET state = 'finished' WHERE id = ?1", [plan])?;
    Ok(())
}

pub fn running_runs(c: &Connection) -> R<Vec<(String, String)>> {
    let mut st = c.prepare("SELECT id, plan_id FROM cleanup_runs WHERE state = 'running'")?;
    let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<R<Vec<_>>>()?;
    Ok(v)
}

/// Mark an interrupted run. In-flight items become "interrupted" (their
/// outcome is unknown); items that never started become "canceled".
pub fn mark_interrupted(c: &Connection, run: &str, plan: &str, at: i64) -> R<()> {
    c.execute(
        "UPDATE cleanup_items SET state = 'interrupted', reason = 'Freeboard stopped while this item was being handled. Scan again to see its current state.', updated_at = ?2 WHERE run_id = ?1 AND state IN ('checking','running')",
        params![run, at],
    )?;
    c.execute(
        "UPDATE cleanup_items SET state = 'canceled', reason = 'Not started because Freeboard stopped.', updated_at = ?2 WHERE run_id = ?1 AND state = 'waiting'",
        params![run, at],
    )?;
    let trash: i64 = c.query_row("SELECT COALESCE(SUM(bytes),0) FROM trash_items WHERE run_id = ?1", [run], |r| r.get(0))?;
    c.execute(
        "UPDATE cleanup_runs SET state = 'interrupted', finished_at = ?2, trash_bytes = ?3 WHERE id = ?1",
        params![run, at, trash],
    )?;
    c.execute("UPDATE cleanup_plans SET state = 'finished' WHERE id = ?1", [plan])?;
    Ok(())
}

pub fn delete_suggestion(c: &Connection, id: &str) -> R<()> {
    c.execute("DELETE FROM suggestions WHERE id = ?1", [id])?;
    c.execute("DELETE FROM suggestion_parts WHERE suggestion_id = ?1", [id])?;
    Ok(())
}

/// Remove cleaned parts from a split item. The parts left stay listed with
/// their own sizes; the item's size and target follow them. Returns true when
/// parts remain, false when the item is gone (or was not found).
pub fn remove_parts(c: &Connection, item: &str, gone: &[&str]) -> R<bool> {
    let row: Option<(String, String)> =
        c.query_row("SELECT json, target FROM suggestions WHERE id = ?1", [item], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    let Some((json, target)) = row else { return Ok(false) };
    for p in gone {
        c.execute("DELETE FROM suggestion_parts WHERE suggestion_id = ?1 AND part_id = ?2", params![item, p])?;
    }
    let (Ok(mut s), Ok(mut t)) = (serde_json::from_str::<Suggestion>(&json), serde_json::from_str::<Target>(&target)) else {
        // Unreadable: drop it rather than leave a stale combined item.
        delete_suggestion(c, item)?;
        return Ok(false);
    };
    // The removed parts' paths and resources leave the combined target too.
    let mut part_paths: Vec<String> = vec![];
    let mut part_resources: Vec<String> = vec![];
    {
        let mut st = c.prepare("SELECT json FROM suggestion_parts WHERE suggestion_id = ?1")?;
        let left: Vec<String> = st.query_map([item], |r| r.get(0))?.collect::<R<Vec<_>>>()?;
        for j in left {
            if let Ok(f) = serde_json::from_str::<Finding>(&j) {
                match f.target {
                    Target::Paths { paths } => part_paths.extend(paths),
                    Target::Native { resources, .. } => part_resources.extend(resources),
                    Target::None => {}
                }
            }
        }
    }
    s.parts.retain(|p| !gone.contains(&p.id.as_str()));
    if s.parts.is_empty() {
        delete_suggestion(c, item)?;
        return Ok(false);
    }
    s.full_size_bytes = s.parts.iter().map(|p| p.size_bytes).sum();
    s.size_bytes = s.parts.iter().filter(|p| p.selected).map(|p| p.size_bytes).sum();
    match &mut t {
        Target::Paths { paths } => paths.retain(|p| part_paths.contains(p)),
        Target::Native { resources, .. } => resources.retain(|r| part_resources.contains(r)),
        Target::None => {}
    }
    c.execute(
        "UPDATE suggestions SET json = ?2, target = ?3 WHERE id = ?1",
        params![item, serde_json::to_string(&s).unwrap_or(json), serde_json::to_string(&t).unwrap_or(target)],
    )?;
    Ok(true)
}

pub fn trash_failed(c: &Connection, item: &str) -> R<bool> {
    let n: i64 = c.query_row(
        "SELECT COUNT(*) FROM cleanup_items WHERE item_id = ?1 AND state = 'failed' AND reason LIKE 'Trash unavailable%'",
        [item],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

pub fn insert_trash(c: &Connection, rec: &TrashRecord) -> R<()> {
    c.execute(
        "INSERT INTO trash_items(run_id, item_id, original_path, trashed_at, bytes, identity, state) VALUES (?1,?2,?3,?4,?5,?6,'held')",
        params![rec.run_id, rec.item_id, rec.original_path, rec.trashed_at, rec.bytes as i64, rec.identity],
    )?;
    Ok(())
}

pub fn held_trash(c: &Connection) -> R<Vec<TrashRecord>> {
    let mut st = c.prepare(
        "SELECT id, run_id, item_id, original_path, trashed_at, bytes, identity FROM trash_items WHERE state = 'held' ORDER BY id",
    )?;
    let v = st
        .query_map([], |r| {
            Ok(TrashRecord {
                id: r.get(0)?,
                run_id: r.get(1)?,
                item_id: r.get(2)?,
                original_path: r.get(3)?,
                trashed_at: r.get(4)?,
                bytes: r.get::<_, i64>(5)? as u64,
                identity: r.get(6)?,
            })
        })?
        .collect::<R<Vec<_>>>()?;
    Ok(v)
}

pub fn set_trash_state(c: &Connection, id: i64, state: &str) -> R<()> {
    c.execute("UPDATE trash_items SET state = ?2 WHERE id = ?1", params![id, state])?;
    Ok(())
}

fn ts(t: i64) -> String {
    rfc3339(t).unwrap_or_default()
}

pub fn load_run(c: &Connection, run: &str) -> R<Option<RunDetail>> {
    let row = c
        .query_row(
            "SELECT id, plan_id, started_at, finished_at, state, free_before, free_after, expected_bytes, trash_bytes, involves_docker FROM cleanup_runs WHERE id = ?1",
            [run],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, Option<i64>>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, i64>(8)?,
                    r.get::<_, i64>(9)?,
                ))
            },
        )
        .optional()?;
    let Some((id, plan_id, started, finished, state, before, after, expected, trash, docker)) = row else { return Ok(None) };
    // Which item each part belongs to, from the confirmed plan.
    let plan_json: String = c.query_row("SELECT plan_json FROM cleanup_runs WHERE id = ?1", [run], |r| r.get(0))?;
    let mut parent_of: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if let Ok(plan) = serde_json::from_str::<CleanupPlan>(&plan_json) {
        for pi in plan.plan_items {
            for part in pi.part_ids {
                parent_of.insert(part, pi.suggestion_id.clone());
            }
        }
    }
    let mut st = c.prepare("SELECT item_id, snapshot, state, reason, expected_bytes, action FROM cleanup_items WHERE run_id = ?1 ORDER BY ord")?;
    let items = st
        .query_map([run], |r| {
            let snap: String = r.get(1)?;
            let s: Option<Suggestion> = serde_json::from_str(&snap).ok();
            let item_id: String = r.get(0)?;
            Ok(RunItem {
                parent_id: parent_of.get(&item_id).cloned(),
                item_id,
                title: s.as_ref().map(|s| s.title.clone()).unwrap_or_default(),
                action: r.get(5)?,
                action_label: s.as_ref().map(|s| s.action_label.clone()).unwrap_or_default(),
                expected_bytes: r.get::<_, i64>(4)? as u64,
                state: ItemState::parse(&r.get::<_, String>(2)?),
                reason: r.get(3)?,
            })
        })?
        .collect::<R<Vec<_>>>()?;
    // A live run reports what is held so far.
    let trash = if state == "running" {
        c.query_row("SELECT COALESCE(SUM(bytes),0) FROM trash_items WHERE run_id = ?1", [run], |r| r.get::<_, i64>(0))?
    } else {
        trash
    };
    Ok(Some(RunDetail {
        id,
        plan_id,
        started_at: ts(started),
        finished_at: finished.map(ts),
        state,
        free_before: before as u64,
        free_after: after.map(|a| a as u64),
        expected_bytes: expected as u64,
        measured_bytes_freed: after.map(|a| a - before),
        trash_bytes: trash as u64,
        skipped: items.iter().filter(|i| i.state == ItemState::Skipped).count() as u32,
        failed: items.iter().filter(|i| i.state == ItemState::Failed).count() as u32,
        involves_docker: docker != 0,
        items,
        free_bytes_now: None,
    }))
}

pub fn list_runs(c: &Connection, page: u32, per_page: u32) -> R<HistoryPage> {
    let mut st = c.prepare(
        "SELECT r.id, r.started_at, r.free_before, r.free_after, r.trash_bytes, r.state,
                (SELECT COUNT(*) FROM cleanup_items i WHERE i.run_id = r.id),
                (SELECT COUNT(*) FROM cleanup_items i WHERE i.run_id = r.id AND i.state = 'skipped')
         FROM cleanup_runs r ORDER BY r.started_at DESC, r.rowid DESC LIMIT ?1 OFFSET ?2",
    )?;
    let mut runs = st
        .query_map(params![per_page as i64 + 1, (page as i64) * per_page as i64], |r| {
            let before: i64 = r.get(2)?;
            let after: Option<i64> = r.get(3)?;
            Ok(RunSummary {
                id: r.get(0)?,
                started_at: ts(r.get(1)?),
                items: r.get::<_, i64>(6)? as u32,
                measured_bytes_freed: after.map(|a| a - before),
                trash_bytes: r.get::<_, i64>(4)? as u64,
                skipped: r.get::<_, i64>(7)? as u32,
                removed: 0,
                state: r.get(5)?,
            })
        })?
        .collect::<R<Vec<_>>>()?;
    let has_more = runs.len() > per_page as usize;
    runs.truncate(per_page as usize);
    for r in &mut runs {
        if let Some(d) = load_run(c, &r.id)? {
            r.removed = removed_crates(&d);
        }
    }
    Ok(HistoryPage { runs, page, has_more })
}

/// Crates (plan items) of a run that were removed or moved to the Trash, at
/// least in part. A split item counts once, however many parts it had.
pub fn removed_crates(d: &RunDetail) -> u32 {
    let mut seen: Vec<&str> = vec![];
    for i in &d.items {
        if matches!(i.state, ItemState::Done | ItemState::MovedToTrash) {
            let key = i.parent_id.as_deref().unwrap_or(&i.item_id);
            if !seen.contains(&key) {
                seen.push(key);
            }
        }
    }
    seen.len() as u32
}
