#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! More merge cases: photos, records that point at something removed, an older owner tree, an
//! older schema in the copy, and the property that no change is lost silently.

mod common;

use common::*;
use gt_session::merge::{MergeHistory, MergePreviewDto, MergeWhy, open_received_copy};
use gt_session::*;
use gt_store::Project;
use proptest::prelude::*;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use uuid::Uuid;

const LATER_MS: i64 = NOW_MS + 86_400_000 * 40;

fn koval(given: &str) -> PersonDraft {
    PersonDraft {
        given: Some(given.into()),
        surnames: vec!["Koval".into()],
        patronymic: None,
        sex: SexKind::Unknown,
        birth_date_raw: None,
        birth_place_text: None,
    }
}

fn add(s: &mut Session, given: &str) -> Uuid {
    s.apply(EditCommand::CreatePerson {
        draft: koval(given),
    })
    .unwrap()
    .focus
    .unwrap()
}

fn update(s: &mut Session, id: Uuid, sex: Option<SexKind>, private: Option<bool>) {
    s.apply(EditCommand::UpdatePerson {
        id,
        sex,
        sex_text: None,
        living_override: LivingPatch::Keep,
        private,
    })
    .unwrap();
}

fn open_at(dir: &Path, ms: i64) -> Session {
    Session::open(dir, SessionClock::fixed(today(), ms)).unwrap()
}

fn new_owner(dir: &TempDir, name: &str) -> Session {
    let proj = dir.path().join(name);
    drop(Project::create(&proj).unwrap());
    open_at(&proj, NOW_MS)
}

fn relative_of(owner: &Session, dir: &TempDir, name: &str) -> Session {
    let zip = dir.path().join(format!("{name}-in.zip"));
    owner.save_copy_zip(&zip).unwrap();
    let dest = dir.path().join(name);
    open_received_copy(&zip, &dest).unwrap();
    open_at(&dest, LATER_MS)
}

fn send(rel: &Session, dir: &TempDir, name: &str) -> PathBuf {
    let zip = dir.path().join(name);
    rel.save_copy_zip(&zip).unwrap();
    zip
}

fn preview(s: &mut Session, zip: &Path) -> MergePreviewDto {
    s.merge_preview(zip, "Olena's copy").unwrap()
}

/// Rewrites a zip, letting `f` change or drop each entry.
fn rewrite_zip(src: &Path, dst: &Path, f: impl Fn(&str, Vec<u8>) -> Option<Vec<u8>>) {
    use std::io::{Read, Write};
    let mut a = zip::ZipArchive::new(std::fs::File::open(src).unwrap()).unwrap();
    let mut w = zip::ZipWriter::new(std::fs::File::create(dst).unwrap());
    for i in 0..a.len() {
        let mut e = a.by_index(i).unwrap();
        let name = e.name().unwrap().to_string();
        let mut data = Vec::new();
        e.read_to_end(&mut data).unwrap();
        if let Some(data) = f(&name, data) {
            w.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(&data).unwrap();
        }
    }
    w.finish().unwrap();
}

fn pdf(body: &str) -> Vec<u8> {
    format!("%PDF-1.4\n%{body}\n").into_bytes()
}

/// The relative attaches a document to Ivan; the zip they send back.
fn with_photo(dir: &TempDir, owner: &Session, ivan: Uuid) -> (Session, PathBuf) {
    let mut rel = relative_of(owner, dir, "rel");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    let file = src.join("letter.pdf");
    std::fs::write(&file, pdf("a letter from 1911")).unwrap();
    let r = rel
        .attach_files(
            OwnerRef {
                kind: OwnerKind::Person,
                id: ivan,
            },
            &[file],
        )
        .unwrap();
    assert_eq!(r.attached.len(), 1, "{:?}", r.failed);
    let zip = send(&rel, dir, "back.zip");
    (rel, zip)
}

fn media_files(s: &Session) -> usize {
    fn count(p: &Path) -> usize {
        std::fs::read_dir(p).map_or(0, |d| {
            d.filter_map(Result::ok)
                .map(|e| {
                    if e.file_type().unwrap().is_dir() {
                        count(&e.path())
                    } else {
                        usize::from(!e.file_name().to_string_lossy().starts_with(".part-"))
                    }
                })
                .sum()
        })
    }
    count(&s.project().dir.join("media"))
}

// ---- photos ---------------------------------------------------------------------------------

