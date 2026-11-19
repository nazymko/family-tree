#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Old maps on the Map: added from a file (with or without an OziExplorer .map), placed by four
//! corners, shown or hidden, reordered, taken off the map (the picture stays), undone, and the
//! rule that an agent changes only the layers it laid and nobody changed since. Invented data.

mod common;

use common::*;
use gt_session::*;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn me() -> AgentCtx {
    AgentCtx {
        id: Uuid::from_u128(9),
        name: "Map reader".into(),
    }
}

fn jpeg(w: u32, h: u32, shade: u8) -> Vec<u8> {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb([shade, 200, 170])));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Jpeg).unwrap();
    out.into_inner()
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

fn corners(n: f64, w: f64, s: f64, e: f64) -> Vec<MapCorner> {
    let c = |lat: f64, lon: f64| MapCorner {
        lat_e6: (lat * 1e6).round() as i32,
        lon_e6: (lon * 1e6).round() as i32,
    };
    vec![c(n, w), c(n, e), c(s, e), c(s, w)]
}

fn layer(c: Option<Vec<MapCorner>>) -> MapLayerInput {
    MapLayerInput {
        series: Some("Three-verst map".into()),
        year: Some("1865".into()),
        source: Some("https://example.org/sheet".into()),
        corners: c,
        opacity: 70,
        visible: true,
    }
}

fn session() -> (tempfile::TempDir, Session) {
    let tree = tree_of(SAMPLE);
    open_tree(&tree)
}

#[test]
fn owner_adds_places_hides_reorders_and_takes_a_map_off_and_it_is_kept() {
    let (dir, mut s) = session();
    let a = write(dir.path(), "sheet-a.jpg", &jpeg(60, 40, 10));
    let b = write(dir.path(), "sheet-b.jpg", &jpeg(40, 60, 90));
    let ma = s
        .add_map_layer_file(&a, Some("Sheet A".into()), &layer(None))
        .unwrap();
    let mb = s
        .add_map_layer_file(
            &b,
            Some("Sheet B".into()),
            &layer(Some(corners(50.6, 30.0, 49.9, 31.2))),
        )
        .unwrap();
    let list = s.map_layers();
    assert_eq!(
        list.iter().map(|l| l.media).collect::<Vec<_>>(),
        vec![ma, mb]
    );
    assert_eq!(list[0].corners, None);
    assert_eq!(list[0].width, Some(60));
    assert_eq!(list[1].corners.as_ref().map(Vec::len), Some(4));
    assert_eq!(list[1].year.as_deref(), Some("1865"));
    // The picture went into the tree's media store, not left where it was.
    let stored = &s.tree().media[&ma].path;
    assert!(!stored.is_empty());
    assert!(!Path::new(stored).starts_with(dir.path()), "{stored}");

    // Placed by its corners, then undone and redone.
    let mut placed = layer(Some(corners(50.5, 29.9, 50.0, 30.8)));
    placed.opacity = 60;
    let r = s
        .apply(EditCommand::SetMapLayer {
            media: ma,
            layer: Some(placed.clone()),
            title: None,
        })
        .unwrap();
    assert_eq!(r.label, "Change map layer");
    assert_eq!(s.map_layers()[0].opacity, 60);
    s.undo().unwrap().unwrap();
    assert_eq!(s.map_layers()[0].corners, None);
    s.redo().unwrap().unwrap();
    assert_eq!(s.map_layers()[0].corners, placed.corners);

    // Reordered: B under A.
    let r = s
        .apply(EditCommand::ReorderMapLayers {
            order: vec![mb, ma],
        })
        .unwrap();
    assert_eq!(r.label, "Reorder map layers");
    assert_eq!(
        s.map_layers().iter().map(|l| l.media).collect::<Vec<_>>(),
        vec![mb, ma]
    );

    // Taken off the map: the picture stays a media record.
    let r = s
        .apply(EditCommand::SetMapLayer {
            media: mb,
            layer: None,
            title: None,
        })
        .unwrap();
    assert_eq!(r.label, "Take map off the Map");
    assert_eq!(s.map_layers().len(), 1);
    assert!(s.tree().media.contains_key(&mb));

    // It is all saved.
    drop(s);
    let s = Session::open(
        &dir.path().join("project"),
        SessionClock::fixed(today(), NOW_MS),
    )
    .unwrap();
    let list = s.map_layers();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].media, ma);
    assert_eq!(list[0].opacity, 60);
    assert_eq!(list[0].corners, placed.corners);
}

#[test]
fn the_same_picture_added_twice_is_one_layer_and_a_bad_layer_is_refused() {
    let (dir, mut s) = session();
    let a = write(dir.path(), "sheet.jpg", &jpeg(50, 50, 30));
    let m1 = s.add_map_layer_file(&a, None, &layer(None)).unwrap();
    let m2 = s.add_map_layer_file(&a, None, &layer(None)).unwrap();
    assert_eq!(m1, m2);
    assert_eq!(s.map_layers().len(), 1);

    let mut bad = layer(Some(corners(95.0, 30.0, 49.9, 31.2)));
    assert!(
        s.apply(EditCommand::SetMapLayer {
            media: m1,
            layer: Some(bad.clone()),
            title: None,
        })
        .is_err()
    );
    bad.corners = Some(corners(50.6, 30.0, 49.9, 31.2)[..3].to_vec());
    assert!(
        s.apply(EditCommand::SetMapLayer {
            media: m1,
            layer: Some(bad.clone()),
            title: None,
        })
        .is_err()
    );
    bad.corners = None;
    bad.opacity = 101;
    assert!(
        s.apply(EditCommand::SetMapLayer {
            media: m1,
            layer: Some(bad),
            title: None,
        })
        .is_err()
    );
    // A text file is not a map.
    let txt = write(dir.path(), "notes.txt", b"not a picture");
    assert!(s.add_map_layer_file(&txt, None, &layer(None)).is_err());
}

