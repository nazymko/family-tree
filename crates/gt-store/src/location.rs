//! Warn when a project lives where SQLite is unsafe: sync folders and network shares.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationWarning {
    /// The folder is inside a cloud-sync folder of this provider.
    SyncedFolder(&'static str),
    /// The folder is on a Windows network share (UNC path).
    NetworkShare,
}

fn provider(component: &str) -> Option<&'static str> {
    let c = component.to_lowercase();
    if c.starts_with("onedrive") {
        Some("OneDrive")
    } else if c == "dropbox" || c.starts_with("dropbox (") {
        Some("Dropbox")
    } else if matches!(
        c.as_str(),
        "icloud drive" | "iclouddrive" | "mobile documents"
    ) {
        Some("iCloud")
    } else if matches!(c.as_str(), "google drive" | "googledrive" | "my drive") {
        Some("Google Drive")
    } else {
        None
    }
}

/// Checks a project folder. Linux mounts under `/mnt` or `/media` are deliberately not
/// flagged (too many false alarms).
pub fn check_location(dir: &Path) -> Option<LocationWarning> {
    let text = dir.to_string_lossy();
    // `\\server\share` is a share; `\\?\C:\` and `\\.\` are local device paths.
    if text.starts_with("\\\\") {
        let local = text.starts_with("\\\\?\\") || text.starts_with("\\\\.\\");
        let verbatim_unc = text.to_uppercase().starts_with("\\\\?\\UNC\\");
        if !local || verbatim_unc {
            return Some(LocationWarning::NetworkShare);
        }
    }
    if let Some(p) = text.split(['/', '\\']).find_map(provider) {
        return Some(LocationWarning::SyncedFolder(p));
    }
    for var in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
        if let Some(root) = std::env::var_os(var).filter(|v| !v.is_empty())
            && dir.starts_with(Path::new(&root))
        {
            return Some(LocationWarning::SyncedFolder("OneDrive"));
        }
    }
    None
}
