#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use super::*;
use gt_core::model::{EventKind, NoteKind, ParentRel, Role, UnionKind};

/// Words that name an id after its number.
struct Stub;
impl Stub {
    fn n(prefix: &str, id: Uuid) -> String {
        format!("{prefix} {}", id.as_u128())
    }
}
impl Words for Stub {
    fn person(&self, id: Uuid) -> String {
        Self::n("Person", id)
    }
    fn family(&self, id: Uuid) -> String {
        Self::n("Family", id)
    }
    fn place(&self, id: Uuid) -> String {
        Self::n("Place", id)
    }
    fn source(&self, id: Uuid) -> String {
        Self::n("Source", id)
    }
    fn repository(&self, id: Uuid) -> String {
        Self::n("Repository", id)
    }
    fn citation(&self, id: Uuid) -> String {
        Self::n("Citation", id)
    }
    fn note(&self, id: Uuid) -> String {
        Self::n("Note", id)
    }
    fn media(&self, id: Uuid) -> String {
        Self::n("Media", id)
    }
}

fn u(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn common(n: u128) -> Common {
    Common {
        id: u(n),
        private: false,
        changed: None,
        citations: vec![],
        notes: vec![],
        media: vec![],
        ext_ids: vec![],
        unknown: vec![],
    }
}

type Rows = Vec<(String, Option<String>, Option<String>)>;

fn diff(b: Option<Record>, a: Option<Record>) -> Rows {
    record_field_changes(b.as_ref(), a.as_ref(), &Stub)
        .into_iter()
        .map(|f| (f.field, f.before, f.after))
        .collect()
}

fn s(v: &str) -> Option<String> {
    Some(v.to_string())
}

fn row(f: &str, b: Option<&str>, a: Option<&str>) -> (String, Option<String>, Option<String>) {
    (f.to_string(), b.map(str::to_string), a.map(str::to_string))
}

fn note(text: &str) -> Note {
    Note {
        common: common(1),
        kind: NoteKind::Story,
        text: text.into(),
        lang: None,
        shared: false,
    }
}

#[test]
fn a_note_added_lists_its_filled_fields_with_the_whole_text() {
    let long = "word ".repeat(300);
    let got = diff(None, Some(Record::Note(note(&long))));
    assert_eq!(
        got,
        vec![
            row("noteKind", None, Some("story")),
            row("text", None, Some(&long)),
        ]
    );
}

#[test]
fn a_note_removed_lists_them_against_empty_and_an_edit_only_what_changed() {
    let got = diff(Some(Record::Note(note("old"))), None);
    assert_eq!(got[1], row("text", Some("old"), None));
    let got = diff(
        Some(Record::Note(note("old"))),
        Some(Record::Note(note("new"))),
    );
    assert_eq!(got, vec![row("text", Some("old"), Some("new"))]);
    assert!(diff(Some(Record::Note(note("a"))), Some(Record::Note(note("a")))).is_empty());
}

fn event(place: Option<Uuid>, role: Role) -> Event {
    Event {
        common: common(2),
        kind: EventKind::Custom("Pilgrimage".into()),
        date: None,
        place,
        description: Some("Walked".into()),
        age: None,
        cause: None,
        negative: false,
        assertion: Assertion {
            preferred: true,
            proof: None,
            confidence: Some(2),
        },
        participants: vec![Participant {
            target: ParticipantRef::Person(u(9)),
            role,
        }],
    }
}

#[test]
fn an_event_names_its_place_and_participants() {
    let got = diff(
        None,
        Some(Record::Event(event(Some(u(5)), Role::Principal))),
    );
    assert_eq!(
        got,
        vec![
            row("eventKind", None, Some("custom:Pilgrimage")),
            row("place", None, Some("Place 5")),
            row("description", None, Some("Walked")),
            row("confidence", None, Some("2")),
            row("participant", None, Some("principal")),
        ]
    );
    let changes = record_field_changes(
        Some(&Record::Event(event(Some(u(5)), Role::Principal))),
        Some(&Record::Event(event(Some(u(6)), Role::Witness))),
        &Stub,
    );
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].after.as_deref(), Some("Place 6"));
    assert_eq!(changes[1].of.as_deref(), Some("Person 9"));
    assert_eq!(changes[1].before.as_deref(), Some("principal"));
    assert_eq!(changes[1].after.as_deref(), Some("witness"));
}

fn family(children: Vec<ChildLink>) -> Family {
    Family {
        common: common(3),
        partner1: Some(u(10)),
        partner2: None,
        kind: UnionKind::Married,
        children,
    }
}

