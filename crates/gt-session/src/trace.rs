//! The "trace a tree picture" work: the picture being traced, the saved draft (both live inside
//! the project, so closing and reopening the tree keeps them) and the step that adds a draft.
//!
//! The picture is copied to `<project>/trace/picture.<ext>`; the draft text is kept in the
//! project's `meta` table under `trace_draft`. Neither is part of the undo history.

use crate::command::{DraftFamily, DraftPerson};
use crate::error::{ApiError, ErrorCode};
use crate::media::{Kind, sniff};
use crate::ops::draft::StagedPicture;
use crate::session::Session;
use crate::thumbs;
use gt_core::model::Media;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use ts_rs::TS;

const DRAFT_KEY: &str = "trace_draft";
const IMAGE_NAME_KEY: &str = "trace_image_name";
const IMAGE_FILE_KEY: &str = "trace_image_file";
/// A saved draft is a few kilobytes of text; this stops a runaway caller.
const MAX_DRAFT_BYTES: usize = 8 * 1024 * 1024;
const MAX_PICTURE_BYTES: u64 = 200 * 1024 * 1024;

/// The picture that is being traced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TraceImageInfo {
    /// File name as picked, for display.
    pub name: String,
    #[ts(type = "number | null")]
    pub width: Option<u32>,
    #[ts(type = "number | null")]
    pub height: Option<u32>,
}

/// What is saved for the open tree: the draft text (opaque to Rust) and the picture, if any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TraceDraftState {
    pub json: Option<String>,
    pub image: Option<TraceImageInfo>,
}

fn ext(kind: Kind) -> &'static str {
    match kind {
        Kind::Jpeg => "jpg",
        Kind::Png => "png",
        Kind::WebP => "webp",
        Kind::Gif => "gif",
        Kind::Tiff => "tiff",
        Kind::Pdf => "pdf",
        // Only pictures are staged; sound never reaches here.
        _ => "bin",
    }
}

impl Session {
    fn trace_dir(&self) -> PathBuf {
        self.project.dir.join("trace")
    }

    /// The stored picture file, when the recorded name is a plain file name inside `trace/`.
    fn trace_file(&self) -> Result<Option<PathBuf>, ApiError> {
        let Some(name) = self
            .project
            .meta_get(IMAGE_FILE_KEY)?
            .filter(|n| !n.is_empty())
        else {
            return Ok(None);
        };
        if name.contains(['/', '\\']) || name.starts_with('.') {
            return Ok(None);
        }
        let f = self.trace_dir().join(name);
        Ok(f.is_file().then_some(f))
    }

