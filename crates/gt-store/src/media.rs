//! Content-addressed media store: `media/<sha256[0..2]>/<sha256>.<ext>`.

use crate::StoreError;
use crate::project::Project;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

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
    (!ext.is_empty()).then_some(ext)
}

fn sha256_of(path: &Path) -> Result<String, StoreError> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

impl Project {
    /// Copies `src` into `media/<sha256[0..2]>/<sha256>.<ext>`; returns the path relative to the
    /// project folder (forward slashes) and the hash. Identical content is stored once.
    pub fn add_media_file(&self, src: &Path) -> Result<(String, String), StoreError> {
        let sha = sha256_of(src)?;
        let file_name = match extension(src) {
            Some(ext) => format!("{sha}.{ext}"),
            None => sha.clone(),
        };
        let rel = format!("media/{}/{file_name}", &sha[..2]);
        let parent = self.dir.join("media").join(&sha[..2]);
        let dest = parent.join(&file_name);
        if !dest.exists() {
            std::fs::create_dir_all(&parent)?;
            // Copy beside the destination, then rename, so a crash never leaves a partial file.
            let tmp = parent.join(format!(".{sha}.part"));
            std::fs::copy(src, &tmp)?;
            if let Err(e) = std::fs::rename(&tmp, &dest) {
                let _ = std::fs::remove_file(&tmp);
                if !dest.exists() {
                    return Err(e.into());
                }
            }
        }
        Ok((rel, sha))
    }
}
