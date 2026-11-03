#![allow(clippy::unwrap_used)]
use gt_core::ids::new_id;
use gt_core::model::*;
use gt_core::tree::Tree;
use std::collections::HashSet;

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

fn person() -> Person {
    Person {
        common: common(),
        sex: Sex::Unknown,
        names: vec![],
        living_override: None,
    }
}

fn assertion() -> Assertion {
    Assertion {
        preferred: true,
        proof: None,
        confidence: None,
    }
}

#[test]
fn ids_are_v7_and_unique() {
    let mut seen = HashSet::new();
    for _ in 0..1000 {
        let id = new_id();
        assert_eq!(id.get_version_num(), 7);
        assert!(seen.insert(id));
    }
}

#[test]
fn tree_relationship_queries() {
    let (p1, p2, c) = (person(), person(), person());
    let (p1id, p2id, cid) = (p1.common.id, p2.common.id, c.common.id);
    let fam = Family {
        common: common(),
        partner1: Some(p1id),
        partner2: Some(p2id),
        kind: UnionKind::Married,
        children: vec![ChildLink {
            person: cid,
            rel1: ParentRel::Birth,
            rel2: ParentRel::Adopted,
            preferred: true,
            assertion: assertion(),
            citations: vec![],
            unknown: vec![],
        }],
    };
    let fid = fam.common.id;
    let ev = Event {
        common: common(),
        kind: EventKind::Birth,
        date: None,
        place: None,
        description: None,
        age: None,
        cause: None,
        negative: false,
        assertion: assertion(),
        participants: vec![Participant {
            target: ParticipantRef::Person(cid),
            role: Role::Principal,
        }],
    };
    let mut t = Tree::new();
    t.persons.insert(p1id, p1);
    t.persons.insert(p2id, p2);
    t.persons.insert(cid, c);
    t.families.insert(fid, fam);
    t.events.insert(ev.common.id, ev);

    let fs: Vec<_> = t.families_as_partner(p1id).collect();
    assert_eq!(fs.len(), 1);
    assert_eq!(fs[0].common.id, fid);

    let cs: Vec<_> = t.families_as_child(cid).collect();
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].0.common.id, fid);
    assert_eq!(cs[0].1.rel2, ParentRel::Adopted);

    let es: Vec<_> = t.events_of(cid).collect();
    assert_eq!(es.len(), 1);
    assert_eq!(*es[0].1, Role::Principal);
}

#[test]
fn normalized_for_compare_strips_xrefs_and_changed() {
    let mut p = person();
    p.common.changed = Some(5);
    p.common.ext_ids = vec![
        ExternalId {
            kind: ExtIdKind::Xref,
            value: "@I1@".into(),
        },
        ExternalId {
            kind: ExtIdKind::Uid,
            value: "ABC".into(),
        },
    ];
    let id = p.common.id;
    let mut t = Tree::new();
    t.persons.insert(id, p);
    let n = t.normalized_for_compare();
    let np = &n.persons[&id];
    assert_eq!(np.common.changed, None);
    assert_eq!(
        np.common.ext_ids,
        vec![ExternalId {
            kind: ExtIdKind::Uid,
            value: "ABC".into()
        }]
    );
    assert_eq!(t.persons[&id].common.changed, Some(5));
}
