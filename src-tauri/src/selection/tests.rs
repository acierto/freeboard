//! One test per row of the spec's "Building the pile" table, plus the
//! filling rules. Sizes are the prototype's example data (GB).
use super::*;
use crate::detectors::common::base;
use crate::model::{Action, SuggestionPart, GB};

fn item(id: &str, group: Group, gb: f64, risk: Risk) -> Suggestion {
    let mut s = base("test.x", group, id);
    s.id = id.into();
    s.size_bytes = (gb * GB as f64) as u64;
    s.full_size_bytes = s.size_bytes;
    s.risk = risk;
    s.action = Action::Delete;
    s
}

/// The prototype's Safe items, a few Review and Risky ones, and a blocked worktree.
fn harbor() -> Vec<Suggestion> {
    use Group::*;
    let mut v = vec![
        item("b-node", Projects, 18.7, Risk::Safe),
        item("d-cache", Docker, 14.8, Risk::Safe),
        item("c-derived", Caches, 11.3, Risk::Safe),
        item("b-target", Projects, 9.2, Risk::Safe),
        item("d-dangling", Docker, 6.2, Risk::Safe),
        item("c-pnpm", Caches, 5.6, Risk::Safe),
        item("c-jb", Caches, 5.1, Risk::Safe),
        item("c-apps", Caches, 3.9, Risk::Safe),
        item("c-pw", Caches, 3.6, Risk::Safe),
        item("c-pip", Caches, 2.6, Risk::Safe),
        item("c-brew", Caches, 2.1, Risk::Safe),
        item("ai-ollama", AiModels, 41.2, Risk::Review),
        item("d-pg", Docker, 3.4, Risk::Review),
        item("d-vol", Docker, 7.6, Risk::Risky),
        item("wt-pay", Projects, 6.9, Risk::Risky),
    ];
    let vol = v.iter_mut().find(|s| s.id == "d-vol").unwrap();
    vol.needs_confirmation = true;
    let wt = v.iter_mut().find(|s| s.id == "wt-pay").unwrap();
    wt.blocked_reason = Some("Commit and push it first.".into());
    wt.action = Action::None;
    v
}

fn ids(p: &PileState) -> Vec<&str> {
    p.item_ids.iter().map(String::as_str).collect()
}
fn gb(p: &PileState) -> f64 {
    (p.total_bytes as f64 / GB as f64 * 10.0).round() / 10.0
}
fn store(goal_gb: u64) -> PileStore {
    PileStore::new(Goal::Bytes(goal_gb * GB), vec![])
}
fn s(id: &str) -> Vec<String> {
    vec![id.to_string()]
}

#[test]
fn choose_a_size_fills_to_at_least_that_amount() {
    // Row 1: Choose 20 GB, 50 GB or 100 GB → fill automatically to at least that amount.
    let items = harbor();
    let p = fill(&store(50), &items);
    assert_eq!(ids(&p), ["b-node", "d-cache", "c-derived", "b-target"]);
    assert_eq!(gb(&p), 54.0, "the prototype's 'Toss 54.0 GB overboard'");
    assert_eq!(p.mode, Mode::Auto);
    assert_eq!(ids(&fill(&store(20), &items)), ["b-node", "d-cache"]);
    let hundred = fill(&store(100), &items);
    assert_eq!(gb(&hundred), 83.1, "Safe items stop short of 100 GB: every Safe item, nothing else");
    assert_eq!(hundred.item_ids.len(), 11);
}

#[test]
fn all_i_can_adds_every_safe_item_in_the_chosen_areas() {
    // Row 2.
    let items = harbor();
    let mut p = PileStore::new(Goal::All, vec![]);
    assert_eq!(fill(&p, &items).item_ids.len(), 11);
    set_areas(&mut p, vec![Group::Caches]);
    let f = fill(&p, &items);
    assert_eq!(gb(&f), 34.2, "the prototype's Caches total");
    assert!(f.item_ids.iter().all(|i| i.starts_with("c-")));
}

#[test]
fn where_from_limits_automatic_filling_and_everywhere_clears_it() {
    // Row 3.
    let items = harbor();
    let mut p = store(50);
    set_areas(&mut p, vec![Group::Docker]);
    assert_eq!(ids(&fill(&p, &items)), ["d-cache", "d-dangling"]);
    set_areas(&mut p, vec![]);
    assert_eq!(ids(&fill(&p, &items)), ["b-node", "d-cache", "c-derived", "b-target"]);
}

#[test]
fn an_added_item_stays_in_the_pile_even_if_it_is_not_safe() {
    // Row 4. Automatic filling starts with items the user added.
    let items = harbor();
    let mut p = store(50);
    add(&mut p, &s("ai-ollama"));
    let f = fill(&p, &items);
    assert_eq!(ids(&f), ["ai-ollama", "b-node"], "41.2 + 18.7 reaches 50 GB");
    // A Risky item that needs its own confirmation can be added by hand.
    add(&mut p, &s("d-vol"));
    assert!(fill(&p, &items).item_ids.contains(&"d-vol".to_string()));
}

#[test]
fn taking_a_crate_out_removes_it_switches_to_manual_and_does_not_replace_it() {
    // Row 5.
    let items = harbor();
    let mut p = store(50);
    take_out(&mut p, &s("d-cache"), &items);
    let f = fill(&p, &items);
    assert_eq!(f.mode, Mode::Manual);
    assert_eq!(ids(&f), ["b-node", "c-derived", "b-target"]);
    assert_eq!(gb(&f), 39.2, "54.0 − 14.8, not refilled");
    assert_eq!(f.taken_out_ids, s("d-cache"));
}

