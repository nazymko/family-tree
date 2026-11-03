#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use gt_core::date::calendar::{Calendar, first_jdn};
use gt_core::privacy::{LivingMode, PrivacyPolicy, filter};
use gt_gedcom::ExportOptions as GedExportOptions;
use gt_session::*;
use gt_store::Project;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const NOW_MS: i64 = 1_793_664_000_000; // 2026-11-03

fn today() -> i32 {
    first_jdn(Calendar::Gregorian, 2026, Some(11), Some(3)).unwrap()
}

fn clock() -> SessionClock {
    SessionClock::fixed(today(), NOW_MS)
}

fn ged(records: &str) -> String {
    format!("0 HEAD\n1 SOUR TEST\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n{records}0 TRLR\n")
}

/// Anna (dead, 1850) and Zed (no dates: counts as living); a note of Anna names Zed; one photo
/// is linked from both.
const LIVING: &str = "0 @I1@ INDI\n1 NAME Anna /Mueller/\n1 SEX F\n1 BIRT\n2 DATE 1850\n1 DEAT Y\n\
1 NOTE Anna often wrote to Zed Livingstone about the farm and the long road between them\n1 OBJE @M1@\n\
0 @I2@ INDI\n1 NAME Zed /Livingstone/\n1 SEX M\n1 OBJE @M1@\n\
0 @M1@ OBJE\n1 FILE group.jpg\n2 TITL Group photo\n";

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, text).unwrap();
    p
}

fn request(file: &Path, project: &Path, roots: &[&Path]) -> ImportRequest {
    ImportRequest {
        file: file.to_string_lossy().into_owned(),
        project_dir: project.to_string_lossy().into_owned(),
        project_name: "Test".into(),
        media_roots: roots
            .iter()
            .map(|r| r.to_string_lossy().into_owned())
            .collect(),
        day_order: DayOrder::Dmy,
    }
}

fn import_text(records: &str) -> (TempDir, Session, ImportReportDto) {
    let tmp = tempfile::tempdir().unwrap();
    let f = write(tmp.path(), "in/tree.ged", &ged(records));
    let (s, r) = import_gedcom(
        &request(&f, &tmp.path().join("proj"), &[]),
        clock(),
        &|_| {},
    )
    .unwrap();
    (tmp, s, r)
}

fn opts(version: ExportVersion, living: LivingExport) -> ExportOptions {
    ExportOptions {
        version,
        living,
        include_private: false,
    }
}

#[test]
fn import_creates_project_and_report() {
    let tmp = tempfile::tempdir().unwrap();
    let f = Path::new("../../tests/fixtures/gedcom/basic551.ged");
    let proj = tmp.path().join("proj");
    let (s, r) = import_gedcom(&request(f, &proj, &[]), clock(), &|_| {}).unwrap();
    assert_eq!(r.counts["INDI"], 2);
    assert_eq!(r.producer.as_deref(), Some("ANCESTRY"));
    assert_eq!(r.unmapped_tags["_MTTAG"], 1);
    assert_eq!(r.version.as_deref(), Some("5.5.1"));
    assert!(r.text.contains("Producer: ANCESTRY"));
    assert!(!r.truncated);
    assert_eq!(r.project.person_count, 2);
    assert_eq!(r.project.name, "Test");
    assert!(proj.join("tree.db").is_file());
    assert_eq!(s.tree().persons.len(), 2);
    assert!(!proj.with_file_name("proj.partial-1").exists());
}

#[test]
fn import_copies_media_and_rewrites_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("pics");
    write(&src, "photo.jpg", "not really a jpeg");
    let f = write(
        tmp.path(),
        "in/tree.ged",
        &ged("0 @I1@ INDI\n1 NAME A /B/\n1 OBJE @M1@\n0 @M1@ OBJE\n1 FILE photo.jpg\n"),
    );
    let proj = tmp.path().join("proj");
    let (s, r) = import_gedcom(&request(&f, &proj, &[&src]), clock(), &|_| {}).unwrap();
    assert!(r.missing_media.is_empty());
    let m = s.tree().media.values().next().unwrap();
    let sha = m.sha256.clone().unwrap();
    assert_eq!(sha.len(), 64);
    assert_eq!(m.path, format!("media/{}/{sha}.jpg", &sha[..2]));
    assert!(proj.join(&m.path).is_file());
    assert_eq!(m.original_name.as_deref(), Some("photo.jpg"));
}

