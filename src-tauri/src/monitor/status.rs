//! Disk status rules with recovery hysteresis ("Disk status" table).

use crate::model::{DiskStatus, GB};

/// Status improves only after free space rises this far above its threshold.
pub const HYSTERESIS: u64 = 2 * GB;

/// Raw status with no hysteresis: Low below `low`, Critical below `critical`.
pub fn raw_status(free: u64, low: u64, critical: u64) -> DiskStatus {
    if free < critical {
        DiskStatus::Critical
    } else if free < low {
        DiskStatus::Low
    } else {
        DiskStatus::Healthy
    }
}

fn severity(s: DiskStatus) -> u8 {
    match s {
        DiskStatus::Healthy => 0,
        DiskStatus::Low => 1,
        DiskStatus::Critical => 2,
    }
}

/// Next status given the previous one. Worsening applies immediately;
/// improving requires free space at least `HYSTERESIS` above the threshold
/// being left.
pub fn next_status(prev: Option<DiskStatus>, free: u64, low: u64, critical: u64) -> DiskStatus {
    let raw = raw_status(free, low, critical);
    let Some(prev) = prev else { return raw };
    if severity(raw) >= severity(prev) {
        return raw;
    }
    // Improving: leave Critical only at critical + 2 GB, leave Low only at low + 2 GB.
    let mut s = prev;
    if s == DiskStatus::Critical && free >= critical + HYSTERESIS {
        s = DiskStatus::Low;
    }
    if s == DiskStatus::Low && free >= low + HYSTERESIS {
        s = DiskStatus::Healthy;
    }
    // Never report better than the raw status.
    if severity(s) < severity(raw) {
        raw
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const L: u64 = 20 * GB;
    const C: u64 = 8 * GB;

    fn walk(points: &[f64]) -> Vec<DiskStatus> {
        let mut prev = None;
        points
            .iter()
            .map(|gb| {
                let s = next_status(prev, (gb * GB as f64) as u64, L, C);
                prev = Some(s);
                s
            })
            .collect()
    }

    #[test]
    fn exact_boundaries() {
        assert_eq!(raw_status(20 * GB, L, C), DiskStatus::Healthy);
        assert_eq!(raw_status(20 * GB - 1, L, C), DiskStatus::Low);
        assert_eq!(raw_status(8 * GB, L, C), DiskStatus::Low);
        assert_eq!(raw_status(8 * GB - 1, L, C), DiskStatus::Critical);
    }

    #[test]
    fn low_oscillation_stays_low_until_22() {
        use DiskStatus::*;
        let s = walk(&[25.0, 19.0, 21.0, 19.5, 21.9, 20.5, 22.0]);
        assert_eq!(s, vec![Healthy, Low, Low, Low, Low, Low, Healthy]);
    }

    #[test]
    fn critical_recovery_spec_scenario() {
        use DiskStatus::*;
        let s = walk(&[7.0, 9.0, 10.0]);
        assert_eq!(s, vec![Critical, Critical, Low]);
    }

    #[test]
    fn jump_from_critical_to_healthy_needs_low_plus_two() {
        use DiskStatus::*;
        assert_eq!(walk(&[5.0, 21.0]), vec![Critical, Low]);
        assert_eq!(walk(&[5.0, 22.0]), vec![Critical, Healthy]);
    }

    #[test]
    fn worsening_is_immediate() {
        use DiskStatus::*;
        assert_eq!(walk(&[30.0, 7.9]), vec![Healthy, Critical]);
    }

    #[test]
    fn configurable_thresholds() {
        let s = next_status(Some(DiskStatus::Low), 41 * GB, 40 * GB, 10 * GB);
        assert_eq!(s, DiskStatus::Low);
        let s = next_status(Some(DiskStatus::Low), 42 * GB, 40 * GB, 10 * GB);
        assert_eq!(s, DiskStatus::Healthy);
    }
}
