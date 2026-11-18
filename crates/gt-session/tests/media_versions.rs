#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Versions of one page: files grouped so the best copy is shown and opened first, with the others
//! a click away. Grouping is an ordinary undoable edit and never touches files, text or links.

mod common;

use common::*;
use gt_session::*;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn jpeg(w: u32, h: u32, shade: u8) -> Vec<u8> {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb([shade, 190, 160])));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Jpeg).unwrap();
    out.into_inner()
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

fn person(id: Uuid) -> OwnerRef {
    OwnerRef {
        kind: OwnerKind::Person,
        id,
    }
}

fn group(primary: Uuid, others: &[Uuid]) -> EditCommand {
    EditCommand::GroupMediaVersions {
        primary,
        others: others.to_vec(),
    }
}

fn ungroup(media: Uuid, new_primary: Option<Uuid>) -> EditCommand {
    EditCommand::UngroupMediaVersion { media, new_primary }
}

/// Anna with three copies of one register page: a large scan, a small copy and a middle one.
struct Fixture {
    dir: tempfile::TempDir,
    s: Session,
    anna: Uuid,
    big: Uuid,
    small: Uuid,
    mid: Uuid,
}

fn fixture() -> Fixture {
    let tree = tree_of(SAMPLE);
    let anna = find(&tree, "Anna");
    let (dir, mut s) = open_tree(&tree);
    let files = [
        write(dir.path(), "register-original.jpg", &jpeg(80, 60, 10)),
        write(dir.path(), "register-small.jpg", &jpeg(20, 15, 20)),
        write(dir.path(), "register-mid.jpg", &jpeg(40, 30, 30)),
    ];
    let r = s.attach_files(person(anna), &files).unwrap();
    let ids: Vec<Uuid> = r.attached.iter().map(|a| a.id).collect();
    Fixture {
        dir,
        s,
        anna,
        big: ids[0],
        small: ids[1],
        mid: ids[2],
    }
}

fn version_of(s: &Session, m: Uuid) -> Option<Uuid> {
    s.tree().media[&m].version_of
}

#[test]
fn grouping_points_the_others_at_the_primary_and_undo_takes_it_back() {
    let mut f = fixture();
    let links_before = f.s.get_person(f.anna).unwrap().media.len();
    let r = f.s.apply(group(f.big, &[f.small, f.mid])).unwrap();
    assert_eq!(r.label, "Group versions");
    assert_eq!(version_of(&f.s, f.big), None);
    assert_eq!(version_of(&f.s, f.small), Some(f.big));
    assert_eq!(version_of(&f.s, f.mid), Some(f.big));
    // Every link stays; each one carries the whole group, primary first, then largest first.
    let media = f.s.get_person(f.anna).unwrap().media;
    assert_eq!(media.len(), links_before);
    for l in &media {
        let ids: Vec<Uuid> = l.media.versions.iter().map(|v| v.id).collect();
        assert_eq!(ids, vec![f.big, f.mid, f.small]);
        assert!(l.media.versions.iter().all(|v| v.versions.is_empty()));
    }
    assert_eq!(media[0].media.versions[0].width, Some(80));

    f.s.undo().unwrap().unwrap();
    assert!(f.s.tree().media.values().all(|m| m.version_of.is_none()));
    assert!(
        f.s.get_person(f.anna)
            .unwrap()
            .media
            .iter()
            .all(|l| l.media.versions.is_empty())
    );
    f.s.redo().unwrap().unwrap();
    assert_eq!(version_of(&f.s, f.small), Some(f.big));
}

#[test]
fn the_group_is_kept_in_the_project() {
    let mut f = fixture();
    f.s.apply(group(f.big, &[f.small])).unwrap();
    let (big, small) = (f.big, f.small);
    drop(f.s);
    let s = Session::open(
        &f.dir.path().join("project"),
        SessionClock::fixed(today(), NOW_MS),
    )
    .unwrap();
    assert_eq!(version_of(&s, small), Some(big));
    assert_eq!(s.tree().version_group(small), vec![big, small]);
}

