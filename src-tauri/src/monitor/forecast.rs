//! Free-space forecast: linear fit over the last 14 days, excluding cleanup
//! jumps. Shown only with at least 3 days of samples and falling free space.

use crate::model::{Forecast, GB};

pub const WINDOW_SECS: i64 = 14 * 86_400;
pub const MIN_SPAN_SECS: i64 = 3 * 86_400;
/// A rise of this much between consecutive samples counts as a cleanup jump
/// even without a recorded Freeboard cleanup (manual deletion, other tools).
pub const JUMP_BYTES: i64 = GB as i64;
/// Trends shallower than this are treated as flat (no forecast).
pub const FLAT_BYTES_PER_DAY: f64 = 0.1 * GB as f64;

/// `samples`: (unix secs, free bytes), any order. `cleanup_times`: finish
/// times of Freeboard cleanups and Trash emptying.
pub fn forecast(samples: &[(i64, u64)], now: i64, cleanup_times: &[i64]) -> Option<Forecast> {
    let mut s: Vec<(i64, i64)> = samples
        .iter()
        .filter(|(t, _)| *t >= now - WINDOW_SECS && *t <= now)
        .map(|(t, f)| (*t, *f as i64))
        .collect();
    s.sort_by_key(|p| p.0);
    if s.len() < 2 || s.last()?.0 - s.first()?.0 < MIN_SPAN_SECS {
        return None;
    }
    // Remove jumps: shift everything before a jump up by the jump size so the
    // fit sees only organic change.
    let mut adjusted = vec![s[0].1 as f64];
    let mut offset = 0f64;
    for w in s.windows(2) {
        let (t0, f0) = w[0];
        let (t1, f1) = w[1];
        let delta = f1 - f0;
        let cleanup_between = cleanup_times.iter().any(|c| *c > t0 && *c <= t1);
        if delta > 0 && (cleanup_between || delta >= JUMP_BYTES) {
            offset += delta as f64;
        }
        adjusted.push(f1 as f64 - offset);
    }
    let n = s.len() as f64;
    let xs: Vec<f64> = s.iter().map(|(t, _)| (*t - s[0].0) as f64 / 86_400.0).collect();
    let mx = xs.iter().sum::<f64>() / n;
    let my = adjusted.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den = 0.0;
    for (x, y) in xs.iter().zip(&adjusted) {
        num += (x - mx) * (y - my);
        den += (x - mx) * (x - mx);
    }
    if den == 0.0 {
        return None;
    }
    let slope = num / den; // bytes per day
    if slope > -FLAT_BYTES_PER_DAY {
        return None;
    }
    let current = s.last()?.1 as f64;
    Some(Forecast { bytes_per_day: slope, days_left: (current / -slope).max(0.0) })
}

#[cfg(test)]
mod tests {
    use super::*;
    const DAY: i64 = 86_400;
    const NOW: i64 = 1_800_000_000;

    fn series(days: i64, start_gb: f64, per_day_gb: f64) -> Vec<(i64, u64)> {
        (0..=days * 24)
            .map(|h| {
                let t = NOW - days * DAY + h * 3600;
                let gb = start_gb + per_day_gb * (h as f64 / 24.0);
                (t, (gb * GB as f64) as u64)
            })
            .collect()
    }

    #[test]
    fn falling_series_forecasts_days_left() {
        let f = forecast(&series(14, 41.0, -2.2), NOW, &[]).unwrap();
        assert!((f.bytes_per_day / GB as f64 + 2.2).abs() < 0.01, "{f:?}");
        let current = 41.0 - 2.2 * 14.0;
        assert!((f.days_left - current / 2.2).abs() < 0.1);
    }

    #[test]
    fn flat_and_rising_have_no_forecast() {
        assert!(forecast(&series(14, 40.0, 0.0), NOW, &[]).is_none());
        assert!(forecast(&series(14, 40.0, 0.05), NOW, &[]).is_none());
        assert!(forecast(&series(14, 20.0, 0.5), NOW, &[]).is_none());
    }

    #[test]
    fn sparse_series_under_three_days_has_no_forecast() {
        assert!(forecast(&series(2, 40.0, -3.0), NOW, &[]).is_none());
        assert!(forecast(&[(NOW, 1)], NOW, &[]).is_none());
        assert!(forecast(&[], NOW, &[]).is_none());
        // Three days exactly is enough.
        assert!(forecast(&series(3, 40.0, -3.0), NOW, &[]).is_some());
    }

    #[test]
    fn only_last_fourteen_days_count() {
        // Old rising data outside the window must not hide a recent fall.
        let mut s = series(30, 10.0, 1.0);
        s.retain(|(t, _)| *t < NOW - 15 * DAY);
        s.extend(series(10, 60.0, -2.0));
        let f = forecast(&s, NOW, &[]).unwrap();
        assert!((f.bytes_per_day / GB as f64 + 2.0).abs() < 0.05);
    }

    #[test]
    fn cleanup_jumps_are_excluded() {
        // Falling 2 GB/day, with a +30 GB cleanup halfway.
        let mut s = series(14, 50.0, -2.0);
        let mid = NOW - 7 * DAY;
        for p in s.iter_mut() {
            if p.0 >= mid {
                p.1 += 30 * GB;
            }
        }
        let f = forecast(&s, NOW, &[mid]).expect("still falling");
        assert!((f.bytes_per_day / GB as f64 + 2.0).abs() < 0.05, "{f:?}");
        // Without exclusion the fit would be rising; the jump heuristic also catches it.
        let f2 = forecast(&s, NOW, &[]).unwrap();
        assert!((f2.bytes_per_day / GB as f64 + 2.0).abs() < 0.05);
    }
}