#[test]
fn import_missing_media_listed() {
    let (_t, s, r) =
        import_text("0 @I1@ INDI\n1 NAME A /B/\n1 OBJE @M1@\n0 @M1@ OBJE\n1 FILE nothing.jpg\n");
    assert_eq!(r.missing_media.len(), 1);
    assert_eq!(r.missing_media[0].path, "nothing.jpg");
    assert_eq!(r.missing_media[0].reason, MissingReasonDto::NotFound);
    assert!(s.tree().media.values().next().unwrap().sha256.is_none());
}

#[test]
fn import_progress_phases_in_order() {
    let tmp = tempfile::tempdir().unwrap();
    let f = write(tmp.path(), "t.ged", &ged("0 @I1@ INDI\n1 NAME A /B/\n"));
    let seen = RefCell::new(Vec::new());
    import_gedcom(&request(&f, &tmp.path().join("p"), &[]), clock(), &|p| {
        assert!(p.done <= p.total);
        seen.borrow_mut().push(p.phase);
    })
    .unwrap();
    let mut phases = seen.into_inner();
    phases.dedup();
    assert_eq!(
        phases,
        [
            ImportPhase::Reading,
            ImportPhase::Parsing,
            ImportPhase::CopyingMedia,
            ImportPhase::Saving,
            ImportPhase::Indexing
        ]
    );
}

#[test]
fn import_into_existing_conflicts() {
    let tmp = tempfile::tempdir().unwrap();
    let f = write(tmp.path(), "t.ged", &ged("0 @I1@ INDI\n1 NAME A /B/\n"));
    let proj = tmp.path().join("p");
    let (_s, _) = import_gedcom(&request(&f, &proj, &[]), clock(), &|_| {}).unwrap();
    let err = import_gedcom(&request(&f, &proj, &[]), clock(), &|_| {})
        .err()
        .unwrap();
    assert_eq!(err.code, ErrorCode::Conflict);
    // A non-empty folder that is not a project is refused as well, and left alone.
    let other = tmp.path().join("other");
    write(&other, "keep.txt", "x");
    let err = import_gedcom(&request(&f, &other, &[]), clock(), &|_| {})
        .err()
        .unwrap();
    assert_eq!(err.code, ErrorCode::Conflict);
    assert!(other.join("keep.txt").is_file());
}

#[test]
fn import_missing_file_is_io_error_and_leaves_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("p");
    let err = import_gedcom(
        &request(&tmp.path().join("nope.ged"), &proj, &[]),
        clock(),
        &|_| {},
    )
    .err()
    .unwrap();
    assert_eq!(err.code, ErrorCode::Io);
    assert!(!proj.exists());
}

#[test]
fn import_in_onedrive_path_reports_warning() {
    let tmp = tempfile::tempdir().unwrap();
    let f = write(tmp.path(), "t.ged", &ged("0 @I1@ INDI\n1 NAME A /B/\n"));
    let proj = tmp.path().join("OneDrive").join("trees");
    let (_s, r) = import_gedcom(&request(&f, &proj, &[]), clock(), &|_| {}).unwrap();
    let w = r.project.location_warning.unwrap();
    assert_eq!(w.kind, LocationKind::SyncedFolder);
    assert_eq!(w.provider.as_deref(), Some("OneDrive"));
}

#[test]
fn import_report_caps_lists_and_flags_truncated() {
    let mut recs = String::new();
    for n in 0..6000 {
        recs.push_str(&format!(
            "0 @I{n}@ INDI\n1 NAME P{n} /X/\n1 BIRT\n2 DATE !!{n}\n"
        ));
    }
    let (_t, _s, r) = import_text(&recs);
    assert_eq!(r.unparsed_dates.len(), 5000);
    assert!(r.truncated);
    // The text report is not capped.
    assert!(r.text.contains("(6000)"));
}

