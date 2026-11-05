//! Notification policy: once on entering Low or Critical, then every six
//! hours while Critical is unchanged. Healthy never notifies. Paused or
//! disabled notifications are suppressed (and not replayed later).

use crate::model::DiskStatus;

pub const CRITICAL_REMINDER_SECS: i64 = 6 * 3600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    EnteredLow,
    EnteredCritical,
    CriticalReminder,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub kind: NoticeKind,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Default)]
pub struct NotifyPolicy {
    last_status: Option<DiskStatus>,
    /// When the current Critical episode last produced (or would have
    /// produced) a notification; drives the six-hour reminder.
    critical_mark: Option<i64>,
}

impl NotifyPolicy {
    /// Feed one status observation. Returns the notification to show, if any.
    pub fn observe(&mut self, status: DiskStatus, now: i64, enabled: bool, paused: bool) -> Option<NoticeKind> {
        let prev = self.last_status.replace(status);
        let kind = match (prev, status) {
            (_, DiskStatus::Healthy) => {
                self.critical_mark = None;
                None
            }
            (Some(DiskStatus::Critical), DiskStatus::Critical) => {
                let due = self.critical_mark.map(|m| now - m >= CRITICAL_REMINDER_SECS).unwrap_or(true);
                if due {
                    self.critical_mark = Some(now);
                    Some(NoticeKind::CriticalReminder)
                } else {
                    None
                }
            }
            (_, DiskStatus::Critical) => {
                self.critical_mark = Some(now);
                Some(NoticeKind::EnteredCritical)
            }
            // Entering Low from Healthy (or at startup). Improving from
            // Critical to Low is good news and does not notify.
            (None | Some(DiskStatus::Healthy), DiskStatus::Low) => {
                self.critical_mark = None;
                Some(NoticeKind::EnteredLow)
            }
            (_, DiskStatus::Low) => {
                self.critical_mark = None;
                None
            }
        };
        if !enabled || paused {
            return None;
        }
        kind
    }
}

pub fn notice_text(kind: NoticeKind, free_gb: f64, safe_gb: f64) -> Notice {
    let title = match kind {
        NoticeKind::EnteredLow => "Disk space is low",
        NoticeKind::EnteredCritical | NoticeKind::CriticalReminder => "Disk space is critical",
    };
    Notice {
        kind,
        title: title.into(),
        // Before any scan (or with nothing Safe found), don't claim "0.0 GB".
        body: if safe_gb >= 0.05 {
            format!("{free_gb:.1} GB left. {safe_gb:.1} GB can be freed from Safe items.")
        } else {
            format!("{free_gb:.1} GB left. Open Freeboard to see what you can remove.")
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use DiskStatus::*;

    fn count(seq: &[(DiskStatus, i64)], enabled: bool, paused: bool) -> Vec<NoticeKind> {
        let mut p = NotifyPolicy::default();
        seq.iter().filter_map(|(s, t)| p.observe(*s, *t, enabled, paused)).collect()
    }

    #[test]
    fn healthy_never_notifies() {
        assert!(count(&[(Healthy, 0), (Healthy, 300), (Healthy, 99999)], true, false).is_empty());
    }

    #[test]
    fn entering_low_notifies_exactly_once() {
        let seq: Vec<_> = (0..100).map(|i| (Low, i * 300)).collect();
        let mut all = vec![(Healthy, -300)];
        all.extend(seq);
        assert_eq!(count(&all, true, false), vec![NoticeKind::EnteredLow]);
    }

    #[test]
    fn sustained_critical_reminds_every_six_hours() {
        // Five-minute samples for 13 hours.
        let mut seq = vec![(Healthy, -300)];
        seq.extend((0..=(13 * 12)).map(|i| (Critical, i * 300)));
        let got = count(&seq, true, false);
        assert_eq!(
            got,
            vec![NoticeKind::EnteredCritical, NoticeKind::CriticalReminder, NoticeKind::CriticalReminder]
        );
    }

    #[test]
    fn six_hours_after_entry_sends_one_reminder() {
        let mut seq = vec![(Healthy, -300)];
        seq.extend((0..=(6 * 12)).map(|i| (Critical, i * 300)));
        assert_eq!(count(&seq, true, false), vec![NoticeKind::EnteredCritical, NoticeKind::CriticalReminder]);
    }

    #[test]
    fn low_to_critical_and_back() {
        let seq = [(Healthy, 0), (Low, 300), (Critical, 600), (Low, 900), (Critical, 1200)];
        assert_eq!(
            count(&seq, true, false),
            vec![NoticeKind::EnteredLow, NoticeKind::EnteredCritical, NoticeKind::EnteredCritical]
        );
    }

    #[test]
    fn paused_and_disabled_suppress() {
        let seq = [(Healthy, 0), (Low, 300), (Critical, 600)];
        assert!(count(&seq, false, false).is_empty());
        assert!(count(&seq, true, true).is_empty());
    }

    #[test]
    fn text_matches_prototype_notice() {
        let n = notice_text(NoticeKind::EnteredLow, 12.4, 54.6);
        assert_eq!(n.title, "Disk space is low");
        assert_eq!(n.body, "12.4 GB left. 54.6 GB can be freed from Safe items.");
        let n = notice_text(NoticeKind::EnteredLow, 34.4, 0.0);
        assert_eq!(n.body, "34.4 GB left. Open Freeboard to see what you can remove.");
    }
}
