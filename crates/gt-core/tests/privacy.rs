#![allow(clippy::unwrap_used)]
use gt_core::date::calendar::{Calendar, first_jdn};
use gt_core::date::{DayOrder, GenDate};
use gt_core::ids::new_id;
use gt_core::model::*;
use gt_core::privacy::{LivingMode, PrivacyPolicy, filter};
use gt_core::tree::Tree;
use uuid::Uuid;

fn common() -> Common {
    Common {
        id: new_id(),
        private: false,
        changed: None,
        citations: vec![],
        notes: vec![],
        media: vec![],
        ext_ids: vec![],
        unknown: vec![],
    }
}

fn assertion() -> Assertion {
    Assertion {
        preferred: true,
        proof: None,
        confidence: None,
    }
}

fn name(given: &str, sur: &str) -> Name {
    Name {
        id: new_id(),
        kind: NameKind::Birth,
        assertion: assertion(),
        private: false,
        prefix: None,
        given: Some(given.into()),
        call: None,
        surname_prefix: None,
        surnames: vec![sur.into()],
        patronymic: None,
        suffix: None,
        nickname: None,
        lang: None,
        date: None,
        raw: None,
        citations: vec![],
        notes: vec![],
        unknown: vec![],
    }
}

fn add_person(t: &mut Tree, given: &str, sur: &str) -> Uuid {
    let c = common();
    let id = c.id;
    t.persons.insert(
        id,
        Person {
            common: c,
            sex: Sex::Female,
            names: vec![name(given, sur)],
            living_override: None,
        },
    );
    id
}

fn add_event(t: &mut Tree, kind: EventKind, date: Option<&str>, who: &[(Uuid, Role)]) -> Uuid {
    let c = common();
    let id = c.id;
    t.events.insert(
        id,
        Event {
            common: c,
            kind,
            date: date.map(|d| GenDate::parse(d, DayOrder::Dmy)),
            place: None,
            description: None,
            age: None,
            cause: None,
            negative: false,
            assertion: assertion(),
            participants: who
                .iter()
                .map(|(p, r)| Participant {
                    target: ParticipantRef::Person(*p),
                    role: r.clone(),
                })
                .collect(),
        },
    );
    id
}

fn add_note(t: &mut Tree, text: &str) -> Uuid {
    let c = common();
    let id = c.id;
    t.notes.insert(
        id,
        Note {
            common: c,
            kind: NoteKind::General,
            text: text.into(),
            lang: None,
            shared: false,
        },
    );
    id
}

fn today() -> i32 {
    first_jdn(Calendar::Gregorian, 2026, Some(11), Some(3)).unwrap()
}

fn policy(mode: LivingMode) -> PrivacyPolicy {
    PrivacyPolicy {
        living: mode,
        ..PrivacyPolicy::default()
    }
}

/// Dead parents (born 1850), a living child with a birth event and a note.
fn family_tree() -> (Tree, Uuid, Uuid, Uuid, Uuid) {
    let mut t = Tree::new();
    let dad = add_person(&mut t, "Hans", "Schmidt");
    let mum = add_person(&mut t, "Greta", "Schmidt");
    let kid = add_person(&mut t, "Anna", "Schmidt");
    add_event(
        &mut t,
        EventKind::Death,
        Some("1920"),
        &[(dad, Role::Principal)],
    );
    add_event(
        &mut t,
        EventKind::Death,
        Some("1925"),
        &[(mum, Role::Principal)],
    );
    add_event(
        &mut t,
        EventKind::Birth,
        Some("1990"),
        &[(kid, Role::Principal)],
    );
    let note = add_note(&mut t, "Anna likes cats");
    t.persons.get_mut(&kid).unwrap().common.notes.push(note);
    let c = common();
    let fam = c.id;
    t.families.insert(
        fam,
        Family {
            common: c,
            partner1: Some(dad),
            partner2: Some(mum),
            kind: UnionKind::Married,
            children: vec![ChildLink {
                person: kid,
                rel1: ParentRel::Birth,
                rel2: ParentRel::Birth,
                preferred: true,
                assertion: assertion(),
                citations: vec![],
                unknown: vec![],
            }],
        },
    );
    (t, dad, mum, kid, fam)
}

#[test]
fn placeholder_hides_everything_but_structure() {
    let (t, dad, _mum, kid, fam) = family_tree();
    let (out, s) = filter(&t, &policy(LivingMode::Placeholder), today());
    let k = &out.persons[&kid];
    assert_eq!(k.names.len(), 1);
    assert_eq!(k.names[0].given.as_deref(), Some("Living"));
    assert!(k.names[0].surnames.is_empty());
    assert_eq!(out.events_of(kid).count(), 0);
    assert!(k.common.notes.is_empty());
    assert!(out.notes.is_empty());
    assert_eq!(out.families[&fam].children[0].person, kid);
    assert_eq!(out.events_of(dad).count(), 1);
    assert_eq!(out.persons[&dad].names[0].given.as_deref(), Some("Hans"));
    assert_eq!(s.placeholder_persons, 1);
    assert_eq!(s.hidden_persons, 1);
}