#[test]
fn discard_import_only_before_edit() {
    let (tmp, mut s, _) = import_text("0 @I1@ INDI\n1 NAME A /B/\n");
    let dir = tmp.path().join("proj");
    s.apply(EditCommand::CreatePerson {
        draft: PersonDraft {
            given: Some("Otto".into()),
            surnames: vec!["Z".into()],
            sex: SexKind::Male,
            birth_date_raw: None,
            birth_place_text: None,
        },
    })
    .unwrap();
    let err = Session::discard_import(&dir).unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
    assert!(dir.join("tree.db").is_file());
    drop(s);

    // Untouched import: the folder goes.
    let f = write(tmp.path(), "t.ged", &ged("0 @I1@ INDI\n1 NAME A /B/\n"));
    let dir2 = tmp.path().join("fresh");
    let (s2, _) = import_gedcom(&request(&f, &dir2, &[]), clock(), &|_| {}).unwrap();
    s2.close();
    Session::discard_import(&dir2).unwrap();
    assert!(!dir2.exists());
}

#[test]
fn discard_import_refuses_a_project_that_was_not_imported() {
    let (tmp, s, _) = import_text("0 @I1@ INDI\n1 NAME A /B/\n");
    let copy = tmp.path().join("copy");
    s.save_copy(&copy).unwrap();
    // A saved copy is not an import.
    assert_eq!(
        Session::discard_import(&copy).unwrap_err().code,
        ErrorCode::Conflict
    );
    assert_eq!(
        Session::discard_import(&tmp.path().join("nowhere"))
            .unwrap_err()
            .code,
        ErrorCode::NoProject
    );
}

/// Two media that were not found at import (`a.jpg`, `b.jpg`).
fn two_missing() -> (TempDir, Session) {
    let (t, s, r) = import_text(
        "0 @I1@ INDI\n1 NAME A /B/\n1 OBJE @M1@\n1 OBJE @M2@\n\
         0 @M1@ OBJE\n1 FILE a.jpg\n0 @M2@ OBJE\n1 FILE b.jpg\n",
    );
    assert_eq!(r.missing_media.len(), 2);
    (t, s)
}

fn media_state(s: &Session) -> Vec<(String, bool)> {
    s.tree()
        .media
        .values()
        .map(|m| (m.path.clone(), m.sha256.is_some()))
        .collect()
}

#[test]
fn relink_media_basename_search_and_undo() {
    let (tmp, mut s) = two_missing();
    let folder = tmp.path().join("found");
    write(&folder, "sub/deeper/A.JPG", "picture a");
    let before = media_state(&s);
    let res = s.relink_media(&folder).unwrap();
    assert_eq!(res.relinked, 1);
    assert_eq!(res.still_missing, ["b.jpg"]);
    let m: Vec<_> = s.tree().media.values().collect();
    assert!(m[0].sha256.is_some() && m[0].path.starts_with("media/"));
    assert!(tmp.path().join("proj").join(&m[0].path).is_file());
    assert!(m[1].sha256.is_none());
    assert_eq!(
        s.undo_state().undo_label.as_deref(),
        Some("Link media files")
    );

    s.undo().unwrap();
    assert_eq!(media_state(&s), before);
    s.redo().unwrap();
    assert!(s.tree().media.values().next().unwrap().sha256.is_some());
}

#[test]
fn relink_media_uses_relative_path_first() {
    let (t, s, _) =
        import_text("0 @I1@ INDI\n1 NAME A /B/\n1 OBJE @M1@\n0 @M1@ OBJE\n1 FILE pics/x.jpg\n");
    let mut s = s;
    let folder = t.path().join("found");
    write(&folder, "pics/x.jpg", "right one");
    write(&folder, "other/x.jpg", "wrong one");
    let res = s.relink_media(&folder).unwrap();
    assert_eq!(res.relinked, 1);
    let sha = s
        .tree()
        .media
        .values()
        .next()
        .unwrap()
        .sha256
        .clone()
        .unwrap();
    // The file under pics/ was taken, not the ambiguous basename.
    let stored = std::fs::read_to_string(
        t.path()
            .join("proj")
            .join(&s.tree().media.values().next().unwrap().path),
    )
    .unwrap();
    assert_eq!(stored, "right one");
    assert_eq!(sha.len(), 64);
}

