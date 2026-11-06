#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Merging a relative's copy: the must-never-happen list, the zip, and a full round trip.
//! Trees are built through the session API, so their change logs are the real ones.

mod common;

use common::*;
use gt_session::merge::{MergeHistory, MergePreviewDto, MergeWhy, open_received_copy};
use gt_session::*;
use gt_store::Project;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use uuid::Uuid;

const LATER_MS: i64 = NOW_MS + 86_400_000 * 40;
const EARLIER_MS: i64 = NOW_MS - 86_400_000 * 40;

fn koval(given: &str, patronymic: Option<&str>) -> PersonDraft {
    PersonDraft {
        given: Some(given.into()),
        surnames: vec!["Koval".into()],
        patronymic: patronymic.map(Into::into),
        sex: SexKind::Unknown,
        birth_date_raw: None,
        birth_place_text: None,
    }
}

fn add(s: &mut Session, given: &str) -> Uuid {
    s.apply(EditCommand::CreatePerson {
        draft: koval(given, None),
    })
    .unwrap()
    .focus
    .unwrap()
}

fn set_sex(s: &mut Session, id: Uuid, sex: SexKind) {
    s.apply(EditCommand::UpdatePerson {
        id,
        sex: Some(sex),
        sex_text: None,
        living_override: LivingPatch::Keep,
        private: None,
    })
    .unwrap();
}

fn set_private(s: &mut Session, id: Uuid, private: bool) {
    s.apply(EditCommand::UpdatePerson {
        id,
        sex: None,
        sex_text: None,
        living_override: LivingPatch::Keep,
        private: Some(private),
    })
    .unwrap();
}

fn set_living(s: &mut Session, id: Uuid, living: LivingPatch) {
    s.apply(EditCommand::UpdatePerson {
        id,
        sex: None,
        sex_text: None,
        living_override: living,
        private: None,
    })
    .unwrap();
}

fn add_note(s: &mut Session, id: Uuid, text: &str) {
    s.apply(EditCommand::SaveNote {
        target: OwnerRef {
            kind: OwnerKind::Person,
            id,
        },
        note: NoteInput {
            id: None,
            kind: NoteKindDto::General,
            text: text.into(),
        },
    })
    .unwrap();
}

fn sex_of(s: &Session, id: Uuid) -> gt_core::model::Sex {
    s.tree().persons[&id].sex.clone()
}

fn open_at(dir: &Path, ms: i64) -> Session {
    Session::open(dir, SessionClock::fixed(today(), ms)).unwrap()
}

/// The owner's tree with Ivan and Olena.
struct Family {
    dir: TempDir,
    owner: Session,
    ivan: Uuid,
    olena: Uuid,
}

fn family() -> Family {
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("owner");
    drop(Project::create(&proj).unwrap());
    let mut owner = open_at(&proj, NOW_MS);
    let ivan = owner
        .apply(EditCommand::CreatePerson {
            draft: koval("Ivan", Some("Petrovych")),
        })
        .unwrap()
        .focus
        .unwrap();
    let olena = add(&mut owner, "Olena");
    Family {
        dir,
        owner,
        ivan,
        olena,
    }
}

impl Family {
    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Sends a zip of the owner's tree and opens it as the relative at `ms`.
    fn relative(&self, name: &str, ms: i64) -> Session {
        let zip = self.path(&format!("{name}-in.zip"));
        self.owner.save_copy_zip(&zip).unwrap();
        let dest = self.path(name);
        open_received_copy(&zip, &dest).unwrap();
        open_at(&dest, ms)
    }
}

fn send(rel: &Session, dir: &Family, name: &str) -> PathBuf {
    let zip = dir.path(name);
    rel.save_copy_zip(&zip).unwrap();
    zip
}

fn preview(s: &mut Session, zip: &Path) -> MergePreviewDto {
    s.merge_preview(zip, "Olena's copy").unwrap()
}

fn groups(s: &Session) -> usize {
    s.project().log_groups_in_order().unwrap().len()
}

// ---- must never happen ---------------------------------------------------------------------

