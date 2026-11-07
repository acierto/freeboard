//! "What Risky means here": why an item is Review or Risky, and what the user
//! can do first to make removing it safer. Shown next to every Review and
//! Risky item (decided by the user 2026-11-07).
//!
//! The commands are only shown, for the user to run in their own terminal.
//! Freeboard never runs them; they are read-only or make a copy.

use crate::detectors::{caches, docker, files, git, projects};
use crate::model::{Risk, SaferStep, Suggestion};

fn step(label: &str, command: impl Into<String>) -> SaferStep {
    SaferStep { label: label.into(), command: command.into() }
}

/// A location as a shell argument: `~/x y` becomes `"$HOME/x y"`.
fn sh(loc: &str) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"").replace('$', "\\$").replace('`', "\\`");
    match loc.strip_prefix("~/") {
        Some(rest) => format!("\"$HOME/{}\"", esc(rest)),
        None => format!("\"{}\"", esc(loc)),
    }
}

/// Docker volume names are restricted to `[A-Za-z0-9_.-]` before they are
/// listed (docker::safe_arg), so they need no quoting.
fn volume_steps(name: &str) -> Vec<SaferStep> {
    vec![
        step("Look inside it", format!("docker run --rm -v {name}:/data alpine ls -la /data")),
        step(
            "Save a copy to ~/Backups first",
            format!("mkdir -p ~/Backups && docker run --rm -v {name}:/data -v ~/Backups:/backup alpine tar czf /backup/{name}.tgz -C /data ."),
        ),
    ]
}

fn database_hint(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["postgres", "pgdata", "pg_", "_pg", "mysql", "maria", "mongo", "redis", "elastic", "db", "sql"].iter().any(|k| n.contains(k))
}

/// Fill in `risk_why` and `safer` for a Review or Risky item.
pub fn annotate(s: &mut Suggestion) {
    if s.risk == Risk::Safe {
        s.risk_why.clear();
        s.safer.clear();
        return;
    }
    let (why, safer) = notes(s);
    s.risk_why = why;
    s.safer = safer;
}

