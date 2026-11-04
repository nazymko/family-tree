#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use common::*;
use gt_session::*;
use uuid::Uuid;

fn person(key: &str, given: &str, surname: &str) -> DraftPerson {
    DraftPerson {
        key: key.into(),
        existing: None,
        given: Some(given.into()),
        surname: Some(surname.into()),
        patronymic: None,
        sex: SexKind::Unknown,
        birth_date_raw: None,
        death_date_raw: None,
        notes: None,
    }
}

fn existing(key: &str, id: Uuid) -> DraftPerson {
    DraftPerson {
        existing: Some(id),
        given: None,
        surname: None,
        ..person(key, "", "")
    }
}

fn fam(p1: Option<&str>, p2: Option<&str>, kids: &[&str]) -> DraftFamily {
    DraftFamily {
        partner1: p1.map(Into::into),
        partner2: p2.map(Into::into),
        existing_family: None,
        children: kids.iter().map(|k| (*k).into()).collect(),
    }
}

fn cmd(persons: Vec<DraftPerson>, families: Vec<DraftFamily>, pic: bool) -> EditCommand {
    EditCommand::ImportDraft {
        persons,
        families,
        attach_picture: pic,
    }
}

fn setup() -> (tempfile::TempDir, Session) {
    open_tree(&tree_of(SAMPLE))
}

fn disk(s: &Session) -> gt_core::tree::Tree {
    s.project().load_tree().unwrap()
}

fn family_draft() -> EditCommand {
    let mut ivan = person("a", "Ivan", "Koval");
    ivan.patronymic = Some("Petrovych".into());
    ivan.birth_date_raw = Some("1901".into());
    ivan.death_date_raw = Some("1975".into());
    ivan.notes = Some("Wall chart, top row.".into());
    cmd(
        vec![
            ivan,
            person("b", "Olena", "Koval"),
            person("c", "Petro", "Koval"),
        ],
        vec![fam(Some("a"), Some("b"), &["c"])],
        false,
    )
}

/// 1x1 PNG.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xFF, 0xFF, 0x3F,
    0x00, 0x05, 0xFE, 0x02, 0xFE, 0xA7, 0x35, 0x81, 0x84, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

#[test]
fn creates_people_events_and_family_in_one_step() {
    let (_d, mut s) = setup();
    let before = s.tree().clone();
    let r = s.apply(family_draft()).unwrap();
    assert_eq!(r.label, "Add traced people");
    let t = s.tree();
    assert_eq!(t.persons.len(), before.persons.len() + 3);
    assert_eq!(t.families.len(), before.families.len() + 1);
    let ivan = t
        .persons
        .values()
        .find(|p| p.names[0].given.as_deref() == Some("Ivan"))
        .unwrap();
    assert_eq!(ivan.names[0].patronymic.as_deref(), Some("Petrovych"));
    assert_eq!(ivan.common.notes.len(), 1);
    // 2 events (birth + death) and 1 note were added.
    assert_eq!(t.events.len(), before.events.len() + 2);
    assert_eq!(t.notes.len(), before.notes.len() + 1);
    assert_eq!(s.tree(), &disk(&s));
    // Exactly one undo step.
    assert_eq!(
        s.undo_state().undo_label.as_deref(),
        Some("Add traced people")
    );
    s.undo().unwrap();
    assert!(!s.undo_state().can_undo);
    assert_eq!(s.tree(), &before);
}

#[test]
fn undo_and_redo_are_exact() {
    let (_d, mut s) = setup();
    let original = disk(&s);
    s.apply(family_draft()).unwrap();
    let after = disk(&s);
    assert_ne!(after, original);
    s.undo().unwrap();
    assert_eq!(disk(&s), original);
    assert_eq!(s.tree(), &original);
    s.redo().unwrap();
    assert_eq!(disk(&s), after);
    assert_eq!(s.tree(), &after);
    s.undo().unwrap();
    assert_eq!(disk(&s), original);
}

#[test]
fn links_to_existing_people_without_duplicating_them() {
    let (_d, mut s) = setup();
    let hans = find(s.tree(), "Hans");
    let people_before = s.tree().persons.len();
    // Hans (already in the tree) is the partner of a new woman; they have a new child.
    s.apply(cmd(
        vec![
            existing("h", hans),
            person("w", "Maria", "Bauer"),
            person("k", "Lena", "Schmidt"),
        ],
        vec![fam(Some("h"), Some("w"), &["k"])],
        false,
    ))
    .unwrap();
    assert_eq!(s.tree().persons.len(), people_before + 2);
    let f = s
        .tree()
        .families
        .values()
        .find(|f| f.partner1 == Some(hans) || f.partner2 == Some(hans))
        .filter(|f| !f.children.is_empty())
        .unwrap();
    assert_eq!(f.children.len(), 1);
    s.undo().unwrap();
    assert_eq!(s.tree().persons.len(), people_before);
}