#[test]
fn a_record_missing_from_the_copy_is_not_a_deletion() {
    let mut f = family();
    let rel = f.relative("rel", LATER_MS);
    // The owner adds someone after the copy was made; the copy has never heard of them.
    let late = add(&mut f.owner, "Taras");
    let mut rel = rel;
    let ivan = f.ivan;
    set_sex(&mut rel, ivan, SexKind::Male);
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    assert_eq!(p.history, MergeHistory::Full);
    assert!(p.removed.is_empty());
    f.owner.merge_apply(p.token).unwrap();
    assert!(f.owner.tree().persons.contains_key(&late), "Taras stays");
    assert_eq!(sex_of(&f.owner, ivan), gt_core::model::Sex::Male);
}

#[test]
fn a_removal_in_the_copy_is_listed_and_never_applied() {
    let mut f = family();
    let mut rel = f.relative("rel", LATER_MS);
    rel.apply(EditCommand::DeletePerson { id: f.olena })
        .unwrap();
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    assert_eq!(p.removed.len(), 1, "{p:?}");
    assert_eq!(p.to_apply, 0);
    let r = f.owner.merge_apply(p.token).unwrap();
    assert_eq!(r.added + r.changed, 0);
    assert!(f.owner.tree().persons.contains_key(&f.olena));
}

#[test]
fn a_field_changed_on_both_sides_is_listed_and_the_owners_stays() {
    let mut f = family();
    let mut rel = f.relative("rel", LATER_MS);
    let ivan = f.ivan;
    set_sex(&mut rel, ivan, SexKind::Female);
    set_sex(&mut f.owner, ivan, SexKind::Male);
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    assert_eq!(p.undecided.len(), 1, "{p:?}");
    assert_eq!(p.undecided[0].why, Some(MergeWhy::BothChanged));
    assert!(
        !p.undecided[0].yours.is_empty(),
        "shows what the owner changed"
    );
    f.owner.merge_apply(p.token).unwrap();
    assert_eq!(sex_of(&f.owner, ivan), gt_core::model::Sex::Male);
}

#[test]
fn different_fields_of_one_person_both_survive() {
    let mut f = family();
    let mut rel = f.relative("rel", LATER_MS);
    let ivan = f.ivan;
    set_sex(&mut rel, ivan, SexKind::Male);
    set_private(&mut f.owner, ivan, true);
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    assert_eq!(p.changed.len(), 1, "{p:?}");
    assert!(p.changed[0].also_edited_here);
    f.owner.merge_apply(p.token).unwrap();
    let ivan_rec = &f.owner.tree().persons[&ivan];
    assert_eq!(ivan_rec.sex, gt_core::model::Sex::Male);
    assert!(
        ivan_rec.common.private,
        "the owner's own edit is still there"
    );
}

#[test]
fn the_clock_decides_nothing() {
    for ms in [EARLIER_MS, LATER_MS] {
        let mut f = family();
        let mut rel = f.relative("rel", ms);
        let ivan = f.ivan;
        set_sex(&mut rel, ivan, SexKind::Female);
        set_sex(&mut f.owner, ivan, SexKind::Male);
        let zip = send(&rel, &f, "back.zip");
        let p = preview(&mut f.owner, &zip);
        assert_eq!(p.undecided.len(), 1, "clock {ms}: {p:?}");
        f.owner.merge_apply(p.token).unwrap();
        assert_eq!(
            sex_of(&f.owner, ivan),
            gt_core::model::Sex::Male,
            "clock {ms}"
        );
    }
}

#[test]
fn making_a_private_person_public_is_never_taken_by_itself() {
    let mut f = family();
    let ivan = f.ivan;
    set_private(&mut f.owner, ivan, true);
    let mut rel = f.relative("rel", LATER_MS);
    set_private(&mut rel, ivan, false);
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    assert_eq!(p.undecided.len(), 1, "{p:?}");
    assert_eq!(p.undecided[0].why, Some(MergeWhy::MoreVisible));
    f.owner.merge_apply(p.token).unwrap();
    assert!(f.owner.tree().persons[&ivan].common.private);
}

#[test]
fn loosening_a_living_override_is_never_taken_by_itself() {
    let mut f = family();
    let ivan = f.ivan;
    set_living(
        &mut f.owner,
        ivan,
        LivingPatch::Set(LivingOverrideDto::Living),
    );
    let mut rel = f.relative("rel", LATER_MS);
    set_living(
        &mut rel,
        ivan,
        LivingPatch::Set(LivingOverrideDto::Deceased),
    );
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    assert_eq!(p.undecided.len(), 1, "{p:?}");
    assert_eq!(p.undecided[0].why, Some(MergeWhy::MoreVisible));
    f.owner.merge_apply(p.token).unwrap();
    assert_eq!(
        f.owner.tree().persons[&ivan].living_override,
        Some(gt_core::model::LivingOverride::Living)
    );
}

