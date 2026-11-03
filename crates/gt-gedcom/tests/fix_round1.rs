#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use gt_core::date::GedcomVersion;
use gt_core::model::*;
use gt_gedcom::{ExportOptions, ImportOptions, MissingReason, export, import};
use std::path::PathBuf;

fn opts(roots: Vec<PathBuf>, dir: Option<PathBuf>) -> ImportOptions {
    ImportOptions {
        media_roots: roots,
        gedcom_dir: dir,
        ..Default::default()
    }
}

fn imp(s: &str) -> gt_gedcom::ImportOutcome {
    import(s.as_bytes(), &opts(vec![], None))
}

const BOTH: [GedcomVersion; 2] = [GedcomVersion::V551, GedcomVersion::V70];

#[test]
fn text_shaped_like_pointer_round_trips() {
    let src = "0 HEAD\n0 @I1@ INDI\n1 OCCU @@N1@\n1 NOTE @@N1@\n0 @N1@ NOTE real\n0 TRLR\n";
    let a = imp(src);
    assert!(a.tree.notes.values().any(|n| n.text == "@N1@" && !n.shared));
    let ev = a
        .tree
        .events
        .values()
        .find(|e| e.description.as_deref() == Some("@N1@"));
    assert!(ev.is_some(), "OCCU text lost on first import");
    for v in BOTH {
        let out = export(&a.tree, &ExportOptions { version: v });
        let text = String::from_utf8(out.clone()).unwrap();
        assert!(text.contains("@@N1@"), "{v:?}: {text}");
        let b = import(&out, &opts(vec![], None));
        assert_eq!(
            a.tree.normalized_for_compare(),
            b.tree.normalized_for_compare(),
            "{v:?}"
        );
    }
}

#[test]
fn sex_x_survives_551_via_extension_tag() {
    let a = imp("0 HEAD\n1 GEDC\n2 VERS 7.0\n0 @I1@ INDI\n1 SEX X\n0 TRLR\n");
    assert_eq!(a.tree.persons.values().next().unwrap().sex, Sex::Intersex);
    let t551 = String::from_utf8(export(
        &a.tree,
        &ExportOptions {
            version: GedcomVersion::V551,
        },
    ))
    .unwrap();
    assert!(
        t551.contains("1 SEX U") && t551.contains("1 _SEX X"),
        "{t551}"
    );
    assert!(!t551.contains("1 SEX X"));
    let b = import(t551.as_bytes(), &opts(vec![], None));
    assert_eq!(b.tree.persons.values().next().unwrap().sex, Sex::Intersex);
    let t70 = String::from_utf8(export(
        &a.tree,
        &ExportOptions {
            version: GedcomVersion::V70,
        },
    ))
    .unwrap();
    assert!(t70.contains("1 SEX X") && !t70.contains("_SEX"), "{t70}");
    let c = import(t70.as_bytes(), &opts(vec![], None));
    assert_eq!(c.tree.persons.values().next().unwrap().sex, Sex::Intersex);
}

fn obje(file: &str) -> String {
    format!("0 HEAD\n0 @O1@ OBJE\n1 FILE {file}\n0 TRLR\n")
}

#[test]
fn absolute_path_outside_roots_is_not_resolved() {
    let outside = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let f = outside.path().join("secret.jpg");
    std::fs::write(&f, b"x").unwrap();
    let src = obje(&f.to_string_lossy().replace('\\', "/"));
    let o = import(src.as_bytes(), &opts(vec![root.path().to_path_buf()], None));
    assert!(o.resolved_media.is_empty());
    assert_eq!(o.report.missing_media.len(), 1);
    assert_eq!(
        o.report.missing_media[0].reason,
        MissingReason::OutsideRoots
    );
    // Under a root, or under the GEDCOM's own folder, it resolves.
    let o = import(
        src.as_bytes(),
        &opts(vec![outside.path().to_path_buf()], None),
    );
    assert_eq!(o.resolved_media.len(), 1);
    let o = import(
        src.as_bytes(),
        &opts(vec![], Some(outside.path().to_path_buf())),
    );
    assert_eq!(o.resolved_media.len(), 1);
}

#[test]
fn file_urls_become_local_paths_and_other_urls_are_listed() {
    let outside = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("my pic.jpg"), b"x").unwrap();
    let f = outside.path().join("o.jpg");
    std::fs::write(&f, b"x").unwrap();
    let url = format!(
        "file:///{}",
        f.to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
    );
    let o = import(
        obje(&url).as_bytes(),
        &opts(vec![root.path().to_path_buf()], None),
    );
    assert_eq!(
        o.report.missing_media[0].reason,
        MissingReason::OutsideRoots,
        "{url}"
    );
    let o = import(
        obje("file:///C:/x/my%20pic.jpg").as_bytes(),
        &opts(vec![root.path().to_path_buf()], None),
    );
    assert_eq!(o.resolved_media.len(), 1);
    let src = "0 HEAD\n0 @O1@ OBJE\n1 FILE https://e.com/a.jpg\n0 @O2@ OBJE\n1 FILE data:image/png;base64,AAAA\n0 TRLR\n";
    let o = imp(src);
    assert!(o.report.missing_media.is_empty());
    assert_eq!(o.report.media_urls.len(), 2);
    assert!(o.report.to_text().contains("https://e.com/a.jpg"));
}

#[test]
fn ambiguous_basename_is_reported_with_candidates() {
    let root = tempfile::tempdir().unwrap();
    for d in ["a", "b"] {
        std::fs::create_dir_all(root.path().join(d)).unwrap();
        std::fs::write(root.path().join(d).join("p.jpg"), b"x").unwrap();
    }
    let o = import(
        obje("C:\\old\\p.jpg").as_bytes(),
        &opts(vec![root.path().to_path_buf()], None),
    );
    assert!(o.resolved_media.is_empty());
    let m = &o.report.missing_media[0];
    assert_eq!(m.reason, MissingReason::Ambiguous);
    assert_eq!(m.candidates.len(), 2);
}

#[test]
fn at_escaping_is_version_specific() {
    let src = "0 HEAD\n0 @I1@ INDI\n1 OCCU @@N1@\n1 EMAIL a@@b.com\n1 NOTE @@N1@\n2 CONT @@x@@y\n0 TRLR\n";
    let a = imp(src);
    assert!(a.tree.notes.values().any(|n| n.text == "@N1@\n@x@y"));
    for v in BOTH {
        let out = export(&a.tree, &ExportOptions { version: v });
        let text = String::from_utf8(out.clone()).unwrap();
        if v == GedcomVersion::V70 {
            assert!(text.contains("a@b.com"), "{text}");
            assert!(!text.contains("a@@b.com"), "{text}");
            assert!(text.contains("@@N1@") && text.contains("@@x@y"), "{text}");
        } else {
            assert!(text.contains("a@@b.com"), "{text}");
        }
        let b = import(&out, &opts(vec![], None));
        assert_eq!(
            a.tree.normalized_for_compare(),
            b.tree.normalized_for_compare(),
            "{v:?}"
        );
    }
}