#[test]
fn keeping_an_item_removes_it_from_the_pile_and_automatic_choices() {
    // Row 6.
    let mut items = harbor();
    let mut p = store(50);
    add(&mut p, &s("ai-ollama"));
    let mut d = Decisions::default();
    d.decision.insert("ai-ollama".into(), ItemDecision::Keep);
    d.decision.insert("b-node".into(), ItemDecision::Keep);
    on_keep(&mut p, "ai-ollama");
    apply_decisions(&mut items, &d);
    let f = fill(&p, &items);
    assert!(!f.item_ids.contains(&"ai-ollama".to_string()));
    assert!(!f.item_ids.contains(&"b-node".to_string()), "a kept Safe item is never chosen automatically");
    assert!(p.added.is_empty());
}

#[test]
fn stop_keeping_makes_the_item_available_again() {
    // Row 7.
    let mut items = harbor();
    let p = store(50);
    let mut d = Decisions::default();
    d.decision.insert("b-node".into(), ItemDecision::Keep);
    apply_decisions(&mut items, &d);
    assert!(!fill(&p, &items).item_ids.contains(&"b-node".to_string()));
    d.decision.remove("b-node");
    apply_decisions(&mut items, &d);
    assert_eq!(fill(&p, &items).item_ids[0], "b-node");
}

#[test]
fn choosing_a_size_again_leaves_manual_mode_and_clears_taken_out_items() {
    // Row 8.
    let items = harbor();
    let mut p = store(50);
    take_out(&mut p, &s("d-cache"), &items);
    set_goal(&mut p, Goal::Bytes(50 * GB));
    let f = fill(&p, &items);
    assert_eq!(f.mode, Mode::Auto);
    assert!(f.taken_out_ids.is_empty());
    assert_eq!(gb(&f), 54.0, "the crate can be chosen again");
}

#[test]
fn changing_where_from_leaves_manual_mode_but_keeps_taken_out_items_out() {
    // Row 9.
    let items = harbor();
    let mut p = store(50);
    take_out(&mut p, &s("d-cache"), &items);
    set_areas(&mut p, vec![Group::Docker]);
    let f = fill(&p, &items);
    assert_eq!(f.mode, Mode::Auto);
    assert_eq!(ids(&f), ["d-dangling"], "the taken-out build cache stays out");
}

#[test]
fn review_and_risky_items_are_never_added_automatically() {
    let items = harbor();
    let f = fill(&PileStore::new(Goal::All, vec![]), &items);
    for id in ["ai-ollama", "d-pg", "d-vol", "wt-pay"] {
        assert!(!f.item_ids.contains(&id.to_string()), "{id}");
    }
}

#[test]
fn blocked_changed_and_empty_items_are_never_in_the_pile() {
    let mut items = harbor();
    items.iter_mut().find(|s| s.id == "b-node").unwrap().skip_reason = Some("Changed after the scan.".into());
    let mut p = store(50);
    add(&mut p, &s("wt-pay"));
    let f = fill(&p, &items);
    assert!(!f.item_ids.contains(&"wt-pay".to_string()), "blocked");
    assert!(!f.item_ids.contains(&"b-node".to_string()), "changed during a cleanup");
}

#[test]
fn an_empty_pile_in_manual_mode_stays_empty() {
    // "The pile is empty. Pick a size to fill it again."
    let items = harbor();
    let mut p = store(20);
    take_out(&mut p, &["b-node".into(), "d-cache".into()], &items);
    let f = fill(&p, &items);
    assert_eq!(f.mode, Mode::Manual);
    assert!(f.item_ids.is_empty());
    assert_eq!(f.total_bytes, 0);
}

#[test]
fn adding_in_manual_mode_keeps_manual_mode() {
    let items = harbor();
    let mut p = store(50);
    take_out(&mut p, &s("d-cache"), &items);
    add(&mut p, &s("d-pg"));
    let f = fill(&p, &items);
    assert_eq!(f.mode, Mode::Manual);
    assert!(f.item_ids.contains(&"d-pg".to_string()));
}

#[test]
fn item_size_follows_its_selected_parts() {
    let mut items = vec![item("ai-ollama", Group::AiModels, 41.2, Risk::Review)];
    items[0].parts = [("mixtral", 26.0), ("codellama", 8.2), ("phi3", 7.0)]
        .iter()
        .map(|(l, g)| SuggestionPart { id: format!("p-{l}"), label: (*l).into(), location: String::new(), size_bytes: (*g * GB as f64) as u64, selected: true })
        .collect();
    let mut d = Decisions::default();
    d.deselected.insert("ai-ollama".into(), ["p-mixtral".to_string()].into());
    apply_decisions(&mut items, &d);
    assert!(!items[0].parts[0].selected && items[0].parts[1].selected);
    assert_eq!(items[0].size_bytes, (8.2 * GB as f64) as u64 + (7.0 * GB as f64) as u64, "15.2 GB");
    // No parts selected: nothing to hold.
    d.deselected.insert("ai-ollama".into(), ["p-mixtral", "p-codellama", "p-phi3"].map(String::from).into());
    apply_decisions(&mut items, &d);
    assert_eq!(items[0].size_bytes, 0);
    let mut p = store(50);
    add(&mut p, &s("ai-ollama"));
    assert!(fill(&p, &items).item_ids.is_empty());
}
