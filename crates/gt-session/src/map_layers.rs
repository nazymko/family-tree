//! Old scanned maps on the Map: the owner adds a picture as a map, and the Map reads every map
//! with where it lies.

use crate::command::{MapCorner, MapLayerInput, OwnerKind};
use crate::error::{ApiError, ErrorCode};
use crate::media::{PreparedAttach, prepare_attach};
use crate::session::Session;
use gt_core::model::Media;
use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;
use uuid::Uuid;

/// One old map on the Map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MapLayerDto {
    /// The picture's media record.
    pub media: Uuid,
    pub title: Option<String>,
    pub series: Option<String>,
    pub year: Option<String>,
    pub source: Option<String>,
    /// Top left, top right, bottom right, bottom left of the picture; absent while not placed.
    pub corners: Option<Vec<MapCorner>>,
    /// 0 to 100.
    pub opacity: u8,
    pub visible: bool,
    /// The place in the stack; a higher one lies over a lower one.
    pub pos: i32,
    /// The picture's size in pixels, upright by EXIF, when it can be read.
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// Step one of adding a map: copy the picture into `<dir>/media`, without the session. Only a
/// picture is taken.
pub fn prepare_map_file(dir: &Path, path: &Path) -> Result<PreparedAttach, ApiError> {
    let prepared = prepare_attach(dir, OwnerKind::Source, &[path.to_path_buf()]);
    if let Some(f) = prepared.failed.first() {
        return Err(ApiError::new(ErrorCode::Invalid, f.reason.clone()));
    }
    match prepared.files.first() {
        Some(f) if f.mime.starts_with("image/") => Ok(prepared),
        _ => Err(ApiError::new(
            ErrorCode::Invalid,
            "Only a picture (JPEG, PNG, WebP, GIF or TIFF) can be laid on the Map.",
        )),
    }
}

/// The corners an OziExplorer `.map` file gives a picture. The picture's own size is read when
/// the file does not name it.
pub fn ozi_corners(map: &Path, picture: &Path) -> Result<crate::ozi::OziCorners, ApiError> {
    crate::ozi::read_ozi_map_file(map, crate::thumbs::image_dims(picture))
}

/// The OziExplorer `.map` file lying next to a picture, if there is one: a `.map` in the same
/// folder that names the picture's file on its second line, or else one whose name starts with
/// the picture's name ("30-13.jpg" and "30-13_ozf.map").
pub fn find_ozi_map(picture: &Path) -> Option<std::path::PathBuf> {
    let dir = picture.parent()?;
    let name = picture.file_name()?.to_string_lossy().to_lowercase();
    let stem = picture.file_stem()?.to_string_lossy().to_lowercase();
    let maps: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .take(5000)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("map")) && p.is_file())
        .collect();
    let names_picture = |p: &Path| {
        let Ok(meta) = std::fs::metadata(p) else {
            return false;
        };
        if meta.len() > crate::ozi::OZI_MAP_MAX {
            return false;
        }
        let Ok(bytes) = std::fs::read(p) else {
            return false;
        };
        let text = String::from_utf8_lossy(&bytes);
        let second = text
            .lines()
            .nth(1)
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        let second = second.rsplit(['\\', '/']).next().unwrap_or_default();
        text.starts_with("OziExplorer") && second == name
    };
    if let Some(p) = maps.iter().take(200).find(|p| names_picture(p)) {
        return Some(p.clone());
    }
    maps.into_iter().find(|p| {
        p.file_stem().is_some_and(|s| {
            let s = s.to_string_lossy().to_lowercase();
            // "30-13_ozf" goes with "30-13", never with "30-1".
            s.strip_prefix(&stem)
                .is_some_and(|rest| rest.chars().next().is_none_or(|c| !c.is_alphanumeric()))
        })
    })
}

/// The input that keeps a layer as it is.
pub fn input_of(l: &gt_core::model::MapLayer) -> MapLayerInput {
    MapLayerInput {
        series: l.series.clone(),
        year: l.year.clone(),
        source: l.source.clone(),
        corners: l.corners.map(|c| {
            c.iter()
                .map(|p| MapCorner {
                    lat_e6: p.lat_e6,
                    lon_e6: p.lon_e6,
                })
                .collect()
        }),
        opacity: l.opacity,
        visible: l.visible,
    }
}

impl Session {
    /// Every old map on the Map, the lowest first.
    pub fn map_layers(&self) -> Vec<MapLayerDto> {
        let mut out: Vec<MapLayerDto> = self
            .tree
            .media
            .values()
            .filter_map(|m| {
                let l = m.map_layer.as_ref()?;
                let (width, height) = self.media_dims(m.common.id, &m.path);
                let input = input_of(l);
                Some(MapLayerDto {
                    media: m.common.id,
                    title: m.title.clone(),
                    series: input.series,
                    year: input.year,
                    source: input.source,
                    corners: input.corners,
                    opacity: l.opacity,
                    visible: l.visible,
                    pos: l.pos,
                    width,
                    height,
                })
            })
            .collect();
        out.sort_by_key(|l| (l.pos, l.media));
        out
    }

    /// Step two of adding a map: records the picture [`prepare_map_file`] copied in and lays it on
    /// the Map, as one change the owner can undo. A picture already in the tree is laid on the
    /// Map instead of being added twice. Returns the media record's id.
    pub fn commit_map_layer(
        &mut self,
        prepared: PreparedAttach,
        title: Option<String>,
        layer: &MapLayerInput,
    ) -> Result<Uuid, ApiError> {
        if prepared.dir != self.project.dir {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "The project changed while the file was being copied; nothing was added.",
            ));
        }
        let Some(f) = prepared.files.into_iter().next() else {
            return Err(ApiError::new(ErrorCode::Invalid, "No file was copied."));
        };
        let (_, now) = (self.clock.0)();
        let media = Media {
            version_of: None,
            map_layer: None,
            place: None,
            common: crate::ops::new_common(now),
            path: f.rel,
            original_name: f.original_name,
            mime: Some(f.mime.into()),
            title: crate::ops::text(title.as_deref()),
            date: None,
            description: None,
            sha256: Some(f.sha),
            transcription: Vec::new(),
            rotation: None,
        };
        let (cs, id) = crate::ops::map_layers::plan_new_layer(
            &self.tree,
            media,
            layer,
            now,
            self.day_order(),
        )?;
        if !cs.changes.is_empty() {
            self.commit_group(cs)?;
        }
        Ok(id)
    }

    /// Both steps of adding a map under one `&mut self` (tests and tools; the app copies first).
    pub fn add_map_layer_file(
        &mut self,
        path: &Path,
        title: Option<String>,
        layer: &MapLayerInput,
    ) -> Result<Uuid, ApiError> {
        let prepared = prepare_map_file(&self.project.dir, path)?;
        self.commit_map_layer(prepared, title, layer)
    }
}
