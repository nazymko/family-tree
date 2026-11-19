//! Old scanned maps on the app's Map: adding one, reading where they lie, and placing one's own.

use crate::tools_read::{ToolResult, answer, invalid, str_arg, uuid_arg};
use crate::tools_write::{apply, batch_arg, short, with_batch, write};
use crate::{AgentCtx, Host, Tool};
use gt_session::map_layers::{input_of, ozi_corners};
use gt_session::{
    EditCommand, MapCorner, MapLayerInput, OwnerKind, Session, check_agent_path, prepare_agent_file,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(crate) fn tools<H: Host>() -> Vec<Tool<H>> {
    let text = |what: &str| json!({"type": "string", "description": what, "maxLength": 1000});
    let id = |what: &str| json!({"type": "string", "description": what});
    let corners = json!({
        "type": "array",
        "minItems": 4,
        "maxItems": 4,
        "description": "Where the picture's corners lie, in this order: top left, top right, bottom right, bottom left of the picture as it is upright, in decimal degrees (WGS 84).",
        "items": {
            "type": "object",
            "properties": {
                "lat": {"type": "number", "minimum": -90, "maximum": 90},
                "lon": {"type": "number", "minimum": -180, "maximum": 180},
            },
            "required": ["lat", "lon"],
            "additionalProperties": false,
        },
    });
    let ozi = text(
        "The full path of the OziExplorer .map file that came with the scan (as with the three-verst sheets): the corners are read from it, Pulkovo 1942 coordinates moved to WGS 84. Give this or corners, not both.",
    );
    vec![
        Tool {
            name: "add_map_layer",
            description: "Adds an old scanned map (a two- or three-verst sheet, a square of an old atlas: a JPEG, PNG, WebP, GIF or TIFF picture of at most 50 MiB) to the tree and lays it on the app's Map under the places, so the owner can see where the ancestors lived. The file is copied in; the original stays. Give the title, where the scan comes from (source: a web address or an archive reference), and if known the series and year. Give the corners only when you know them: from the OziExplorer .map file that came with the scan (ozi_map_path), or as four points you have read off the map's frame; otherwise leave them out and the owner places it. A guess is never a corner. The same picture again answers alreadyThere. The answer's id is the picture's media id.",
            schema: with_batch(
                &["file_path", "title", "source"],
                json!({
                    "file_path": text("The full path of the picture on this computer."),
                    "title": text("A title, such as Sheet 30-13, Three-verst map of the Taurida Governorate."),
                    "series": text("The series or atlas, such as Three-verst military topographic map."),
                    "year": text("The year or years the map shows, as written: 1865, or 1846-1863."),
                    "source": text("Where the scan comes from: a web address or an archive reference."),
                    "corners": corners.clone(),
                    "ozi_map_path": ozi.clone(),
                }),
            ),
            call: add_map_layer::<H>,
        },
        Tool {
            name: "list_map_layers",
            description: "The old maps laid on the app's Map, the lowest first: the media id, title, series, year, source, the four corners in decimal degrees (top left, top right, bottom right, bottom left; absent while nobody has placed it), opacity 0 to 100, whether it is shown, the picture's size, and yours: true on a map you laid there yourself that nobody has changed since (only those can you place again with set_own_map_layer_corners).",
            schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
            call: list_map_layers::<H>,
        },
        Tool {
            name: "set_own_map_layer_corners",
            description: "Places an old map you laid on the Map yourself with add_map_layer, when nobody has changed it since: its four corners, or the OziExplorer .map file that came with the scan. A map the owner or anyone else laid or changed stays as it is: say in your report where you think it lies. Saying again what it says answers alreadyThere.",
            schema: with_batch(
                &["media"],
                json!({
                    "media": id("The map's media id, from add_map_layer or list_map_layers."),
                    "corners": corners,
                    "ozi_map_path": ozi,
                }),
            ),
            call: set_own_map_layer_corners::<H>,
        },
    ]
}

/// Decimal degrees as the tree keeps them.
fn e6(v: f64) -> i32 {
    (v * 1e6).round() as i32
}

/// The `corners` argument, when given: four points in decimal degrees.
fn corners_arg(args: &Value) -> Result<Option<Vec<MapCorner>>, gt_session::ApiError> {
    let Some(given) = args.get("corners").filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    let list = given.as_array().filter(|l| l.len() == 4).ok_or_else(|| {
        invalid(
            "corners must be four points: top left, top right, bottom right, bottom left.".into(),
        )
    })?;
    list.iter()
        .map(|p| {
            let lat = p
                .get("lat")
                .and_then(Value::as_f64)
                .filter(|v| (-90.0..=90.0).contains(v));
            let lon = p
                .get("lon")
                .and_then(Value::as_f64)
                .filter(|v| (-180.0..=180.0).contains(v));
            match (lat, lon) {
                (Some(lat), Some(lon)) => Ok(MapCorner {
                    lat_e6: e6(lat),
                    lon_e6: e6(lon),
                }),
                _ => Err(invalid(
                    "every corner needs lat (-90 to 90) and lon (-180 to 180) in decimal degrees"
                        .into(),
                )),
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

/// The corners from `corners` or from `ozi_map_path` (read against `picture`), or none.
fn placement(args: &Value, picture: &Path) -> Result<Option<Vec<MapCorner>>, gt_session::ApiError> {
    let given = corners_arg(args)?;
    let ozi = short(args, "ozi_map_path")?.filter(|p| !p.trim().is_empty());
    match (given, ozi) {
        (Some(_), Some(_)) => Err(invalid("Give corners or ozi_map_path, not both.".into())),
        (Some(c), None) => Ok(Some(c)),
        (None, Some(path)) => {
            let path = PathBuf::from(path);
            check_agent_path(&path)?;
            Ok(Some(ozi_corners(&path, picture)?.corners.to_vec()))
        }
        (None, None) => Ok(None),
    }
}

fn add_map_layer<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let path = PathBuf::from(str_arg(args, "file_path")?);
    let title = short(args, "title")?.filter(|t| !t.trim().is_empty());
    let source = short(args, "source")?.filter(|t| !t.trim().is_empty());
    let (Some(title), Some(source)) = (title, source) else {
        return Err(invalid(
            "Give the map a title, and say where the scan comes from in source.".into(),
        ));
    };
    // The batch first: a refused call learns nothing about the paths and copies nothing.
    let dir = host.with_session(&mut |s| {
        s.check_batch_open(ctx, batch)?;
        Ok(s.project_dir())
    })?;
    let corners = placement(args, &path)?;
    let layer = MapLayerInput {
        series: short(args, "series")?,
        year: short(args, "year")?,
        source: Some(source),
        corners,
        opacity: 70,
        visible: true,
    };
    // Checked, copied and hashed without the tree's lock; removed again if not recorded.
    let prepared = prepare_agent_file(&dir, &path, OwnerKind::Person)?;
    let file = std::cell::RefCell::new(Some(prepared));
    let media = std::cell::Cell::new(None);
    apply(
        host,
        ctx,
        |s, _| {
            let file = file
                .borrow_mut()
                .take()
                .ok_or_else(|| invalid("The file was already handled.".into()))?;
            let (write, reached, id) =
                s.add_map_layer_as_agent(file, Some(title.clone()), &layer, ctx, batch)?;
            media.set(Some(id));
            Ok((write, reached, None))
        },
        |_, _| media.get(),
    )
}

fn degrees(v: i32) -> f64 {
    f64::from(v) / 1e6
}

fn list_map_layers<H: Host>(host: &H, ctx: &AgentCtx, _args: &Value) -> ToolResult {
    let rows = host.with_session(&mut |s: &mut Session| {
        let mut rows = Vec::new();
        for l in s.map_layers() {
            let mut row = json!({
                "id": l.media,
                "title": l.title,
                "opacity": l.opacity,
                "visible": l.visible,
                "yours": s.map_layer_is_own(l.media, ctx.id)?,
            });
            for (k, v) in [
                ("series", &l.series),
                ("year", &l.year),
                ("source", &l.source),
            ] {
                if let Some(v) = v {
                    row[k] = json!(v);
                }
            }
            if let Some(c) = &l.corners {
                row["corners"] = json!(
                    c.iter()
                        .map(|p| json!({"lat": degrees(p.lat_e6), "lon": degrees(p.lon_e6)}))
                        .collect::<Vec<_>>()
                );
            }
            if let (Some(w), Some(h)) = (l.width, l.height) {
                row["width"] = json!(w);
                row["height"] = json!(h);
            }
            rows.push(row);
        }
        Ok(rows)
    })?;
    answer(&json!({"items": rows}))
}

fn set_own_map_layer_corners<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let media = uuid_arg(args, "media")?;
    let picture = host.with_session(&mut |s| {
        s.check_batch_open(ctx, batch)?;
        s.media_original_path(media)
    })?;
    let corners = placement(args, &picture)?
        .ok_or_else(|| invalid("Give corners or ozi_map_path.".into()))?;
    write(
        host,
        ctx,
        batch,
        |s, _| {
            let m = s
                .tree()
                .media
                .get(&media)
                .ok_or_else(|| gt_session::ApiError::not_found("media", media))?;
            let Some(l) = &m.map_layer else {
                return Err(invalid(
                    "This picture is not on the Map. Lay it there with add_map_layer.".into(),
                ));
            };
            let mut input = input_of(l);
            input.corners = Some(corners.clone());
            Ok(EditCommand::SetMapLayer {
                media,
                layer: Some(input),
                title: None,
            })
        },
        |_, _| Some(media),
    )
}