#[test]
fn making_someone_more_private_is_taken() {
    let mut f = family();
    let mut rel = f.relative("rel", LATER_MS);
    let ivan = f.ivan;
    set_private(&mut rel, ivan, true);
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    assert_eq!(p.changed.len(), 1, "{p:?}");
    f.owner.merge_apply(p.token).unwrap();
    assert!(f.owner.tree().persons[&ivan].common.private);
}

#[test]
fn an_edit_between_preview_and_apply_forces_a_new_preview() {
    let mut f = family();
    let mut rel = f.relative("rel", LATER_MS);
    let ivan = f.ivan;
    set_sex(&mut rel, ivan, SexKind::Male);
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    let olena = f.olena;
    set_sex(&mut f.owner, olena, SexKind::Female);
    let err = f.owner.merge_apply(p.token).unwrap_err();
    assert_eq!(err.code, ErrorCode::Stale);
    assert_eq!(sex_of(&f.owner, ivan), gt_core::model::Sex::Unknown);
    // A fresh preview works.
    let p = preview(&mut f.owner, &zip);
    f.owner.merge_apply(p.token).unwrap();
    assert_eq!(sex_of(&f.owner, ivan), gt_core::model::Sex::Male);
}

#[test]
fn a_token_from_another_preview_is_refused() {
    let mut f = family();
    let mut rel = f.relative("rel", LATER_MS);
    let ivan = f.ivan;
    set_sex(&mut rel, ivan, SexKind::Male);
    let zip = send(&rel, &f, "back.zip");
    preview(&mut f.owner, &zip);
    let err = f.owner.merge_apply(gt_core::ids::new_id()).unwrap_err();
    assert_eq!(err.code, ErrorCode::Stale);
    assert_eq!(sex_of(&f.owner, ivan), gt_core::model::Sex::Unknown);
}

#[test]
fn merging_the_same_copy_twice_changes_nothing_and_the_source_is_never_touched() {
    let mut f = family();
    let mut rel = f.relative("rel", LATER_MS);
    let ivan = f.ivan;
    set_sex(&mut rel, ivan, SexKind::Male);
    let taras = add(&mut rel, "Taras");
    add_note(&mut rel, taras, "Lived in Lviv.");
    let zip = send(&rel, &f, "back.zip");
    let bytes_before = std::fs::read(&zip).unwrap();

    let p = preview(&mut f.owner, &zip);
    assert!(p.to_apply >= 3, "{p:?}");
    f.owner.merge_apply(p.token).unwrap();
    let tree_after_first = f.owner.tree().clone();
    let log_after_first = groups(&f.owner);

    let p2 = preview(&mut f.owner, &zip);
    assert!(p2.nothing_new, "{p2:?}");
    let r2 = f.owner.merge_apply(p2.token).unwrap();
    assert_eq!(r2.added + r2.changed, 0);
    assert_eq!(f.owner.tree(), &tree_after_first);
    assert_eq!(groups(&f.owner), log_after_first);
    assert_eq!(
        std::fs::read(&zip).unwrap(),
        bytes_before,
        "the zip is not modified"
    );
}

#[test]
fn a_folder_source_is_not_modified_either() {
    let mut f = family();
    let mut rel = f.relative("rel", LATER_MS);
    let ivan = f.ivan;
    set_sex(&mut rel, ivan, SexKind::Male);
    let copy = f.path("folder-copy");
    rel.save_copy(&copy).unwrap();
    let snapshot = |dir: &Path| -> Vec<(String, Vec<u8>)> {
        let mut v: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap())
            .filter(|e| e.file_type().unwrap().is_file())
            .map(|e| {
                (
                    e.file_name().to_string_lossy().into_owned(),
                    std::fs::read(e.path()).unwrap(),
                )
            })
            .collect();
        v.sort();
        v
    };
    let before = snapshot(&copy);
    let p = f.owner.merge_preview(&copy, "Olena").unwrap();
    f.owner.merge_apply(p.token).unwrap();
    assert_eq!(snapshot(&copy), before);
    assert_eq!(sex_of(&f.owner, ivan), gt_core::model::Sex::Male);
}