#[test]
fn a_photo_comes_across_after_its_bytes_are_checked() {
    let dir = tempfile::tempdir().unwrap();
    let mut owner = new_owner(&dir, "owner");
    let ivan = add(&mut owner, "Ivan");
    let (_rel, zip) = with_photo(&dir, &owner, ivan);
    let p = preview(&mut owner, &zip);
    assert_eq!(p.photos, 1, "{p:?}");
    assert!(p.photo_bytes > 0);
    assert!(p.blocked.is_empty(), "{p:?}");
    let r = owner.merge_apply(p.token).unwrap();
    assert_eq!(r.photos_copied, 1);
    assert_eq!(media_files(&owner), 1);
    assert_eq!(owner.tree().media.len(), 1);
    assert_eq!(owner.tree().persons[&ivan].common.media.len(), 1);
}

#[test]
fn a_photo_whose_bytes_do_not_match_its_hash_is_blocked_with_what_needs_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut owner = new_owner(&dir, "owner");
    let ivan = add(&mut owner, "Ivan");
    let (_rel, zip) = with_photo(&dir, &owner, ivan);
    let bad = dir.path().join("bad.zip");
    rewrite_zip(&zip, &bad, |name, data| {
        Some(if name.starts_with("media/") {
            pdf("somebody swapped the bytes")
        } else {
            data
        })
    });
    let before = owner.tree().clone();
    let p = preview(&mut owner, &bad);
    assert!(
        p.blocked
            .iter()
            .any(|l| l.why == Some(MergeWhy::PhotoDamaged)),
        "{p:?}"
    );
    assert!(
        p.blocked
            .iter()
            .any(|l| l.why == Some(MergeWhy::NeedsMissing)),
        "Ivan's link to the photo waits too: {p:?}"
    );
    owner.merge_apply(p.token).unwrap();
    assert_eq!(owner.tree(), &before);
    assert_eq!(media_files(&owner), 0);
}

#[test]
fn a_photo_missing_from_the_copy_is_blocked() {
    let dir = tempfile::tempdir().unwrap();
    let mut owner = new_owner(&dir, "owner");
    let ivan = add(&mut owner, "Ivan");
    let (_rel, zip) = with_photo(&dir, &owner, ivan);
    let thin = dir.path().join("thin.zip");
    rewrite_zip(&zip, &thin, |name, data| {
        (!name.starts_with("media/")).then_some(data)
    });
    let p = preview(&mut owner, &thin);
    assert!(
        p.blocked
            .iter()
            .any(|l| l.why == Some(MergeWhy::PhotoMissing)),
        "{p:?}"
    );
}

#[test]
fn a_photo_path_outside_media_is_blocked_and_nothing_is_read_from_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut owner = new_owner(&dir, "owner");
    let ivan = add(&mut owner, "Ivan");
    let (rel, _zip) = with_photo(&dir, &owner, ivan);
    let rel_dir = rel.project().dir.clone();
    drop(rel);
    let c = rusqlite::Connection::open(rel_dir.join("tree.db")).unwrap();
    c.execute_batch("UPDATE media SET path = '../../outside.txt'; DELETE FROM change_person; DELETE FROM change_log;")
        .unwrap();
    drop(c);
    let rel = open_at(&rel_dir, LATER_MS);
    let zip = send(&rel, &dir, "evil.zip");
    let p = preview(&mut owner, &zip);
    assert_eq!(p.history, MergeHistory::None);
    assert!(
        p.blocked
            .iter()
            .any(|l| l.why == Some(MergeWhy::PhotoUnsafe)),
        "{p:?}"
    );
    owner.merge_apply(p.token).unwrap();
    assert!(owner.tree().media.is_empty());
}

// ---- records that point at something removed ---------------------------------------------------

#[test]
fn a_record_pointing_at_someone_the_owner_removed_is_not_written() {
    let dir = tempfile::tempdir().unwrap();
    let mut owner = new_owner(&dir, "owner");
    let olena = add(&mut owner, "Olena");
    let mut rel = relative_of(&owner, &dir, "rel");
    // The relative adds a child to Olena; the owner removes Olena meanwhile.
    rel.apply(EditCommand::QuickAdd {
        anchor: olena,
        relation: Relation::Child,
        draft: koval("Taras"),
        family: None,
        new_parent_set: None,
        parent_rel: None,
    })
    .unwrap();
    owner
        .apply(EditCommand::DeletePerson { id: olena })
        .unwrap();
    let zip = send(&rel, &dir, "back.zip");
    let p = preview(&mut owner, &zip);
    assert!(
        p.blocked
            .iter()
            .any(|l| l.why == Some(MergeWhy::NeedsMissing)),
        "{p:?}"
    );
    owner.merge_apply(p.token).unwrap();
    assert!(!owner.tree().persons.contains_key(&olena), "stays removed");
    for f in owner.tree().families.values() {
        for p in f.partner1.iter().chain(&f.partner2) {
            assert!(owner.tree().persons.contains_key(p), "no dangling partner");
        }
    }
    owner.verify_derived_state().unwrap();
}