#[test]
fn relink_media_ambiguous_basename_stays_missing() {
    let (t, mut s) = two_missing();
    let folder = t.path().join("found");
    write(&folder, "one/a.jpg", "1");
    write(&folder, "two/a.jpg", "2");
    let res = s.relink_media(&folder).unwrap();
    assert_eq!(res.relinked, 0);
    assert_eq!(res.still_missing, ["a.jpg", "b.jpg"]);
    assert!(!s.undo_state().can_undo);
}

#[test]
fn relink_media_search_depth_is_eight() {
    let (t, mut s) = two_missing();
    let folder = t.path().join("found");
    write(&folder, "1/2/3/4/5/6/7/8/a.jpg", "depth eight");
    write(&folder, "1/2/3/4/5/6/7/8/9/b.jpg", "depth nine");
    let res = s.relink_media(&folder).unwrap();
    assert_eq!(res.relinked, 1);
    assert_eq!(res.still_missing, ["b.jpg"]);
}

#[test]
fn relink_media_does_not_leave_the_folder() {
    // A stored path with `..` must not be joined onto the folder, and an absolute stored path is
    // never opened: both fall back to the name search inside the folder only.
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "outside/c.jpg", "outside file");
    write(tmp.path(), "outside/d.jpg", "outside file too");
    let abs = tmp.path().join("outside").join("d.jpg");
    let recs = format!(
        "0 @I1@ INDI\n1 NAME A /B/\n1 OBJE @M1@\n1 OBJE @M2@\n\
         0 @M1@ OBJE\n1 FILE ../outside/c.jpg\n0 @M2@ OBJE\n1 FILE {}\n",
        abs.to_string_lossy().replace('\\', "/")
    );
    let f = write(tmp.path(), "in/tree.ged", &ged(&recs));
    let (mut s, _) = import_gedcom(
        &request(&f, &tmp.path().join("proj"), &[]),
        clock(),
        &|_| {},
    )
    .unwrap();
    let folder = tmp.path().join("found");
    std::fs::create_dir_all(&folder).unwrap();
    let res = s.relink_media(&folder).unwrap();
    assert_eq!(res.relinked, 0);
    assert_eq!(res.still_missing.len(), 2);
}

#[test]
fn relink_media_does_not_follow_symlinks() {
    let (t, mut s) = two_missing();
    write(t.path(), "elsewhere/a.jpg", "secret");
    let folder = t.path().join("found");
    std::fs::create_dir_all(&folder).unwrap();
    let link = folder.join("link");
    let target = t.path().join("elsewhere");
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(&target, &link);
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&target, &link);
    if made.is_err() {
        return; // no permission to create symlinks here
    }
    let res = s.relink_media(&folder).unwrap();
    assert_eq!(res.relinked, 0);
}

#[test]
fn relink_media_rejects_a_file_as_folder() {
    let (t, mut s) = two_missing();
    let f = write(t.path(), "plain.txt", "x");
    assert_eq!(s.relink_media(&f).unwrap_err().code, ErrorCode::Invalid);
}

#[test]
fn preview_default_is_placeholder_and_counts_undated_person_as_hidden() {
    let (_t, s, _) = import_text(LIVING);
    let p = s
        .preview_export(&opts(ExportVersion::V551, LivingExport::Placeholder))
        .unwrap();
    assert_eq!(p.hidden_persons, 1);
    assert_eq!(p.placeholder_persons, 1);
    assert_eq!(p.exported_persons, 1);
    assert_eq!(p.default_version, ExportVersion::V551);
    assert_eq!(p.lossless_notes.len(), 1);
    assert!(p.lossless_notes[0].starts_with("Roles, 'did not happen' records"));
    let ex = s
        .preview_export(&opts(ExportVersion::V70, LivingExport::Exclude))
        .unwrap();
    assert_eq!(
        (
            ex.exported_persons,
            ex.placeholder_persons,
            ex.hidden_persons
        ),
        (1, 0, 1)
    );
    assert!(ex.lossless_notes.is_empty());
}

