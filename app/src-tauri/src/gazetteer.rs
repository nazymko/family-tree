//! The offline gazetteer's one network use: downloading a GeoNames country file on the person's explicit click.
//!
//! `https://download.geonames.org/export/dump/<CC>.zip` is fetched to a temporary file with progress, converted into
//! the compact index (`gazetteer/<CC>.gtgz` in the app's config folder) and the zip is deleted. Nothing else here
//! touches the network.

use gt_gazetteer::{Gazetteer, Index, index::index_path};
use gt_session::{ApiError, ErrorCode, GazetteerCountryDto, GazetteerProgressDto, GazetteerStatusDto};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const BASE: &str = "https://download.geonames.org/export/dump";
/// A download larger than this is refused (the largest country file, a few hundred megabytes, stays well below).
const MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// The folder of the indexes.
pub fn dir(config_dir: &Path) -> PathBuf {
    config_dir.join("gazetteer")
}

/// Where the zips come from. Tests (debug builds only) point `GT_GAZETTEER_BASE` at a local server.
fn base() -> String {
    #[cfg(debug_assertions)]
    if let Ok(b) = std::env::var("GT_GAZETTEER_BASE") {
        return b;
    }
    BASE.to_string()
}

/// Two ASCII letters; anything else could be a path or a different file.
pub fn country_code(cc: &str) -> Result<String, ApiError> {
    let cc = cc.trim().to_ascii_uppercase();
    if cc.len() == 2 && cc.bytes().all(|b| b.is_ascii_alphabetic()) {
        Ok(cc)
    } else {
        Err(ApiError::new(
            ErrorCode::Invalid,
            "A country is two letters, such as UA or PL.",
        ))
    }
}

fn net_err(what: &str, e: impl std::fmt::Display) -> ApiError {
    ApiError::new(
        ErrorCode::Io,
        format!("{what}: {e}. Check the internet connection and try again."),
    )
}

fn io_err(what: &str, e: impl std::fmt::Display) -> ApiError {
    ApiError::new(ErrorCode::Io, format!("{what}: {e}."))
}

fn client(total: Duration) -> Result<reqwest::blocking::Client, ApiError> {
    reqwest::blocking::Client::builder()
        .user_agent("family-tree-gazetteer")
        .connect_timeout(Duration::from_secs(20))
        .timeout(total)
        .build()
        .map_err(|e| net_err("Could not start the download", e))
}

/// The size of a country's zip, from a HEAD request (`None` when the server does not say).
pub fn remote_size(cc: &str) -> Result<Option<u64>, ApiError> {
    let cc = country_code(cc)?;
    let resp = client(Duration::from_secs(30))?
        .head(format!("{}/{cc}.zip", base()))
        .send()
        .map_err(|e| net_err("Could not reach GeoNames", e))?;
    if !resp.status().is_success() {
        return Err(ApiError::new(
            ErrorCode::NotFound,
            format!("GeoNames has no file for {cc}."),
        ));
    }
    Ok(resp.content_length())
}

/// Downloads and indexes one country. `progress` gets the download (bytes) and then the indexing phase.
pub fn download(
    config_dir: &Path,
    cc: &str,
    progress: &dyn Fn(GazetteerProgressDto),
) -> Result<GazetteerCountryDto, ApiError> {
    let cc = country_code(cc)?;
    let folder = dir(config_dir);
    std::fs::create_dir_all(&folder).map_err(|e| io_err("Could not create the place data folder", e))?;
    let part = folder.join(format!("{cc}.zip.part"));
    let result = download_to(&cc, &part, progress).and_then(|()| index_zip(&folder, &cc, &part, progress));
    let _ = std::fs::remove_file(&part);
    result
}

