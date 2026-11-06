//! The pile: what the main Toss button removes (spec "Building the pile"),
//! and the user's decisions about items (Keep, Ask me later, part choices).
//!
//! Everything here is a pure function over plain data (design D3). The core
//! stores `PileStore` and `Decisions`, applies the decisions to the latest
//! scan's suggestions, and calls `fill` to compute the pile. The interface
//! never computes the pile; it shows what `get_pile` returns.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::model::{Group, ItemDecision, Risk, Suggestion};
use crate::settings::Goal;

/// Automatic filling, or the crates the user chose by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Auto,
    Manual,
}

/// What the core stores between commands (`pile_state` table).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PileStore {
    pub goal: Goal,
    /// Empty means everywhere.
    pub areas: Vec<Group>,
    /// The frozen set of crates in manual mode (prototype `S.manual`).
    pub manual: Option<Vec<String>>,
    /// Items the user added; kept in the pile even if they are not Safe.
    pub added: Vec<String>,
    /// Crates the user took out; never refilled until a size is chosen again.
    pub taken_out: Vec<String>,
}

impl PileStore {
    pub fn new(goal: Goal, areas: Vec<Group>) -> PileStore {
        PileStore { goal, areas, manual: None, added: vec![], taken_out: vec![] }
    }
}

/// What `get_pile` returns (spec `PileState`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PileState {
    pub goal_bytes: Goal,
    pub areas: Vec<Group>,
    pub mode: Mode,
    pub added_ids: Vec<String>,
    pub taken_out_ids: Vec<String>,
    /// The current result, largest first.
    pub item_ids: Vec<String>,
    pub total_bytes: u64,
}

/// The user's standing choices, by stable resource ID (`item_decisions`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Decisions {
    pub decision: BTreeMap<String, ItemDecision>,
    /// Part IDs the user deselected, per item.
    pub deselected: BTreeMap<String, BTreeSet<String>>,
}

/// Apply Keep / Ask me later and part choices to a scan's suggestions. An
/// item's size becomes the sum of its selected parts (spec "Item-level
/// controls").
pub fn apply_decisions(items: &mut [Suggestion], d: &Decisions) {
    for s in items {
        s.decision = d.decision.get(&s.id).copied();
        if s.parts.is_empty() {
            continue;
        }
        let off = d.deselected.get(&s.id);
        for p in &mut s.parts {
            p.selected = !off.is_some_and(|o| o.contains(&p.id));
        }
        s.size_bytes = s.parts.iter().filter(|p| p.selected).map(|p| p.size_bytes).sum();
    }
}

/// An item the pile may hold at all: not blocked, has an action, has
/// something selected, not kept, and not changed during a cleanup.
fn can_hold(s: &Suggestion) -> bool {
    s.blocked_reason.is_none()
        && s.action != crate::model::Action::None
        && s.size_bytes > 0
        && s.decision != Some(ItemDecision::Keep)
        && s.skip_reason.is_none()
}

/// An item automatic filling may choose: Safe only, never one that needs
/// its own confirmation (invariants 9 and 10).
fn auto_eligible(s: &Suggestion, p: &PileStore) -> bool {
    can_hold(s)
        && s.risk == Risk::Safe
        && !s.needs_confirmation
        && !p.taken_out.contains(&s.id)
        && (p.areas.is_empty() || p.areas.contains(&s.group))
}

fn largest_first(a: &&Suggestion, b: &&Suggestion) -> std::cmp::Ordering {
    b.size_bytes.cmp(&a.size_bytes).then(a.id.cmp(&b.id))
}

/// Compute the pile (prototype `picks()`). In manual mode the pile is
/// exactly the crates the user kept. Otherwise it starts with items the
/// user added, then adds Safe items, largest first, until the goal is met;
/// "All I can" adds every Safe item in the chosen areas.
pub fn fill(p: &PileStore, items: &[Suggestion]) -> PileState {
    let chosen: Vec<&Suggestion> = if let Some(manual) = &p.manual {
        let mut v: Vec<&Suggestion> = items.iter().filter(|s| manual.contains(&s.id) && can_hold(s)).collect();
        v.sort_by(largest_first);
        v
    } else {
        let mut v: Vec<&Suggestion> = items.iter().filter(|s| p.added.contains(&s.id) && can_hold(s)).collect();
        v.sort_by(largest_first);
        let mut have: u64 = v.iter().map(|s| s.size_bytes).sum();
        let mut safe: Vec<&Suggestion> = items.iter().filter(|s| auto_eligible(s, p) && !p.added.contains(&s.id)).collect();
        safe.sort_by(largest_first);
        for s in safe {
            let more = match p.goal {
                Goal::All => true,
                Goal::Bytes(g) => have < g,
            };
            if !more {
                break;
            }
            have += s.size_bytes;
            v.push(s);
        }
        v
    };
    PileState {
        goal_bytes: p.goal,
        areas: p.areas.clone(),
        mode: if p.manual.is_some() { Mode::Manual } else { Mode::Auto },
        added_ids: p.added.clone(),
        taken_out_ids: p.taken_out.clone(),
        item_ids: chosen.iter().map(|s| s.id.clone()).collect(),
        total_bytes: chosen.iter().map(|s| s.size_bytes).sum(),
    }
}

/// Choose 20, 50 or 100 GB, or All I can: leave manual mode and clear
/// taken-out items.
pub fn set_goal(p: &mut PileStore, goal: Goal) {
    p.goal = goal;
    p.manual = None;
    p.taken_out.clear();
}

/// Choose Where from: leave manual mode, but keep taken-out items out.
pub fn set_areas(p: &mut PileStore, areas: Vec<Group>) {
    let mut a: Vec<Group> = Group::ALL.into_iter().filter(|g| areas.contains(g)).collect();
    a.dedup();
    p.areas = a;
    p.manual = None;
}

/// Add an item or part: it stays in the pile even if it isn't Safe.
pub fn add(p: &mut PileStore, ids: &[String]) {
    for id in ids {
        if !p.added.contains(id) {
            p.added.push(id.clone());
        }
        p.taken_out.retain(|x| x != id);
        if let Some(m) = &mut p.manual {
            if !m.contains(id) {
                m.push(id.clone());
            }
        }
    }
}

/// Take a crate out: freeze the pile as it is (manual mode), remove the
/// crate, and do not replace it.
pub fn take_out(p: &mut PileStore, ids: &[String], items: &[Suggestion]) {
    if p.manual.is_none() {
        p.manual = Some(fill(p, items).item_ids);
    }
    for id in ids {
        if let Some(m) = &mut p.manual {
            m.retain(|x| x != id);
        }
        p.added.retain(|x| x != id);
        if !p.taken_out.contains(id) {
            p.taken_out.push(id.clone());
        }
    }
}

/// Keep an item: out of the pile and all automatic choices. (`fill` already
/// skips kept items; this also forgets that the user added it.)
pub fn on_keep(p: &mut PileStore, id: &str) {
    p.added.retain(|x| x != id);
    if let Some(m) = &mut p.manual {
        m.retain(|x| x != id);
    }
}

#[cfg(test)]
mod tests;