#[test]
fn a_record_the_owner_removed_is_not_brought_back_by_an_old_copy() {
    let dir = tempfile::tempdir().unwrap();
    let mut owner = new_owner(&dir, "owner");
    let ivan = add(&mut owner, "Ivan");
    let olena = add(&mut owner, "Olena");
    let rel = relative_of(&owner, &dir, "rel");
    owner
        .apply(EditCommand::DeletePerson { id: olena })
        .unwrap();
    let zip = send(&rel, &dir, "back.zip");
    let p = preview(&mut owner, &zip);
    assert!(p.added.is_empty(), "{p:?}");
    owner.merge_apply(p.token).unwrap();
    assert!(!owner.tree().persons.contains_key(&olena));
    assert!(owner.tree().persons.contains_key(&ivan));
}

// ---- an older owner tree, an older copy -----------------------------------------------------------

#[test]
fn an_owner_who_restored_an_older_backup_still_takes_the_copys_additions() {
    let dir = tempfile::tempdir().unwrap();
    let mut owner = new_owner(&dir, "owner");
    let ivan = add(&mut owner, "Ivan");
    let backup = owner
        .project()
        .backup(gt_store::BackupReason::Manual, 10)
        .unwrap();
    let olena = add(&mut owner, "Olena");
    let mut rel = relative_of(&owner, &dir, "rel");
    update(&mut rel, ivan, Some(SexKind::Male), None);
    let zip = send(&rel, &dir, "back.zip");
    // The owner goes back to the backup: no Olena, and her group is not in this log.
    let proj = owner.project().dir.clone();
    drop(owner);
    for suffix in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(proj.join(format!("tree.db{suffix}")));
    }
    std::fs::copy(&backup, proj.join("tree.db")).unwrap();
    let mut owner = open_at(&proj, NOW_MS);
    assert!(!owner.tree().persons.contains_key(&olena));
    let p = preview(&mut owner, &zip);
    assert_eq!(p.history, MergeHistory::Full, "{p:?}");
    owner.merge_apply(p.token).unwrap();
    assert!(owner.tree().persons.contains_key(&olena));
    assert_eq!(owner.tree().persons[&ivan].sex, gt_core::model::Sex::Male);
}

#[test]
fn a_copy_saved_by_the_previous_schema_is_read_in_a_scratch_copy_only() {
    let dir = tempfile::tempdir().unwrap();
    let mut owner = new_owner(&dir, "owner");
    let ivan = add(&mut owner, "Ivan");
    let mut rel = relative_of(&owner, &dir, "rel");
    update(&mut rel, ivan, Some(SexKind::Male), None);
    let rel_dir = rel.project().dir.clone();
    drop(rel);
    let db = rel_dir.join("tree.db");
    let c = rusqlite::Connection::open(&db).unwrap();
    c.execute_batch("DROP TABLE merged_group; DROP TABLE merge_alias; PRAGMA user_version = 4;")
        .unwrap();
    drop(c);
    let before = std::fs::read(&db).unwrap();
    let p = owner.merge_preview(&rel_dir, "Olena").unwrap();
    assert_eq!(p.changed.len(), 1, "{p:?}");
    owner.merge_apply(p.token).unwrap();
    let after = std::fs::read(&db).unwrap();
    assert_eq!(before, after, "the copy was not migrated in place");
}

// ---- no change is lost silently -----------------------------------------------------------------

#[derive(Clone, Debug)]
enum Op {
    Sex(usize, u8),
    Private(usize, bool),
    Note(usize),
    Add,
    Delete(usize),
    /// Saves a new name and removes the first one, in one go.
    SwapName(usize),
}

fn name_input(given: &str) -> NameInput {
    NameInput {
        id: None,
        kind: NameKindDto::Birth,
        kind_text: None,
        preferred: false,
        private: false,
        prefix: None,
        given: Some(given.into()),
        call: None,
        surname_prefix: None,
        surnames: vec!["Koval".into()],
        patronymic: None,
        suffix: None,
        nickname: None,
        lang: None,
        date_raw: None,
    }
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0usize..4).prop_map(Op::SwapName),
        (0usize..4, 0u8..3).prop_map(|(i, s)| Op::Sex(i, s)),
        (0usize..4, any::<bool>()).prop_map(|(i, p)| Op::Private(i, p)),
        (0usize..4).prop_map(Op::Note),
        Just(Op::Add),
        (0usize..4).prop_map(Op::Delete),
    ]
}

fn sex_kind(n: u8) -> SexKind {
    match n {
        0 => SexKind::Male,
        1 => SexKind::Female,
        _ => SexKind::Unknown,
    }
}