fn download_to(cc: &str, part: &Path, progress: &dyn Fn(GazetteerProgressDto)) -> Result<(), ApiError> {
    let mut resp = client(Duration::from_secs(60 * 60))?
        .get(format!("{}/{cc}.zip", base()))
        .send()
        .map_err(|e| net_err("Could not reach GeoNames", e))?;
    if !resp.status().is_success() {
        return Err(ApiError::new(
            ErrorCode::NotFound,
            format!("GeoNames has no file for {cc}."),
        ));
    }
    let total = resp.content_length().unwrap_or(0);
    if total > MAX_BYTES {
        return Err(ApiError::new(ErrorCode::Invalid, "That file is larger than expected."));
    }
    let mut file = std::fs::File::create(part).map_err(|e| io_err("Could not write the download", e))?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    let mut last = Instant::now() - Duration::from_secs(1);
    loop {
        let n = resp.read(&mut buf).map_err(|e| net_err("The download stopped", e))?;
        if n == 0 {
            break;
        }
        file.write_all(buf.get(..n).unwrap_or_default())
            .map_err(|e| io_err("Could not write the download", e))?;
        done += n as u64;
        if done > MAX_BYTES {
            return Err(ApiError::new(ErrorCode::Invalid, "That file is larger than expected."));
        }
        if last.elapsed() >= Duration::from_millis(120) {
            last = Instant::now();
            progress(GazetteerProgressDto {
                country: cc.to_string(),
                phase: "download".into(),
                done,
                total,
            });
        }
    }
    file.flush().map_err(|e| io_err("Could not write the download", e))?;
    progress(GazetteerProgressDto {
        country: cc.to_string(),
        phase: "download".into(),
        done,
        total: done.max(total),
    });
    Ok(())
}

/// Converts a downloaded zip into the country's index file.
pub fn index_zip(
    folder: &Path,
    cc: &str,
    zip_path: &Path,
    progress: &dyn Fn(GazetteerProgressDto),
) -> Result<GazetteerCountryDto, ApiError> {
    progress(GazetteerProgressDto {
        country: cc.to_string(),
        phase: "index".into(),
        done: 0,
        total: 1,
    });
    let file = std::fs::File::open(zip_path).map_err(|e| io_err("Could not read the download", e))?;
    let index = Index::from_zip(cc, file).map_err(|e| {
        ApiError::new(
            ErrorCode::Invalid,
            format!("The downloaded file could not be used: {e}."),
        )
    })?;
    let path = index_path(folder, cc);
    index
        .save(&path)
        .map_err(|e| io_err("Could not save the place data", e))?;
    progress(GazetteerProgressDto {
        country: cc.to_string(),
        phase: "index".into(),
        done: 1,
        total: 1,
    });
    Ok(GazetteerCountryDto {
        country: cc.to_string(),
        rows: u32::try_from(index.len()).unwrap_or(u32::MAX),
        bytes: std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
    })
}

/// Deletes a country's index.
pub fn remove(config_dir: &Path, cc: &str) -> Result<(), ApiError> {
    let cc = country_code(cc)?;
    match std::fs::remove_file(index_path(&dir(config_dir), &cc)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_err("Could not remove the place data", e)),
    }
}

/// The loaded indexes (every country in the folder).
pub fn load(config_dir: &Path) -> Arc<Gazetteer> {
    Arc::new(Gazetteer::load_dir(&dir(config_dir)).0)
}

/// Sizes of what is downloaded, without loading the indexes.
pub fn status(config_dir: &Path) -> GazetteerStatusDto {
    let mut countries = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir(config_dir)) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("gtgz") {
                continue;
            }
            let Some(cc) = p.file_stem().and_then(|s| s.to_str()).map(str::to_string) else {
                continue;
            };
            let rows = gt_gazetteer::index::peek_rows(&p).unwrap_or(0);
            countries.push(GazetteerCountryDto {
                country: cc,
                rows: u32::try_from(rows).unwrap_or(u32::MAX),
                bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
            });
        }
    }
    countries.sort_by(|a, b| a.country.cmp(&b.country));
    GazetteerStatusDto {
        countries,
        attribution: gt_gazetteer::ATTRIBUTION.to_string(),
    }
}
