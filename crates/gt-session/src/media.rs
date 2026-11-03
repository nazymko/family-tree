//! Media pipeline: attaching files (content sniffing, one undoable change set), serving bytes
//! for the `gtmedia:` protocol, EXIF suggestions and image sizes.
//!
//! Protocol rule for the shell: `gtmedia://localhost/<media-uuid>/<thumb|preview|original>`
//! (Windows/Android: `http://gtmedia.localhost/...`); only the id is read from the URL. Replies
//! carry `Content-Type` and `Cache-Control: private, max-age=31536000, immutable`.

use crate::command::OwnerRef;
use crate::dto::MediaDto;
use crate::error::{ApiError, ErrorCode};
use crate::session::Session;
use crate::thumbs::{self, ThumbSize};
use gt_core::model::{Media, MediaLink};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use ts_rs::TS;
use uuid::Uuid;

/// Which rendition `media_bytes` returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaSize {
    Thumb,
    Preview,
    Original,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExifSuggestion {
    /// GEDCOM style, e.g. "12 MAR 1901".
    pub date: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AttachFailure {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AttachResult {
    pub attached: Vec<MediaDto>,
    pub failed: Vec<AttachFailure>,
}

/// File types the pipeline accepts, found from the first bytes only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Jpeg,
    Png,
    WebP,
    Gif,
    Tiff,
    Pdf,
}

impl Kind {
    pub(crate) fn mime(self) -> &'static str {
        match self {
            Kind::Jpeg => "image/jpeg",
            Kind::Png => "image/png",
            Kind::WebP => "image/webp",
            Kind::Gif => "image/gif",
            Kind::Tiff => "image/tiff",
            Kind::Pdf => "application/pdf",
        }
    }

    pub(crate) fn is_image(self) -> bool {
        self != Kind::Pdf
    }
}

/// Detects the type by magic bytes; the file name is never consulted.
pub(crate) fn sniff(head: &[u8]) -> Option<Kind> {
    if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Kind::Jpeg)
    } else if head.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some(Kind::Png)
    } else if head.len() >= 12 && head.starts_with(b"RIFF") && head.get(8..12) == Some(b"WEBP") {
        Some(Kind::WebP)
    } else if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        Some(Kind::Gif)
    } else if head.starts_with(&[b'I', b'I', 0x2A, 0x00])
        || head.starts_with(&[b'M', b'M', 0x00, 0x2A])
    {
        Some(Kind::Tiff)
    } else if head.starts_with(b"%PDF-") {
        Some(Kind::Pdf)
    } else {
        None
    }
}

fn read_head(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut head = Vec::with_capacity(16);
    std::fs::File::open(path)?.take(16).read_to_end(&mut head)?;
    Ok(head)
}

/// Image sizes already read, per media id (only for files that were found).
pub(crate) type DimsCache = RefCell<HashMap<Uuid, Option<(u32, u32)>>>;

struct Staged {
    media: Media,
    too_large: bool,
    src: String,
}

fn fail(failed: &mut Vec<AttachFailure>, path: &Path, reason: &str) {
    failed.push(AttachFailure {
        path: path.to_string_lossy().into_owned(),
        reason: reason.into(),
    });
}

