#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use common::*;
use gt_core::model::{Common, Media};
use gt_session::*;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use uuid::Uuid;

// ---- generated inputs (nothing binary is committed) ----

fn encode(img: &DynamicImage, fmt: ImageFormat) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, fmt).unwrap();
    out.into_inner()
}

fn solid(w: u32, h: u32, rgb: [u8; 3]) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb(rgb)))
}

fn jpeg(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    encode(&solid(w, h, rgb), ImageFormat::Jpeg)
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, bytes).unwrap();
    p
}

fn entry(tag: u16, typ: u16, count: u32, data: Vec<u8>) -> (u16, u16, u32, Vec<u8>) {
    (tag, typ, count, data)
}

fn block_size(entries: &[(u16, u16, u32, Vec<u8>)]) -> usize {
    2 + 12 * entries.len()
        + 4
        + entries
            .iter()
            .filter(|e| e.3.len() > 4)
            .map(|e| e.3.len() + e.3.len() % 2)
            .sum::<usize>()
}

/// One little-endian TIFF IFD placed at `start` (offsets are relative to the TIFF header).
fn block(entries: &[(u16, u16, u32, Vec<u8>)], start: usize) -> Vec<u8> {
    let mut out = (entries.len() as u16).to_le_bytes().to_vec();
    let mut spill = Vec::new();
    let spill_at = start + 2 + 12 * entries.len() + 4;
    for (tag, typ, count, data) in entries {
        out.extend(tag.to_le_bytes());
        out.extend(typ.to_le_bytes());
        out.extend(count.to_le_bytes());
        if data.len() <= 4 {
            let mut d = data.clone();
            d.resize(4, 0);
            out.extend(d);
        } else {
            out.extend(((spill_at + spill.len()) as u32).to_le_bytes());
            spill.extend(data);
            if data.len() % 2 == 1 {
                spill.push(0);
            }
        }
    }
    out.extend(0u32.to_le_bytes());
    out.extend(spill);
    out
}

fn rationals(v: &[(u32, u32)]) -> Vec<u8> {
    v.iter()
        .flat_map(|(n, d)| [n.to_le_bytes(), d.to_le_bytes()].concat())
        .collect()
}

fn tiff_header() -> Vec<u8> {
    let mut h = b"II".to_vec();
    h.extend(42u16.to_le_bytes());
    h.extend(8u32.to_le_bytes());
    h
}

/// Inserts an EXIF APP1 segment right after the SOI marker of a JPEG.
fn with_exif(jpeg: &[u8], tiff: &[u8]) -> Vec<u8> {
    let mut seg = b"Exif\0\0".to_vec();
    seg.extend(tiff);
    let mut out = jpeg[..2].to_vec();
    out.extend([0xFF, 0xE1]);
    out.extend(((seg.len() + 2) as u16).to_be_bytes());
    out.extend(seg);
    out.extend(&jpeg[2..]);
    out
}

fn jpeg_with_orientation(w: u32, h: u32, orientation: u16) -> Vec<u8> {
    let e = vec![entry(0x0112, 3, 1, orientation.to_le_bytes().to_vec())];
    let mut tiff = tiff_header();
    tiff.extend(block(&e, 8));
    with_exif(&jpeg(w, h, [200, 30, 30]), &tiff)
}

