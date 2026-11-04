//! Detectors: docker, worktrees, projects, caches, files.
//!
//! One `Detector` per source detector ID (`Detector::id()` equals each
//! finding's `suggestion.detector`). Detectors only read (invariant 1).

pub mod caches;
pub mod common;
pub mod docker;
pub mod files;
pub mod git;
pub mod projects;

#[cfg(test)]
pub(crate) mod tests;

use crate::guard::Guard;
use crate::model::Finding;
use crate::scan::{Detector, ScanContext};

/// All detectors in source order (detector summary table).
pub fn all() -> Vec<Box<dyn Detector>> {
    vec![
        Box::new(docker::BuildCache),
        Box::new(docker::DanglingImages),
        Box::new(docker::UnusedImages),
        Box::new(docker::StoppedContainers),
        Box::new(docker::Volumes),
        Box::new(git::Worktrees),
        Box::new(projects::Dependencies),
        Box::new(projects::BuildOutput),
        Box::new(caches::Pnpm),
        Box::new(caches::Pip),
        Box::new(caches::Npm),
        Box::new(caches::Homebrew),
        Box::new(caches::Cargo),
        Box::new(caches::Gradle),
        Box::new(caches::Xcode),
        Box::new(caches::Simulators),
        Box::new(files::Installers),
        Box::new(files::WindowsTemp),
    ]
}

/// Result of re-checking one finding immediately before cleanup.
#[derive(Debug, Clone, PartialEq)]
pub enum Probe {
    /// Identity, fingerprint and tool status unchanged; safe to proceed.
    Unchanged,
    /// Something changed since the scan; skip with this reason.
    Changed(String),
    /// The check failed or timed out; skip (unknown means less safe).
    Unknown(String),
}

/// Re-run the detector-specific checks for a single finding (pre-cleanup
/// checks table). Never mutates anything. Uses the system guard for `ctx.home`.
pub fn probe(finding: &Finding, ctx: &ScanContext) -> Probe {
    let guard = Guard::for_system(&ctx.home, &[]);
    probe_with(finding, ctx, &guard)
}

/// Same as `probe`, with the cleanup engine's guard.
pub fn probe_with(finding: &Finding, ctx: &ScanContext, guard: &Guard) -> Probe {
    let det = finding.suggestion.detector.as_str();
    if det.starts_with("docker.") {
        docker::probe(finding, ctx)
    } else if det == git::ID {
        git::probe(finding, ctx)
    } else if det.starts_with("project.") {
        projects::probe(finding, ctx, guard)
    } else if det.starts_with("cache.") {
        caches::probe(finding, ctx, guard)
    } else if det.starts_with("files.") {
        files::probe_paths(finding, ctx, guard)
    } else {
        Probe::Unknown("Unknown detector.".into())
    }
}
