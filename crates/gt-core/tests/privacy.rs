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
            sibling_marker: false,
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

fn bare_media(path: &str) -> Media {
    Media {
        common: common(),
        path: path.into(),
        original_name: None,
        mime: None,
        title: None,
        date: None,
        description: None,
        sha256: None,
    }
}

#[test]
fn include_all_with_private_is_identity() {
    let (mut t, _, _, _, _) = family_tree();
    let src = Source {
        common: common(),
        title: Some("Orphan".into()),
        author: None,
        publication: None,
        abbreviation: None,
        text: None,
        repositories: vec![],
    };
    t.sources.insert(src.common.id, src);
    let repo = Repository {
        common: common(),
        name: "Archive".into(),
        address: None,
        urls: vec![],
    };
    t.repositories.insert(repo.common.id, repo);
    add_note(&mut t, "standalone");
    let m = bare_media("orphan.jpg");
    t.media.insert(m.common.id, m);
    // empty family carrying an event and a note
    let ev = add_event(&mut t, EventKind::Marriage, Some("1800"), &[]);
    let note = add_note(&mut t, "family note");
    let c = common();
    let fid = c.id;
    let mut c = c;
    c.notes.push(note);
    t.families.insert(
        fid,
        Family {
            common: c,
            partner1: None,
            partner2: None,
            kind: UnionKind::Unknown,
            children: vec![],
        },
    );
    t.events
        .get_mut(&ev)
        .unwrap()
        .participants
        .push(Participant {
            target: ParticipantRef::Family(fid),
            role: Role::Principal,
        });
    let mut pol = policy(LivingMode::IncludeAll);
    pol.include_private = true;
    let (out, s) = filter(&t, &pol, today());
    assert_eq!(out, t);
    assert_eq!(s.removed_records, 0);
}

#[test]
fn family_with_hidden_partner_loses_family_level_data() {
    let (mut t, dad, _mum, kid, fam) = family_tree();
    // make a living partner: the kid becomes a parent of a new family
    let spouse = add_person(&mut t, "Old", "Spouse");
    add_event(
        &mut t,
        EventKind::Death,
        Some("1900"),
        &[(spouse, Role::Principal)],
    );
    let note = add_note(&mut t, "family secret");
    let m = bare_media("fam.jpg");
    let mid = m.common.id;
    t.media.insert(mid, m);
    let c = common();
    let fid = c.id;
    let mut c = c;
    c.notes.push(note);
    c.media.push(MediaLink {
        media: mid,
        region: None,
        primary: false,
    });
    t.families.insert(
        fid,
        Family {
            common: c,
            partner1: Some(kid),
            partner2: Some(spouse),
            kind: UnionKind::Married,
            children: vec![],
        },
    );
    let _ = (dad, fam);
    let (out, _) = filter(&t, &policy(LivingMode::Placeholder), today());
    assert!(out.families[&fid].common.notes.is_empty());
    assert!(out.families[&fid].common.media.is_empty());
    assert!(!out.notes.contains_key(&note));
    assert!(!out.media.contains_key(&mid));
}

#[test]
fn hidden_child_link_and_placeholder_residue_cleared() {
    let (mut t, _, _, kid, fam) = family_tree();
    let cit = Citation {
        common: common(),
        source: None,
        page: None,
        date: None,
        text: None,
        quality: None,
        evidence: Evidence {
            original: None,
            primary_info: None,
            direct: None,
        },
    };
    let cid = cit.common.id;
    t.citations.insert(cid, cit);
    let link = &mut t.families.get_mut(&fam).unwrap().children[0];
    link.citations.push(cid);
    link.unknown.push(gt_core::raw::RawNode {
        tag: "_X".into(),
        xref: None,
        value: None,
        pos: 0,
        children: vec![],
    });
    let p = t.persons.get_mut(&kid).unwrap();
    p.common.changed = Some(5);
    p.living_override = Some(LivingOverride::Living);
    let (out, _) = filter(&t, &policy(LivingMode::Placeholder), today());
    let l = &out.families[&fam].children[0];
    assert!(l.citations.is_empty() && l.unknown.is_empty());
    assert_eq!(l.rel1, ParentRel::Birth);
    assert!(!out.citations.contains_key(&cid));
    let p = &out.persons[&kid];
    assert_eq!(p.common.changed, None);
    assert_eq!(p.living_override, None);
}

#[test]
fn family_event_with_hidden_partner_is_dropped_and_counted() {
    let (mut t, _, _, kid, _) = family_tree();
    let ev = add_event(&mut t, EventKind::Marriage, Some("2015"), &[]);
    let c = common();
    let fid = c.id;
    t.families.insert(
        fid,
        Family {
            common: c,
            partner1: Some(kid),
            partner2: None,
            kind: UnionKind::Married,
            children: vec![],
        },
    );
    t.events
        .get_mut(&ev)
        .unwrap()
        .participants
        .push(Participant {
            target: ParticipantRef::Family(fid),
            role: Role::Principal,
        });
    let (out, s) = filter(&t, &policy(LivingMode::Placeholder), today());
    assert!(!out.events.contains_key(&ev));
    assert!(out.families.contains_key(&fid));
    assert!(s.removed_records >= 1);
}
