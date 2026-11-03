#![allow(clippy::unwrap_used)]
use gt_core::date::calendar::{Calendar, first_jdn};
use gt_core::date::{DayOrder, GenDate};
use gt_core::ids::new_id;
use gt_core::living::{LivingRule, compute_living};
use gt_core::model::*;
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

fn add_person(t: &mut Tree) -> Uuid {
    let c = common();
    let id = c.id;
    t.persons.insert(
        id,
        Person {
            common: c,
            sex: Sex::Unknown,
            names: vec![],
            living_override: None,
        },
    );
    id
}

fn add_event(t: &mut Tree, p: Uuid, kind: EventKind, date: Option<&str>) {
    let c = common();
    t.events.insert(
        c.id,
        Event {
            common: c,
            kind,
            date: date.map(|d| GenDate::parse(d, DayOrder::Dmy)),
            place: None,
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
                target: ParticipantRef::Person(p),
                role: Role::Principal,
            }],
        },
    );
}

fn family(t: &mut Tree, parent: Uuid, child: Uuid) {
    let c = common();
    t.families.insert(
        c.id,
        Family {
            common: c,
            partner1: Some(parent),
            partner2: None,
            kind: UnionKind::Unknown,
            children: vec![ChildLink {
                person: child,
                rel1: ParentRel::Birth,
                rel2: ParentRel::Birth,
                preferred: true,
                assertion: Assertion {
                    preferred: true,
                    proof: None,
                    confidence: None,
                },
                citations: vec![],
                unknown: vec![],
            }],
        },
    );
}

fn today() -> i32 {
    first_jdn(Calendar::Gregorian, 2026, Some(11), Some(3)).unwrap()
}

fn living_with_birth(d: &str) -> bool {
    let mut t = Tree::new();
    let p = add_person(&mut t);
    add_event(&mut t, p, EventKind::Birth, Some(d));
    compute_living(&t, &LivingRule::default(), today())[&p]
}

#[test]
fn no_dates_anywhere_is_living() {
    let mut t = Tree::new();
    let p = add_person(&mut t);
    let c = add_person(&mut t);
    family(&mut t, p, c);
    let m = compute_living(&t, &LivingRule::default(), today());
    assert!(m[&p] && m[&c]);
}

#[test]
fn death_without_date_is_dead() {
    let mut t = Tree::new();
    let p = add_person(&mut t);
    add_event(&mut t, p, EventKind::Death, None);
    assert!(!compute_living(&t, &LivingRule::default(), today())[&p]);
}

#[test]
fn birth_bounds() {
    assert!(!living_with_birth("ABT 1920"));
    assert!(living_with_birth("ABT 1930"));
    assert!(!living_with_birth("BEF 1920"));
    assert!(living_with_birth("AFT 1900"));
    assert!(living_with_birth("1 JAN 1927"));
    assert!(!living_with_birth("1 JAN 1926"));
}

#[test]
fn old_child_proves_parent_dead() {
    let mut t = Tree::new();
    let p = add_person(&mut t);
    let c = add_person(&mut t);
    family(&mut t, p, c);
    add_event(&mut t, c, EventKind::Birth, Some("1910"));
    let m = compute_living(&t, &LivingRule::default(), today());
    assert!(!m[&p]);
    assert!(!m[&c]);
}

#[test]
fn old_parent_proves_child_dead() {
    let mut t = Tree::new();
    let p = add_person(&mut t);
    let c = add_person(&mut t);
    family(&mut t, p, c);
    add_event(&mut t, p, EventKind::Birth, Some("1850"));
    assert!(!compute_living(&t, &LivingRule::default(), today())[&c]);
    // a parent born 1900 does not prove the child dead
    let mut t = Tree::new();
    let p = add_person(&mut t);
    let c = add_person(&mut t);
    family(&mut t, p, c);
    add_event(&mut t, p, EventKind::Birth, Some("1900"));
    assert!(compute_living(&t, &LivingRule::default(), today())[&c]);
}

#[test]
fn override_wins() {
    let mut t = Tree::new();
    let p = add_person(&mut t);
    add_event(&mut t, p, EventKind::Death, None);
    t.persons.get_mut(&p).unwrap().living_override = Some(LivingOverride::Living);
    assert!(compute_living(&t, &LivingRule::default(), today())[&p]);
    t.persons.get_mut(&p).unwrap().living_override = Some(LivingOverride::Deceased);
    assert!(!compute_living(&t, &LivingRule::default(), today())[&p]);
}