#[test]
fn a_new_child_of_an_existing_couple_joins_their_family() {
    let (_d, mut s) = setup();
    let anna = find(s.tree(), "Anna");
    let karl = find(s.tree(), "Karl");
    let families = s.tree().families.len();
    s.apply(cmd(
        vec![
            existing("a", anna),
            existing("k", karl),
            person("c", "Greta", "Schmidt"),
        ],
        vec![fam(Some("a"), Some("k"), &["c"])],
        false,
    ))
    .unwrap();
    assert_eq!(s.tree().families.len(), families);
    let f = s.tree().families.values().next().unwrap();
    assert_eq!(f.children.len(), 2);
}

#[test]
fn refused_drafts_leave_nothing_behind() {
    let (_d, mut s) = setup();
    let original = disk(&s);
    let bad: Vec<EditCommand> = vec![
        // no name
        cmd(vec![person("a", "", "")], vec![], false),
        // nobody
        cmd(vec![], vec![], false),
        // repeated key
        cmd(
            vec![person("a", "A", "B"), person("a", "C", "D")],
            vec![],
            false,
        ),
        // unknown key in a family
        cmd(
            vec![person("a", "A", "B")],
            vec![fam(Some("a"), Some("zz"), &[])],
            false,
        ),
        // own child
        cmd(
            vec![person("a", "A", "B")],
            vec![fam(Some("a"), None, &["a"])],
            false,
        ),
        // same partner twice
        cmd(
            vec![person("a", "A", "B")],
            vec![fam(Some("a"), Some("a"), &[])],
            false,
        ),
        // no parents at all
        cmd(
            vec![person("a", "A", "B")],
            vec![fam(None, None, &["a"])],
            false,
        ),
        // child listed twice
        cmd(
            vec![person("a", "A", "B"), person("b", "C", "D")],
            vec![fam(Some("a"), None, &["b", "b"])],
            false,
        ),
        // missing existing person
        cmd(vec![existing("x", Uuid::now_v7())], vec![], false),
        // loop: a parent of b, b parent of a
        cmd(
            vec![person("a", "A", "B"), person("b", "C", "D")],
            vec![fam(Some("a"), None, &["b"]), fam(Some("b"), None, &["a"])],
            false,
        ),
        // picture requested but none staged
        cmd(vec![person("a", "A", "B")], vec![], true),
    ];
    for c in bad {
        let before_undo = s.undo_state();
        assert!(s.apply(c.clone()).is_err(), "should refuse {c:?}");
        assert_eq!(disk(&s), original);
        assert_eq!(s.tree(), &original);
        assert_eq!(s.undo_state(), before_undo);
    }
}

#[test]
fn a_loop_through_the_existing_tree_is_refused() {
    let (_d, mut s) = setup();
    let anna = find(s.tree(), "Anna");
    let hans = find(s.tree(), "Hans");
    // Hans is Anna's child; making Anna a child of Hans closes a loop.
    let r = s.apply(cmd(
        vec![existing("h", hans), existing("a", anna)],
        vec![fam(Some("h"), None, &["a"])],
        false,
    ));
    assert!(r.is_err());
}

#[test]
fn too_many_people_are_refused() {
    let (_d, mut s) = setup();
    let many: Vec<DraftPerson> = (0..501)
        .map(|i| person(&format!("p{i}"), "A", "B"))
        .collect();
    assert!(s.apply(cmd(many, vec![], false)).is_err());
}

#[test]
fn draft_text_saves_loads_and_is_discarded() {
    let (_d, mut s) = setup();
    assert_eq!(s.load_trace_draft().unwrap().json, None);
    s.save_trace_draft("{\"cards\":[1,2]}").unwrap();
    s.save_trace_draft("{\"cards\":[1,2,3]}").unwrap();
    assert_eq!(
        s.load_trace_draft().unwrap().json.as_deref(),
        Some("{\"cards\":[1,2,3]}")
    );
    s.discard_trace_draft().unwrap();
    let st = s.load_trace_draft().unwrap();
    assert_eq!(st.json, None);
    assert_eq!(st.image, None);
}

#[test]
fn draft_and_picture_survive_closing_the_tree() {
    let (d, mut s) = setup();
    let src = d.path().join("wall chart.png");
    std::fs::write(&src, PNG).unwrap();
    let info = s.stage_trace_image(&src).unwrap();
    assert_eq!(info.name, "wall chart.png");
    assert_eq!((info.width, info.height), (Some(1), Some(1)));
    s.save_trace_draft("draft-1").unwrap();
    let dir = s.project().dir.clone();
    drop(s);
    let s = Session::open(&dir, SessionClock::fixed(today(), NOW_MS)).unwrap();
    let st = s.load_trace_draft().unwrap();
    assert_eq!(st.json.as_deref(), Some("draft-1"));
    assert_eq!(st.image.unwrap().name, "wall chart.png");
    let (bytes, mime) = s.trace_image_bytes().unwrap();
    assert_eq!(bytes, PNG);
    assert_eq!(mime, "image/png");
}