fn jpeg_with_date_and_gps(lat_ref: &str, lon_ref: &str) -> Vec<u8> {
    let date = b"1901:03:12 14:00:00\0".to_vec();
    let exif_ifd = vec![entry(0x9003, 2, 20, date)];
    let gps = vec![
        entry(1, 2, 2, [lat_ref.as_bytes(), b"\0"].concat()),
        entry(2, 5, 3, rationals(&[(48, 1), (30, 1), (0, 1)])),
        entry(3, 2, 2, [lon_ref.as_bytes(), b"\0"].concat()),
        entry(4, 5, 3, rationals(&[(11, 1), (15, 1), (0, 1)])),
    ];
    let mut ifd0 = vec![
        entry(0x8769, 4, 1, vec![0; 4]),
        entry(0x8825, 4, 1, vec![0; 4]),
    ];
    let ifd0_len = block_size(&ifd0);
    let exif_at = 8 + ifd0_len;
    let gps_at = exif_at + block_size(&exif_ifd);
    ifd0[0].3 = (exif_at as u32).to_le_bytes().to_vec();
    ifd0[1].3 = (gps_at as u32).to_le_bytes().to_vec();
    let mut tiff = tiff_header();
    tiff.extend(block(&ifd0, 8));
    tiff.extend(block(&exif_ifd, exif_at));
    tiff.extend(block(&gps, gps_at));
    with_exif(&jpeg(40, 30, [10, 120, 10]), &tiff)
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for b in data {
        c ^= u32::from(*b);
        for _ in 0..8 {
            c = if c & 1 == 1 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut body = kind.to_vec();
    body.extend(data);
    let mut out = (data.len() as u32).to_be_bytes().to_vec();
    out.extend(&body);
    out.extend(crc32(&body).to_be_bytes());
    out
}

/// A PNG whose header claims `w` x `h` pixels (1-bit gray) with no usable pixel data.
fn png_claiming(w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = w.to_be_bytes().to_vec();
    ihdr.extend(h.to_be_bytes());
    ihdr.extend([1, 0, 0, 0, 0]);
    out.extend(png_chunk(b"IHDR", &ihdr));
    out.extend(png_chunk(b"IDAT", &[0x78, 0x9C, 0x03, 0x00]));
    out.extend(png_chunk(b"IEND", &[]));
    out
}

fn setup() -> (tempfile::TempDir, Session, Uuid, PathBuf) {
    let tree = tree_of(SAMPLE);
    let eva = find(&tree, "Eva");
    let (dir, s) = open_tree(&tree);
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    (dir, s, eva, src)
}

fn person(id: Uuid) -> OwnerRef {
    OwnerRef {
        kind: OwnerKind::Person,
        id,
    }
}

fn media_count(s: &Session, id: Uuid) -> usize {
    s.get_person(id).unwrap().media.len()
}

fn reasons(r: &AttachResult) -> Vec<&str> {
    r.failed.iter().map(|f| f.reason.as_str()).collect()
}

fn decode_dims(bytes: &[u8]) -> (u32, u32) {
    let img = image::load_from_memory(bytes).unwrap();
    (img.width(), img.height())
}

fn project_dir(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("project")
}

// ---- attach ----

#[test]
fn attach_mixed_batch_per_file_results() {
    let (_d, mut s, eva, src) = setup();
    let ok = write(&src, "photo.jpg", &jpeg(60, 40, [1, 2, 3]));
    let fake = write(&src, "fake.jpg", b"MZ\x90\x00 this is an executable");
    let zero = write(&src, "zero.bin", b"");
    let folder = src.join("album");
    std::fs::create_dir_all(&folder).unwrap();
    let pdf = write(&src, "doc.pdf", b"%PDF-1.4\n%fake body\n");
    let missing = src.join("nope.jpg");
    let r = s
        .attach_files(person(eva), &[ok, fake.clone(), zero, folder, pdf, missing])
        .unwrap();
    assert_eq!(r.attached.len(), 2);
    assert_eq!(
        reasons(&r),
        vec![
            "unsupported type",
            "file is empty",
            "folders are not attached; drop the files inside",
            "cannot read file"
        ]
    );
    assert_eq!(r.failed[0].path, fake.to_string_lossy());
    assert_eq!(r.attached[0].mime.as_deref(), Some("image/jpeg"));
    assert_eq!(
        (r.attached[0].width, r.attached[0].height),
        (Some(60), Some(40))
    );
    assert_eq!(r.attached[0].original_name.as_deref(), Some("photo.jpg"));
    assert!(r.attached[0].resolved);
    assert_eq!(r.attached[1].mime.as_deref(), Some("application/pdf"));
    assert_eq!(r.attached[1].width, None);
    assert_eq!(media_count(&s, eva), 2);
    // One change set: a single undo removes both links.
    assert_eq!(s.undo_state().undo_label.as_deref(), Some("Attach media"));
    s.undo().unwrap().unwrap();
    assert_eq!(media_count(&s, eva), 0);
    assert!(!s.undo_state().can_undo);
}

#[test]
fn type_is_detected_by_content_not_extension() {
    let (_d, mut s, eva, src) = setup();
    let png = encode(&solid(8, 8, [9, 9, 9]), ImageFormat::Png);
    let tif = encode(&solid(8, 8, [8, 8, 8]), ImageFormat::Tiff);
    let gif = encode(&solid(8, 8, [7, 7, 7]), ImageFormat::Gif);
    let webp = encode(&solid(8, 8, [6, 6, 6]), ImageFormat::WebP);
    let files = [
        write(&src, "a.dat", &png),
        write(&src, "b.txt", &tif),
        write(&src, "c", &gif),
        write(&src, "d.jpg", &webp),
    ];
    let r = s.attach_files(person(eva), &files).unwrap();
    assert!(r.failed.is_empty(), "{:?}", r.failed);
    let mimes: Vec<_> = r.attached.iter().map(|m| m.mime.clone().unwrap()).collect();
    assert_eq!(
        mimes,
        ["image/png", "image/tiff", "image/gif", "image/webp"]
    );
    for m in &r.attached {
        let (b, mime) = s.media_bytes(m.id, MediaSize::Thumb).unwrap();
        assert_eq!(mime, "image/jpeg");
        assert_eq!(decode_dims(&b), (8, 8));
    }
}

#[test]
fn attach_same_photo_twice_reports_already_attached() {
    let (_d, mut s, eva, src) = setup();
    let a = write(&src, "a.jpg", &jpeg(30, 30, [5, 5, 5]));
    let copy = write(&src, "copy-of-a.jpg", &jpeg(30, 30, [5, 5, 5]));
    let first = s
        .attach_files(person(eva), std::slice::from_ref(&a))
        .unwrap();
    assert_eq!(first.attached.len(), 1);
    let again = s.attach_files(person(eva), &[a, copy.clone()]).unwrap();
    assert!(again.attached.is_empty());
    assert_eq!(
        reasons(&again),
        vec!["already attached", "already attached"]
    );
    assert_eq!(again.failed[1].path, copy.to_string_lossy());
    assert_eq!(media_count(&s, eva), 1);
    // The failed attempt made no undo step of its own.
    s.undo().unwrap().unwrap();
    assert_eq!(media_count(&s, eva), 0);
}

#[test]
fn same_content_on_another_person_reuses_the_media_record() {
    let (_d, mut s, eva, src) = setup();
    let anna = find(&s.tree().clone(), "Anna");
    let a = write(&src, "a.jpg", &jpeg(30, 30, [5, 5, 5]));
    let r1 = s
        .attach_files(person(eva), std::slice::from_ref(&a))
        .unwrap();
    let r2 = s.attach_files(person(anna), &[a]).unwrap();
    assert_eq!(r1.attached[0].id, r2.attached[0].id);
}

#[test]
fn attach_non_ascii_path() {
    let (_d, mut s, eva, src) = setup();
    let p = write(&src, "Zoë/foto Ünï.jpg", &jpeg(20, 10, [3, 3, 3]));
    let r = s.attach_files(person(eva), &[p]).unwrap();
    assert!(r.failed.is_empty(), "{:?}", r.failed);
    assert_eq!(r.attached[0].original_name.as_deref(), Some("foto Ünï.jpg"));
    assert!(s.media_bytes(r.attached[0].id, MediaSize::Original).is_ok());
}

#[test]
fn attach_to_unknown_owner_is_not_found() {
    let (_d, mut s, _eva, src) = setup();
    let a = write(&src, "a.jpg", &jpeg(10, 10, [1, 1, 1]));
    let err = s.attach_files(person(Uuid::now_v7()), &[a]).unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
}

#[test]
fn undo_attach_removes_links_but_keeps_file() {
    let (d, mut s, eva, src) = setup();
    let a = write(&src, "a.jpg", &jpeg(30, 30, [4, 4, 4]));
    let r = s
        .attach_files(person(eva), std::slice::from_ref(&a))
        .unwrap();
    let id = r.attached[0].id;
    s.undo().unwrap().unwrap();
    assert_eq!(media_count(&s, eva), 0);
    assert_eq!(
        s.media_bytes(id, MediaSize::Original).unwrap_err().code,
        ErrorCode::NotFound
    );
    let stored: Vec<_> = walk(&project_dir(&d).join("media"));
    assert_eq!(stored.len(), 1, "file stays in media/: {stored:?}");
    // Redo links it again; attaching the same content again reuses the same file.
    s.redo().unwrap().unwrap();
    assert_eq!(media_count(&s, eva), 1);
    assert!(s.media_bytes(id, MediaSize::Original).is_ok());
    assert_eq!(walk(&project_dir(&d).join("media")).len(), 1);
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

// ---- thumbnails ----

#[test]
fn thumb_dimensions_and_orientation() {
    let (_d, mut s, eva, src) = setup();
    // 400x200 landscape with EXIF orientation 6 (rotate 90 clockwise): displays as portrait.
    let p = write(&src, "rot.jpg", &jpeg_with_orientation(400, 200, 6));
    let r = s.attach_files(person(eva), &[p]).unwrap();
    let m = &r.attached[0];
    assert_eq!(
        (m.width, m.height),
        (Some(200), Some(400)),
        "reported size is as displayed"
    );
    let (bytes, mime) = s.media_bytes(m.id, MediaSize::Thumb).unwrap();
    assert_eq!(mime, "image/jpeg");
    assert_eq!(decode_dims(&bytes), (128, 256));
    let plain = write(&src, "wide.jpg", &jpeg(1000, 500, [9, 9, 9]));
    let r = s.attach_files(person(eva), &[plain]).unwrap();
    let (bytes, _) = s.media_bytes(r.attached[0].id, MediaSize::Thumb).unwrap();
    assert_eq!(decode_dims(&bytes), (256, 128));
    let (bytes, _) = s.media_bytes(r.attached[0].id, MediaSize::Preview).unwrap();
    assert_eq!(decode_dims(&bytes), (1000, 500));
}

#[test]
fn thumb_cached_and_regenerated_after_delete() {
    let (d, mut s, eva, src) = setup();
    let p = write(&src, "a.jpg", &jpeg(600, 300, [50, 60, 70]));
    let r = s.attach_files(person(eva), &[p]).unwrap();
    let m = &r.attached[0];
    let first = s.media_bytes(m.id, MediaSize::Thumb).unwrap().0;
    let cached = project_dir(&d)
        .join("thumbs")
        .join(format!("{}-thumb.jpg", m.sha256.as_ref().unwrap()));
    assert!(cached.is_file());
    assert_eq!(std::fs::read(&cached).unwrap(), first);
    // A cache hit is served as stored.
    std::fs::write(&cached, b"cached-marker").unwrap();
    assert_eq!(
        s.media_bytes(m.id, MediaSize::Thumb).unwrap().0,
        b"cached-marker"
    );
    std::fs::remove_file(&cached).unwrap();
    let again = s.media_bytes(m.id, MediaSize::Thumb).unwrap().0;
    assert_eq!(again, first);
    assert!(cached.is_file());
}

#[test]
fn preview_not_upscaled() {
    let (_d, mut s, eva, src) = setup();
    let p = write(
        &src,
        "small.png",
        &encode(&solid(120, 80, [1, 200, 1]), ImageFormat::Png),
    );
    let r = s.attach_files(person(eva), &[p]).unwrap();
    let id = r.attached[0].id;
    assert_eq!(
        decode_dims(&s.media_bytes(id, MediaSize::Preview).unwrap().0),
        (120, 80)
    );
    assert_eq!(
        decode_dims(&s.media_bytes(id, MediaSize::Thumb).unwrap().0),
        (120, 80)
    );
    let big = write(&src, "big.jpg", &jpeg(3200, 1600, [1, 1, 200]));
    let r = s.attach_files(person(eva), &[big]).unwrap();
    assert_eq!(
        decode_dims(
            &s.media_bytes(r.attached[0].id, MediaSize::Preview)
                .unwrap()
                .0
        ),
        (1600, 800)
    );
}

#[test]
fn transparent_png_is_flattened_on_white() {
    let (_d, mut s, eva, src) = setup();
    let img = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        10,
        10,
        image::Rgba([0, 0, 0, 0]),
    ));
    let p = write(&src, "t.png", &encode(&img, ImageFormat::Png));
    let r = s.attach_files(person(eva), &[p]).unwrap();
    let (b, _) = s.media_bytes(r.attached[0].id, MediaSize::Thumb).unwrap();
    let px = image::load_from_memory(&b)
        .unwrap()
        .to_rgb8()
        .get_pixel(5, 5)
        .0;
    assert!(px.iter().all(|c| *c > 240), "{px:?}");
}

#[test]
fn pdf_has_no_thumb_but_original_served() {
    let (_d, mut s, eva, src) = setup();
    let body = b"%PDF-1.4\n1 0 obj<<>>endobj\n";
    let p = write(&src, "doc.pdf", body);
    let r = s.attach_files(person(eva), &[p]).unwrap();
    let id = r.attached[0].id;
    let (bytes, mime) = s.media_bytes(id, MediaSize::Original).unwrap();
    assert_eq!(
        (bytes.as_slice(), mime.as_str()),
        (&body[..], "application/pdf")
    );
    for size in [MediaSize::Thumb, MediaSize::Preview] {
        assert_eq!(
            s.media_bytes(id, size).unwrap_err().code,
            ErrorCode::NotFound
        );
    }
}

#[test]
fn huge_image_stored_but_no_preview() {
    let (_d, mut s, eva, src) = setup();
    // Header claims 200 megapixels; nothing is ever allocated for it.
    let p = write(&src, "huge.png", &png_claiming(20_000, 10_000));
    let r = s
        .attach_files(person(eva), std::slice::from_ref(&p))
        .unwrap();
    assert_eq!(r.attached.len(), 1, "stored and attached");
    assert_eq!(reasons(&r), vec!["image too large to preview"]);
    assert_eq!(
        (r.attached[0].width, r.attached[0].height),
        (Some(20_000), Some(10_000))
    );
    let id = r.attached[0].id;
    assert_eq!(media_count(&s, eva), 1);
    for size in [MediaSize::Thumb, MediaSize::Preview] {
        let e = s.media_bytes(id, size).unwrap_err();
        assert_eq!(e.message, "image too large to preview");
    }
    assert!(s.media_bytes(id, MediaSize::Original).is_ok());
}

#[test]
fn corrupt_image_is_attached_but_has_no_thumb() {
    let (_d, mut s, eva, src) = setup();
    let mut bytes = jpeg(50, 50, [1, 2, 3]);
    bytes.truncate(40);
    let p = write(&src, "cut.jpg", &bytes);
    let r = s.attach_files(person(eva), &[p]).unwrap();
    assert_eq!(r.attached.len(), 1);
    assert_eq!(r.attached[0].width, None);
    assert_eq!(
        s.media_bytes(r.attached[0].id, MediaSize::Thumb)
            .unwrap_err()
            .code,
        ErrorCode::Invalid
    );
}

// ---- EXIF ----

#[test]
fn exif_date_gps_suggestion() {
    let (_d, mut s, eva, src) = setup();
    let p = write(&src, "geo.jpg", &jpeg_with_date_and_gps("N", "W"));
    let r = s.attach_files(person(eva), &[p]).unwrap();
    let ex = s.exif_suggestion(r.attached[0].id).unwrap().unwrap();
    assert_eq!(ex.date.as_deref(), Some("12 MAR 1901"));
    assert!((ex.lat.unwrap() - 48.5).abs() < 1e-9);
    assert!(
        (ex.lon.unwrap() + 11.25).abs() < 1e-9,
        "west is negative: {:?}",
        ex.lon
    );
    let p = write(&src, "geo2.jpg", &jpeg_with_date_and_gps("S", "E"));
    let r = s.attach_files(person(eva), &[p]).unwrap();
    let ex = s.exif_suggestion(r.attached[0].id).unwrap().unwrap();
    assert!((ex.lat.unwrap() + 48.5).abs() < 1e-9);
    assert!((ex.lon.unwrap() - 11.25).abs() < 1e-9);
}

#[test]
fn no_exif_returns_none() {
    let (_d, mut s, eva, src) = setup();
    let a = write(&src, "plain.jpg", &jpeg(20, 20, [1, 1, 1]));
    let b = write(
        &src,
        "plain.png",
        &encode(&solid(9, 9, [2, 2, 2]), ImageFormat::Png),
    );
    let c = write(&src, "doc.pdf", b"%PDF-1.4\n");
    let r = s.attach_files(person(eva), &[a, b, c]).unwrap();
    for m in &r.attached {
        assert_eq!(s.exif_suggestion(m.id).unwrap(), None);
    }
    // Orientation alone is not a suggestion.
    let o = write(&src, "o.jpg", &jpeg_with_orientation(20, 10, 3));
    let r = s.attach_files(person(eva), &[o]).unwrap();
    assert_eq!(s.exif_suggestion(r.attached[0].id).unwrap(), None);
}

// ---- lookup security ----

#[test]
fn media_bytes_unknown_id_not_found() {
    let (_d, s, _eva, _src) = setup();
    for size in [MediaSize::Thumb, MediaSize::Preview, MediaSize::Original] {
        assert_eq!(
            s.media_bytes(Uuid::now_v7(), size).unwrap_err().code,
            ErrorCode::NotFound
        );
    }
    assert_eq!(
        s.exif_suggestion(Uuid::now_v7()).unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[test]
fn stored_paths_outside_media_are_never_served() {
    let mut tree = tree_of(SAMPLE);
    let dir = tempfile::tempdir().unwrap();
    let secret = write(dir.path(), "secret.jpg", &jpeg(10, 10, [1, 1, 1]));
    let inside_project = write(
        &dir.path().join("project"),
        "notes.jpg",
        &jpeg(10, 10, [2, 2, 2]),
    );
    let _ = inside_project;
    let paths = [
        secret.to_string_lossy().into_owned(),
        "../secret.jpg".to_string(),
        "media/../notes.jpg".to_string(),
        "notes.jpg".to_string(),
        "media/../../secret.jpg".to_string(),
        "\\\\server\\share\\x.jpg".to_string(),
        String::new(),
    ];
    let mut ids = Vec::new();
    for p in &paths {
        let id = Uuid::now_v7();
        ids.push(id);
        tree.media.insert(
            id,
            Media {
                common: Common {
                    id,
                    private: false,
                    changed: None,
                    citations: vec![],
                    notes: vec![],
                    media: vec![],
                    ext_ids: vec![],
                    unknown: vec![],
                },
                path: p.clone(),
                original_name: None,
                mime: None,
                title: None,
                date: None,
                description: None,
                sha256: Some("../../../etc/passwd".into()),
            },
        );
    }
    let proj = dir.path().join("project");
    let mut p = gt_store::Project::create(&proj).unwrap();
    p.save_tree(&tree, today()).unwrap();
    drop(p);
    let s = Session::open(&proj, SessionClock::fixed(today(), NOW_MS)).unwrap();
    for id in ids {
        for size in [MediaSize::Thumb, MediaSize::Preview, MediaSize::Original] {
            assert_eq!(
                s.media_bytes(id, size).unwrap_err().code,
                ErrorCode::NotFound
            );
        }
        assert_eq!(s.exif_suggestion(id).unwrap_err().code, ErrorCode::NotFound);
    }
}

#[test]
fn served_original_mime_comes_from_content() {
    let (d, mut s, eva, src) = setup();
    let p = write(&src, "a.jpg", &jpeg(10, 10, [1, 1, 1]));
    let r = s.attach_files(person(eva), &[p]).unwrap();
    let id = r.attached[0].id;
    // Swap the stored file for non-media content: it must not be served as an image.
    let stored = walk(&project_dir(&d).join("media")).remove(0);
    std::fs::write(&stored, b"<html><script>alert(1)</script></html>").unwrap();
    assert_eq!(
        s.media_bytes(id, MediaSize::Original).unwrap_err().code,
        ErrorCode::Invalid
    );
}

// ---- performance (release only) ----

#[test]
#[ignore = "release only: cargo test -p gt-session --release --test media -- --ignored"]
fn thumb_generation_under_80ms_for_12mp() {
    let (_d, mut s, eva, src) = setup();
    let p = write(&src, "12mp.jpg", &jpeg(4000, 3000, [120, 80, 40]));
    let r = s.attach_files(person(eva), &[p]).unwrap();
    let t = std::time::Instant::now();
    s.media_bytes(r.attached[0].id, MediaSize::Thumb).unwrap();
    let ms = t.elapsed().as_millis();
    assert!(ms < 80, "took {ms} ms");
}