fn notes(s: &Suggestion) -> (Vec<String>, Vec<SaferStep>) {
    let no_undo = "Docker deletes it outright. It does not go to the Trash and cannot be undone.".to_string();
    match s.detector.as_str() {
        docker::VOLUMES if s.title.starts_with("Volume ") => {
            let name = s.title.trim_start_matches("Volume ");
            let mut why = vec![no_undo, "Freeboard can't see inside a volume, so it can't tell whether the data matters.".into()];
            if let Some(p) = s.location.strip_prefix("Compose project ").and_then(|r| r.split(',').next()) {
                why.push(format!("It belongs to the Compose project {p}, which may expect to find it again."));
            }
            if database_hint(name) {
                why.push("The name suggests it holds a database.".into());
            }
            (why, if s.blocked_reason.is_some() { vec![] } else { volume_steps(name) })
        }
        docker::VOLUMES => (
            vec![
                "They have no names, so there is no clue what is inside.".into(),
                "No container uses them, but one may still hold the only copy of some data.".into(),
                "Docker deletes volumes outright. There is no undo.".into(),
            ],
            vec![
                step("See each volume and its size", "docker system df -v"),
                step("Look inside one (use a name from that list)", "docker run --rm -v VOLUME_NAME:/data alpine ls -la /data"),
            ],
        ),
        docker::UNUSED => (
            vec![
                "Docker can pull or build them again, but that takes time and network.".into(),
                "An image built only on this computer and never pushed has to be built again from its source.".into(),
            ],
            vec![step("See them first", "docker image ls")],
        ),
        docker::STOPPED => (
            vec!["Files a container wrote outside its volumes are removed with it.".into()],
            vec![
                step("List them", "docker ps -a --filter status=exited"),
                step("See what changed inside one (use a name from that list)", "docker container diff CONTAINER_NAME"),
            ],
        ),
        git::ID if s.risk == Risk::Risky => {
            let loc = sh(&s.location);
            let mut why = vec![format!("{} Your work may exist only here.", s.reason)];
            why.push("Freeboard will not remove it until that is no longer true.".into());
            let mut safer = vec![step("See what is only here", format!("git -C {loc} status"))];
            let r = s.reason.to_ascii_lowercase();
            if r.contains("unpushed") || r.contains("no upstream") {
                safer.push(step("Push the branch", format!("git -C {loc} push -u origin HEAD")));
            }
            if r.contains("uncommitted") || r.contains("untracked") {
                safer.push(step("Commit what you want to keep, then push", format!("git -C {loc} add -p && git -C {loc} commit && git -C {loc} push")));
            }
            (why, safer)
        }
        git::ID => {
            let mut why = vec![if s.reason.contains("merged") && !s.reason.contains("not merged") {
                "The branch is merged, so its commits are safe in the main branch.".to_string()
            } else {
                "Each branch is merged or pushed, so its commits are safe elsewhere.".to_string()
            }];
            why.push("Files Git ignores, such as .env files and local settings, are removed with the worktree.".into());
            let safer = if s.parts.is_empty() {
                vec![step("Check for ignored files you want to keep", format!("git -C {} status --ignored", sh(&s.location)))]
            } else {
                vec![step("Check one worktree for ignored files (use a path from Choose which)", "git -C WORKTREE_PATH status --ignored")]
            };
            (why, safer)
        }
        projects::DEPS => (
            vec!["A virtual environment is usually rebuilt from requirements, but packages installed by hand with pip install are not listed there.".into()],
            vec![step("Save the exact package list first (in each project)", ".venv/bin/pip freeze > requirements-lock.txt")],
        ),
        caches::SIMULATORS => (vec!["Data you saved inside these simulators, such as test accounts and photos, is removed with them.".into()], vec![step("List them", "xcrun simctl list devices unavailable")]),
        caches::CARGO | caches::GRADLE | caches::XCODE => (
            vec![
                "The tool downloads or rebuilds it when needed, which takes time and network.".into(),
                "Builds that must work offline need it until then.".into(),
            ],
            vec![],
        ),
        files::INSTALLERS | files::WINDOWS_TEMP => (
            vec!["Freeboard can't tell whether you still need these files.".into(), "They go to the Trash first, so you can put them back.".into()],
            vec![],
        ),
        _ if s.risk == Risk::Risky => {
            let mut why = vec!["Freeboard can't prove it is safe to remove.".to_string()];
            if !s.recoverable {
                why.push("It can't be undone.".into());
            }
            (why, vec![])
        }
        _ => {
            let mut c = s.recovery.chars();
            let rec = c.next().map(|f| f.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default();
            (vec![format!("It comes back, but with some effort: {rec}")], vec![])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detectors::common::base;
    use crate::model::Group;

    #[test]
    fn a_named_volume_says_why_and_how_to_keep_a_copy() {
        let mut s = base(docker::VOLUMES, Group::Docker, "billing_pgdata");
        s.title = "Volume billing_pgdata".into();
        s.location = "Compose project billing, not attached to a container".into();
        s.risk = Risk::Risky;
        annotate(&mut s);
        assert!(s.risk_why[0].contains("cannot be undone"));
        assert!(s.risk_why.iter().any(|w| w.contains("Compose project billing")));
        assert!(s.risk_why.iter().any(|w| w.contains("database")));
        assert_eq!(s.safer.len(), 2);
        assert!(s.safer[1].command.contains("tar czf /backup/billing_pgdata.tgz"));
    }

    #[test]
    fn a_blocked_worktree_explains_and_offers_status_and_push() {
        let mut s = base(git::ID, Group::Projects, "wt");
        s.risk = Risk::Risky;
        s.reason = "3 uncommitted files.".into();
        s.location = "~/work/my app".into();
        annotate(&mut s);
        assert!(s.risk_why[0].starts_with("3 uncommitted files."));
        assert_eq!(s.safer[0].command, "git -C \"$HOME/work/my app\" status");
        assert!(s.safer.iter().any(|x| x.label.starts_with("Commit")));
        let mut p = base(git::ID, Group::Projects, "wt2");
        p.risk = Risk::Risky;
        p.reason = "2 unpushed commits.".into();
        p.location = "/tmp/$x".into();
        annotate(&mut p);
        assert!(p.safer.iter().any(|x| x.command == "git -C \"/tmp/\\$x\" push -u origin HEAD"), "{:?}", p.safer);
    }

    #[test]
    fn safe_items_have_no_notes_and_every_other_item_has_a_reason() {
        let mut s = base(caches::PIP, Group::Caches, "pip");
        s.risk = Risk::Safe;
        annotate(&mut s);
        assert!(s.risk_why.is_empty() && s.safer.is_empty());
        let mut r = base("some.detector", Group::YourFiles, "x");
        r.risk = Risk::Review;
        r.recovery = "Restore it from the Trash.".into();
        annotate(&mut r);
        assert_eq!(r.risk_why, vec!["It comes back, but with some effort: restore it from the Trash.".to_string()]);
    }
}
