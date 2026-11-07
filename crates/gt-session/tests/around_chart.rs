#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The chart built around one person: their own line only, ancestors up and descendants down.

mod common;

use common::*;
use gt_session::*;

/// Ivan Petrovych Koval + Olena have Taras and Maria; Taras + Oksana have Andriy. Ivan's parents are Petro and
/// Halyna, who also have Hryhorii (Ivan's brother, with a daughter Sofia). Olena's parents are Stepan and Nina.
/// Oksana's father is Dmytro Bondar. Zed is alone.
const KOVAL: &str = "\
0 @I1@ INDI\n1 NAME Ivan Petrovych /Koval/\n1 SEX M\n1 FAMS @F1@\n1 FAMC @F2@\n\
0 @I2@ INDI\n1 NAME Olena /Koval/\n1 SEX F\n1 FAMS @F1@\n1 FAMC @F3@\n\
0 @I3@ INDI\n1 NAME Taras /Koval/\n1 SEX M\n1 FAMC @F1@\n1 FAMS @F4@\n\
0 @I4@ INDI\n1 NAME Maria /Koval/\n1 SEX F\n1 FAMC @F1@\n\
0 @I5@ INDI\n1 NAME Oksana /Bondar/\n1 SEX F\n1 FAMS @F4@\n1 FAMC @F5@\n\
0 @I6@ INDI\n1 NAME Andriy /Koval/\n1 SEX M\n1 FAMC @F4@\n\
0 @I7@ INDI\n1 NAME Petro /Koval/\n1 SEX M\n1 FAMS @F2@\n\
0 @I8@ INDI\n1 NAME Halyna /Koval/\n1 SEX F\n1 FAMS @F2@\n\
0 @I9@ INDI\n1 NAME Stepan /Moroz/\n1 SEX M\n1 FAMS @F3@\n\
0 @I10@ INDI\n1 NAME Nina /Moroz/\n1 SEX F\n1 FAMS @F3@\n\
0 @I11@ INDI\n1 NAME Zed /Solo/\n1 SEX M\n\
0 @I12@ INDI\n1 NAME Hryhorii /Koval/\n1 SEX M\n1 FAMC @F2@\n1 FAMS @F6@\n\
0 @I13@ INDI\n1 NAME Sofia /Koval/\n1 SEX F\n1 FAMC @F6@\n\
0 @I14@ INDI\n1 NAME Dmytro /Bondar/\n1 SEX M\n1 FAMS @F5@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n1 CHIL @I4@\n\
0 @F2@ FAM\n1 HUSB @I7@\n1 WIFE @I8@\n1 CHIL @I1@\n1 CHIL @I12@\n\
0 @F3@ FAM\n1 HUSB @I9@\n1 WIFE @I10@\n1 CHIL @I2@\n\
0 @F4@ FAM\n1 HUSB @I3@\n1 WIFE @I5@\n1 CHIL @I6@\n\
0 @F5@ FAM\n1 HUSB @I14@\n1 CHIL @I5@\n\
0 @F6@ FAM\n1 HUSB @I12@\n1 CHIL @I13@\n";

fn everything() -> Rect {
    Rect {
        x: -1e6,
        y: -1e6,
        w: 2e6,
        h: 2e6,
    }
}

fn request(s: &Session, given: &str, generations: u32) -> ChartRequest {
    ChartRequest {
        kind: ChartKind::Around,
        root: find(s.tree(), given),
        generations,
        aunts_uncles: None,
        kin: None,
    }
}

fn shown(s: &Session, m: &ChartMeta) -> Vec<(String, f32)> {
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    let mut v: Vec<(String, f32)> = w
        .nodes
        .iter()
        .filter_map(|n| n.person.as_ref().map(|p| (p.name.clone(), n.y)))
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

#[test]
fn only_the_line_is_shown_ancestors_above_and_descendants_below() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    let m = s.chart_open(&request(&s, "Ivan Petrovych", 255)).unwrap();
    assert_eq!(m.kind, ChartKind::Around);
    assert!(!m.truncated);
    let v = shown(&s, &m);
    let names: Vec<&str> = v.iter().map(|(n, _)| n.as_str()).collect();
    for want in [
        "Ivan Petrovych Koval",
        "Olena Koval",
        "Taras Koval",
        "Maria Koval",
        "Oksana Bondar",
        "Andriy Koval",
        "Petro Koval",
        "Halyna Koval",
    ] {
        assert!(names.contains(&want), "{want} is shown: {names:?}");
    }
    // The brother, his daughter, the in-laws and the stranger are not.
    for absent in [
        "Hryhorii Koval",
        "Sofia Koval",
        "Dmytro Bondar",
        "Stepan Moroz",
        "Nina Moroz",
        "Zed Solo",
    ] {
        assert!(!names.contains(&absent), "{absent} is not shown");
    }
    assert_eq!(v.len(), 8);
    let y = |name: &str| v.iter().find(|(n, _)| n == name).unwrap().1;
    assert!(y("Petro Koval") < y("Ivan Petrovych Koval"));
    assert!(y("Taras Koval") > y("Ivan Petrovych Koval"));
    assert!(y("Andriy Koval") > y("Taras Koval"));
    assert_eq!(y("Olena Koval"), y("Ivan Petrovych Koval"));
}

