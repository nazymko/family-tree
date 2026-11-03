#![allow(dead_code, clippy::unwrap_used)]
//! Shared tree builders. (The GEDCOM importer lives in another lane, so the trees are built by hand.)

use gt_core::DayOrder;
use gt_core::Uuid;
use gt_core::date::GenDate;
use gt_core::model::*;
use gt_core::raw::RawNode;
use gt_core::tree::Tree;

pub fn id() -> Uuid {
    Uuid::now_v7()
}

pub fn common(id: Uuid) -> Common {
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

pub fn date(s: &str) -> Option<GenDate> {
    Some(GenDate::parse(s, DayOrder::Dmy))
}

pub fn raw(tag: &str, value: &str, children: Vec<RawNode>) -> RawNode {
    RawNode {
        tag: tag.into(),
        xref: None,
        value: Some(value.into()),
        pos: 0,
        children,
    }
}

pub fn name(given: &str, surname: &str) -> Name {
    Name {
        id: id(),
        kind: NameKind::Birth,
        assertion: Assertion {
            preferred: false,
            proof: None,
            confidence: None,
        },
        private: false,
        prefix: None,
        given: Some(given.into()),
        call: None,
        surname_prefix: None,
        surnames: vec![surname.into()],
        patronymic: None,
        suffix: None,
        nickname: None,
        lang: None,
        date: None,
        raw: Some(format!("{given} /{surname}/")),
        citations: vec![],
        notes: vec![],
        unknown: vec![],
    }
}

pub fn person(given: &str, surname: &str) -> Person {
    Person {
        common: common(id()),
        sex: Sex::Unknown,
        names: vec![name(given, surname)],
        living_override: None,
    }
}

pub fn add_person(t: &mut Tree, p: Person) -> Uuid {
    let id = p.common.id;
    t.persons.insert(id, p);
    id
}

pub fn place(name: &str) -> Place {
    Place {
        common: common(id()),
        names: vec![PlaceName {
            name: name.into(),
            lang: Some("en".into()),
            date: None,
        }],
        parent: None,
        kind: None,
        lat: None,
        lon: None,
        raw: Some(name.into()),
    }
}

pub fn event(kind: EventKind, who: Uuid, when: &str, at: Option<Uuid>) -> Event {
    Event {
        common: common(id()),
        kind,
        date: date(when),
        place: at,
        description: None,
        age: None,
        cause: None,
        negative: false,
        assertion: Assertion {
            preferred: true,
            proof: None,
            confidence: None,
        },
        participants: vec![Participant {
            target: ParticipantRef::Person(who),
            role: Role::Principal,
        }],
    }
}

/// A tree that sets every kind of field, with all ids in REVERSE creation order so that
/// sorting by id would not reproduce the insertion order.
pub fn rich_tree() -> Tree {
    let mut t = Tree::new();
    t.header.producer = Some("Test".into());
    t.header.source_version = Some("1.2".into());
    t.header.unknown = vec![raw("_HDR", "x", vec![raw("_SUB", "y", vec![])])];

    // Pre-allocate ids, then insert in reverse so insertion order != id order.
    let ids: Vec<Uuid> = (0..4).map(|_| id()).collect();
    let rev: Vec<Uuid> = ids.iter().rev().copied().collect();

    let mut repo = Repository {
        common: common(rev[0]),
        name: "Archive".into(),
        address: Some("1 Main St".into()),
        urls: vec!["https://example.org".into(), "https://example.net".into()],
    };
    repo.common.private = true;
    t.repositories.insert(repo.common.id, repo);

    let cite_id = rev[0];
    let note_id = rev[1];
    let media_id = rev[2];
    let source_id = rev[3];
    let place_a = id();
    let place_b = id();

    let mut src = Source {
        common: common(source_id),
        title: Some("Parish register".into()),
        author: Some("Rev. Smith".into()),
        publication: Some("1850".into()),
        abbreviation: Some("PR".into()),
        text: Some("Baptisms 1800-1850".into()),
        repositories: vec![RepoRef {
            repository: t.repositories.keys().next().copied().unwrap(),
            call_number: Some("A/1".into()),
        }],
    };
    src.common.unknown = vec![raw("_UNK", "u", vec![])];
    t.sources.insert(source_id, src);

    t.citations.insert(
        cite_id,
        Citation {
            common: common(cite_id),
            source: Some(source_id),
            page: Some("p. 4".into()),
            date: date("1 JAN 1850"),
            text: Some("quoted".into()),
            quality: Some(3),
            evidence: Evidence {
                original: Some(true),
                primary_info: Some(false),
                direct: None,
            },
        },
    );
    t.notes.insert(
        note_id,
        Note {
            common: common(note_id),
            kind: NoteKind::Other("diary".into()),
            text: "A long story about the family.".into(),
            lang: Some("en".into()),
            shared: true,
        },
    );
    t.media.insert(
        media_id,
        Media {
            common: common(media_id),
            path: "media/ab/abcdef.jpg".into(),
            original_name: Some("photo.jpg".into()),
            mime: Some("image/jpeg".into()),
            title: Some("Portrait".into()),
            date: date("ABT 1900"),
            description: Some("desc".into()),
            sha256: Some("abcdef".into()),
        },
    );

    // Places in reverse parent order: child first, parent after.
    let mut pb = place("Springfield");
    pb.common.id = place_b;
    pb.parent = Some(place_a);
    pb.lat = Some(39.78);
    pb.lon = Some(-89.65);
    pb.kind = Some("city".into());
    pb.names.push(PlaceName {
        name: "Springfeld".into(),
        lang: Some("de".into()),
        date: date("FROM 1800 TO 1850"),
    });
    t.places.insert(place_b, pb);
    let mut pa = place("Illinois");
    pa.common.id = place_a;
    pa.common.ext_ids = vec![ExternalId {
        kind: ExtIdKind::Exid {
            scheme: "geonames".into(),
        },
        value: "4896861".into(),
    }];
    t.places.insert(place_a, pa);

    // People, reverse id order.
    let mut p2 = person("Anna", "Müller");
    p2.common.id = rev[1];
    p2.sex = Sex::Female;
    p2.names[0].assertion = Assertion {
        preferred: true,
        proof: Some(Proof::Proven),
        confidence: Some(2),
    };
    p2.names.push({
        let mut n = name("Anna", "Schmidt");
        n.kind = NameKind::Married;
        n.prefix = Some("Dr.".into());
        n.suffix = Some("Jr.".into());
        n.surname_prefix = Some("von".into());
        n.surnames = vec!["Schmidt".into(), "Kovač".into()];
        n.date = date("BEF 1900");
        n.citations = vec![cite_id];
        n.notes = vec![note_id];
        n.unknown = vec![raw("_X", "1", vec![])];
        n
    });
    p2.common.changed = Some(1_700_000_000_000);
    p2.common.citations = vec![cite_id];
    p2.common.notes = vec![note_id];
    p2.common.media = vec![
        MediaLink {
            media: media_id,
            region: Some(Region {
                x: 0.1,
                y: 0.2,
                w: 0.3,
                h: 0.4,
            }),
            primary: true,
        },
        MediaLink {
            media: media_id,
            region: None,
            primary: false,
        },
    ];
    p2.common.ext_ids = vec![
        ExternalId {
            kind: ExtIdKind::Uid,
            value: "u-1".into(),
        },
        ExternalId {
            kind: ExtIdKind::Other("rin".into()),
            value: "7".into(),
        },
    ];
    p2.living_override = Some(LivingOverride::Deceased);
    let p2_id = p2.common.id;
    t.persons.insert(p2_id, p2);

    let mut p1 = person("Karl", "Smith");
    p1.common.id = rev[0];
    p1.sex = Sex::Other("x".into());
    p1.common.private = true;
    let p1_id = p1.common.id;
    t.persons.insert(p1_id, p1);
    let mut p0 = person("Zed", "Zulu");
    p0.common.id = rev[2];
    let p0_id = p0.common.id;
    t.persons.insert(p0_id, p0);

    // Events: reverse order too.
    let mut ev = event(
        EventKind::Custom("Fame".into()),
        p1_id,
        "1900",
        Some(place_b),
    );
    ev.common.id = rev[3];
    ev.negative = true;
    ev.description = Some("famous".into());
    ev.age = Some("30y".into());
    ev.cause = Some("none".into());
    ev.assertion.proof = Some(Proof::Disproven);
    ev.assertion.confidence = Some(1);
    ev.participants.push(Participant {
        target: ParticipantRef::Person(p2_id),
        role: Role::Witness,
    });
    ev.participants.push(Participant {
        target: ParticipantRef::Person(p0_id),
        role: Role::Other("herald".into()),
    });
    let mut ev2 = event(EventKind::Birth, p2_id, "12 MAR 1851", Some(place_b));
    ev2.common.id = rev[1];
    let ev2_id = ev2.common.id;
    let ev_id = ev.common.id;

    // Family with children in non-id order.
    let fam_id = rev[2];
    let mut fam = Family {
        common: common(fam_id),
        partner1: Some(p1_id),
        partner2: Some(p2_id),
        kind: UnionKind::CivilUnion,
        children: vec![],
    };
    for who in [p0_id, p2_id] {
        fam.children.push(ChildLink {
            person: who,
            rel1: ParentRel::Adopted,
            rel2: ParentRel::Other("odd".into()),
            preferred: who == p0_id,
            assertion: Assertion {
                preferred: who == p0_id,
                proof: Some(Proof::Proven),
                confidence: Some(3),
            },
            citations: vec![cite_id],
            unknown: vec![raw("_CH", "c", vec![])],
        });
    }
    t.families.insert(fam_id, fam);
    let mut marr = event(EventKind::Marriage, p1_id, "1875", None);
    marr.common.id = rev[0];
    marr.participants = vec![Participant {
        target: ParticipantRef::Family(fam_id),
        role: Role::Principal,
    }];
    let marr_id = marr.common.id;
    t.events.insert(ev_id, ev);
    t.events.insert(ev2_id, ev2);
    t.events.insert(marr_id, marr);
    t
}

/// Large synthetic tree: `n` persons in couples with children, births and places.
pub fn big_tree(n: usize) -> Tree {
    let mut t = Tree::new();
    let places: Vec<Uuid> = (0..50)
        .map(|i| {
            let p = place(&format!("Town {i}"));
            let id = p.common.id;
            t.places.insert(id, p);
            id
        })
        .collect();
    let mut prev: Option<Uuid> = None;
    for i in 0..n {
        let mut p = person(&format!("Given{i}"), &format!("Family{}", i % 997));
        p.common.ext_ids.push(ExternalId {
            kind: ExtIdKind::Uid,
            value: format!("uid-{i}"),
        });
        let pid = add_person(&mut t, p);
        let ev = event(
            EventKind::Birth,
            pid,
            &format!("{} JAN {}", 1 + i % 28, 1800 + i % 150),
            Some(places[i % places.len()]),
        );
        t.events.insert(ev.common.id, ev);
        if i % 2 == 1 {
            let fam = Family {
                common: common(id()),
                partner1: prev,
                partner2: Some(pid),
                kind: UnionKind::Married,
                children: vec![],
            };
            t.families.insert(fam.common.id, fam);
        }
        prev = Some(pid);
    }
    t
}