const OZI: &str = "OziExplorer Map Data File Version 2.2\r\nsheet-c.jpg\r\nsheet-c.jpg\r\n1 ,Map Code,\r\nWGS 84,WGS 84,   0.0000,   0.0000,WGS 84\r\nReserved 1\r\nReserved 2\r\nMagnetic Variation,,,E\r\nMap Projection,Latitude/Longitude,PolyCal,No,AutoCalOnly,No,BSBUseWPX,No\r\nPoint01,xy,    0,    0,in, deg,  50,36.000,N,  30, 0.000,E, grid,   ,           ,           ,N\r\nPoint02,xy,  100,    0,in, deg,  50,36.000,N,  31,12.000,E, grid,   ,           ,           ,N\r\nPoint03,xy,  100,   50,in, deg,  49,54.000,N,  31,12.000,E, grid,   ,           ,           ,N\r\nPoint04,xy,    0,   50,in, deg,  49,54.000,N,  30, 0.000,E, grid,   ,           ,           ,N\r\nProjection Setup,,,,,,,,,,\r\nMap Feature = MF ; Map Comment = MC     These follow if they exist\r\nMM0,Yes\r\nMMPNUM,4\r\nMMPXY,1,0,0\r\nMMPXY,2,100,0\r\nMMPXY,3,100,50\r\nMMPXY,4,0,50\r\nMMPLL,1,  30.000000,  50.600000\r\nMMPLL,2,  31.200000,  50.600000\r\nMMPLL,3,  31.200000,  49.900000\r\nMMPLL,4,  30.000000,  49.900000\r\nIWH,Map Image Width/Height,100,50\r\n";

#[test]
fn a_map_with_its_ozi_file_is_found_and_placed() {
    let (dir, s) = session();
    let pic = write(dir.path(), "sheet-c.jpg", &jpeg(200, 100, 50));
    let map = write(dir.path(), "sheet-c_ozf.map", OZI.as_bytes());
    assert_eq!(map_layers::find_ozi_map(&pic), Some(map.clone()));
    let c = map_layers::ozi_corners(&map, &pic).unwrap();
    assert_eq!(c.corners[0], corners(50.6, 30.0, 49.9, 31.2)[0]);
    assert_eq!(c.corners[2], corners(50.6, 30.0, 49.9, 31.2)[2]);
    // A picture with no .map beside it is placed by hand.
    let other = write(dir.path(), "atlas.jpg", &jpeg(20, 20, 50));
    assert_eq!(map_layers::find_ozi_map(&other), None);
    drop(s);
}

#[test]
fn an_agent_changes_only_the_layers_it_laid_and_nobody_changed_since() {
    let (dir, mut s) = session();
    let owners = write(dir.path(), "owners.jpg", &jpeg(40, 40, 20));
    let owner_map = s.add_map_layer_file(&owners, None, &layer(None)).unwrap();
    let b = s.begin_batch(&me(), "Sheets 1865", None).unwrap();

    let file = write(dir.path(), "agents.jpg", &jpeg(40, 30, 120));
    let f = prepare_agent_file(&s.project_dir(), &file, OwnerKind::Source).unwrap();
    let (w, _, own) = s
        .add_map_layer_as_agent(f, Some("Sheet 30-13".into()), &layer(None), &me(), b)
        .unwrap();
    assert!(matches!(w, AgentWrite::Done(_)), "{w:?}");
    assert!(s.map_layer_is_own(own, me().id).unwrap());
    assert!(!s.map_layer_is_own(owner_map, me().id).unwrap());

    let place = |media| EditCommand::SetMapLayer {
        media,
        layer: Some(layer(Some(corners(50.6, 30.0, 49.9, 31.2)))),
        title: None,
    };
    let w = s.apply_as_agent(place(own), &me(), b).unwrap();
    assert!(matches!(w, AgentWrite::Done(_)), "{w:?}");
    // The owner's layer is not the agent's to move, and neither is the order of the layers.
    assert!(s.apply_as_agent(place(owner_map), &me(), b).is_err());
    assert!(
        s.apply_as_agent(
            EditCommand::ReorderMapLayers {
                order: vec![own, owner_map]
            },
            &me(),
            b
        )
        .is_err()
    );

    // Once the owner changes the agent's layer, it is the owner's.
    let mut mine = layer(Some(corners(50.5, 30.1, 49.8, 31.1)));
    mine.opacity = 40;
    s.apply(EditCommand::SetMapLayer {
        media: own,
        layer: Some(mine),
        title: None,
    })
    .unwrap();
    assert!(!s.map_layer_is_own(own, me().id).unwrap());
    assert!(s.apply_as_agent(place(own), &me(), b).is_err());
    assert_eq!(
        s.map_layers()
            .iter()
            .find(|l| l.media == own)
            .unwrap()
            .opacity,
        40
    );

    // Undoing the agent's batch never touches the owner's own map.
    s.undo_batch(b).unwrap();
    assert!(s.map_layers().iter().any(|l| l.media == owner_map));
}
