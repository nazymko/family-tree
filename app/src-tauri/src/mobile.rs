//! Phone and tablet storage helpers.
//!
//! On Android the file pickers hand back `content://` URIs, not paths, and a tree must live in the app's own
//! data folder. Three small commands bridge that for the front end; everything else keeps working on plain paths.
//! They compile on every platform (a plain path is copied with `std::fs`), so desktop tests cover them.

use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, Runtime};

/// Where trees, picked files and files waiting to be saved live on this device.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileInfo {
    /// Every tree is a folder in here (`tree.db` + `media/`).
    pub trees_dir: String,
    /// Scratch space for picked files and for files made to be saved or shared.
    pub cache_dir: String,
}

fn is_content_uri(s: &str) -> bool {
    s.starts_with("content://")
}

/// A file name that is safe inside our own folders: no separators, no control characters, never empty.
pub fn safe_file_name(raw: &str) -> String {
    let tail = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = tail
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() {
        "file".to_string()
    } else {
        cleaned
    }
}

/// The name Android gives a picked document is the last path segment of its URI (percent-encoded,
/// often `primary:Download/x.ged`).
fn name_from_uri(uri: &str) -> String {
    let last = uri.rsplit('/').next().unwrap_or("");
    let decoded = percent_decode(last);
    safe_file_name(
        decoded
            .rsplit('/')
            .next()
            .unwrap_or(&decoded)
            .rsplit(':')
            .next()
            .unwrap_or(&decoded),
    )
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(v) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(target_os = "android")]
fn open_uri<R: Runtime>(app: &AppHandle<R>, uri: &str, write: bool) -> io::Result<fs::File> {
    use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};
    let url = uri
        .parse()
        .map_err(|e| io::Error::other(format!("not a document address: {e}")))?;
    let mut opts = OpenOptions::new();
    if write {
        opts.read(false).write(true).truncate(true);
    } else {
        opts.read(true);
    }
    app.fs().open(FilePath::Url(url), opts)
}

#[cfg(not(target_os = "android"))]
fn open_uri<R: Runtime>(_app: &AppHandle<R>, _uri: &str, _write: bool) -> io::Result<fs::File> {
    Err(io::Error::other(
        "content:// addresses exist only on Android",
    ))
}

/// Copies a picked document into `dir` and returns the local path. A plain path is copied too, so the caller
/// never holds a handle to a file another app may change.
pub fn stage_into<R: Runtime>(app: &AppHandle<R>, src: &str, dir: &Path) -> io::Result<PathBuf> {
    if !is_content_uri(src) {
        return copy_plain(src, dir);
    }
    fs::create_dir_all(dir)?;
    let dest = dir.join(name_from_uri(src));
    let mut from = open_uri(app, src, false)?;
    let mut to = fs::File::create(&dest)?;
    io::copy(&mut from, &mut to)?;
    Ok(dest)
}

/// The plain-path half of stage_into: copy the file into dir under a safe name.
fn copy_plain(src: &str, dir: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let dest = dir.join(safe_file_name(src));
    fs::copy(src, &dest)?;
    Ok(dest)
}

/// Writes a local file to the place the person chose in the save dialog.
pub fn deliver_to<R: Runtime>(app: &AppHandle<R>, src: &Path, dest: &str) -> io::Result<u64> {
    if is_content_uri(dest) {
        let mut from = fs::File::open(src)?;
        let mut to = open_uri(app, dest, true)?;
        io::copy(&mut from, &mut to)
    } else {
        fs::copy(src, dest)
    }
}

fn dirs<R: Runtime>(app: &AppHandle<R>) -> Result<MobileInfo, String> {
    let trees = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("trees");
    let cache = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&trees).map_err(|e| e.to_string())?;
    // Files made for the person to save or send are written here first (see `mobile_deliver`).
    fs::create_dir_all(cache.join("export")).map_err(|e| e.to_string())?;
    Ok(MobileInfo {
        trees_dir: trees.to_string_lossy().into_owned(),
        cache_dir: cache.to_string_lossy().into_owned(),
    })
}

/// The two folders the front end needs; they are created on first use.
#[tauri::command]
pub fn mobile_info<R: Runtime>(app: AppHandle<R>) -> Result<MobileInfo, String> {
    dirs(&app)
}

/// Copies a picked file (a `content://` address on Android) into the cache folder; the answer is a path every
/// other command can read.
#[tauri::command(async)]
pub fn mobile_stage_pick<R: Runtime>(app: AppHandle<R>, uri: String) -> Result<String, String> {
    let info = dirs(&app)?;
    let dir = PathBuf::from(info.cache_dir).join("picked");
    stage_into(&app, &uri, &dir)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| format!("Could not read the chosen file: {e}"))
}

/// Copies a file made in the cache folder to where the person chose in the save dialog, then removes the cache copy.
#[tauri::command(async)]
pub fn mobile_deliver<R: Runtime>(
    app: AppHandle<R>,
    src: String,
    dest: String,
) -> Result<u64, String> {
    let info = dirs(&app)?;
    let cache = PathBuf::from(&info.cache_dir);
    let src_path = PathBuf::from(&src);
    // Only files this app made for delivery may leave through here.
    let inside = src_path
        .canonicalize()
        .ok()
        .zip(cache.canonicalize().ok())
        .is_some_and(|(s, c)| s.starts_with(c));
    if !inside {
        return Err("That file is not one the app made for saving.".to_string());
    }
    let n =
        deliver_to(&app, &src_path, &dest).map_err(|e| format!("Could not save the file: {e}"))?;
    let _ = fs::remove_file(&src_path);
    Ok(n)
}

/// Handler for the three commands above.
pub fn handler<R: Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![mobile_info, mobile_stage_pick, mobile_deliver]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_lose_their_folders() {
        assert_eq!(safe_file_name("C:\\a\\b\\tree.zip"), "tree.zip");
        assert_eq!(safe_file_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_file_name(".."), "file");
        assert_eq!(safe_file_name("a:b*c.ged"), "a_b_c.ged");
    }

    #[test]
    fn a_picked_address_gives_a_readable_name() {
        assert_eq!(name_from_uri("content://x/document/msf%3A123"), "123");
        assert_eq!(
            name_from_uri("content://x/document/primary%3ADownload%2Ffamily.ged"),
            "family.ged"
        );
        assert_eq!(name_from_uri("content://x/document/"), "file");
    }

    #[test]
    fn a_plain_path_is_copied_in() {
        let tmp = tempfile::tempdir().expect("tmp");
        let src = tmp.path().join("a.ged");
        fs::write(&src, b"0 HEAD").expect("write");
        let out = copy_plain(src.to_str().expect("utf8"), &tmp.path().join("in")).expect("stage");
        assert_eq!(fs::read(out).expect("read"), b"0 HEAD");
    }
}