#[test]
fn apply_is_all_or_nothing() {
    let mut f = family();
    let mut rel = f.relative("rel", LATER_MS);
    let ivan = f.ivan;
    set_sex(&mut rel, ivan, SexKind::Male);
    let taras = add(&mut rel, "Taras");
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    assert!(p.to_apply >= 2);
    // Something goes wrong inside the transaction: a ledger row for one of the groups appears
    // after the preview. Nothing from the merge may stay.
    let before = f.owner.tree().clone();
    let log_before = f.owner.project().log_stamp().unwrap();
    let their_group: String = {
        let rel_proj = rel.project();
        let g = rel_proj.log_groups_in_order().unwrap();
        g.last().unwrap().to_string()
    };
    f.owner
        .project()
        .conn()
        .execute(
            "INSERT INTO merged_group(source_group, merge_group, batch_id, source_name, merged_ts) \
             VALUES (?1, NULL, 'x', 'x', 0)",
            [their_group],
        )
        .unwrap();
    assert!(f.owner.merge_apply(p.token).is_err());
    assert_eq!(f.owner.tree(), &before);
    assert_eq!(f.owner.project().log_stamp().unwrap(), log_before);
    assert!(!f.owner.tree().persons.contains_key(&taras));
    f.owner.verify_derived_state().unwrap();
    // The reopened database agrees.
    let dir = f.owner.project().dir.clone();
    drop(f.owner);
    let again = open_at(&dir, NOW_MS);
    assert_eq!(again.tree(), &before);
}

// ---- the whole trip ------------------------------------------------------------------------

#[test]
fn send_open_edit_send_back_merge_undo() {
    let mut f = family();
    let mut rel = f.relative("rel", LATER_MS);
    let ivan = f.ivan;
    set_sex(&mut rel, ivan, SexKind::Male);
    let taras = add(&mut rel, "Taras");
    add_note(&mut rel, taras, "Born in Lviv.");
    let zip = send(&rel, &f, "back.zip");
    drop(rel);

    let owner_before = f.owner.tree().clone();
    let p = preview(&mut f.owner, &zip);
    assert!(p.added.iter().any(|l| l.change.kind == "person"));
    let r = f.owner.merge_apply(p.token).unwrap();
    assert!(r.added >= 2 && r.changed >= 1, "{r:?}");
    assert!(f.owner.tree().persons.contains_key(&taras));
    assert_eq!(sex_of(&f.owner, ivan), gt_core::model::Sex::Male);
    // The change log names the copy as the author.
    let (g, _) = f
        .owner
        .project()
        .log_page(
            &gt_store::LogFilter {
                batch: Some(r.batch),
                ..Default::default()
            },
            0,
            100,
        )
        .unwrap();
    assert!(g.iter().all(
        |x| matches!(&x.actor, Some(gt_store::Actor::Copy { name }) if name == "Olena's copy")
    ));
    // A backup was made first.
    let backups = std::fs::read_dir(f.owner.project().dir.join("backups"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().contains("before-merge"))
        .count();
    assert_eq!(backups, 1);

    let report = f.owner.undo_merge(r.batch).unwrap();
    assert!(report.skipped.is_empty(), "{report:?}");
    assert_eq!(f.owner.tree().persons.len(), owner_before.persons.len());
    assert!(!f.owner.tree().persons.contains_key(&taras));
    assert_eq!(sex_of(&f.owner, ivan), gt_core::model::Sex::Unknown);
    f.owner.verify_derived_state().unwrap();
    // Undone in full: the same copy is offered again.
    let again = preview(&mut f.owner, &zip);
    assert!(again.to_apply >= 3, "{again:?}");
}

#[test]
fn a_merge_undone_but_left_in_the_ledger_is_offered_again() {
    let mut f = family();
    let ivan = f.ivan;
    let mut rel = f.relative("rel", LATER_MS);
    set_sex(&mut rel, ivan, SexKind::Male);
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    let r = f.owner.merge_apply(p.token).unwrap();
    // A crash between the undo and the ledger clean-up: the undo is done, the ledger stays.
    let report = f.owner.undo_batch(r.batch).unwrap();
    assert!(report.skipped.is_empty(), "{report:?}");
    assert!(
        !f.owner
            .project()
            .merged_groups_of(r.batch)
            .unwrap()
            .is_empty(),
        "the ledger was left in place"
    );
    let again = preview(&mut f.owner, &zip);
    assert_eq!(again.to_apply, 1, "{again:?}");
    assert!(!again.nothing_new, "{again:?}");
}

// ---- a copy is untrusted --------------------------------------------------------------------

#[test]
fn a_tree_that_is_not_a_copy_of_this_one_is_refused() {
    let mut f = family();
    let other = tempfile::tempdir().unwrap();
    let proj = other.path().join("other");
    drop(Project::create(&proj).unwrap());
    let mut o = open_at(&proj, NOW_MS);
    add(&mut o, "Stranger");
    let zip = other.path().join("other.zip");
    o.save_copy_zip(&zip).unwrap();
    let err = f.owner.merge_preview(&zip, "x").unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid, "{err:?}");
}

