//! Free-space monitoring: sampling, status, notifications and forecast.

pub mod forecast;
pub mod notify;
pub mod status;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::model::{DiskStatus, StatusChanged, TrendPoint, VolumeStatus};
use crate::settings::Settings;
use crate::store::{Store, StoreError};

pub const SAMPLE_INTERVAL_SECS: u64 = 300;
pub const PERSIST_INTERVAL_SECS: i64 = 3600;

#[derive(Debug, Clone, PartialEq)]
pub struct VolumeReading {
    pub id: String,
    pub name: String,
    pub mount_point: String,
    pub free: u64,
    pub total: u64,
}

pub trait VolumeSource: Send + Sync {
    /// Home volume first, then the requested extra fixed volumes that exist.
    fn read(&self, home: &Path, extra: &[String]) -> Vec<VolumeReading>;
    /// Disk map: bytes the platform uses on the home volume's container
    /// outside what Freeboard measures by walking (macOS APFS system
    /// volumes). Fakes report none.
    fn platform_volume_bytes(&self, _platform: &str) -> u64 {
        0
    }
    /// Disk map: macOS Full Disk Access (None when unknown or not macOS).
    fn full_disk_access(&self, _platform: &str) -> Option<bool> {
        None
    }
    /// Other disks the person could choose to watch (Settings): not the home
    /// volume, not system or virtual mounts. Fakes offer none.
    fn candidates(&self, _home: &Path) -> Vec<VolumeReading> {
        Vec::new()
    }
}

/// Mount points that are the system's own, not a disk a person would watch.
pub fn is_system_mount(mount: &str) -> bool {
    mount == "/"
        || ["/System/Volumes/", "/private/var/vm", "/boot", "/snap/", "/dev", "/proc", "/sys", "/var/lib/", "/tmp"].iter().any(|p| mount.starts_with(p))
        // Fedora and others mount USB drives in /run/media/<user>.
        || (mount.starts_with("/run/") && !mount.starts_with("/run/media/"))
        || mount.starts_with("/Volumes/com.apple.")
        || mount.contains("/.timemachine")
}

/// Real volumes through `sysinfo`. Removable volumes are excluded unless the
/// user added them explicitly; WSL mounts are never included.
#[derive(Default)]
pub struct SysVolumes;

impl VolumeSource for SysVolumes {
    fn read(&self, home: &Path, extra: &[String]) -> Vec<VolumeReading> {
        let Ok(disks) = sysinfo::Disks::new_with_refreshed_list() else { return Vec::new() };
        let list: Vec<VolumeReading> = disks
            .iter()
            .filter(|d| !crate::platform::is_wsl_path(d.mount_point()))
            .map(|d| VolumeReading {
                id: d.mount_point().to_string_lossy().into_owned(),
                name: d.name().to_string_lossy().into_owned(),
                mount_point: d.mount_point().to_string_lossy().into_owned(),
                free: real_free(d),
                total: d.total_space(),
            })
            .collect();
        let home = crate::platform::plain_path(&std::fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf()));
        let mut out = Vec::new();
        if let Some(h) = containing(&list, &home) {
            let mut h = h.clone();
            h.id = "home".into();
            out.push(h);
        }
        for e in extra {
            if let Some(v) = list.iter().find(|v| Path::new(&v.mount_point) == Path::new(e)) {
                if !out.iter().any(|o| o.mount_point == v.mount_point) {
                    out.push(v.clone());
                }
            }
        }
        out
    }
    fn platform_volume_bytes(&self, platform: &str) -> u64 {
        crate::diskmap::platform_volume_bytes(platform)
    }
    fn candidates(&self, home: &Path) -> Vec<VolumeReading> {
        let home_mount = self.read(home, &[]).into_iter().next().map(|h| h.mount_point);
        let Ok(disks) = sysinfo::Disks::new_with_refreshed_list() else { return Vec::new() };
        let mut v: Vec<VolumeReading> = disks
            .iter()
            .map(|d| VolumeReading {
                id: d.mount_point().to_string_lossy().into_owned(),
                name: d.name().to_string_lossy().into_owned(),
                mount_point: d.mount_point().to_string_lossy().into_owned(),
                free: real_free(d),
                total: d.total_space(),
            })
            .filter(|r| {
                !crate::platform::is_wsl_path(Path::new(&r.mount_point))
                    && !is_system_mount(&r.mount_point)
                    && Some(&r.mount_point) != home_mount.as_ref()
                    && r.total >= 1_000_000_000
            })
            .collect();
        v.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
        v.dedup_by(|a, b| a.mount_point == b.mount_point);
        v
    }
    fn full_disk_access(&self, platform: &str) -> Option<bool> {
        crate::diskmap::full_disk_access(platform)
    }
}

