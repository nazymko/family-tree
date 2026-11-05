//! Media pipeline: attaching files (content sniffing, one undoable change set), serving bytes
//! for the `gtmedia:` protocol, EXIF suggestions and image sizes.
//!
//! Protocol rule for the shell: `gtmedia://localhost/<media-uuid>/<thumb|preview|original>`
//! (Windows/Android: `http://gtmedia.localhost/...`); only the id is read from the URL. Replies
//! carry `Content-Type` and `Cache-Control: private, max-age=31536000, immutable`.

use crate::command::{OwnerKind, OwnerRef};
use crate::dto::{MediaDto, MediaLinkDto};
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

/// Why a picture or document is refused on a note.
pub(crate) const NOTE_SOUND_ONLY: &str = "only sound recordings can be attached to a note";

/// The most one ranged reply carries. A player asks again for the next piece, so a long
/// recording never has to sit in memory whole.
pub const MAX_RANGE_BYTES: u64 = 4 << 20;

/// One piece of an original file, for a `206 Partial Content` reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaSlice {
    pub bytes: Vec<u8>,
    pub mime: String,
    /// Offset of the first byte in the file.
    pub start: u64,
    /// Size of the whole file.
    pub total: u64,
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
    /// MPEG audio (layer III) or anything starting with an ID3 tag.
    Mp3,
    Wav,
    /// Ogg container: Vorbis, Opus or FLAC-in-Ogg.
    Ogg,
    /// MPEG-4 audio (`.m4a`).
    M4a,
    /// Raw AAC in ADTS frames.
    Aac,
    Flac,
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
            Kind::Mp3 => "audio/mpeg",
            Kind::Wav => "audio/wav",
            Kind::Ogg => "audio/ogg",
            Kind::M4a => "audio/mp4",
            Kind::Aac => "audio/aac",
            Kind::Flac => "audio/flac",
        }
    }

    pub(crate) fn is_audio(self) -> bool {
        matches!(
            self,
            Kind::Mp3 | Kind::Wav | Kind::Ogg | Kind::M4a | Kind::Aac | Kind::Flac
        )
    }

    pub(crate) fn is_image(self) -> bool {
        !self.is_audio() && self != Kind::Pdf
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
        sniff_audio(head)
    }
}