#[test]
fn a_copy_without_history_is_shown_for_deciding_and_removes_nothing() {
    let mut f = family();
    let mut rel = f.relative("rel", LATER_MS);
    let ivan = f.ivan;
    set_sex(&mut rel, ivan, SexKind::Male);
    let taras = add(&mut rel, "Taras");
    rel.apply(EditCommand::DeletePerson { id: f.olena })
        .unwrap();
    let dir = rel.project().dir.clone();
    drop(rel);
    // Strip the log, as an older app or an import would leave it.
    let c = rusqlite::Connection::open(dir.join("tree.db")).unwrap();
    c.execute_batch("DELETE FROM change_person; DELETE FROM change_log;")
        .unwrap();
    drop(c);
    let rel = open_at(&dir, LATER_MS);
    let zip = send(&rel, &f, "stripped.zip");
    let p = preview(&mut f.owner, &zip);
    assert_eq!(p.history, MergeHistory::None);
    assert!(p.removed.is_empty());
    assert_eq!(
        p.added.iter().filter(|l| l.change.kind == "person").count(),
        1
    );
    assert!(
        p.undecided
            .iter()
            .all(|l| l.why == Some(MergeWhy::NoHistory))
    );
    assert!(p.undecided.iter().any(|l| l.change.id == ivan));
    f.owner.merge_apply(p.token).unwrap();
    assert!(f.owner.tree().persons.contains_key(&taras));
    assert!(
        f.owner.tree().persons.contains_key(&f.olena),
        "never a removal"
    );
    assert_eq!(
        sex_of(&f.owner, ivan),
        gt_core::model::Sex::Unknown,
        "left for the owner"
    );
}

#[test]
fn a_tampered_log_falls_back_to_the_no_base_rule() {
    let mut f = family();
    let mut rel = f.relative("rel", LATER_MS);
    let ivan = f.ivan;
    set_sex(&mut rel, ivan, SexKind::Female);
    let dir = rel.project().dir.clone();
    drop(rel);
    // The copy's record is changed without a log row: its log no longer explains it.
    let c = rusqlite::Connection::open(dir.join("tree.db")).unwrap();
    let json: String = c
        .query_row(
            "SELECT after_json FROM change_log WHERE target_id = ?1 ORDER BY id DESC LIMIT 1",
            [ivan.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    let tampered = json.replace("Female", "Male");
    c.execute(
        "UPDATE change_log SET after_json = ?1 WHERE id = (SELECT MAX(id) FROM change_log WHERE target_id = ?2)",
        [&tampered, &ivan.to_string()],
    )
    .unwrap();
    drop(c);
    let rel = open_at(&dir, LATER_MS);
    let zip = send(&rel, &f, "tampered.zip");
    let p = preview(&mut f.owner, &zip);
    // Owner never edited Ivan: the record differs and nothing explains it, so it is not taken
    // silently.
    assert_eq!(p.undecided.len(), 1, "{p:?}");
    assert_eq!(p.undecided[0].why, Some(MergeWhy::NoHistory));
}

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    use std::io::Write;
    let f = std::fs::File::create(path).unwrap();
    let mut z = zip::ZipWriter::new(f);
    for (name, data) in entries {
        z.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(data).unwrap();
    }
    z.finish().unwrap();
}

#[test]
fn hostile_zips_are_refused_in_plain_words() {
    let mut f = family();
    let dir = tempfile::tempdir().unwrap();
    type Entries<'a> = Vec<(&'a str, &'a [u8])>;
    let cases: Vec<(&str, Entries)> = vec![
        ("traversal", vec![("tree.db", b"x"), ("../evil.txt", b"x")]),
        ("no database", vec![("readme.txt", b"hi")]),
        (
            "garbage database",
            vec![("tree.db", b"this is not sqlite at all, not even close")],
        ),
    ];
    for (name, entries) in cases {
        let zip = dir.path().join(format!("{name}.zip"));
        write_zip(&zip, &entries);
        let err = f.owner.merge_preview(&zip, "x").unwrap_err();
        assert_eq!(err.code, ErrorCode::Invalid, "{name}: {err:?}");
        assert!(!err.message.contains("rusqlite"), "{name}: {}", err.message);
        assert!(!dir.path().join("evil.txt").exists());
        assert!(!dir.path().parent().unwrap().join("evil.txt").exists());
    }
    let notzip = dir.path().join("plain.zip");
    std::fs::write(&notzip, b"nope").unwrap();
    assert!(f.owner.merge_preview(&notzip, "x").is_err());
    assert!(
        f.owner
            .merge_preview(&dir.path().join("missing.zip"), "x")
            .is_err()
    );
}

#[test]
fn a_copy_from_a_newer_app_is_refused_with_a_plain_message() {
    let mut f = family();
    let rel = f.relative("rel", LATER_MS);
    let dir = rel.project().dir.clone();
    drop(rel);
    let c = rusqlite::Connection::open(dir.join("tree.db")).unwrap();
    c.pragma_update(None, "user_version", 99).unwrap();
    drop(c);
    let err = f.owner.merge_preview(&dir, "x").unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid);
    assert!(err.message.contains("newer Gene Tree"), "{}", err.message);
}