    fn trace_info(&self, file: &Path) -> Result<TraceImageInfo, ApiError> {
        let name = self
            .project
            .meta_get(IMAGE_NAME_KEY)?
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "picture".into());
        let dims = thumbs::image_dims(file);
        Ok(TraceImageInfo {
            name,
            width: dims.map(|d| d.0),
            height: dims.map(|d| d.1),
        })
    }

    /// Copies a picture into the project as the one being traced (it replaces the previous one).
    pub fn stage_trace_image(&mut self, path: &Path) -> Result<TraceImageInfo, ApiError> {
        let io = |_| ApiError::new(ErrorCode::Io, "Cannot read that file. Pick it again.");
        let meta = std::fs::metadata(path).map_err(io)?;
        if !meta.is_file() || meta.len() == 0 {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "That is not a picture file. Pick a JPG, PNG, WebP, TIFF or GIF.",
            ));
        }
        if meta.len() > MAX_PICTURE_BYTES {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "That picture is too large. Pick a smaller file.",
            ));
        }
        let mut head = Vec::with_capacity(16);
        std::fs::File::open(path)
            .and_then(|f| f.take(16).read_to_end(&mut head))
            .map_err(io)?;
        let kind = sniff(&head).filter(|k| k.is_image()).ok_or_else(|| {
            ApiError::new(
                ErrorCode::Invalid,
                "That is not a picture file. Pick a JPG, PNG, WebP, TIFF or GIF.",
            )
        })?;
        let dir = self.trace_dir();
        std::fs::create_dir_all(&dir).map_err(io)?;
        let file_name = format!("picture.{}", ext(kind));
        let dest = dir.join(&file_name);
        let part = dir.join(".part");
        std::fs::copy(path, &part).map_err(io)?;
        // Another extension from an earlier picture goes away.
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n.starts_with("picture.") && n != file_name {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        let _ = std::fs::remove_file(&dest);
        std::fs::rename(&part, &dest).map_err(io)?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "picture".into());
        self.project.meta_set(IMAGE_FILE_KEY, &file_name)?;
        self.project.meta_set(IMAGE_NAME_KEY, &name)?;
        self.trace_info(&dest)
    }

    /// Bytes and MIME type of the picture being traced.
    pub fn trace_image_bytes(&self) -> Result<(Vec<u8>, String), ApiError> {
        let file = self
            .trace_file()?
            .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "There is no picture to trace."))?;
        let bytes = std::fs::read(&file)
            .map_err(|_| ApiError::new(ErrorCode::Io, "Cannot read the picture."))?;
        let kind = sniff(&bytes)
            .filter(|k| k.is_image())
            .ok_or_else(|| ApiError::new(ErrorCode::Invalid, "unsupported type"))?;
        Ok((bytes, kind.mime().into()))
    }

    /// Saves the draft text. Replaces the earlier one.
    pub fn save_trace_draft(&mut self, json: &str) -> Result<(), ApiError> {
        if json.len() > MAX_DRAFT_BYTES {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "The trace is too large to save. Add part of it to the tree first.",
            ));
        }
        self.project.meta_set(DRAFT_KEY, json)?;
        Ok(())
    }

    pub fn load_trace_draft(&self) -> Result<TraceDraftState, ApiError> {
        let json = self.project.meta_get(DRAFT_KEY)?.filter(|j| !j.is_empty());
        let image = match self.trace_file()? {
            Some(f) => Some(self.trace_info(&f)?),
            None => None,
        };
        Ok(TraceDraftState { json, image })
    }

    /// Forgets the draft and deletes the picture copy.
    pub fn discard_trace_draft(&mut self) -> Result<(), ApiError> {
        self.project.meta_set(DRAFT_KEY, "")?;
        self.project.meta_set(IMAGE_FILE_KEY, "")?;
        self.project.meta_set(IMAGE_NAME_KEY, "")?;
        let _ = std::fs::remove_dir_all(self.trace_dir());
        Ok(())
    }

    /// Stores the traced picture as a media record (the file goes into the project's media
    /// folder, like any attached file).
    fn stage_picture_media(&mut self, now: i64) -> Result<StagedPicture, ApiError> {
        let file = self.trace_file()?.ok_or_else(|| {
            ApiError::new(
                ErrorCode::NotFound,
                "The picture is missing. Pick it again or untick keeping it.",
            )
        })?;
        let head = {
            let mut h = Vec::with_capacity(16);
            std::fs::File::open(&file)
                .and_then(|f| f.take(16).read_to_end(&mut h))
                .map_err(|_| ApiError::new(ErrorCode::Io, "Cannot read the picture."))?;
            h
        };
        let kind = sniff(&head).filter(|k| k.is_image()).ok_or_else(|| {
            ApiError::new(ErrorCode::Invalid, "The picture is not a supported type.")
        })?;
        let (rel, sha) = self.project.add_media_file(&file)?;
        let name = self
            .project
            .meta_get(IMAGE_NAME_KEY)?
            .filter(|n| !n.is_empty());
        Ok(StagedPicture {
            media: Media {
                common: crate::ops::new_common(now),
                path: rel,
                original_name: name,
                mime: Some(kind.mime().into()),
                title: None,
                date: None,
                description: None,
                sha256: Some(sha),
            },
        })
    }

    /// Plans `importDraft`. With a picture the draft is first checked without it, so a refused
    /// draft never copies a file.
    pub(crate) fn plan_import_draft(
        &mut self,
        persons: &[DraftPerson],
        families: &[DraftFamily],
        attach_picture: bool,
        now: i64,
    ) -> Result<(gt_store::ChangeSet, Option<uuid::Uuid>), ApiError> {
        let order = self.day_order();
        let checked = crate::ops::plan_draft(&self.tree, persons, families, None, now, order)?;
        if !attach_picture {
            return Ok(checked);
        }
        let pic = self.stage_picture_media(now)?;
        crate::ops::plan_draft(&self.tree, persons, families, Some(&pic), now, order)
    }
}