#[test]
fn exclude_removes_and_cleans() {
    let (t, _dad, _mum, kid, fam) = family_tree();
    let (out, s) = filter(&t, &policy(LivingMode::Exclude), today());
    assert!(!out.persons.contains_key(&kid));
    assert!(out.families[&fam].children.is_empty());
    assert!(out.notes.is_empty());
    assert_eq!(out.events.len(), 2);
    assert_eq!(s.hidden_persons, 1);
    assert_eq!(s.placeholder_persons, 0);
}

#[test]
fn include_all_keeps_everyone() {
    let (t, _, _, kid, _) = family_tree();
    let (out, _) = filter(&t, &policy(LivingMode::IncludeAll), today());
    assert_eq!(out.persons[&kid].names[0].given.as_deref(), Some("Anna"));
    assert_eq!(out.events.len(), 3);
}

#[test]
fn private_beats_include_all() {
    let mut t = Tree::new();
    let p = add_person(&mut t, "Old", "Secret");
    add_event(
        &mut t,
        EventKind::Death,
        Some("1900"),
        &[(p, Role::Principal)],
    );
    t.persons.get_mut(&p).unwrap().common.private = true;
    let (out, _) = filter(&t, &policy(LivingMode::IncludeAll), today());
    assert!(out.persons.is_empty());
    assert!(out.events.is_empty());
    let mut pol = policy(LivingMode::IncludeAll);
    pol.include_private = true;
    let (out, _) = filter(&t, &pol, today());
    assert_eq!(out.persons.len(), 1);
}

#[test]
fn private_fact_removed() {
    let mut t = Tree::new();
    let p = add_person(&mut t, "Old", "Timer");
    add_event(
        &mut t,
        EventKind::Death,
        Some("1900"),
        &[(p, Role::Principal)],
    );
    let occ = add_event(&mut t, EventKind::Occupation, None, &[(p, Role::Principal)]);
    t.events.get_mut(&occ).unwrap().common.private = true;
    let (out, s) = filter(&t, &policy(LivingMode::Placeholder), today());
    assert!(out.persons.contains_key(&p));
    assert_eq!(out.events.len(), 1);
    assert_eq!(s.removed_records, 1);
}

#[test]
fn shared_census_keeps_others() {
    let mut t = Tree::new();
    let dead = add_person(&mut t, "Old", "Timer");
    let live = add_person(&mut t, "Young", "Witness");
    add_event(
        &mut t,
        EventKind::Death,
        Some("1900"),
        &[(dead, Role::Principal)],
    );
    let census = add_event(
        &mut t,
        EventKind::Census,
        Some("1890"),
        &[(dead, Role::Principal), (live, Role::Witness)],
    );
    let (out, _) = filter(&t, &policy(LivingMode::Placeholder), today());
    let e = &out.events[&census];
    assert_eq!(e.participants.len(), 1);
    assert_eq!(e.participants[0].target, ParticipantRef::Person(dead));
}

#[test]
fn mention_warning() {
    let mut t = Tree::new();
    let dead = add_person(&mut t, "Old", "Timer");
    let live = add_person(&mut t, "Anna", "Schmidt");
    add_event(
        &mut t,
        EventKind::Death,
        Some("1900"),
        &[(dead, Role::Principal)],
    );
    let note = add_note(&mut t, "Her granddaughter anna SCHMIDT visited.");
    t.persons.get_mut(&dead).unwrap().common.notes.push(note);
    let (_, s) = filter(&t, &policy(LivingMode::Placeholder), today());
    assert_eq!(s.mention_warnings, vec![(note, live)]);
}

#[test]
fn undated_living_hidden_in_export() {
    let mut t = Tree::new();
    add_person(&mut t, "No", "Dates");
    let (out, s) = filter(&t, &policy(LivingMode::Placeholder), today());
    assert_eq!(s.placeholder_persons, 1);
    assert_eq!(
        out.persons.values().next().unwrap().names[0]
            .given
            .as_deref(),
        Some("Living")
    );
}

#[test]
fn shared_media_is_reported_and_exclusive_media_dropped() {
    let mut t = Tree::new();
    let dead = add_person(&mut t, "Old", "Timer");
    let live = add_person(&mut t, "Young", "One");
    add_event(
        &mut t,
        EventKind::Death,
        Some("1900"),
        &[(dead, Role::Principal)],
    );
    let group = Media {
        common: common(),
        path: "group.jpg".into(),
        original_name: None,
        mime: None,
        title: None,
        date: None,
        description: None,
        sha256: None,
    };
    let solo = Media {
        common: common(),
        path: "solo.jpg".into(),
        ..group.clone()
    };
    let (gid, sid) = (group.common.id, solo.common.id);
    t.media.insert(gid, group);
    t.media.insert(sid, solo);
    let link = |m| MediaLink {
        media: m,
        region: None,
        primary: false,
    };
    t.persons
        .get_mut(&dead)
        .unwrap()
        .common
        .media
        .push(link(gid));
    let l = t.persons.get_mut(&live).unwrap();
    l.common.media.push(link(gid));
    l.common.media.push(link(sid));
    let (out, s) = filter(&t, &policy(LivingMode::Placeholder), today());
    assert!(out.media.contains_key(&gid));
    assert!(!out.media.contains_key(&sid));
    assert_eq!(s.group_media, vec![gid]);
}