fn run(s: &mut Session, ids: &[Uuid], ops: &[Op]) {
    for (n, o) in ops.iter().enumerate() {
        let alive = |i: usize, s: &Session| s.tree().persons.contains_key(&ids[i]);
        match o {
            Op::Sex(i, k) if alive(*i, s) => update(s, ids[*i], Some(sex_kind(*k)), None),
            Op::Private(i, p) if alive(*i, s) => update(s, ids[*i], None, Some(*p)),
            Op::Note(i) if alive(*i, s) => {
                s.apply(EditCommand::SaveNote {
                    target: OwnerRef {
                        kind: OwnerKind::Person,
                        id: ids[*i],
                    },
                    note: NoteInput {
                        id: None,
                        kind: NoteKindDto::General,
                        text: format!("note {n}"),
                    },
                })
                .unwrap();
            }
            Op::Add => {
                add(s, &format!("New{n}"));
            }
            Op::SwapName(i) if alive(*i, s) => {
                let first = s.tree().persons[&ids[*i]].names.first().map(|n| n.id);
                s.apply(EditCommand::SaveName {
                    person: ids[*i],
                    name: name_input(&format!("Swap{n}")),
                })
                .unwrap();
                if let Some(first) = first {
                    s.apply(EditCommand::DeleteName {
                        person: ids[*i],
                        name: first,
                    })
                    .unwrap();
                }
            }
            Op::Delete(i) if alive(*i, s) => {
                s.apply(EditCommand::DeletePerson { id: ids[*i] }).unwrap();
            }
            _ => {}
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(40))]

    #[test]
    fn nothing_of_the_owners_is_lost_and_nothing_of_the_copys_is_lost_silently(
        owner_ops in proptest::collection::vec(op(), 0..7),
        rel_ops in proptest::collection::vec(op(), 0..7),
    ) {
        let dir = tempfile::tempdir().unwrap();
        let mut owner = new_owner(&dir, "owner");
        let ids: Vec<Uuid> = ["Ivan", "Olena", "Taras", "Oksana"]
            .iter()
            .map(|n| add(&mut owner, n))
            .collect();
        let mut rel = relative_of(&owner, &dir, "rel");
        let base = owner.tree().clone();
        run(&mut owner, &ids, &owner_ops);
        run(&mut rel, &ids, &rel_ops);
        let zip = send(&rel, &dir, "back.zip");
        let before = owner.tree().clone();
        let p = preview(&mut owner, &zip);
        owner.merge_apply(p.token).unwrap();
        let after = owner.tree().clone();

        for id in &ids {
            let in_base = base.persons.get(id);
            let (b, o, r, a) = (
                in_base,
                before.persons.get(id),
                rel.tree().persons.get(id),
                after.persons.get(id),
            );
            // Never a deletion; never a person the owner removed coming back.
            prop_assert_eq!(o.is_some(), a.is_some(), "presence of {}", id);
            let (Some(b), Some(o), Some(a)) = (b, o, a) else { continue };
            // The owner's own changes are never overwritten.
            if o.sex != b.sex {
                prop_assert_eq!(&a.sex, &o.sex);
            }
            if o.common.private != b.common.private {
                prop_assert_eq!(a.common.private, o.common.private);
            }
            if let Some(r) = r {
                // What only the copy changed comes across, except a change that makes a
                // private person visible.
                if o.sex == b.sex && r.sex != b.sex {
                    prop_assert_eq!(&a.sex, &r.sex);
                }
                if o.common.private == b.common.private && r.common.private != b.common.private {
                    if r.common.private {
                        prop_assert!(a.common.private);
                    } else {
                        prop_assert!(a.common.private == b.common.private, "listed, not taken");
                    }
                }
            }
            // The owner's names all remain: a swap in the copy is a loss, so it is listed.
            for n in &o.names {
                prop_assert!(a.names.iter().any(|m| m.id == n.id), "name {} of {}", n.id, id);
            }
            // The owner's notes all remain.
            for n in &o.common.notes {
                prop_assert!(a.common.notes.contains(n));
            }
        }
        // Every record that stood before stands after, unless the owner's own record was a
        // person (checked above): notes and events the owner had are all still there.
        for id in before.notes.keys() {
            prop_assert!(after.notes.contains_key(id));
        }
        // The people the copy added come across.
        for (id, p) in &rel.tree().persons {
            if !base.persons.contains_key(id) {
                prop_assert!(after.persons.contains_key(id), "{} was added in the copy", p.common.id);
            }
        }
        owner.verify_derived_state().unwrap();
        // No dangling references.
        for f in after.families.values() {
            for p in f.partner1.iter().chain(&f.partner2) {
                prop_assert!(after.persons.contains_key(p));
            }
        }
        // Merged: asking again has nothing to apply; undone: back to what stood before.
        let again = preview(&mut owner, &zip);
        prop_assert_eq!(again.to_apply, 0, "{:?}", again);
        let batch_groups = owner.project().log_groups_in_order().unwrap();
        let _ = batch_groups;
    }
}
