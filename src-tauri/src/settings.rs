//! User settings: source defaults ("Default settings" table), validation and
//! partial updates. Persisted as key/JSON rows in the `settings` table.

use serde::{Deserialize, Serialize};

use crate::model::GB;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuickScanSchedule {
    Hourly,
    Every6Hours,
    Daily,
    OnRequest,
}

impl QuickScanSchedule {
    pub fn interval_secs(self) -> Option<u64> {
        match self {
            QuickScanSchedule::Hourly => Some(3600),
            QuickScanSchedule::Every6Hours => Some(6 * 3600),
            QuickScanSchedule::Daily => Some(24 * 3600),
            QuickScanSchedule::OnRequest => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeepScanSchedule {
    OnRequest,
    Daily,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub low_threshold_bytes: u64,
    pub critical_threshold_bytes: u64,
    pub quick_scan: QuickScanSchedule,
    pub deep_scan: DeepScanSchedule,
    pub inactive_project_days: u32,
    pub old_installer_days: u32,
    pub pause_deep_scans_on_battery: bool,
    pub start_at_login: bool,
    pub notifications: bool,
    /// Chosen during first run; empty until then.
    pub scan_folders: Vec<String>,
    pub exclusions: Vec<String>,
    /// Additional fixed volumes (mount points) to monitor besides home.
    pub extra_volumes: Vec<String>,
    pub onboarding_complete: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            low_threshold_bytes: 20 * GB,
            critical_threshold_bytes: 8 * GB,
            quick_scan: QuickScanSchedule::Hourly,
            deep_scan: DeepScanSchedule::OnRequest,
            inactive_project_days: 30,
            old_installer_days: 90,
            pause_deep_scans_on_battery: true,
            start_at_login: true,
            notifications: true,
            scan_folders: Vec::new(),
            exclusions: Vec::new(),
            extra_volumes: Vec::new(),
            onboarding_complete: false,
        }
    }
}

/// Partial update sent by `update_settings`. Unknown fields are rejected.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatch {
    pub low_threshold_bytes: Option<u64>,
    pub critical_threshold_bytes: Option<u64>,
    pub quick_scan: Option<QuickScanSchedule>,
    pub deep_scan: Option<DeepScanSchedule>,
    pub inactive_project_days: Option<u32>,
    pub old_installer_days: Option<u32>,
    pub pause_deep_scans_on_battery: Option<bool>,
    pub start_at_login: Option<bool>,
    pub notifications: Option<bool>,
    pub scan_folders: Option<Vec<String>>,
    pub exclusions: Option<Vec<String>>,
    pub extra_volumes: Option<Vec<String>>,
    pub onboarding_complete: Option<bool>,
}

/// The subset of settings the interface may change with `update_settings`.
/// It has no path fields: folders change only through backend-owned pickers
/// (`add_folder` / `remove_folder`), so the interface never sends a path.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiSettingsPatch {
    pub low_threshold_bytes: Option<u64>,
    pub critical_threshold_bytes: Option<u64>,
    pub quick_scan: Option<QuickScanSchedule>,
    pub deep_scan: Option<DeepScanSchedule>,
    pub inactive_project_days: Option<u32>,
    pub old_installer_days: Option<u32>,
    pub pause_deep_scans_on_battery: Option<bool>,
    pub start_at_login: Option<bool>,
    pub notifications: Option<bool>,
    pub onboarding_complete: Option<bool>,
}