fn raw_dims(path: &Path) -> Option<(u32, u32)> {
    image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

impl Session {
    /// The file behind a stored media path, only when it lies inside `<project>/media`.
    /// Lexical check first (no filesystem call before it), then a symlink-resolving check.
    pub(crate) fn resolve_media_file(&self, stored: &str) -> Option<PathBuf> {
        let root = self.project.dir.join("media");
        let cand = self.project.dir.join(stored);
        if stored.is_empty() || !gt_gedcom::is_lexically_under(&cand, std::slice::from_ref(&root)) {
            return None;
        }
        let real = cand.canonicalize().ok()?;
        let real_root = root.canonicalize().ok()?;
        (real.starts_with(&real_root) && real.is_file()).then_some(real)
    }

    /// Width and height as displayed, when the file is an image that can be measured.
    pub(crate) fn media_dims(&self, id: Uuid, stored: &str) -> (Option<u32>, Option<u32>) {
        if let Some(hit) = self.media_dims.borrow().get(&id) {
            return hit.map_or((None, None), |(w, h)| (Some(w), Some(h)));
        }
        let Some(file) = self.resolve_media_file(stored) else {
            return (None, None);
        };
        let dims = thumbs::image_dims(&file);
        self.media_dims.borrow_mut().insert(id, dims);
        dims.map_or((None, None), |(w, h)| (Some(w), Some(h)))
    }

    /// Copies the files into the project and links them to `target` as ONE undoable change set
    /// ("Attach media"). Files that cannot be attached are listed with a reason; the rest still
    /// go through. A very large image is stored and attached but also listed as having no
    /// preview.
    pub fn attach_files(
        &mut self,
        target: OwnerRef,
        paths: &[PathBuf],
    ) -> Result<AttachResult, ApiError> {
        let (_, now) = (self.clock.0)();
        let mut failed = Vec::new();
        let mut staged: Vec<Staged> = Vec::new();
        for path in paths {
            let Ok(meta) = std::fs::metadata(path) else {
                fail(&mut failed, path, "cannot read file");
                continue;
            };
            if meta.is_dir() {
                fail(
                    &mut failed,
                    path,
                    "folders are not attached; drop the files inside",
                );
                continue;
            }
            if meta.len() == 0 {
                fail(&mut failed, path, "file is empty");
                continue;
            }
            let Ok(head) = read_head(path) else {
                fail(&mut failed, path, "cannot read file");
                continue;
            };
            let Some(kind) = sniff(&head) else {
                fail(&mut failed, path, "unsupported type");
                continue;
            };
            let Ok((rel, sha)) = self.project.add_media_file(path) else {
                fail(&mut failed, path, "cannot read file");
                continue;
            };
            let stored = self.project.dir.join(&rel);
            let too_large = kind.is_image()
                && raw_dims(&stored)
                    .is_some_and(|(w, h)| u64::from(w) * u64::from(h) > thumbs::MAX_PIXELS);
            staged.push(Staged {
                media: Media {
                    common: crate::ops::new_common(now),
                    path: rel,
                    original_name: path.file_name().map(|n| n.to_string_lossy().into_owned()),
                    mime: Some(kind.mime().into()),
                    title: None,
                    date: None,
                    description: None,
                    sha256: Some(sha),
                },
                too_large,
                src: path.to_string_lossy().into_owned(),
            });
        }

        let order = self.day_order();
        let records: Vec<Media> = staged.iter().map(|s| s.media.clone()).collect();
        let (cs, outcomes) = crate::ops::plan_attach(&self.tree, target, records, now, order)?;
        if !cs.changes.is_empty() {
            self.commit_group(cs)?;
        }
        let mut attached = Vec::new();
        for (s, linked) in staged.iter().zip(outcomes) {
            let Some(id) = linked else {
                failed.push(AttachFailure {
                    path: s.src.clone(),
                    reason: "already attached".into(),
                });
                continue;
            };
            if s.too_large {
                failed.push(AttachFailure {
                    path: s.src.clone(),
                    reason: thumbs::TOO_LARGE.into(),
                });
            }
            let link = MediaLink {
                media: id,
                region: None,
                primary: false,
            };
            if let Some(dto) = self.media_dtos(&[link]).into_iter().next() {
                attached.push(dto.media);
            }
        }
        Ok(AttachResult { attached, failed })
    }

    /// Bytes and MIME type of one rendition. Looked up by media id only; the file must lie
    /// under `<project>/media`. Thumbnails and previews are JPEG, cached under `thumbs/`.
    pub fn media_bytes(&self, id: Uuid, size: MediaSize) -> Result<(Vec<u8>, String), ApiError> {
        let m = self
            .tree
            .media
            .get(&id)
            .ok_or_else(|| ApiError::not_found("media", id))?;
        let file = self
            .resolve_media_file(&m.path)
            .ok_or_else(|| ApiError::not_found("media file", id))?;
        let read = |f: &Path| {
            std::fs::read(f).map_err(|_| ApiError::new(ErrorCode::Io, "cannot read file"))
        };
        let thumb_size = match size {
            MediaSize::Original => {
                let bytes = read(&file)?;
                let kind = sniff(&bytes)
                    .ok_or_else(|| ApiError::new(ErrorCode::Invalid, "unsupported type"))?;
                return Ok((bytes, kind.mime().into()));
            }
            MediaSize::Thumb => ThumbSize::Thumb,
            MediaSize::Preview => ThumbSize::Preview,
        };
        let no_preview = || ApiError::new(ErrorCode::NotFound, "No preview for this file type.");
        if m.mime.as_deref() == Some("application/pdf") {
            return Err(no_preview());
        }
        let bytes =
            thumbs::cached_render(&self.project.dir, m.sha256.as_deref(), thumb_size, || {
                let bytes = read(&file)?;
                match sniff(&bytes) {
                    Some(k) if k.is_image() => Ok(bytes),
                    Some(_) => Err(no_preview()),
                    None => Err(ApiError::new(ErrorCode::Invalid, "unsupported type")),
                }
            })?;
        Ok((bytes, "image/jpeg".into()))
    }

    /// Capture date and GPS position from the picture's EXIF data, when it has any.
    pub fn exif_suggestion(&self, id: Uuid) -> Result<Option<ExifSuggestion>, ApiError> {
        let m = self
            .tree
            .media
            .get(&id)
            .ok_or_else(|| ApiError::not_found("media", id))?;
        let file = self
            .resolve_media_file(&m.path)
            .ok_or_else(|| ApiError::not_found("media file", id))?;
        let Ok(f) = std::fs::File::open(file) else {
            return Err(ApiError::new(ErrorCode::Io, "cannot read file"));
        };
        Ok(read_exif(&mut std::io::BufReader::new(f)))
    }
}

fn ascii(f: &exif::Field) -> Option<String> {
    match &f.value {
        exif::Value::Ascii(v) => v.first().map(|b| {
            String::from_utf8_lossy(b)
                .trim_end_matches('\0')
                .trim()
                .to_string()
        }),
        _ => None,
    }
}

/// "1901:03:12 14:00:00" -> "12 MAR 1901".
fn gedcom_date(s: &str) -> Option<String> {
    const MONTHS: [&str; 12] = [
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];
    let mut it = s.split([' ', ':']);
    let y: u32 = it.next()?.parse().ok()?;
    let m: usize = it.next()?.parse().ok()?;
    let d: u32 = it.next()?.parse().ok()?;
    if y == 0 || !(1..=31).contains(&d) {
        return None;
    }
    Some(format!("{d} {} {y}", MONTHS.get(m.checked_sub(1)?)?))
}

fn degrees(f: &exif::Field) -> Option<f64> {
    let exif::Value::Rational(v) = &f.value else {
        return None;
    };
    let part = |i: usize, div: f64| {
        let r = v.get(i)?;
        (r.denom != 0).then(|| r.to_f64() / div)
    };
    Some(part(0, 1.0)? + part(1, 60.0).unwrap_or(0.0) + part(2, 3600.0).unwrap_or(0.0))
}

fn read_exif<R: std::io::BufRead + std::io::Seek>(src: &mut R) -> Option<ExifSuggestion> {
    let ex = exif::Reader::new().read_from_container(src).ok()?;
    let get = |t| ex.get_field(t, exif::In::PRIMARY);
    let date = get(exif::Tag::DateTimeOriginal)
        .or_else(|| get(exif::Tag::DateTimeDigitized))
        .and_then(ascii)
        .and_then(|s| gedcom_date(&s));
    let signed = |tag, reference, max: f64, neg: &str| {
        let v = degrees(get(tag)?)?;
        let negative = get(reference)
            .and_then(ascii)
            .is_some_and(|r| r.eq_ignore_ascii_case(neg));
        let v = if negative { -v } else { v };
        (v.is_finite() && v.abs() <= max).then_some(v)
    };
    let lat = signed(exif::Tag::GPSLatitude, exif::Tag::GPSLatitudeRef, 90.0, "S");
    let lon = signed(
        exif::Tag::GPSLongitude,
        exif::Tag::GPSLongitudeRef,
        180.0,
        "W",
    );
    let (lat, lon) = if lat.is_some() && lon.is_some() {
        (lat, lon)
    } else {
        (None, None)
    };
    (date.is_some() || lat.is_some()).then_some(ExifSuggestion { date, lat, lon })
}