#[test]
fn preview_include_all_counts_zero_hidden() {
    let (_t, s, _) = import_text(LIVING);
    let p = s
        .preview_export(&opts(ExportVersion::V70, LivingExport::All))
        .unwrap();
    assert_eq!(p.hidden_persons, 0);
    assert_eq!(p.placeholder_persons, 0);
    assert_eq!(p.exported_persons, 2);
    assert!(p.mention_warnings.is_empty());
    assert!(p.group_media.is_empty());
}

#[test]
fn preview_mentions_and_group_media() {
    let (_t, s, _) = import_text(LIVING);
    let p = s
        .preview_export(&opts(ExportVersion::V70, LivingExport::Placeholder))
        .unwrap();
    assert_eq!(p.mention_warnings.len(), 1);
    assert_eq!(p.mention_warnings[0].hidden_person, "Zed Livingstone");
    assert_eq!(p.mention_warnings[0].note_excerpt.chars().count(), 80);
    assert!(
        p.mention_warnings[0]
            .note_excerpt
            .starts_with("Anna often wrote to Zed")
    );
    assert_eq!(p.group_media.len(), 1);
    assert_eq!(p.group_media[0].title, "Group photo");
}

#[test]
fn export_requires_current_token() {
    let (t, mut s, _) = import_text(LIVING);
    let o = opts(ExportVersion::V70, LivingExport::Placeholder);
    let p = s.preview_export(&o).unwrap();
    // Same state, same token.
    assert_eq!(s.preview_export(&o).unwrap().token, p.token);
    let other = s
        .preview_export(&opts(ExportVersion::V70, LivingExport::Exclude))
        .unwrap();
    assert_ne!(other.token, p.token);
    let out = t.path().join("o.ged");
    // Token from another option set is stale.
    assert_eq!(
        s.export_gedcom(&o, &out, &other.token).unwrap_err().code,
        ErrorCode::Stale
    );
    s.apply(EditCommand::CreatePerson {
        draft: PersonDraft {
            given: Some("Otto".into()),
            surnames: vec!["Z".into()],
            sex: SexKind::Male,
            birth_date_raw: None,
            birth_place_text: None,
        },
    })
    .unwrap();
    assert_eq!(
        s.export_gedcom(&o, &out, &p.token).unwrap_err().code,
        ErrorCode::Stale
    );
    assert!(!out.exists());
    let fresh = s.preview_export(&o).unwrap();
    assert_ne!(fresh.token, p.token);
    assert!(s.export_gedcom(&o, &out, &fresh.token).unwrap() > 0);
}

#[test]
fn export_file_has_no_living_surname_in_placeholder_mode() {
    let (t, s, _) = import_text(LIVING);
    let o = opts(ExportVersion::V551, LivingExport::Placeholder);
    let p = s.preview_export(&o).unwrap();
    let out = t.path().join("o.ged");
    let n = s.export_gedcom(&o, &out, &p.token).unwrap();
    let bytes = std::fs::read(&out).unwrap();
    assert_eq!(n, bytes.len() as u64);
    let text = String::from_utf8(bytes).unwrap();
    // The note that names Zed is kept (the preview warns about it); the person is not.
    assert!(!text.contains("SURN Livingstone"), "{text}");
    assert!(!text.contains("/Livingstone/"), "{text}");
    assert!(!text.contains("NAME Zed"), "{text}");
    assert!(text.contains("NAME Living"), "{text}");
    assert!(text.contains("Mueller"), "{text}");
}

#[test]
fn export_version_follows_source() {
    let tmp = tempfile::tempdir().unwrap();
    let f = Path::new("../../tests/fixtures/gedcom/family7.ged");
    let (s, _) = import_gedcom(&request(f, &tmp.path().join("p"), &[]), clock(), &|_| {}).unwrap();
    assert_eq!(s.default_export_version(), ExportVersion::V70);
    let o = opts(ExportVersion::V70, LivingExport::All);
    let p = s.preview_export(&o).unwrap();
    assert_eq!(p.default_version, ExportVersion::V70);
    let out = tmp.path().join("o.ged");
    s.export_gedcom(&o, &out, &p.token).unwrap();
    assert!(
        std::fs::read_to_string(&out)
            .unwrap()
            .contains("2 VERS 7.0")
    );
    let (_t, s551, _) = import_text(LIVING);
    assert_eq!(s551.default_export_version(), ExportVersion::V551);
}