#[test]
fn only_pictures_can_be_traced() {
    let (d, mut s) = setup();
    let txt = d.path().join("notes.png");
    std::fs::write(&txt, b"this is not an image").unwrap();
    assert!(s.stage_trace_image(&txt).is_err());
    let pdf = d.path().join("a.pdf");
    std::fs::write(&pdf, b"%PDF-1.4 hello").unwrap();
    assert!(s.stage_trace_image(&pdf).is_err());
    assert!(s.stage_trace_image(&d.path().join("missing.png")).is_err());
    assert_eq!(s.load_trace_draft().unwrap().image, None);
}

#[test]
fn the_picture_becomes_media_on_every_new_person_and_a_source() {
    let (d, mut s) = setup();
    let src = d.path().join("tree.png");
    std::fs::write(&src, PNG).unwrap();
    s.stage_trace_image(&src).unwrap();
    let before = disk(&s);
    let EditCommand::ImportDraft {
        mut persons,
        families,
        ..
    } = family_draft()
    else {
        unreachable!()
    };
    let hans = find(s.tree(), "Hans");
    persons.push(existing("h", hans));
    s.apply(cmd(persons, families, true)).unwrap();
    let t = s.tree();
    assert_eq!(t.media.len(), before.media.len() + 1);
    let m = t.media.values().last().unwrap();
    assert_eq!(m.original_name.as_deref(), Some("tree.png"));
    let linked = t
        .persons
        .values()
        .filter(|p| p.common.media.iter().any(|l| l.media == m.common.id))
        .count();
    assert_eq!(
        linked, 3,
        "new people only; the existing person is untouched"
    );
    let source = t
        .sources
        .values()
        .find(|x| x.title.as_deref() == Some("Family tree picture"))
        .unwrap();
    assert!(source.common.media.iter().any(|l| l.media == m.common.id));
    assert!(
        t.persons
            .values()
            .all(|p| p.common.media.iter().all(|l| !l.primary))
    );
    // One undo takes all of it away, one redo brings it back.
    let after = disk(&s);
    s.undo().unwrap();
    assert_eq!(disk(&s), before);
    s.redo().unwrap();
    assert_eq!(disk(&s), after);
}

#[test]
fn a_child_of_one_parent_from_the_tree_joins_that_parents_family() {
    let (_d, mut s) = setup();
    let anna = find(s.tree(), "Anna");
    let families = s.tree().families.len();
    s.apply(cmd(
        vec![existing("a", anna), person("c", "Greta", "Schmidt")],
        vec![fam(Some("a"), None, &["c"])],
        false,
    ))
    .unwrap();
    // No second family for Anna: the child is in the family she already has with Karl.
    assert_eq!(s.tree().families.len(), families);
    assert_eq!(s.tree().families.values().next().unwrap().children.len(), 2);
    s.undo().unwrap();
    assert_eq!(s.tree().families.values().next().unwrap().children.len(), 1);
}

#[test]
fn a_parent_in_several_families_must_say_which_one() {
    let (_d, mut s) = setup();
    let karl = find(s.tree(), "Karl");
    let eva = find(s.tree(), "Eva");
    let first = s.tree().families.keys().next().copied().unwrap();
    // Karl gets a second family, with Eva.
    s.apply(cmd(
        vec![existing("k", karl), existing("e", eva)],
        vec![fam(Some("k"), Some("e"), &[])],
        false,
    ))
    .unwrap();
    assert_eq!(s.tree().families.len(), 2);
    let before = disk(&s);
    let kid = || vec![existing("k", karl), person("c", "Greta", "Schmidt")];

    let e = s
        .apply(cmd(kid(), vec![fam(Some("k"), None, &["c"])], false))
        .unwrap_err();
    assert!(e.message.contains("Choose which family"), "{}", e.message);
    assert_eq!(disk(&s), before);

    let mut chosen = fam(Some("k"), None, &["c"]);
    chosen.existing_family = Some(first);
    s.apply(cmd(kid(), vec![chosen], false)).unwrap();
    assert_eq!(s.tree().families.len(), 2);
    assert_eq!(s.tree().families[&first].children.len(), 2);

    // A family that does not include that parent is refused.
    let anna = find(s.tree(), "Anna");
    let other = s
        .tree()
        .families
        .values()
        .find(|f| f.partner1 != Some(anna) && f.partner2 != Some(anna))
        .map(|f| f.common.id)
        .unwrap();
    let mut wrong = fam(Some("a"), None, &["d"]);
    wrong.existing_family = Some(other);
    let e = s
        .apply(cmd(
            vec![existing("a", anna), person("d", "Otto", "Schmidt")],
            vec![wrong],
            false,
        ))
        .unwrap_err();
    assert!(e.message.contains("does not include"), "{}", e.message);
}
