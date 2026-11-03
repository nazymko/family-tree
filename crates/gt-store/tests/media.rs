#![allow(clippy::unwrap_used)]

use gt_store::Project;

fn count_files(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .map(|p| if p.is_dir() { count_files(&p) } else { 1 })
        .sum()
}

#[test]
fn media_dedup() {
    let tmp = tempfile::tempdir().unwrap();
    let p = Project::create(&tmp.path().join("t.genetree")).unwrap();
    let a = tmp.path().join("a.JPG");
    let b = tmp.path().join("copy of a.jpg");
    std::fs::write(&a, b"same bytes").unwrap();
    std::fs::write(&b, b"same bytes").unwrap();

    let (rel1, sha1) = p.add_media_file(&a).unwrap();
    let (rel2, sha2) = p.add_media_file(&b).unwrap();
    assert_eq!((&rel1, &sha1), (&rel2, &sha2));
    assert_eq!(sha1.len(), 64);
    assert_eq!(rel1, format!("media/{}/{}.jpg", &sha1[..2], sha1));
    assert!(p.dir.join(&rel1).is_file());
    assert_eq!(std::fs::read(p.dir.join(&rel1)).unwrap(), b"same bytes");
    assert_eq!(count_files(&p.dir.join("media")), 1);
}

#[test]
fn media_known_hash_and_distinct_content() {
    let tmp = tempfile::tempdir().unwrap();
    let p = Project::create(&tmp.path().join("t.genetree")).unwrap();
    let a = tmp.path().join("abc");
    std::fs::write(&a, b"abc").unwrap();
    let (rel, sha) = p.add_media_file(&a).unwrap();
    assert_eq!(
        sha,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(rel, format!("media/ba/{sha}")); // no extension
    let b = tmp.path().join("other.png");
    std::fs::write(&b, b"abd").unwrap();
    p.add_media_file(&b).unwrap();
    assert_eq!(count_files(&p.dir.join("media")), 2);
}

#[test]
fn media_missing_source_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    let p = Project::create(&tmp.path().join("t.genetree")).unwrap();
    assert!(p.add_media_file(&tmp.path().join("nope.jpg")).is_err());
}

#[test]
fn media_jpeg_and_jpg_share_one_file() {
    let tmp = tempfile::tempdir().unwrap();
    let p = Project::create(&tmp.path().join("t.genetree")).unwrap();
    let a = tmp.path().join("a.jpeg");
    let b = tmp.path().join("b.JPG");
    std::fs::write(&a, b"pixels").unwrap();
    std::fs::write(&b, b"pixels").unwrap();
    let (r1, _) = p.add_media_file(&a).unwrap();
    let (r2, _) = p.add_media_file(&b).unwrap();
    assert_eq!(r1, r2);
    assert!(r1.ends_with(".jpg"));
    assert_eq!(count_files(&p.dir.join("media")), 1);
}

#[test]
fn media_name_matches_stored_content_and_no_tmp_left() {
    let tmp = tempfile::tempdir().unwrap();
    let p = Project::create(&tmp.path().join("t.genetree")).unwrap();
    let a = tmp.path().join("big.bin");
    std::fs::write(&a, vec![7u8; 300_000]).unwrap();
    let (rel, sha) = p.add_media_file(&a).unwrap();
    use sha2::{Digest, Sha256};
    let stored = std::fs::read(p.dir.join(&rel)).unwrap();
    let again: String = Sha256::digest(&stored)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(again, sha);
    assert_eq!(count_files(&p.dir.join("media")), 1);
}