impl From<UiSettingsPatch> for SettingsPatch {
    fn from(p: UiSettingsPatch) -> SettingsPatch {
        SettingsPatch {
            low_threshold_bytes: p.low_threshold_bytes,
            critical_threshold_bytes: p.critical_threshold_bytes,
            quick_scan: p.quick_scan,
            deep_scan: p.deep_scan,
            inactive_project_days: p.inactive_project_days,
            old_installer_days: p.old_installer_days,
            pause_deep_scans_on_battery: p.pause_deep_scans_on_battery,
            start_at_login: p.start_at_login,
            notifications: p.notifications,
            onboarding_complete: p.onboarding_complete,
            ..Default::default()
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SettingsError {
    #[error("the critical threshold must be lower than the low threshold")]
    InconsistentThresholds,
    #[error("{0} is out of range")]
    OutOfRange(&'static str),
    #[error("folder paths must be absolute: {0}")]
    RelativePath(String),
}

impl Settings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.critical_threshold_bytes >= self.low_threshold_bytes {
            return Err(SettingsError::InconsistentThresholds);
        }
        if self.critical_threshold_bytes < GB || self.low_threshold_bytes > 500 * GB {
            return Err(SettingsError::OutOfRange("threshold"));
        }
        if !(1..=3650).contains(&self.inactive_project_days) {
            return Err(SettingsError::OutOfRange("inactiveProjectDays"));
        }
        if !(1..=3650).contains(&self.old_installer_days) {
            return Err(SettingsError::OutOfRange("oldInstallerDays"));
        }
        for p in self
            .scan_folders
            .iter()
            .chain(self.exclusions.iter())
            .chain(self.extra_volumes.iter())
        {
            if !std::path::Path::new(p).is_absolute() {
                return Err(SettingsError::RelativePath(p.clone()));
            }
        }
        Ok(())
    }

    pub fn apply(&self, patch: SettingsPatch) -> Result<Settings, SettingsError> {
        let mut s = self.clone();
        macro_rules! set {
            ($($f:ident),*) => { $( if let Some(v) = patch.$f { s.$f = v; } )* };
        }
        set!(
            low_threshold_bytes,
            critical_threshold_bytes,
            quick_scan,
            deep_scan,
            inactive_project_days,
            old_installer_days,
            pause_deep_scans_on_battery,
            start_at_login,
            notifications,
            scan_folders,
            exclusions,
            extra_volumes,
            onboarding_complete
        );
        s.validate()?;
        Ok(s)
    }

    /// Key/value rows for the `settings` table.
    pub fn to_rows(&self) -> Vec<(String, String)> {
        let v = serde_json::to_value(self).expect("settings serialize");
        v.as_object()
            .expect("object")
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect()
    }

    /// Rebuild from stored rows, falling back to defaults for missing or
    /// unreadable keys (forward compatible with new settings).
    pub fn from_rows(rows: &[(String, String)]) -> Settings {
        let mut base = serde_json::to_value(Settings::default()).expect("defaults");
        let obj = base.as_object_mut().expect("object");
        for (k, v) in rows {
            if obj.contains_key(k) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(v) {
                    obj.insert(k.clone(), val);
                }
            }
        }
        let parsed: Settings = serde_json::from_value(base).unwrap_or_default();
        if parsed.validate().is_ok() {
            parsed
        } else {
            Settings::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_source_table() {
        let s = Settings::default();
        assert_eq!(s.low_threshold_bytes, 20 * GB);
        assert_eq!(s.critical_threshold_bytes, 8 * GB);
        assert_eq!(s.quick_scan, QuickScanSchedule::Hourly);
        assert_eq!(s.deep_scan, DeepScanSchedule::OnRequest);
        assert_eq!(s.inactive_project_days, 30);
        assert_eq!(s.old_installer_days, 90);
        assert!(s.pause_deep_scans_on_battery);
        assert!(s.start_at_login);
        assert!(s.notifications);
        assert!(s.scan_folders.is_empty(), "scan folders are chosen during first run");
        assert!(s.validate().is_ok());
    }

    #[test]
    fn rejects_inconsistent_thresholds() {
        let s = Settings::default();
        let err = s
            .apply(SettingsPatch { critical_threshold_bytes: Some(25 * GB), ..Default::default() })
            .unwrap_err();
        assert_eq!(err, SettingsError::InconsistentThresholds);
        let err = s
            .apply(SettingsPatch { low_threshold_bytes: Some(8 * GB), ..Default::default() })
            .unwrap_err();
        assert_eq!(err, SettingsError::InconsistentThresholds);
    }

    #[test]
    fn rejects_relative_folders() {
        let s = Settings::default();
        assert!(matches!(
            s.apply(SettingsPatch { scan_folders: Some(vec!["projects".into()]), ..Default::default() }),
            Err(SettingsError::RelativePath(_))
        ));
    }

    #[test]
    fn patch_rejects_unknown_fields() {
        let r: Result<SettingsPatch, _> = serde_json::from_str(r#"{"command":"rm -rf /"}"#);
        assert!(r.is_err());
    }

    #[test]
    fn ui_patch_cannot_carry_paths() {
        let r: Result<UiSettingsPatch, _> = serde_json::from_str(r#"{"scanFolders":["/"]}"#);
        assert!(r.is_err());
        let r: Result<UiSettingsPatch, _> = serde_json::from_str(r#"{"lowThresholdBytes":30000000000}"#);
        assert!(r.is_ok());
    }

    #[test]
    fn rows_round_trip() {
        let s = Settings::default()
            .apply(SettingsPatch {
                low_threshold_bytes: Some(30 * GB),
                scan_folders: Some(vec![if cfg!(windows) { "C:\\work".into() } else { "/work".into() }]),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(Settings::from_rows(&s.to_rows()), s);
    }
}
