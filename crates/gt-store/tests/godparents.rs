#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Godparents in the project file: saved and read back in order, and the schema 12 migration
//! that turns an older project's godparent participants into godparents.

mod common;

use common::*;
use gt_core::Uuid;
use gt_core::model::*;
use gt_core::tree::Tree;
use gt_store::{Project, SCHEMA_VERSION};
use rusqlite::Connection;

const TODAY: i32 = 2_461_000;

fn baptism_tree() -> (Tree, Uuid, Uuid, Uuid) {
    let mut t = Tree::new();
    let child = add_person(&mut t, person("Petro", "Koval"));
    let mut godfather = person("Ivan", "Koval");
    godfather.sex = Sex::Male;
    let godfather = add_person(&mut t, godfather);
    let mut godmother = person("Maria", "Shevchuk");
    godmother.sex = Sex::Female;
    let godmother = add_person(&mut t, godmother);
    let mut e = event(EventKind::Baptism, child, "6 FEB 1906", None);
    e.godparents = vec![
        Godparent {
            id: id(),
            person: Some(godfather),
            name: None,
            role: GodparentRole::Godfather,
            note: Some("the uncle".into()),
        },
        Godparent {
            id: id(),
            person: None,
            name: Some("Hanna Bondar, a widow of Ivanivka".into()),
            role: GodparentRole::Godmother,
            note: None,
        },
    ];
    t.events.insert(e.common.id, e);
    (t, child, godfather, godmother)
}

#[test]
fn godparents_are_saved_and_read_back_in_order() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p.genetree");
    let (tree, ..) = baptism_tree();
    let mut p = Project::create(&dir).unwrap();
    p.save_tree(&tree, TODAY).unwrap();
    drop(p);
    let p = Project::open(&dir).unwrap();
    assert_eq!(p.load_tree().unwrap(), tree);
}

#[test]
fn an_older_project_s_godparent_participants_become_godparents() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("old.genetree");
    let (mut tree, child, godfather, godmother) = baptism_tree();
    let mut e = tree.events.values().next().unwrap().clone();
    e.godparents.clear();
    e.participants.push(Participant {
        target: ParticipantRef::Person(godfather),
        role: Role::Godparent,
    });
    e.participants.push(Participant {
        target: ParticipantRef::Person(godmother),
        role: Role::Other("Godmother".into()),
    });
    // A witness stays a witness.
    e.participants.push(Participant {
        target: ParticipantRef::Person(godmother),
        role: Role::Witness,
    });
    tree.events.insert(e.common.id, e.clone());
    {
        let mut p = Project::create(&dir).unwrap();
        p.save_tree(&tree, TODAY).unwrap();
    }
    {
        let c = Connection::open(dir.join("tree.db")).unwrap();
        c.execute_batch("ALTER TABLE note DROP COLUMN story; ALTER TABLE media DROP COLUMN map_layer_json; DROP TABLE godparent; PRAGMA user_version = 11;")
            .unwrap();
    }
    let p = Project::open(&dir).unwrap();
    assert_eq!(p.schema_version(), SCHEMA_VERSION);
    let back = p.load_tree().unwrap();
    let ev = &back.events[&e.common.id];
    assert_eq!(
        ev.participants
            .iter()
            .map(|x| (x.target.clone(), x.role.clone()))
            .collect::<Vec<_>>(),
        vec![
            (ParticipantRef::Person(child), Role::Principal),
            (ParticipantRef::Person(godmother), Role::Witness),
        ]
    );
    let gods: Vec<_> = ev.godparents.iter().map(|g| (g.person, g.role)).collect();
    assert_eq!(
        gods,
        vec![
            (Some(godfather), GodparentRole::Godfather),
            (Some(godmother), GodparentRole::Godmother),
        ]
    );
}
