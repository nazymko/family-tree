//! Content-addressed media store: `media/<sha256[0..2]>/<sha256>.<ext>`.

use crate::StoreError;
use crate::project::Project;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn extension(src: &Path) -> Option<String> {
    let ext: String = src
        .extension()?
        .to_string_lossy()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_lowercase();
    match ext.as_str() {
        "" => None,
        "jpeg" => Some("jpg".into()),
        _ => Some(ext),
    }
}

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Copies `src` to `tmp`, hashing the very bytes written, so the name always matches the content.
fn copy_and_hash(src: &Path, tmp: &Path) -> Result<String, StoreError> {
    let mut input = std::fs::File::open(src)?;
    let mut output = std::fs::File::create(tmp)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        output.write_all(&buf[..n])?;
    }
    output.sync_all()?;
    Ok(hex(&hasher.finalize()))
}

/// The same as [`Project::add_media_file`], from the project folder alone. It touches only
/// files, never the database, so a caller can run it without holding the project open for
/// anyone else (a long copy and hash must not stall the rest of the app).
pub fn add_media_file_in(dir: &Path, src: &Path) -> Result<(String, String), StoreError> {
    add_media_file_to(dir, src)
}

impl Project {
    /// Copies `src` into `media/<sha256[0..2]>/<sha256>.<ext>`; returns the path relative to the
    /// project folder (forward slashes) and the hash. Identical content is stored once.
    pub fn add_media_file(&self, src: &Path) -> Result<(String, String), StoreError> {
        add_media_file_to(&self.dir, src)
    }
}

fn add_media_file_to(dir: &Path, src: &Path) -> Result<(String, String), StoreError> {
    {
        let media = dir.join("media");
        std::fs::create_dir_all(&media)?;
        let tmp = media.join(format!(
            ".part-{}-{}",
            std::process::id(),
            TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let sha = match copy_and_hash(src, &tmp) {
            Ok(sha) => sha,
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                return Err(e);
            }
        };
        let file_name = match extension(src) {
            Some(ext) => format!("{sha}.{ext}"),
            None => sha.clone(),
        };
        let rel = format!("media/{}/{file_name}", &sha[..2]);
        let parent = media.join(&sha[..2]);
        let dest = parent.join(&file_name);
        let placed = if dest.exists() {
            Ok(())
        } else {
            std::fs::create_dir_all(&parent).and_then(|()| std::fs::rename(&tmp, &dest))
        };
        let _ = std::fs::remove_file(&tmp);
        match placed {
            Ok(()) => Ok((rel, sha)),
            Err(_) if dest.exists() => Ok((rel, sha)),
            Err(e) => Err(e.into()),
        }
    }
}
