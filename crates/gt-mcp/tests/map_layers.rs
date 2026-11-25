#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! add_map_layer, list_map_layers and set_own_map_layer_corners: an agent lays old maps on the
//! Map, with corners it gives or from an OziExplorer .map file, and places again only its own.
//! Invented maps and coordinates only.

mod common;

use common::*;
use gt_session::{EditCommand, MapCorner, MapLayerInput};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use serde_json::{Value, json};
use std::io::Cursor;
use std::path::PathBuf;
use uuid::Uuid;

fn begin(host: &TestHost) -> Value {
    call(host, "begin_batch", json!({"title": "Three-verst sheets"}))["batch"].clone()
}

fn jpeg(w: u32, h: u32, shade: u8) -> Vec<u8> {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb([shade, 200, 170])));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Jpeg).unwrap();
    out.into_inner()
}

fn file(host: &TestHost, name: &str, bytes: &[u8]) -> PathBuf {
    let dir = host._dir.as_ref().unwrap().path().join("incoming");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

const FRAME: [[f64; 2]; 4] = [[50.6, 30.0], [50.6, 31.2], [49.9, 31.2], [49.9, 30.0]];

fn frame() -> Value {
    json!(
        FRAME
            .iter()
            .map(|[lat, lon]| json!({"lat": lat, "lon": lon}))
            .collect::<Vec<_>>()
    )
}

/// A synthetic OziExplorer .map: calibrated on a 100 x 50 picture, its corners the frame above.
const OZI: &str = "OziExplorer Map Data File Version 2.2\r\nsheet.jpg\r\nsheet.jpg\r\n1 ,Map Code,\r\nWGS 84,WGS 84,   0.0000,   0.0000,WGS 84\r\nReserved 1\r\nReserved 2\r\nMagnetic Variation,,,E\r\nMap Projection,Latitude/Longitude,PolyCal,No,AutoCalOnly,No,BSBUseWPX,No\r\nPoint01,xy,    0,    0,in, deg,  50,36.000,N,  30, 0.000,E, grid,   ,           ,           ,N\r\nPoint02,xy,  100,    0,in, deg,  50,36.000,N,  31,12.000,E, grid,   ,           ,           ,N\r\nPoint03,xy,  100,   50,in, deg,  49,54.000,N,  31,12.000,E, grid,   ,           ,           ,N\r\nProjection Setup,,,,,,,,,,\r\nMM0,Yes\r\nMMPNUM,4\r\nMMPXY,1,0,0\r\nMMPXY,2,100,0\r\nMMPXY,3,100,50\r\nMMPXY,4,0,50\r\nMMPLL,1,  30.000000,  50.600000\r\nMMPLL,2,  31.200000,  50.600000\r\nMMPLL,3,  31.200000,  49.900000\r\nMMPLL,4,  30.000000,  49.900000\r\nIWH,Map Image Width/Height,100,50\r\n";

fn add(host: &TestHost, batch: &Value, name: &str, shade: u8, extra: Value) -> Value {
    let path = file(host, name, &jpeg(80, 40, shade));
    let mut args = json!({"batch": batch, "file_path": path.to_string_lossy(), "title": name, "source": "https://example.org/sheets", "series": "Three-verst map", "year": "1865"});
    for (k, v) in extra.as_object().unwrap() {
        args[k] = v.clone();
    }
    call_raw(host, "add_map_layer", args)
}

fn id(v: &Value) -> Uuid {
    serde_json::from_value(v["id"].clone()).unwrap()
}

/// The id in a raw tool answer (what `add` returns).
fn added(r: &Value) -> Uuid {
    id(&serde_json::from_str(r["content"][0]["text"].as_str().unwrap()).unwrap())
}

fn items(host: &TestHost) -> Vec<Value> {
    call(host, "list_map_layers", json!({}))["items"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn an_agent_lays_a_map_with_corners_and_a_repeat_is_already_there() {
    let host = empty_host();
    let batch = begin(&host);
    let r = add(
        &host,
        &batch,
        "sheet-a.jpg",
        30,
        json!({"corners": frame()}),
    );
    assert!(
        r.get("isError").is_none() || r["isError"] == json!(false),
        "{r}"
    );
    let first = call(&host, "list_map_layers", json!({}));
    let list = first["items"].as_array().unwrap();
    assert_eq!(list.len(), 1, "{first}");
    assert_eq!(list[0]["title"], json!("sheet-a.jpg"));
    assert_eq!(list[0]["year"], json!("1865"));
    assert_eq!(list[0]["yours"], json!(true));
    assert_eq!(list[0]["corners"][1]["lon"], json!(31.2));
    assert_eq!(list[0]["width"], json!(80));
    let media = serde_json::from_value::<Uuid>(list[0]["id"].clone()).unwrap();

    let again = call(
        &host,
        "add_map_layer",
        json!({"batch": batch, "file_path": file(&host, "sheet-a.jpg", &jpeg(80, 40, 30)).to_string_lossy(), "title": "sheet-a.jpg", "source": "https://example.org/sheets", "corners": frame()}),
    );
    assert_eq!(again["alreadyThere"], json!(true), "{again}");
    assert_eq!(id(&again), media);
    assert_eq!(items(&host).len(), 1);
}

#[test]
fn corners_come_from_an_ozi_file_and_a_wrong_call_is_refused() {
    let host = empty_host();
    let batch = begin(&host);
    let map = file(&host, "sheet_ozf.map", OZI.as_bytes());
    let r = call(&host, "add_map_layer", {
        let pic = file(&host, "sheet.jpg", &jpeg(200, 100, 60));
        json!({"batch": batch, "file_path": pic.to_string_lossy(), "title": "Sheet", "source": "https://example.org/s", "ozi_map_path": map.to_string_lossy()})
    });
    let media = id(&r);
    let l = items(&host)
        .into_iter()
        .find(|l| l["id"] == json!(media))
        .unwrap();
    assert_eq!(l["corners"][0], json!({"lat": 50.6, "lon": 30.0}));
    assert_eq!(l["corners"][2], json!({"lat": 49.9, "lon": 31.2}));

    // Corners and a .map at once, no source, three corners, a corner off the Earth: refused.
    let err = call_err(
        &host,
        "add_map_layer",
        json!({"batch": batch, "file_path": "C:/x.jpg", "title": "x", "source": "x", "corners": frame(), "ozi_map_path": map.to_string_lossy()}),
    );
    assert!(err.contains("not both"), "{err}");
    let err = call_err(
        &host,
        "add_map_layer",
        json!({"batch": batch, "file_path": "C:/x.jpg", "title": "x"}),
    );
    assert!(err.contains("source"), "{err}");
    let pic = file(&host, "b.jpg", &jpeg(30, 30, 9));
    let three = json!(
        FRAME[..3]
            .iter()
            .map(|[lat, lon]| json!({"lat": lat, "lon": lon}))
            .collect::<Vec<_>>()
    );
    assert!(
        call_raw(
            &host,
            "add_map_layer",
            json!({"batch": batch, "file_path": pic.to_string_lossy(), "title": "b", "source": "x", "corners": three})
        )["isError"]
            == json!(true)
    );
    let off = json!([{"lat": 95, "lon": 30}, {"lat": 50, "lon": 31}, {"lat": 49, "lon": 31}, {"lat": 49, "lon": 30}]);
    assert!(
        call_raw(
            &host,
            "add_map_layer",
            json!({"batch": batch, "file_path": pic.to_string_lossy(), "title": "b", "source": "x", "corners": off})
        )["isError"]
            == json!(true)
    );
    assert_eq!(items(&host).len(), 1);
}

/// A map an agent lays is hidden unless it says visible: true; the owner chooses what to show.
#[test]
fn an_agent_lays_maps_hidden_unless_it_says_visible() {
    let host = empty_host();
    let batch = begin(&host);
    let hidden = added(&add(&host, &batch, "hidden.jpg", 10, json!({})));
    let shown = added(&add(
        &host,
        &batch,
        "shown.jpg",
        90,
        json!({"visible": true}),
    ));
    let also_hidden = added(&add(
        &host,
        &batch,
        "false.jpg",
        150,
        json!({"visible": false}),
    ));
    let visible = |m: Uuid| {
        items(&host)
            .into_iter()
            .find(|l| l["id"] == json!(m))
            .unwrap()["visible"]
            .clone()
    };
    assert_eq!(visible(hidden), json!(false));
    assert_eq!(visible(shown), json!(true));
    assert_eq!(visible(also_hidden), json!(false));
    let bad = add(&host, &batch, "bad.jpg", 200, json!({"visible": "yes"}));
    assert_eq!(bad["isError"], json!(true), "{bad}");
    assert_eq!(items(&host).len(), 3);
}

/// A JPEG of another shape than the picture the .map was made for (cropped or framed otherwise)
/// would land kilometres off: the agent is refused and nothing is added.
#[test]
fn a_cropped_picture_with_its_ozi_file_is_refused() {
    let host = empty_host();
    let batch = begin(&host);
    let map = file(&host, "sheet_ozf.map", OZI.as_bytes());
    let pic = file(&host, "sheet.jpg", &jpeg(180, 100, 60));
    let err = call_err(
        &host,
        "add_map_layer",
        json!({"batch": batch, "file_path": pic.to_string_lossy(), "title": "Sheet", "source": "https://example.org/s", "ozi_map_path": map.to_string_lossy()}),
    );
    assert!(err.contains("does not fit this picture"), "{err}");
    assert!(items(&host).is_empty());
}

#[test]
fn an_agent_places_only_its_own_map_and_only_while_nobody_changed_it() {
    let host = empty_host();
    let batch = begin(&host);
    // The owner's map, laid in the app.
    let owners = file(&host, "owners.jpg", &jpeg(50, 50, 140));
    let owner_map = with(&host, |s| {
        s.add_map_layer_file(
            &owners,
            Some("Owner's sheet".into()),
            &MapLayerInput {
                series: None,
                year: None,
                source: None,
                corners: None,
                opacity: 70,
                visible: true,
            },
        )
        .unwrap()
    });
    let own = id(&call(&host, "add_map_layer", {
        let p = file(&host, "agent.jpg", &jpeg(60, 30, 70));
        json!({"batch": batch, "file_path": p.to_string_lossy(), "title": "Agent's sheet", "source": "https://example.org/a"})
    }));
    let yours: Vec<(Value, Value)> = items(&host)
        .into_iter()
        .map(|l| (l["id"].clone(), l["yours"].clone()))
        .collect();
    assert!(yours.contains(&(json!(own), json!(true))), "{yours:?}");
    assert!(
        yours.contains(&(json!(owner_map), json!(false))),
        "{yours:?}"
    );

    let placed = call(
        &host,
        "set_own_map_layer_corners",
        json!({"batch": batch, "media": own, "corners": frame()}),
    );
    assert_eq!(id(&placed), own, "{placed}");
    let again = call(
        &host,
        "set_own_map_layer_corners",
        json!({"batch": batch, "media": own, "corners": frame()}),
    );
    assert_eq!(again["alreadyThere"], json!(true), "{again}");
    let err = call_err(
        &host,
        "set_own_map_layer_corners",
        json!({"batch": batch, "media": owner_map, "corners": frame()}),
    );
    assert!(!err.is_empty());

    // The owner moves the agent's map: from then on it is the owner's.
    with(&host, |s| {
        let c = |lat: f64, lon: f64| MapCorner {
            lat_e6: (lat * 1e6) as i32,
            lon_e6: (lon * 1e6) as i32,
        };
        s.apply(EditCommand::SetMapLayer {
            media: own,
            layer: Some(MapLayerInput {
                series: None,
                year: None,
                source: Some("https://example.org/a".into()),
                corners: Some(vec![
                    c(50.5, 30.1),
                    c(50.5, 31.1),
                    c(49.8, 31.1),
                    c(49.8, 30.1),
                ]),
                opacity: 55,
                visible: true,
            }),
            title: None,
        })
        .unwrap();
    });
    assert!(
        call_raw(
            &host,
            "set_own_map_layer_corners",
            json!({"batch": batch, "media": own, "corners": frame()})
        )["isError"]
            == json!(true)
    );
    let l = items(&host)
        .into_iter()
        .find(|l| l["id"] == json!(own))
        .unwrap();
    assert_eq!(l["yours"], json!(false));
    assert_eq!(l["opacity"], json!(55));
}
