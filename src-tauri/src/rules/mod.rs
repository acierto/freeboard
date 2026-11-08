//! Keep me tidy rules (spec "Keep me tidy rules", design D7).
//!
//! Two rules, both off until the user turns them on. A rule only ever builds a
//! rule-sourced plan from the Safe, unkept items of the detectors it names;
//! the plan runs through the same cleanup engine, with the same pre-cleanup
//! checks and the same Logbook record. The cleanup engine refuses a
//! rule-sourced plan holding anything else (`allows`), so invariant 2's
//! exception is enforced in one place.

use serde::{Deserialize, Serialize};

use crate::model::*;

pub const PACKAGE_CACHES: &str = "package_caches";
pub const DOCKER_BUILD_CACHE: &str = "docker_build_cache";

/// Package caches a rule may clear: the Safe package-manager caches.
pub const PACKAGE_DETECTORS: [&str; 4] = ["cache.pnpm", "cache.pip", "cache.npm", "cache.homebrew"];
pub const DOCKER_DETECTORS: [&str; 1] = ["docker.build_cache"];
/// "over 30 days old".
pub const CACHE_AGE_DAYS: i64 = 30;
pub const WEEK: i64 = 7 * 86_400;
/// The package rule runs at most once a day while space stays Low.
pub const DAY: i64 = 86_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    pub id: &'static str,
    pub detectors: &'static [&'static str],
}

pub const RULES: [Rule; 2] = [
    Rule { id: PACKAGE_CACHES, detectors: &PACKAGE_DETECTORS },
    Rule { id: DOCKER_BUILD_CACHE, detectors: &DOCKER_DETECTORS },
];

/// The rule a command names: a closed set, never free text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TidyRuleId {
    PackageCaches,
    DockerBuildCache,
}

impl TidyRuleId {
    pub fn as_str(self) -> &'static str {
        match self {
            TidyRuleId::PackageCaches => PACKAGE_CACHES,
            TidyRuleId::DockerBuildCache => DOCKER_BUILD_CACHE,
        }
    }
}

pub fn rule(id: &str) -> Option<Rule> {
    RULES.iter().copied().find(|r| r.id == id)
}

/// A rule as the interface shows it: whether it is on, what exactly it
/// removes, and its last run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TidyRule {
    pub id: String,
    pub enabled: bool,
    /// The detectors it covers, as places-checked names ("pnpm store").
    pub removes: Vec<String>,
    pub last_run_at: Option<String>,
    pub last_result: Option<String>,
    pub last_bytes_freed: Option<i64>,
}

/// `tidy_rule_ran` payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TidyRuleRan {
    pub rule_id: String,
    /// "done", "nothing" (no eligible item), or "failed: <reason>".
    pub result: String,
    pub bytes_freed: i64,
    pub run_id: Option<String>,
}

/// Whether a rule may act on this item: a Safe item of one of its detectors,
/// not kept, not blocked, needing no acknowledgment, with a cleanup action.
pub fn allows(rule: &Rule, s: &Suggestion) -> bool {
    rule.detectors.contains(&s.detector.as_str())
        && s.risk == Risk::Safe
        && s.decision != Some(ItemDecision::Keep)
        && s.blocked_reason.is_none()
        && !s.needs_confirmation
        && s.action != Action::None
        && s.skip_reason.is_none()
        && s.size_bytes > 0
}

/// The items a rule would remove now. The package rule takes caches whose
/// newest change is at least 30 days old; tool-reported items without a date
/// (`pnpm store prune`, `brew cleanup`) are already only what no project uses.
pub fn eligible(rule: &Rule, findings: &[Finding], now: i64) -> Vec<String> {
    findings
        .iter()
        .filter(|f| allows(rule, &f.suggestion))
        // Every part must qualify too: a rule never takes a part the item
        // view did not show as Safe.
        .filter(|f| f.parts.iter().all(|p| allows(rule, &p.suggestion) || p.suggestion.size_bytes == 0))
        .filter(|f| {
            if rule.id != PACKAGE_CACHES {
                return true;
            }
            match f.suggestion.last_used_at.as_deref().and_then(crate::detectors::common::parse_time) {
                Some(t) => now - t >= CACHE_AGE_DAYS * DAY,
                None => true,
            }
        })
        .map(|f| f.suggestion.id.clone())
        .collect()
}

/// Whether a rule is due: the package rule while space is Low or Critical,
/// at most once a day; the Docker rule once a week.
pub fn due(rule: &Rule, status: DiskStatus, last_run: Option<i64>, now: i64) -> bool {
    match rule.id {
        PACKAGE_CACHES => status != DiskStatus::Healthy && last_run.is_none_or_older(now, DAY),
        DOCKER_BUILD_CACHE => last_run.is_none_or_older(now, WEEK),
        _ => false,
    }
}

trait Since {
    fn is_none_or_older(&self, now: i64, gap: i64) -> bool;
}
impl Since for Option<i64> {
    fn is_none_or_older(&self, now: i64, gap: i64) -> bool {
        match self {
            None => true,
            Some(t) => now - t >= gap,
        }
    }
}

#[cfg(test)]
mod tests;
