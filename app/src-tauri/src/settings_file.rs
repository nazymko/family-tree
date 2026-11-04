//! App-level settings and the recent-projects list, as JSON files in the app config dir.
//! Writes go to a temp file that is renamed into place, so a crash never leaves half a file.

use gt_session::{ApiError, DayOrder, ErrorCode, Language, NameOrder, RecentProject, Theme};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SETTINGS_FILE: &str = "settings.json";
pub const RECENT_FILE: &str = "recent.json";
/// Most projects the recent list keeps.
pub const MAX_RECENT: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub day_order: DayOrder,
    pub name_order: NameOrder,
    pub theme: Theme,
    pub language: Language,
    pub backups_to_keep: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            day_order: DayOrder::Dmy,
            name_order: NameOrder::GivenFirst,
            theme: Theme::System,
            language: Language::System,
            backups_to_keep: 10,
        }
    }
}

fn io_err(what: &str, e: &std::io::Error) -> ApiError {
    ApiError::new(ErrorCode::Io, format!("Could not save {what}: {e}"))
}

/// Writes `bytes` to `path` through `<path>.tmp` and a rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    {
        use std::io::Write as _;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    let renamed = std::fs::rename(&tmp, path);
    if renamed.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    renamed
}

/// A missing or unreadable file gives the defaults; the app must start either way.
pub fn load_settings(config_dir: &Path) -> AppSettings {
    std::fs::read_to_string(config_dir.join(SETTINGS_FILE))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_settings(config_dir: &Path, s: &AppSettings) -> Result<(), ApiError> {
    let json = serde_json::to_vec_pretty(s)
        .map_err(|e| ApiError::new(ErrorCode::Internal, e.to_string()))?;
    write_atomic(&config_dir.join(SETTINGS_FILE), &json).map_err(|e| io_err("settings", &e))
}

fn load_raw(config_dir: &Path) -> Vec<RecentProject> {
    std::fs::read_to_string(config_dir.join(RECENT_FILE))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// The canonical form of a project folder, so one folder reached by two spellings is one entry.
/// Falls back to the text given when the folder does not exist.
pub fn canonical_key(dir: &str) -> String {
    let p = std::fs::canonicalize(dir).unwrap_or_else(|_| PathBuf::from(dir));
    let s = p.to_string_lossy().into_owned();
    let s = strip_verbatim(s);
    if cfg!(windows) { s.to_lowercase() } else { s }
}

/// `\\?\C:\x` and `C:\x` are the same folder; `\\?\UNC\srv\share` is `\\srv\share`.
fn strip_verbatim(s: String) -> String {
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        s
    }
}

/// Most recent first, `missing` set for folders that no longer hold a project.
pub fn load_recent(config_dir: &Path) -> Vec<RecentProject> {
    load_raw(config_dir)
        .into_iter()
        .take(MAX_RECENT)
        .map(|mut r| {
            r.missing = !Path::new(&r.dir).join("tree.db").is_file();
            r.opened_at = as_millis(r.opened_at);
            r
        })
        .collect()
}

/// `opened_at` is Unix milliseconds; lists written by older builds hold seconds (they showed as January 1970).
/// Any value below 10^11 (March 1973 in milliseconds, year 5138 in seconds) can only be seconds.
fn as_millis(t: i64) -> i64 {
    if (1..100_000_000_000).contains(&t) {
        t.saturating_mul(1000)
    } else {
        t
    }
}

/// Puts a project at the top: an entry for the same folder is replaced, the list is cut to 12.
pub fn touch_recent(
    config_dir: &Path,
    dir: &str,
    name: &str,
    opened_at: i64,
) -> Result<(), ApiError> {
    let key = canonical_key(dir);
    let mut list: Vec<RecentProject> = load_raw(config_dir)
        .into_iter()
        .filter(|r| canonical_key(&r.dir) != key)
        .collect();
    list.insert(
        0,
        RecentProject {
            dir: dir.to_string(),
            name: name.to_string(),
            opened_at,
            missing: false,
        },
    );
    list.truncate(MAX_RECENT);
    write_recent(config_dir, &list)
}

pub fn remove_recent(config_dir: &Path, dir: &str) -> Result<(), ApiError> {
    let key = canonical_key(dir);
    let list: Vec<RecentProject> = load_raw(config_dir)
        .into_iter()
        .filter(|r| canonical_key(&r.dir) != key)
        .collect();
    write_recent(config_dir, &list)
}

fn write_recent(config_dir: &Path, list: &[RecentProject]) -> Result<(), ApiError> {
    let json = serde_json::to_vec_pretty(list)
        .map_err(|e| ApiError::new(ErrorCode::Internal, e.to_string()))?;
    write_atomic(&config_dir.join(RECENT_FILE), &json).map_err(|e| io_err("the recent list", &e))
}

#[cfg(test)]
mod verbatim_tests {
    use super::{as_millis, strip_verbatim};

    #[test]
    fn opened_at_in_seconds_from_older_lists_becomes_millis() {
        assert_eq!(as_millis(1_793_802_000), 1_793_802_000_000);
        assert_eq!(as_millis(1_793_802_000_000), 1_793_802_000_000);
        assert_eq!(as_millis(0), 0);
    }

    #[test]
    fn verbatim_prefix_is_stripped() {
        let b = char::from(92);
        let v = format!("{b}{b}?{b}C:{b}Users{b}x");
        assert_eq!(strip_verbatim(v), format!("C:{b}Users{b}x"));
        let unc = format!("{b}{b}?{b}UNC{b}srv{b}share");
        assert_eq!(strip_verbatim(unc), format!("{b}{b}srv{b}share"));
        assert_eq!(strip_verbatim("/home/x".into()), "/home/x");
    }
}
