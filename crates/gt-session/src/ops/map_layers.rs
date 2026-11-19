//! Old scanned maps laid on the Map: a picture's map layer, set, changed, taken off, and the order
//! of the stack.

use super::{Planner, can_turn, invalid, text};
use crate::command::{MapCorner, MapLayerInput};
use crate::error::ApiError;
use gt_core::model::{GeoPoint, MAP_LAYER_TEXT_MAX, MapLayer, Media};
use gt_core::tree::Tree;
use uuid::Uuid;

/// The most old maps one tree lays on the Map.
pub const MAP_LAYERS_MAX: usize = 500;

/// A short text of a layer: trimmed, empty is none, at most [`MAP_LAYER_TEXT_MAX`] bytes.
fn layer_text(given: Option<&str>) -> Result<Option<String>, ApiError> {
    let t = text(given);
    if t.as_ref().is_some_and(|t| t.len() > MAP_LAYER_TEXT_MAX) {
        return Err(invalid(format!(
            "The series, year and source of a map are at most {MAP_LAYER_TEXT_MAX} characters each."
        )));
    }
    Ok(t)
}

/// The four corners as the tree keeps them, checked: exactly four points on the Earth, and not
/// all on one spot.
pub fn corners_of(given: &[MapCorner]) -> Result<[GeoPoint; 4], ApiError> {
    let points: Vec<GeoPoint> = given
        .iter()
        .map(|c| GeoPoint {
            lat_e6: c.lat_e6,
            lon_e6: c.lon_e6,
        })
        .collect();
    let Ok(four) = <[GeoPoint; 4]>::try_from(points) else {
        return Err(invalid(
            "A map is placed by its four corners, from the top left clockwise.",
        ));
    };
    if !four.iter().all(GeoPoint::is_valid) {
        return Err(invalid(
            "A corner is off the Earth: latitude is -90 to 90 and longitude -180 to 180.",
        ));
    }
    let same = |f: fn(&GeoPoint) -> i32| four.iter().all(|p| f(p) == f(&four[0]));
    if same(|p| p.lat_e6) || same(|p| p.lon_e6) {
        return Err(invalid(
            "The corners lie on one line: a map needs some width and height.",
        ));
    }
    Ok(four)
}

/// The next place on top of the stack.
pub fn next_pos(tree: &Tree) -> i32 {
    tree.media
        .values()
        .filter_map(|m| m.map_layer.as_ref().map(|l| l.pos))
        .max()
        .map_or(0, |p| p.saturating_add(1))
}

/// A layer as the input says, keeping the place in the stack of `old` (or on top for a new one).
pub fn layer_of(
    input: &MapLayerInput,
    old: Option<&MapLayer>,
    top: i32,
) -> Result<MapLayer, ApiError> {
    if input.opacity > 100 {
        return Err(invalid("Opacity is 0 to 100."));
    }
    Ok(MapLayer {
        series: layer_text(input.series.as_deref())?,
        year: layer_text(input.year.as_deref())?,
        source: layer_text(input.source.as_deref())?,
        corners: match &input.corners {
            Some(c) => Some(corners_of(c)?),
            None => None,
        },
        opacity: input.opacity,
        visible: input.visible,
        pos: old.map_or(top, |l| l.pos),
    })
}

/// Whether a media record can be laid on the Map: a picture, not a recording or a document.
pub fn can_lay(m: &Media) -> bool {
    can_turn(m) && m.mime.as_deref().is_none_or(|t| !t.starts_with("video/"))
}

pub(super) fn set_layer(
    p: &mut Planner,
    id: Uuid,
    input: Option<&MapLayerInput>,
    title: Option<&str>,
) -> Result<Option<Uuid>, ApiError> {
    let mut m: Media = p.get(id)?;
    match input {
        Some(input) => {
            if !can_lay(&m) {
                return Err(invalid("Only a picture can be laid on the Map."));
            }
            if m.map_layer.is_none() {
                let layers = p
                    .tree
                    .media
                    .values()
                    .filter(|m| m.map_layer.is_some())
                    .count();
                if layers >= MAP_LAYERS_MAX {
                    return Err(invalid(format!(
                        "A tree holds at most {MAP_LAYERS_MAX} maps on the Map."
                    )));
                }
            }
            let top = next_pos(p.tree);
            m.map_layer = Some(layer_of(input, m.map_layer.as_ref(), top)?);
        }
        None => m.map_layer = None,
    }
    if let Some(t) = title {
        m.title = text(Some(t));
    }
    m.common.changed = Some(p.now);
    p.put(m);
    Ok(None)
}

/// A new picture laid on the Map in one change set: `staged` is the record of a file just copied
/// in. When the tree holds the same content already, that record is laid on the Map instead (its
/// title stays unless it has none). Returns the plan and the media record's id; the plan is
/// empty when that record lies on the Map already.
pub(crate) fn plan_new_layer(
    tree: &Tree,
    staged: Media,
    input: &MapLayerInput,
    now: i64,
    order: gt_core::date::DayOrder,
) -> Result<(gt_store::ChangeSet, Uuid), ApiError> {
    let mut p = Planner::new(tree, now, order);
    let same = staged.sha256.as_deref().and_then(|sha| {
        tree.media
            .values()
            .find(|m| m.sha256.as_deref() == Some(sha))
            .map(|m| m.common.id)
    });
    let id = match same {
        Some(id) => {
            let old: Media = p.get(id)?;
            if old.map_layer.is_some() {
                return Ok((p.finish("Lay map on the Map".into()), id));
            }
            let title = if old.title.is_none() {
                staged.title.clone()
            } else {
                None
            };
            set_layer(&mut p, id, Some(input), title.as_deref())?;
            id
        }
        None => {
            let id = staged.common.id;
            if !can_lay(&staged) {
                return Err(invalid("Only a picture can be laid on the Map."));
            }
            p.put(staged);
            set_layer(&mut p, id, Some(input), None)?;
            id
        }
    };
    Ok((p.finish("Lay map on the Map".into()), id))
}

pub(super) fn reorder(p: &mut Planner, order: &[Uuid]) -> Result<Option<Uuid>, ApiError> {
    let mut layers: Vec<Uuid> = p
        .tree
        .media
        .values()
        .filter(|m| m.map_layer.is_some())
        .map(|m| m.common.id)
        .collect();
    let mut given = order.to_vec();
    layers.sort();
    given.sort();
    if layers != given {
        return Err(invalid(
            "Give every map on the Map once, from the lowest to the top.",
        ));
    }
    for (i, id) in order.iter().enumerate() {
        let mut m: Media = p.get(*id)?;
        let pos = i32::try_from(i).unwrap_or(i32::MAX);
        if let Some(l) = m.map_layer.as_mut()
            && l.pos != pos
        {
            l.pos = pos;
            m.common.changed = Some(p.now);
            p.put(m);
        }
    }
    Ok(None)
}