#[test]
fn another_version_is_made_primary_by_grouping_again() {
    let mut f = fixture();
    f.s.apply(group(f.big, &[f.small, f.mid])).unwrap();
    // The owner picks the small copy as the one to show: the whole group follows it.
    f.s.apply(group(f.small, &[])).unwrap();
    assert_eq!(version_of(&f.s, f.small), None);
    assert_eq!(version_of(&f.s, f.big), Some(f.small));
    assert_eq!(version_of(&f.s, f.mid), Some(f.small));
    // Once undone, the big scan is the primary again.
    f.s.undo().unwrap().unwrap();
    assert_eq!(version_of(&f.s, f.small), Some(f.big));
    assert_eq!(version_of(&f.s, f.big), None);
}

#[test]
fn two_groups_join_into_one() {
    let mut f = fixture();
    f.s.apply(group(f.big, &[f.small])).unwrap();
    let extra = write(f.dir.path(), "register-copy.jpg", &jpeg(30, 20, 40));
    let other = f.s.attach_files(person(f.anna), &[extra]).unwrap().attached[0].id;
    f.s.apply(group(f.mid, &[other])).unwrap();
    f.s.apply(group(f.big, &[f.mid])).unwrap();
    for m in [f.small, f.mid, other] {
        assert_eq!(version_of(&f.s, m), Some(f.big), "{m}");
    }
    assert_eq!(f.s.tree().version_group(other).len(), 4);
}

#[test]
fn ungrouping_a_version_or_the_primary() {
    let mut f = fixture();
    f.s.apply(group(f.big, &[f.small, f.mid])).unwrap();
    let r = f.s.apply(ungroup(f.small, None)).unwrap();
    assert_eq!(r.label, "Ungroup version");
    assert_eq!(version_of(&f.s, f.small), None);
    assert_eq!(version_of(&f.s, f.mid), Some(f.big));
    f.s.undo().unwrap().unwrap();
    // Taking the primary out: the one named becomes the primary of the rest.
    f.s.apply(ungroup(f.big, Some(f.mid))).unwrap();
    assert_eq!(version_of(&f.s, f.big), None);
    assert_eq!(version_of(&f.s, f.mid), None);
    assert_eq!(version_of(&f.s, f.small), Some(f.mid));
    // A group left with one file is no group.
    f.s.apply(ungroup(f.small, None)).unwrap();
    assert!(f.s.tree().version_index().is_empty());
    // Nothing to take a lone file out of.
    assert!(f.s.apply(ungroup(f.small, None)).is_err());
}

#[test]
fn a_new_primary_must_be_one_of_the_group() {
    let mut f = fixture();
    f.s.apply(group(f.big, &[f.small])).unwrap();
    let err = f.s.apply(ungroup(f.big, Some(f.mid))).unwrap_err();
    assert!(err.message.contains("other versions"), "{}", err.message);
}

#[test]
fn deleting_the_primary_keeps_the_rest_together() {
    let mut f = fixture();
    f.s.apply(group(f.big, &[f.small, f.mid])).unwrap();
    f.s.apply(EditCommand::DeleteMedia { id: f.big }).unwrap();
    assert_eq!(version_of(&f.s, f.small), None);
    assert_eq!(version_of(&f.s, f.mid), Some(f.small));
    f.s.undo().unwrap().unwrap();
    assert_eq!(version_of(&f.s, f.small), Some(f.big));
    assert_eq!(version_of(&f.s, f.mid), Some(f.big));
}

#[test]
fn grouping_keeps_transcriptions_and_refuses_odd_requests() {
    let mut f = fixture();
    f.s.apply(EditCommand::SetMediaTranscription {
        id: f.small,
        regions: vec![TextRegionInput {
            id: None,
            area: None,
            rotation: None,
            text: "Anna, born 1850".into(),
            start_ms: None,
            end_ms: None,
            speaker: None,
        }],
        known: None,
    })
    .unwrap();
    f.s.apply(group(f.big, &[f.small])).unwrap();
    assert_eq!(f.s.tree().media[&f.small].transcription.len(), 1);
    assert!(f.s.tree().media[&f.big].transcription.is_empty());
    // No other file named.
    assert!(f.s.apply(group(f.mid, &[])).is_err());
    assert!(f.s.apply(group(f.mid, &[f.mid])).is_err());
    assert!(f.s.apply(group(f.mid, &[Uuid::from_u128(99)])).is_err());
}

#[test]
fn the_picker_lists_one_entry_per_page() {
    let mut f = fixture();
    assert_eq!(f.s.all_pictures().len(), 3);
    f.s.apply(group(f.big, &[f.small, f.mid])).unwrap();
    let listed = f.s.all_pictures();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, f.big);
    assert_eq!(listed[0].versions.len(), 3);
}
