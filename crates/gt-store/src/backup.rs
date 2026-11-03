//! Timestamped, consistent backups of `tree.db` (`VACUUM INTO`, safe while the project is open).

use crate::StoreError;
use crate::codec::corrupt;
use crate::project::Project;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupReason {
    DailyOpen,
    BeforeImport,
    BeforeMerge,
    Manual,
}

impl BackupReason {
    fn slug(self) -> &'static str {
        match self {
            BackupReason::DailyOpen => "daily-open",
            BackupReason::BeforeImport => "before-import",
            BackupReason::BeforeMerge => "before-merge",
            BackupReason::Manual => "manual",
        }
    }
}

/// `yyyyMMdd-HHmmss` in UTC for a Unix time in seconds.
fn utc_stamp(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (the Hinnant days-to-date algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

/// `(timestamp, counter)` of a backup file name, or `None` if it is not one of ours.
fn sort_key(name: &str) -> Option<(String, u32)> {
    let stem = name.strip_prefix("tree-")?.strip_suffix(".db")?;
    let stamp = stem.get(..15)?;
    let rest = stem.get(15..)?;
    if !rest.starts_with('-') {
        return None;
    }
    let counter = rest
        .rsplit('-')
        .next()
        .and_then(|l| l.parse::<u32>().ok())
        .unwrap_or(1);
    Some((stamp.to_string(), counter))
}

impl Project {
    /// `VACUUM INTO backups/tree-<yyyyMMdd-HHmmss>-<reason>.db` (UTC), then prune to the
    /// `keep` newest backups. A second backup in the same second gets a `-2`, `-3`, ... suffix.
    pub fn backup(&self, reason: BackupReason, keep: usize) -> Result<PathBuf, StoreError> {
        let dir = self.dir.join("backups");
        std::fs::create_dir_all(&dir)?;
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_secs()).unwrap_or(0))
            .unwrap_or(0);
        let stamp = utc_stamp(secs);
        // One more than the highest counter already used in this second (pruning can leave gaps).
        let taken = std::fs::read_dir(&dir)?
            .filter_map(Result::ok)
            .filter_map(|e| sort_key(&e.file_name().to_string_lossy()))
            .filter(|(s, _)| *s == stamp)
            .map(|(_, c)| c)
            .max()
            .unwrap_or(0);
        let name = if taken == 0 {
            format!("tree-{stamp}-{}.db", reason.slug())
        } else {
            format!("tree-{stamp}-{}-{}.db", reason.slug(), taken + 1)
        };
        let path = dir.join(name);
        let target = path
            .to_str()
            .ok_or_else(|| corrupt("backup path is not valid UTF-8"))?;
        self.conn.execute("VACUUM INTO ?1", [target])?;
        // The backup exists; failing to tidy old ones must not turn that into an error.
        if let Err(e) = self.prune_backups(keep.max(1), &path) {
            eprintln!("gt-store: could not prune old backups: {e}");
        }
        Ok(path)
    }

    fn prune_backups(&self, keep: usize, newest: &std::path::Path) -> Result<(), StoreError> {
        let dir = self.dir.join("backups");
        let mut files: Vec<((String, u32), PathBuf)> = std::fs::read_dir(&dir)?
            .filter_map(Result::ok)
            .filter_map(|e| {
                let key = sort_key(&e.file_name().to_string_lossy())?;
                Some((key, e.path()))
            })
            .collect();
        files.sort();
        let excess = files.len().saturating_sub(keep);
        for (_, path) in files.into_iter().take(excess) {
            if path != newest {
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_are_utc_calendar() {
        assert_eq!(utc_stamp(0), "19700101-000000");
        assert_eq!(utc_stamp(951_782_400 + 86_399), "20000229-235959");
        assert_eq!(utc_stamp(1_700_000_000), "20231114-221320");
    }

    #[test]
    fn sort_keys() {
        assert_eq!(
            sort_key("tree-20231114-221320-manual.db"),
            Some(("20231114-221320".into(), 1))
        );
        assert_eq!(
            sort_key("tree-20231114-221320-daily-open-3.db"),
            Some(("20231114-221320".into(), 3))
        );
        assert_eq!(sort_key("other.db"), None);
    }
}