#[test]
fn export_atomic_write_leaves_no_tmp_on_success() {
    let (t, s, _) = import_text(LIVING);
    let o = opts(ExportVersion::V70, LivingExport::All);
    let p = s.preview_export(&o).unwrap();
    let out = t.path().join("o.ged");
    s.export_gedcom(&o, &out, &p.token).unwrap();
    assert!(out.is_file());
    assert!(!t.path().join("o.ged.tmp").exists());
}

#[test]
fn export_never_overwrites_silently() {
    let (t, s, _) = import_text(LIVING);
    let o = opts(ExportVersion::V70, LivingExport::All);
    let p = s.preview_export(&o).unwrap();
    let out = write(t.path(), "o.ged", "precious");
    let err = s.export_gedcom(&o, &out, &p.token).unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "precious");
    // The caller confirmed the replacement.
    s.export_gedcom_over(&o, &out, &p.token, true).unwrap();
    assert!(std::fs::read_to_string(&out).unwrap().starts_with("0 HEAD"));
    assert!(!t.path().join("o.ged.tmp").exists());
}

#[test]
fn export_matches_cli_output() {
    let (t, s, _) = import_text(LIVING);
    let o = opts(ExportVersion::V551, LivingExport::Placeholder);
    let p = s.preview_export(&o).unwrap();
    let out = t.path().join("o.ged");
    s.export_gedcom(&o, &out, &p.token).unwrap();
    // What `gt export` does, step by step, on the stored project.
    let project = Project::open(&t.path().join("proj")).unwrap();
    let tree = project.load_tree().unwrap();
    let policy = PrivacyPolicy {
        living: LivingMode::Placeholder,
        include_private: false,
        ..PrivacyPolicy::default()
    };
    let (filtered, _) = filter(&tree, &policy, today());
    let expected = gt_gedcom::export(
        &filtered,
        &GedExportOptions {
            version: gt_gedcom::GedcomVersion::V551,
        },
    );
    assert_eq!(std::fs::read(&out).unwrap(), expected);
}

#[test]
fn save_copy_includes_living_and_media() {
    let tmp = tempfile::tempdir().unwrap();
    let pics = tmp.path().join("pics");
    write(&pics, "group.jpg", "the photo");
    let f = write(tmp.path(), "in/tree.ged", &ged(LIVING));
    let (s, _) = import_gedcom(
        &request(&f, &tmp.path().join("proj"), &[&pics]),
        clock(),
        &|_| {},
    )
    .unwrap();
    let dest = tmp.path().join("copy");
    s.save_copy(&dest).unwrap();
    let copied = Project::open(&dest).unwrap().load_tree().unwrap();
    assert_eq!(copied.persons.len(), 2);
    let names: Vec<_> = copied
        .persons
        .values()
        .flat_map(|p| p.names.iter().filter_map(|n| n.surnames.first().cloned()))
        .collect();
    assert!(names.contains(&"Livingstone".to_string()), "{names:?}");
    let m = copied.media.values().next().unwrap();
    assert_eq!(
        std::fs::read_to_string(dest.join(&m.path)).unwrap(),
        "the photo"
    );
    // The source is untouched and still open.
    assert_eq!(s.tree().persons.len(), 2);
}

#[test]
fn save_copy_opens_as_project() {
    let (t, s, _) = import_text(LIVING);
    let dest = t.path().join("copy");
    s.save_copy(&dest).unwrap();
    let reopened = Session::open(&dest, clock()).unwrap();
    assert_eq!(reopened.info().person_count, 2);
    assert_eq!(reopened.default_export_version(), ExportVersion::V551);
    assert!(!dest.with_file_name("copy.partial-1").exists());
    // Existing non-empty folder, and a folder inside the project, are refused.
    assert_eq!(s.save_copy(&dest).unwrap_err().code, ErrorCode::Conflict);
    assert_eq!(
        s.save_copy(&t.path().join("proj").join("inner"))
            .unwrap_err()
            .code,
        ErrorCode::Invalid
    );
    // An empty folder is a valid target.
    let empty = t.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    s.save_copy(&empty).unwrap();
    assert!(empty.join("tree.db").is_file());
}