#[test]
fn the_brothers_and_sisters_option_adds_them_and_their_children_only() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    let mut req = request(&s, "Ivan Petrovych", 255);
    req.kin = Some(AroundKin::Siblings);
    let m = s.chart_open(&req).unwrap();
    let v = shown(&s, &m);
    let names: Vec<&str> = v.iter().map(|(n, _)| n.as_str()).collect();
    for want in [
        "Hryhorii Koval",
        "Sofia Koval",
        "Taras Koval",
        "Petro Koval",
    ] {
        assert!(names.contains(&want), "{want} is shown: {names:?}");
    }
    for absent in ["Stepan Moroz", "Nina Moroz", "Dmytro Bondar", "Zed Solo"] {
        assert!(!names.contains(&absent), "{absent} is not shown");
    }
    let y = |name: &str| v.iter().find(|(n, _)| n == name).unwrap().1;
    assert_eq!(y("Hryhorii Koval"), y("Ivan Petrovych Koval"));
    assert!(y("Sofia Koval") > y("Hryhorii Koval"));
}

#[test]
fn people_are_named_by_their_relation_and_the_outline_follows_it() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    let m = s.chart_open(&request(&s, "Ivan Petrovych", 255)).unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    let rel = |name: &str| {
        w.nodes
            .iter()
            .find(|n| n.person.as_ref().is_some_and(|p| p.name == name))
            .and_then(|n| n.relation.clone())
    };
    assert_eq!(rel("Petro Koval").as_deref(), Some("father"));
    assert_eq!(rel("Andriy Koval").as_deref(), Some("grandson"));
    let top = s.chart_outline(m.chart_id, None, 10).unwrap();
    assert_eq!(top.len(), 1);
}

#[test]
fn a_person_alone_still_shows_and_a_limit_marks_the_cut() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    let m = s.chart_open(&request(&s, "Zed", 255)).unwrap();
    assert_eq!(m.node_count, 1);
    let m = s.chart_open(&request(&s, "Ivan Petrovych", 1)).unwrap();
    // One row each way: the parents and the children, not the grandparents or grandchildren.
    assert_eq!(m.node_count, 7);
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    assert!(
        w.nodes.iter().any(|n| n.more_down),
        "Taras has a child below the cut"
    );
}

#[test]
fn a_person_who_is_gone_is_not_found() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    let mut r = request(&s, "Ivan Petrovych", 255);
    r.root = uuid::Uuid::max();
    assert!(s.chart_open(&r).is_err());
}

/// The partner roles say whether the union is a marriage, by the same rule as `Kinship.married`: married unless
/// recorded as unmarried or other.
#[test]
fn partner_roles_carry_whether_the_union_is_a_marriage() {
    let mut tree = tree_of(KOVAL);
    let taras = find(&tree, "Taras");
    let unmarried = tree
        .families
        .values_mut()
        .find(|f| f.partner1 == Some(taras))
        .unwrap();
    unmarried.kind = gt_core::model::UnionKind::Unmarried;
    let (_d, mut s) = open_tree(&tree);
    let m = s.chart_open(&request(&s, "Ivan Petrovych", 255)).unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    let role_of = |name: &str| {
        w.nodes
            .iter()
            .find(|n| n.person.as_ref().is_some_and(|p| p.name.starts_with(name)))
            .and_then(|n| n.role)
            .unwrap()
    };
    // Olena shares an imported family with Ivan, which counts as a marriage.
    let olena = role_of("Olena");
    assert_eq!(olena.kind, RoleKind::Partner);
    assert!(olena.married);
    // Oksana's union with Taras is recorded as unmarried.
    let oksana = role_of("Oksana");
    assert_eq!(oksana.kind, RoleKind::PartnerOfDescendant);
    assert!(!oksana.married);
    // Blood relatives never claim to be married.
    assert!(!role_of("Taras").married);
}
