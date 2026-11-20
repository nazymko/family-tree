#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! A note's readable story: saved beside the text, kept when the text is edited, shown in the log and undone.

mod common;

use common::*;
use gt_session::*;
use uuid::Uuid;

fn owner(id: Uuid) -> OwnerRef {
    OwnerRef {
        kind: OwnerKind::Person,
        id,
    }
}

fn save(anna: Uuid, id: Option<Uuid>, text: &str, story: Option<&str>) -> EditCommand {
    EditCommand::SaveNote {
        target: owner(anna),
        note: NoteInput {
            id,
            kind: NoteKindDto::Research,
            text: text.into(),
            story: story.map(str::to_string),
        },
    }
}

fn setup() -> (tempfile::TempDir, Session, Uuid) {
    let t = tree_of(SAMPLE);
    let anna = find(&t, "Anna");
    let (d, s) = open_tree(&t);
    (d, s, anna)
}

fn the_note(s: &Session, anna: Uuid) -> NoteDto {
    s.get_person(anna).unwrap().notes.pop().unwrap()
}

#[test]
fn a_new_note_keeps_its_story_and_the_text_stays_as_written() {
    let (_d, mut s, anna) = setup();
    s.apply(save(
        anna,
        None,
        "Born 1890 (ДАХеО 1-2-3, арк. 5).",
        Some(" Born in 1890. "),
    ))
    .unwrap();
    let n = the_note(&s, anna);
    assert_eq!(n.text, "Born 1890 (ДАХеО 1-2-3, арк. 5).");
    assert_eq!(n.story.as_deref(), Some("Born in 1890."));
    // It is on disk too.
    let disk = s.project().load_tree().unwrap();
    assert_eq!(disk.notes[&n.id].story.as_deref(), Some("Born in 1890."));
}

#[test]
fn editing_the_text_without_a_story_keeps_the_story_and_an_empty_story_takes_it_off() {
    let (_d, mut s, anna) = setup();
    s.apply(save(anna, None, "Text.", Some("A retelling.")))
        .unwrap();
    let id = the_note(&s, anna).id;
    s.apply(save(anna, Some(id), "Text, corrected.", None))
        .unwrap();
    let n = the_note(&s, anna);
    assert_eq!(n.text, "Text, corrected.");
    assert_eq!(n.story.as_deref(), Some("A retelling."));
    s.apply(save(anna, Some(id), "Text, corrected.", Some("  ")))
        .unwrap();
    assert_eq!(the_note(&s, anna).story, None);
}

#[test]
fn a_story_change_is_one_undoable_step_and_shows_in_the_field_changes() {
    let (_d, mut s, anna) = setup();
    s.apply(save(anna, None, "Text.", None)).unwrap();
    let id = the_note(&s, anna).id;
    s.apply(save(anna, Some(id), "Text.", Some("A retelling.")))
        .unwrap();
    assert_eq!(the_note(&s, anna).story.as_deref(), Some("A retelling."));
    let log = s.log(LogFilterDto::default(), 0, 20).unwrap().items;
    let shown = serde_json::to_string(&log[0]).unwrap();
    assert!(shown.contains("story"), "{shown}");
    s.undo().unwrap().unwrap();
    assert_eq!(the_note(&s, anna).story, None);
    assert_eq!(the_note(&s, anna).text, "Text.");
    s.redo().unwrap().unwrap();
    assert_eq!(the_note(&s, anna).story.as_deref(), Some("A retelling."));
}

#[test]
fn a_note_saved_before_the_story_existed_reads_back_with_none() {
    let (_d, mut s, anna) = setup();
    s.apply(save(anna, None, "Text.", Some("A retelling.")))
        .unwrap();
    let id = the_note(&s, anna).id;
    let mut json = serde_json::to_value(&s.tree().notes[&id]).unwrap();
    json.as_object_mut().unwrap().remove("story").unwrap();
    let old: gt_core::model::Note = serde_json::from_value(json).unwrap();
    assert_eq!(old.story, None);
    assert_eq!(old.text, "Text.");
}