/// macOS: `sysinfo` reports Finder's figure (free plus purgeable). Freeboard
/// shows space that is free now and reports purgeable space separately.
fn real_free(d: &sysinfo::Disk) -> u64 {
    #[cfg(target_os = "macos")]
    if let Some(b) = crate::platform::macos::available_bytes(d.mount_point()) {
        return b;
    }
    d.available_space()
}

fn containing<'a>(list: &'a [VolumeReading], p: &Path) -> Option<&'a VolumeReading> {
    list.iter()
        .filter(|v| p.starts_with(&v.mount_point))
        .max_by_key(|v| v.mount_point.len())
}

/// Free space of the volume holding `home` (used for before/after measurements).
pub fn home_free_bytes(source: &dyn VolumeSource, home: &Path) -> u64 {
    source.read(home, &[]).first().map(|v| v.free).unwrap_or(0)
}

#[derive(Debug, Default)]
pub struct Tick {
    pub volumes: Vec<VolumeStatus>,
    pub changes: Vec<StatusChanged>,
    pub notices: Vec<(String, notify::NoticeKind, u64)>,
}

#[derive(Default)]
pub struct Monitor {
    statuses: HashMap<String, DiskStatus>,
    policies: HashMap<String, notify::NotifyPolicy>,
    latest: Vec<VolumeStatus>,
    /// In-memory five-minute samples for the last 24 hours, per volume.
    recent: HashMap<String, Vec<(i64, u64)>>,
}

impl Monitor {
    pub fn new() -> Monitor {
        Monitor::default()
    }

    pub fn latest(&self) -> &[VolumeStatus] {
        &self.latest
    }

    /// Restore the last known status (so a restart does not re-notify).
    pub fn restore(&mut self, store: &Store) {
        if let Some(v) = store.meta_get("monitor_statuses") {
            if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(&v) {
                for (k, s) in m {
                    if let Some(st) = DiskStatus::parse(&s) {
                        self.statuses.insert(k.clone(), st);
                        let mut p = notify::NotifyPolicy::default();
                        // Prime the policy without notifying.
                        p.observe(st, 0, false, false);
                        self.policies.insert(k, p);
                    }
                }
            }
        }
    }

    /// Take one sample of every configured volume.
    pub fn tick(
        &mut self,
        now: i64,
        settings: &Settings,
        readings: Vec<VolumeReading>,
        store: &Store,
        paused: bool,
    ) -> Result<Tick, StoreError> {
        let mut out = Tick::default();
        for r in readings {
            let prev = self.statuses.get(&r.id).copied();
            let st = status::next_status(prev, r.free, settings.low_threshold_bytes, settings.critical_threshold_bytes);
            self.statuses.insert(r.id.clone(), st);
            if prev != Some(st) {
                out.changes.push(StatusChanged { volume: r.id.clone(), status: st, free_bytes: r.free });
            }
            let pol = self.policies.entry(r.id.clone()).or_default();
            if let Some(kind) = pol.observe(st, now, settings.notifications, paused) {
                out.notices.push((r.id.clone(), kind, r.free));
            }
            let recent = self.recent.entry(r.id.clone()).or_default();
            recent.push((now, r.free));
            recent.retain(|(t, _)| *t >= now - 86_400);
            let due = store.last_sample_at(&r.id).map(|t| now - t >= PERSIST_INTERVAL_SECS).unwrap_or(true);
            if due {
                store.insert_sample(&r.id, now, r.free, r.total)?;
            }
            out.volumes.push(VolumeStatus {
                id: r.id,
                name: r.name,
                mount_point: r.mount_point,
                free_bytes: r.free,
                total_bytes: r.total,
                status: st,
                purgeable_bytes: None,
                snapshot_count: None,
            });
        }
        if !out.changes.is_empty() {
            let m: HashMap<String, &str> = self.statuses.iter().map(|(k, v)| (k.clone(), v.as_str())).collect();
            store.meta_set("monitor_statuses", &serde_json::to_string(&m).unwrap_or_default())?;
        }
        self.latest = out.volumes.clone();
        Ok(out)
    }

    /// Recompute status after a threshold change, without notifying. New
    /// thresholds apply immediately (hysteresis protects only against noise
    /// in free space, not against the user's own setting change).
    pub fn rethreshold(&mut self, settings: &Settings) {
        for v in self.latest.iter_mut() {
            let st = status::raw_status(v.free_bytes, settings.low_threshold_bytes, settings.critical_threshold_bytes);
            self.statuses.insert(v.id.clone(), st);
            v.status = st;
        }
    }