#[test]
fn a_database_with_triggers_is_refused() {
    let mut f = family();
    let rel = f.relative("rel", LATER_MS);
    let dir = rel.project().dir.clone();
    drop(rel);
    let c = rusqlite::Connection::open(dir.join("tree.db")).unwrap();
    c.execute_batch("CREATE TRIGGER sneaky AFTER INSERT ON meta BEGIN SELECT 1; END;")
        .unwrap();
    drop(c);
    assert!(f.owner.merge_preview(&dir, "x").is_err());
}

#[test]
fn a_zip_bomb_is_refused() {
    let mut f = family();
    let dir = tempfile::tempdir().unwrap();
    let zip = dir.path().join("bomb.zip");
    let big = vec![0u8; 64 << 20];
    use std::io::Write;
    let file = std::fs::File::create(&zip).unwrap();
    let mut z = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    z.start_file("tree.db", opts).unwrap();
    z.write_all(&big).unwrap();
    z.finish().unwrap();
    assert!(f.owner.merge_preview(&zip, "x").is_err());
}

// ---- the zip itself --------------------------------------------------------------------------

#[test]
fn the_zip_holds_the_database_and_media_and_names_its_size() {
    let f = family();
    let size = f.owner.copy_size().unwrap();
    assert_eq!(size.people, 2);
    let zip = f.path("copy.zip");
    let made = f.owner.save_copy_zip(&zip).unwrap();
    assert_eq!(made.bytes, std::fs::metadata(&zip).unwrap().len());
    let mut names: Vec<String> = zip::ZipArchive::new(std::fs::File::open(&zip).unwrap())
        .unwrap()
        .file_names_raw()
        .map(|n| String::from_utf8_lossy(n).into_owned())
        .collect();
    names.sort();
    assert!(names.contains(&"tree.db".to_string()), "{names:?}");
    assert!(
        names
            .iter()
            .all(|n| n == "tree.db" || n.starts_with("media/")),
        "{names:?}"
    );
    // Not inside the tree's own folder.
    let inside = f.owner.project().dir.join("copy.zip");
    assert!(f.owner.save_copy_zip(&inside).is_err());
    // Received into a folder that has files in it: refused.
    let busy = f.path("busy");
    std::fs::create_dir_all(&busy).unwrap();
    std::fs::write(busy.join("x.txt"), "x").unwrap();
    assert!(open_received_copy(&zip, &busy).is_err());
    // A good one opens as a project.
    let dest = f.path("received");
    open_received_copy(&zip, &dest).unwrap();
    let s = open_at(&dest, NOW_MS);
    assert_eq!(s.tree().persons.len(), 2);
}

// ---- a swap is a loss; a plain edit is not -----------------------------------------------------

fn event_input(id: Option<Uuid>, kind: &str, date: &str, place: Option<&str>) -> EventInput {
    EventInput {
        id,
        kind: kind.into(),
        date_raw: Some(date.into()),
        place_text: place.map(Into::into),
        place_id: None,
        description: None,
        age: None,
        cause: None,
        negative: false,
        private: false,
        preferred: true,
        participants: None,
    }
}

