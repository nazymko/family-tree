#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use gt_core::model::{Common, Media, MediaLink};
use gt_gedcom::{ImportOptions, import};
use gt_session::*;
use gt_store::Project;

fn common(id: uuid::Uuid) -> Common {
    Common {
        id,
        private: false,
        changed: None,
        citations: vec![],
        notes: vec![],
        media: vec![],
        ext_ids: vec![],
        unknown: vec![],
    }
}

/// One person linked to media records with the given stored paths; returns `resolved` per path.
fn resolved_for(paths: &[String], make_files: impl Fn(&std::path::Path)) -> Vec<bool> {
    let ged = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n0 @I1@ INDI\n1 NAME A /B/\n0 TRLR\n";
    let mut tree = import(ged.as_bytes(), &ImportOptions::default()).tree;
    let pid = *tree.persons.keys().next().unwrap();
    for p in paths {
        let id = uuid::Uuid::now_v7();
        tree.media.insert(
            id,
            Media {
                common: common(id),
                path: p.clone(),
                original_name: None,
                mime: None,
                title: None,
                date: None,
                description: None,
                sha256: None,
            },
        );
        tree.persons
            .get_mut(&pid)
            .unwrap()
            .common
            .media
            .push(MediaLink {
                media: id,
                region: None,
                primary: false,
            });
    }
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("project");
    let mut project = Project::create(&proj).unwrap();
    project.save_tree(&tree, 2_461_343).unwrap();
    drop(project);
    make_files(dir.path());
    let s = Session::open(&proj, SessionClock::fixed(2_461_343, 0)).unwrap();
    let d = s.get_person(pid).unwrap();
    d.media.iter().map(|m| m.media.resolved).collect()
}

fn write(p: &std::path::Path) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, b"x").unwrap();
}

#[test]
fn only_files_under_the_media_dir_resolve() {
    let outside_abs = std::env::temp_dir().join("gt-session-outside.jpg");
    std::fs::write(&outside_abs, b"x").unwrap();
    let paths = vec![
        "media/ab/good.jpg".to_string(),            // normal relative, exists
        "media/ab/missing.jpg".to_string(),         // normal relative, absent
        "../outside.jpg".to_string(),               // escapes, file exists
        "media/../../outside.jpg".to_string(),      // escapes after media/
        outside_abs.to_string_lossy().into_owned(), // absolute, exists
        r"\\host\share\x.jpg".to_string(),          // UNC
        "//host/share/x.jpg".to_string(),           // UNC, forward slashes
        r"C:\Windows\win.ini".to_string(),          // drive path
        "notes.txt".to_string(),                    // relative but outside media/
    ];
    let got = resolved_for(&paths, |root| {
        write(&root.join("project/media/ab/good.jpg"));
        write(&root.join("project/notes.txt"));
        write(&root.join("outside.jpg"));
    });
    let _ = std::fs::remove_file(&outside_abs);
    assert_eq!(
        got,
        [true, false, false, false, false, false, false, false, false]
    );
}