fn link(rel: ParentRel) -> ChildLink {
    ChildLink {
        person: u(11),
        rel1: rel,
        rel2: ParentRel::Birth,
        preferred: true,
        assertion: Assertion {
            preferred: true,
            proof: None,
            confidence: None,
        },
        citations: vec![],
        unknown: vec![],
    }
}

#[test]
fn a_family_names_partners_and_each_child_link() {
    let got = diff(
        None,
        Some(Record::Family(family(vec![link(ParentRel::Adopted)]))),
    );
    let fields: Vec<&str> = got.iter().map(|g| g.0.as_str()).collect();
    assert_eq!(
        fields,
        [
            "partner1",
            "unionKind",
            "child",
            "childRel1",
            "childRel2",
            "childPreferred"
        ]
    );
    assert_eq!(got[0].2, s("Person 10"));
    assert_eq!(got[2].2, s("Person 11"));
    let changes = record_field_changes(
        Some(&Record::Family(family(vec![link(ParentRel::Birth)]))),
        Some(&Record::Family(family(vec![link(ParentRel::Adopted)]))),
        &Stub,
    );
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].field, "childRel1");
    assert_eq!(changes[0].of.as_deref(), Some("Person 11"));
    assert_eq!(changes[0].after.as_deref(), Some("adopted"));
    let gone = diff(
        Some(Record::Family(family(vec![link(ParentRel::Birth)]))),
        Some(Record::Family(family(vec![]))),
    );
    assert!(gone.contains(&row("child", Some("Person 11"), None)));
}

#[test]
fn citation_source_place_repository_and_media_have_their_fields() {
    let citation = Citation {
        common: common(4),
        source: Some(u(7)),
        page: Some("12".into()),
        date: None,
        text: Some("Quote".into()),
        quality: Some(3),
        evidence: Evidence {
            original: Some(true),
            primary_info: None,
            direct: None,
        },
    };
    assert_eq!(
        diff(None, Some(Record::Citation(citation))),
        vec![
            row("source", None, Some("Source 7")),
            row("page", None, Some("12")),
            row("text", None, Some("Quote")),
            row("quality", None, Some("3")),
            row("original", None, Some("yes")),
        ]
    );
    let source = Source {
        common: common(5),
        title: Some("Parish book".into()),
        author: None,
        publication: None,
        abbreviation: None,
        text: None,
        repositories: vec![RepoRef {
            repository: u(8),
            call_number: Some("A-1".into()),
        }],
    };
    assert_eq!(
        diff(Some(Record::Source(source)), None),
        vec![
            row("title", Some("Parish book"), None),
            row("repository", Some("Repository 8, A-1"), None),
        ]
    );
    let place = Place {
        common: common(6),
        names: vec![PlaceName {
            name: "Lviv".into(),
            lang: Some("uk".into()),
            date: None,
        }],
        parent: Some(u(12)),
        kind: Some("city".into()),
        lat: Some(49.5),
        lon: None,
        raw: None,
    };
    assert_eq!(
        diff(None, Some(Record::Place(place))),
        vec![
            row("placeNames", None, Some("Lviv (uk)")),
            row("parent", None, Some("Place 12")),
            row("placeKind", None, Some("city")),
            row("lat", None, Some("49.5")),
        ]
    );
    let repo = Repository {
        common: common(7),
        name: "State archive".into(),
        address: None,
        urls: vec!["https://example.org".into()],
    };
    assert_eq!(
        diff(None, Some(Record::Repository(repo))),
        vec![
            row("repoName", None, Some("State archive")),
            row("urls", None, Some("https://example.org")),
        ]
    );
    let media = Media {
        common: common(8),
        path: "media/a.jpg".into(),
        original_name: None,
        mime: Some("image/jpeg".into()),
        title: Some("Portrait".into()),
        date: None,
        description: None,
        sha256: None,
    };
    assert_eq!(
        diff(None, Some(Record::Media(media))),
        vec![
            row("path", None, Some("media/a.jpg")),
            row("mime", None, Some("image/jpeg")),
            row("title", None, Some("Portrait")),
        ]
    );
}

#[test]
fn ids_a_record_lists_are_worded() {
    let mut n = note("t");
    n.common.citations = vec![u(30)];
    n.common.private = true;
    let got = diff(Some(Record::Note(note("t"))), Some(Record::Note(n)));
    assert_eq!(
        got,
        vec![
            row("private", Some("no"), Some("yes")),
            row("citations", None, Some("Citation 30")),
        ]
    );
}