fn save_event(s: &mut Session, person: Uuid, ev: EventInput) {
    s.apply(EditCommand::SaveEvent {
        owner: EventOwner {
            kind: EventOwnerKind::Person,
            id: person,
        },
        event: ev,
    })
    .unwrap();
}

fn name_input(
    id: Option<Uuid>,
    given: &str,
    surnames: &[&str],
    patronymic: Option<&str>,
) -> NameInput {
    NameInput {
        id,
        kind: NameKindDto::Birth,
        kind_text: None,
        preferred: false,
        private: false,
        prefix: None,
        given: Some(given.into()),
        call: None,
        surname_prefix: None,
        surnames: surnames.iter().map(|s| (*s).into()).collect(),
        patronymic: patronymic.map(Into::into),
        suffix: None,
        nickname: None,
        lang: None,
        date_raw: None,
    }
}

fn why_of(p: &MergePreviewDto) -> Vec<MergeWhy> {
    p.undecided.iter().filter_map(|l| l.why).collect()
}

#[test]
fn a_name_swapped_for_another_is_a_loss_not_a_change() {
    let mut f = family();
    let ivan = f.ivan;
    let old_name = f.owner.tree().persons[&ivan].names[0].id;
    let mut rel = f.relative("rel", LATER_MS);
    rel.apply(EditCommand::SaveName {
        person: ivan,
        name: name_input(None, "Ivan", &["Kovalenko"], None),
    })
    .unwrap();
    rel.apply(EditCommand::DeleteName {
        person: ivan,
        name: old_name,
    })
    .unwrap();
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    assert_eq!(why_of(&p), vec![MergeWhy::TakesAway], "{p:?}");
    assert_eq!(p.to_apply, 0, "{p:?}");
    f.owner.merge_apply(p.token).unwrap();
    let names = &f.owner.tree().persons[&ivan].names;
    assert!(
        names.iter().any(|n| n.id == old_name),
        "the owner's name stays"
    );
}

#[test]
fn a_child_link_swapped_for_another_is_a_loss_not_a_change() {
    let mut f = family();
    let ivan = f.ivan;
    let petro = f
        .owner
        .apply(EditCommand::QuickAdd {
            anchor: ivan,
            relation: Relation::Child,
            draft: koval("Petro", None),
            family: None,
            new_parent_set: None,
            parent_rel: None,
        })
        .unwrap()
        .focus
        .unwrap();
    let taras = add(&mut f.owner, "Taras");
    let fam = *f.owner.tree().families.keys().next().unwrap();
    let mut rel = f.relative("rel", LATER_MS);
    rel.apply(EditCommand::UnlinkChild {
        family: fam,
        child: petro,
    })
    .unwrap();
    rel.apply(EditCommand::LinkChild {
        family: fam,
        child: taras,
        rel: (ParentRelId::Birth, ParentRelId::Birth),
        preferred: true,
    })
    .unwrap();
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    assert!(why_of(&p).contains(&MergeWhy::TakesAway), "{p:?}");
    f.owner.merge_apply(p.token).unwrap();
    assert!(
        f.owner.tree().families[&fam]
            .children
            .iter()
            .any(|c| c.person == petro),
        "the owner's child link stays"
    );
}

#[test]
fn clearing_a_place_is_a_loss_not_a_change() {
    let mut f = family();
    let ivan = f.ivan;
    save_event(
        &mut f.owner,
        ivan,
        event_input(None, "birth", "1 Jan 1990", Some("Lviv")),
    );
    let ev = *f.owner.tree().events.keys().next().unwrap();
    let mut rel = f.relative("rel", LATER_MS);
    save_event(
        &mut rel,
        ivan,
        event_input(Some(ev), "birth", "1 Jan 1990", Some("")),
    );
    assert!(rel.tree().events[&ev].place.is_none());
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    assert_eq!(why_of(&p), vec![MergeWhy::TakesAway], "{p:?}");
    f.owner.merge_apply(p.token).unwrap();
    assert!(
        f.owner.tree().events[&ev].place.is_some(),
        "the place stays"
    );
}