/// Audio containers, by their first 16 bytes. Each check wants more than a bare two-byte sync
/// so that an unrelated binary file is not taken for sound.
fn sniff_audio(head: &[u8]) -> Option<Kind> {
    if head.len() >= 12 && head.starts_with(b"RIFF") && head.get(8..12) == Some(b"WAVE") {
        Some(Kind::Wav)
    } else if head.starts_with(b"OggS") {
        Some(Kind::Ogg)
    } else if head.starts_with(b"fLaC") {
        Some(Kind::Flac)
    } else if head.starts_with(b"ID3") {
        Some(Kind::Mp3)
    } else if head.len() >= 12 && head.get(4..8) == Some(b"ftyp") {
        // Only the brands audio recorders write; a video or a HEIC picture is also `ftyp`.
        match head.get(8..12)? {
            b"M4A " | b"M4B " | b"M4P " | b"mp42" | b"isom" => Some(Kind::M4a),
            _ => None,
        }
    } else if let [0xFF, b1, b2, ..] = *head {
        if b1 & 0xF6 == 0xF0 {
            // ADTS: sync 0xFFF, layer bits 00; sampling-frequency index below 13.
            return ((b2 >> 2) & 0x0F < 13).then_some(Kind::Aac);
        }
        // MPEG audio frame: 11 sync bits, version not reserved, layer III, bitrate index
        // neither "free" nor "bad", sampling index not reserved.
        let version = (b1 >> 3) & 3;
        let layer = (b1 >> 1) & 3;
        let bitrate = b2 >> 4;
        let rate = (b2 >> 2) & 3;
        (b1 & 0xE0 == 0xE0
            && version != 1
            && layer == 1
            && bitrate != 0
            && bitrate != 0x0F
            && rate != 3)
            .then_some(Kind::Mp3)
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

/// Why a recording is refused on anything but a note.
pub(crate) const SOUND_ON_NOTE_ONLY: &str = "sound recordings can only be attached to a note";

struct PreparedFile {
    rel: String,
    sha: String,
    mime: &'static str,
    original_name: Option<String>,
    too_large: bool,
    src: String,
}

/// Files already copied into a project's media folder and described, waiting to be recorded by
/// [`Session::commit_attach`]. Made without the session, so a long copy and hash never holds it.
pub struct PreparedAttach {
    dir: PathBuf,
    files: Vec<PreparedFile>,
    failed: Vec<AttachFailure>,
}

impl PreparedAttach {
    /// The files refused so far (before anything was recorded).
    pub fn failed(&self) -> &[AttachFailure] {
        &self.failed
    }
}

/// Step one of attaching: look at each file's content, refuse what the owner cannot hold, copy
/// and hash the rest into `<dir>/media`. Needs only the project folder, never the session. A
/// file placed in the content store but never recorded (the project closed meanwhile) is a
/// harmless unreferenced copy of what the owner picked, found again by hash on the next try;
/// no half-written `.part` file survives.
pub fn prepare_attach(dir: &Path, target: OwnerKind, paths: &[PathBuf]) -> PreparedAttach {
    let mut failed = Vec::new();
    let mut files = Vec::new();
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
        // A note holds recordings only, and only a note holds them: a picture on a note, or a
        // recording on a person, would be attached and then shown nowhere sensible. Refused
        // here, before anything is copied.
        if target == OwnerKind::Note && !kind.is_audio() {
            fail(&mut failed, path, NOTE_SOUND_ONLY);
            continue;
        }
        if target != OwnerKind::Note && kind.is_audio() {
            fail(&mut failed, path, SOUND_ON_NOTE_ONLY);
            continue;
        }
        let Ok((rel, sha)) = gt_store::add_media_file_in(dir, path) else {
            fail(&mut failed, path, "cannot read file");
            continue;
        };
        let too_large = kind.is_image()
            && thumbs::raw_dims(&dir.join(&rel))
                .is_some_and(|(w, h)| u64::from(w) * u64::from(h) > thumbs::MAX_PIXELS);
        files.push(PreparedFile {
            rel,
            sha,
            mime: kind.mime(),
            original_name: path.file_name().map(|n| n.to_string_lossy().into_owned()),
            too_large,
            src: path.to_string_lossy().into_owned(),
        });
    }
    PreparedAttach {
        dir: dir.to_path_buf(),
        files,
        failed,
    }
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
        // Sound has no size, and the file is not opened to find that out.
        if self
            .tree
            .media
            .get(&id)
            .and_then(|m| m.mime.as_deref())
            .is_some_and(|m| m.starts_with("audio/"))
        {
            return (None, None);
        }
        let Some(file) = self.resolve_media_file(stored) else {
            return (None, None);
        };
        let dims = thumbs::image_dims(&file);
        self.media_dims.borrow_mut().insert(id, dims);
        dims.map_or((None, None), |(w, h)| (Some(w), Some(h)))
    }

    /// The project folder, for work that must run without the session (see [`prepare_attach`]).
    pub fn project_dir(&self) -> PathBuf {
        self.project.dir.clone()
    }

    /// Media records that no record links to: what is left after the note or person that held
    /// them was deleted (the record stays so that Undo can bring it back). Listed so the owner
    /// can still remove them.
    pub fn unlinked_media(&self) -> Vec<MediaDto> {
        use std::collections::HashSet;
        let mut linked: HashSet<Uuid> = HashSet::new();
        macro_rules! collect {
            ($($field:ident),*) => {$(
                for x in self.tree.$field.values() {
                    linked.extend(x.common.media.iter().map(|l| l.media));
                }
            )*};
        }
        collect!(
            persons,
            families,
            events,
            places,
            sources,
            repositories,
            citations,
            notes
        );
        let mut loose: Vec<MediaLinkDto> = self
            .tree
            .media
            .keys()
            .filter(|id| !linked.contains(id))
            .flat_map(|id| {
                self.media_dtos(&[MediaLink {
                    media: *id,
                    region: None,
                    primary: false,
                }])
            })
            .collect();
        loose.sort_by(|a, b| {
            (a.media.original_name.as_deref(), a.media.id)
                .cmp(&(b.media.original_name.as_deref(), b.media.id))
        });
        loose.into_iter().map(|l| l.media).collect()
    }

    /// Copies the files into the project and links them to `target` as ONE undoable change set
    /// ("Attach media"). Files that cannot be attached are listed with a reason; the rest still
    /// go through. A very large image is stored and attached but also listed as having no
    /// preview. This runs both steps under one `&mut self`; the app calls
    /// [`prepare_attach`] first, without the session, so a long copy stalls nothing.
    pub fn attach_files(
        &mut self,
        target: OwnerRef,
        paths: &[PathBuf],
    ) -> Result<AttachResult, ApiError> {
        let prepared = prepare_attach(&self.project.dir, target.kind, paths);
        self.commit_attach(target, prepared)
    }

    /// The second step: records what [`prepare_attach`] stored. Refuses, recording nothing, when
    /// the open project is not the one the files were copied into.
    pub fn commit_attach(
        &mut self,
        target: OwnerRef,
        prepared: PreparedAttach,
    ) -> Result<AttachResult, ApiError> {
        if prepared.dir != self.project.dir {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "The project changed while the files were being copied; nothing was attached.",
            ));
        }
        let (_, now) = (self.clock.0)();
        let PreparedAttach {
            files, mut failed, ..
        } = prepared;
        let staged: Vec<Staged> = files
            .into_iter()
            .map(|f| Staged {
                media: Media {
                    common: crate::ops::new_common(now),
                    path: f.rel,
                    original_name: f.original_name,
                    mime: Some(f.mime.into()),
                    title: None,
                    date: None,
                    description: None,
                    sha256: Some(f.sha),
                },
                too_large: f.too_large,
                src: f.src,
            })
            .collect();

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

    /// The original file of one media item, for handing to the OS viewer. Looked up by id only;
    /// the file must lie under `<project>/media` (lexical check, then symlink-resolving check).
    pub fn media_original_path(&self, id: Uuid) -> Result<PathBuf, ApiError> {
        let m = self
            .tree
            .media
            .get(&id)
            .ok_or_else(|| ApiError::not_found("media", id))?;
        self.resolve_media_file(&m.path)
            .ok_or_else(|| ApiError::not_found("media file", id))
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
            // Whole file in memory: the shell (Task 26) is expected to stream big originals.
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
        if m.mime.as_deref() == Some("application/pdf")
            || m.mime.as_deref().is_some_and(|t| t.starts_with("audio/"))
        {
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

    /// The MIME type the file's own first bytes show, when the file is there and is a kind the
    /// pipeline knows. The stored type is not consulted: an imported record has free text such as
    /// `mp3`, or nothing.
    pub fn media_content_mime(&self, id: Uuid) -> Option<&'static str> {
        let m = self.tree.media.get(&id)?;
        let file = self.resolve_media_file(&m.path)?;
        sniff(&read_head(&file).ok()?).map(Kind::mime)
    }

    /// Whether this media is a sound recording. The file's bytes decide; when they are not a
    /// known kind (or the file is missing), a stored type or file name that says sound is enough,
    /// so that a recording is never treated as an ordinary file by a caller that must not send it
    /// anywhere.
    pub fn media_is_sound(&self, id: Uuid) -> bool {
        let Some(m) = self.tree.media.get(&id) else {
            return false;
        };
        if let Some(mime) = self.media_content_mime(id) {
            return mime.starts_with("audio/");
        }
        const WORDS: [&str; 10] = [
            "audio", "mp3", "mpeg", "wav", "ogg", "opus", "m4a", "aac", "flac", "sound",
        ];
        let mime = m.mime.as_deref().unwrap_or_default().to_lowercase();
        let ext = Path::new(&m.path)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        WORDS.iter().any(|w| mime.contains(w))
            || ["mp3", "wav", "ogg", "oga", "opus", "m4a", "aac", "flac"].contains(&ext.as_str())
    }

    /// The size in bytes of the original file, from its metadata alone (for a `416` reply).
    pub fn media_size(&self, id: Uuid) -> Result<u64, ApiError> {
        let m = self
            .tree
            .media
            .get(&id)
            .ok_or_else(|| ApiError::not_found("media", id))?;
        let file = self
            .resolve_media_file(&m.path)
            .ok_or_else(|| ApiError::not_found("media file", id))?;
        std::fs::metadata(file)
            .map(|md| md.len())
            .map_err(|_| ApiError::new(ErrorCode::Io, "cannot read file"))
    }

    /// A piece of the original file, `start..=end` (`end` open: as far as one reply may go),
    /// read without loading the rest. `None` when `start` is past the end of the file.
    pub fn media_range(
        &self,
        id: Uuid,
        start: u64,
        end: Option<u64>,
    ) -> Result<Option<MediaSlice>, ApiError> {
        use std::io::{Seek, SeekFrom};
        let m = self
            .tree
            .media
            .get(&id)
            .ok_or_else(|| ApiError::not_found("media", id))?;
        let file = self
            .resolve_media_file(&m.path)
            .ok_or_else(|| ApiError::not_found("media file", id))?;
        let io = |_| ApiError::new(ErrorCode::Io, "cannot read file");
        let mut f = std::fs::File::open(&file).map_err(io)?;
        let total = f.metadata().map_err(io)?.len();
        let mut head = Vec::with_capacity(16);
        (&mut f).take(16).read_to_end(&mut head).map_err(io)?;
        let kind =
            sniff(&head).ok_or_else(|| ApiError::new(ErrorCode::Invalid, "unsupported type"))?;
        if start >= total {
            return Ok(None);
        }
        let last = end.unwrap_or(u64::MAX).min(total - 1);
        let len = (last.saturating_sub(start) + 1).min(MAX_RANGE_BYTES);
        f.seek(SeekFrom::Start(start)).map_err(io)?;
        let mut bytes = Vec::new();
        f.take(len).read_to_end(&mut bytes).map_err(io)?;
        // The file shrank between the size and the read: nothing at `start` any more.
        if bytes.is_empty() {
            return Ok(None);
        }
        Ok(Some(MediaSlice {
            bytes,
            mime: kind.mime().into(),
            start,
            total,
        }))
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
        Ok(read_exif(f))
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

fn read_exif<R: std::io::Read>(src: R) -> Option<ExifSuggestion> {
    let ex = thumbs::read_exif_capped(src)?;
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