    pub fn overall(&self) -> DiskStatus {
        self.latest
            .iter()
            .map(|v| v.status)
            .max_by_key(|s| match s {
                DiskStatus::Healthy => 0,
                DiskStatus::Low => 1,
                DiskStatus::Critical => 2,
            })
            .unwrap_or(DiskStatus::Healthy)
    }
}

/// One trend point per day for the last 14 days (last sample of each day).
pub fn daily_trend(samples: &[(i64, u64, u64)], now: i64) -> Vec<TrendPoint> {
    let mut by_day: std::collections::BTreeMap<i64, (i64, u64)> = Default::default();
    for (t, f, _) in samples {
        if *t < now - forecast::WINDOW_SECS {
            continue;
        }
        let day = t.div_euclid(86_400);
        let e = by_day.entry(day).or_insert((*t, *f));
        if *t >= e.0 {
            *e = (*t, *f);
        }
    }
    by_day
        .values()
        .map(|(t, f)| TrendPoint {
            at: chrono::DateTime::from_timestamp(*t, 0).map(|d| d.to_rfc3339()).unwrap_or_default(),
            free_bytes: *f,
        })
        .collect()
}

/// Helper used by core: path of the user's home or a fallback.
pub fn home_or_root() -> PathBuf {
    crate::platform::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GB;

    fn reading(free_gb: u64) -> Vec<VolumeReading> {
        vec![VolumeReading { id: "home".into(), name: "Disk".into(), mount_point: "/".into(), free: free_gb * GB, total: 512 * GB }]
    }

    #[test]
    fn hourly_persistence_of_five_minute_samples() {
        let store = Store::open_in_memory().unwrap();
        let mut m = Monitor::new();
        let s = Settings::default();
        let t0 = 1_800_000_000;
        // 3 hours of 5-minute ticks → 3 persisted samples (t0, t0+1h, t0+2h).
        for i in 0..36 {
            m.tick(t0 + i * 300, &s, reading(50), &store, false).unwrap();
        }
        assert_eq!(store.samples("home", 0).unwrap().len(), 3);
    }

    #[test]
    fn per_volume_histories() {
        let store = Store::open_in_memory().unwrap();
        let mut m = Monitor::new();
        let s = Settings::default();
        let mut r = reading(50);
        r.push(VolumeReading { id: "/data".into(), name: "Data".into(), mount_point: "/data".into(), free: 5 * GB, total: 100 * GB });
        let t = m.tick(1_800_000_000, &s, r, &store, false).unwrap();
        assert_eq!(store.samples("home", 0).unwrap().len(), 1);
        assert_eq!(store.samples("/data", 0).unwrap().len(), 1);
        assert_eq!(t.volumes[1].status, DiskStatus::Critical);
        assert_eq!(m.overall(), DiskStatus::Critical);
        assert_eq!(t.notices.len(), 1);
    }

    #[test]
    fn crossing_low_sends_exactly_one_notification() {
        let store = Store::open_in_memory().unwrap();
        let mut m = Monitor::new();
        let s = Settings::default();
        let mut n = 0;
        for (i, gb) in [30, 25, 19, 21, 19, 21, 19, 18, 17].iter().enumerate() {
            n += m.tick(1_800_000_000 + i as i64 * 300, &s, reading(*gb), &store, false).unwrap().notices.len();
        }
        assert_eq!(n, 1);
    }

    #[test]
    fn restart_does_not_renotify() {
        let store = Store::open_in_memory().unwrap();
        let s = Settings::default();
        let mut m = Monitor::new();
        assert_eq!(m.tick(1_800_000_000, &s, reading(10), &store, false).unwrap().notices.len(), 1);
        let mut m2 = Monitor::new();
        m2.restore(&store);
        assert_eq!(m2.tick(1_800_000_300, &s, reading(10), &store, false).unwrap().notices.len(), 0);
    }

    #[test]
    fn daily_trend_has_one_point_per_day() {
        let now = 1_800_000_000;
        let samples: Vec<_> = (0..14 * 24).map(|h| (now - 14 * 86_400 + h * 3600 + 1, (100 - h) as u64, 0)).collect();
        let t = daily_trend(&samples, now);
        assert!(t.len() >= 14 && t.len() <= 15);
    }

    #[test]
    fn real_volumes_include_home() {
        let home = home_or_root();
        let v = SysVolumes.read(&home, &[]);
        assert_eq!(v.first().map(|v| v.id.as_str()), Some("home"));
        assert!(v[0].total > 0);
    }
}