#[test]
fn correcting_a_surname_and_a_date_still_applies() {
    let mut f = family();
    let ivan = f.ivan;
    save_event(
        &mut f.owner,
        ivan,
        event_input(None, "birth", "1 Jan 1990", None),
    );
    let ev = *f.owner.tree().events.keys().next().unwrap();
    let name = f.owner.tree().persons[&ivan].names[0].id;
    let mut rel = f.relative("rel", LATER_MS);
    rel.apply(EditCommand::SaveName {
        person: ivan,
        name: name_input(Some(name), "Ivan", &["Kovalenko"], Some("Petrovych")),
    })
    .unwrap();
    save_event(
        &mut rel,
        ivan,
        event_input(Some(ev), "birth", "2 Jan 1990", None),
    );
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    assert!(p.undecided.is_empty(), "{p:?}");
    assert_eq!(p.changed.len(), 2, "{p:?}");
    f.owner.merge_apply(p.token).unwrap();
    assert_eq!(
        f.owner.tree().persons[&ivan].names[0].surnames,
        vec!["Kovalenko".to_string()]
    );
    assert_eq!(f.owner.tree().events[&ev].date, rel.tree().events[&ev].date);
}

// ---- who stops being living -----------------------------------------------------------------------

fn with_birth() -> (Family, Uuid) {
    let mut f = family();
    let ivan = f.ivan;
    save_event(
        &mut f.owner,
        ivan,
        event_input(None, "birth", "1 Jan 1990", None),
    );
    let ev = *f.owner.tree().events.keys().next().unwrap();
    (f, ev)
}

#[test]
fn a_birth_date_that_makes_a_living_person_dead_is_held() {
    let (mut f, ev) = with_birth();
    let ivan = f.ivan;
    let mut rel = f.relative("rel", LATER_MS);
    save_event(
        &mut rel,
        ivan,
        event_input(Some(ev), "birth", "1 Jan 1850", None),
    );
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    assert_eq!(why_of(&p), vec![MergeWhy::MoreVisible], "{p:?}");
    assert_eq!(p.to_apply, 0);
    let kept = f.owner.tree().events[&ev].date.clone();
    f.owner.merge_apply(p.token).unwrap();
    assert_eq!(
        f.owner.tree().events[&ev].date,
        kept,
        "the owner's date stays"
    );
}

#[test]
fn a_death_event_added_to_a_living_person_is_held() {
    let (mut f, _) = with_birth();
    let ivan = f.ivan;
    let mut rel = f.relative("rel", LATER_MS);
    save_event(
        &mut rel,
        ivan,
        event_input(None, "death", "1 Jan 2020", None),
    );
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    assert!(
        why_of(&p).iter().all(|w| *w == MergeWhy::MoreVisible),
        "{p:?}"
    );
    assert!(!p.undecided.is_empty(), "{p:?}");
    assert_eq!(p.to_apply, 0, "{p:?}");
    f.owner.merge_apply(p.token).unwrap();
    assert_eq!(f.owner.tree().events.len(), 1, "no death event came across");
    assert!(f.owner.tree().persons.contains_key(&ivan));
}

#[test]
fn another_change_to_the_same_person_still_applies() {
    // The date flips them, the sex change does not: only the date is held.
    let (mut f, ev) = with_birth();
    let ivan = f.ivan;
    let mut rel = f.relative("rel", LATER_MS);
    set_sex(&mut rel, ivan, SexKind::Male);
    save_event(
        &mut rel,
        ivan,
        event_input(Some(ev), "birth", "1 Jan 1850", None),
    );
    let zip = send(&rel, &f, "back.zip");
    let p = preview(&mut f.owner, &zip);
    assert_eq!(why_of(&p), vec![MergeWhy::MoreVisible], "{p:?}");
    assert_eq!(p.changed.len(), 1, "{p:?}");
    f.owner.merge_apply(p.token).unwrap();
    assert_eq!(sex_of(&f.owner, ivan), gt_core::model::Sex::Male);
    // A date that leaves them living is an ordinary change.
    let (mut f, ev) = with_birth();
    let ivan = f.ivan;
    let mut rel = f.relative("rel2", LATER_MS);
    save_event(
        &mut rel,
        ivan,
        event_input(Some(ev), "birth", "1 Jan 1991", None),
    );
    let zip = send(&rel, &f, "back2.zip");
    let p = preview(&mut f.owner, &zip);
    assert!(p.undecided.is_empty(), "{p:?}");
    assert_eq!(p.changed.len(), 1, "{p:?}");
}
