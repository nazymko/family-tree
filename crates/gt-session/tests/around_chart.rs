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
        open: None,
        kind: ChartKind::Around,
        root: find(s.tree(), given),
        generations,
        aunts_uncles: None,
        kin: None,
        roots: None,
        direction: None,
        print: None,
        fold: None,
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

#[test]
fn person_count_is_the_number_of_distinct_people_on_the_chart() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    for (given, gens) in [("Zed", 255), ("Ivan Petrovych", 1), ("Ivan Petrovych", 255)] {
        let m = s.chart_open(&request(&s, given, gens)).unwrap();
        let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
        let distinct: std::collections::HashSet<_> = w
            .nodes
            .iter()
            .filter_map(|n| n.person.as_ref().map(|p| p.id))
            .collect();
        assert_eq!(m.person_count as usize, distinct.len(), "{given} {gens}");
        assert!(m.person_count >= 1 && m.person_count <= m.node_count);
    }
}

#[test]
fn a_direction_in_the_request_turns_that_chart_only() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    let base = s.chart_open(&request(&s, "Ivan Petrovych", 3)).unwrap();
    let mut req = request(&s, "Ivan Petrovych", 3);
    req.direction = Some(ChartDirection::Left);
    let turned = s.chart_open(&req).unwrap();
    assert!(
        (turned.bounds.w - base.bounds.w).abs() > 1.0
            || (turned.bounds.h - base.bounds.h).abs() > 1.0,
        "a sideways chart has other bounds than the default one"
    );
    // The settings are not touched: the next chart without a direction comes out like the first.
    let again = s.chart_open(&request(&s, "Ivan Petrovych", 3)).unwrap();
    assert_eq!(again.bounds, base.bounds);
}

#[test]
fn a_print_chart_is_compact_and_never_closes_the_charts_that_are_open() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    let a = s.chart_open(&request(&s, "Ivan Petrovych", 3)).unwrap();
    let b = s.chart_open(&request(&s, "Olena", 3)).unwrap();
    let mut req = request(&s, "Ivan Petrovych", 3);
    req.print = Some(true);
    let mut sizes = Vec::new();
    for d in [
        ChartDirection::Top,
        ChartDirection::Left,
        ChartDirection::Right,
    ] {
        req.direction = Some(d);
        let p = s.chart_open(&req).unwrap();
        sizes.push(p.bounds);
        // Both charts of the person are still there while the print chart is open, and after it is closed.
        assert!(s.chart_window(a.chart_id, &everything(), 1.0).is_ok());
        assert!(s.chart_window(b.chart_id, &everything(), 1.0).is_ok());
        s.chart_close(p.chart_id);
    }
    assert!(s.chart_window(a.chart_id, &everything(), 1.0).is_ok());
    assert!(s.chart_window(b.chart_id, &everything(), 1.0).is_ok());
    // The cards are narrower than on screen.
    assert!(
        sizes[0].w < a.bounds.w,
        "compact cards make the same chart narrower"
    );
}

#[test]
fn an_open_print_chart_is_not_evicted_by_the_persons_charts_and_survives_two_more_of_them() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    for d in [ChartDirection::Left, ChartDirection::Bottom] {
        let mut req = request(&s, "Ivan Petrovych", 3);
        req.print = Some(true);
        req.direction = Some(d);
        let print = s.chart_open(&req).unwrap();
        let first = s.chart_open(&request(&s, "Ivan Petrovych", 2)).unwrap();
        let second = s.chart_open(&request(&s, "Olena", 2)).unwrap();
        let third = s.chart_open(&request(&s, "Taras", 2)).unwrap();
        // Two of the person's own charts are kept (the limit), and the print chart stays.
        assert!(s.chart_window(print.chart_id, &everything(), 1.0).is_ok());
        assert!(s.chart_window(second.chart_id, &everything(), 1.0).is_ok());
        assert!(s.chart_window(third.chart_id, &everything(), 1.0).is_ok());
        assert!(s.chart_window(first.chart_id, &everything(), 1.0).is_err());
        s.chart_close(print.chart_id);
        s.chart_close(second.chart_id);
        s.chart_close(third.chart_id);
    }
}

#[test]
fn a_print_chart_around_several_people_is_compact_turned_and_leaves_the_open_charts_alone() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    let ids = vec![find(s.tree(), "Ivan Petrovych"), find(s.tree(), "Olena")];
    let mut req = request(&s, "Ivan Petrovych", 3);
    req.roots = Some(ids);
    let plain = s.chart_open(&req).unwrap();
    s.chart_close(plain.chart_id);
    let a = s.chart_open(&request(&s, "Ivan Petrovych", 3)).unwrap();
    let b = s.chart_open(&request(&s, "Taras", 3)).unwrap();
    req.print = Some(true);
    req.direction = Some(ChartDirection::Left);
    let p = s.chart_open(&req).unwrap();
    assert!(s.chart_window(a.chart_id, &everything(), 1.0).is_ok());
    assert!(s.chart_window(b.chart_id, &everything(), 1.0).is_ok());
    assert!(s.chart_window(p.chart_id, &everything(), 1.0).is_ok());
    assert!(
        (p.bounds.w - plain.bounds.w).abs() > 1.0 || (p.bounds.h - plain.bounds.h).abs() > 1.0,
        "turned and compact"
    );
    s.chart_close(p.chart_id);
}

#[test]
fn print_charts_are_capped_and_the_oldest_goes() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    let mut req = request(&s, "Ivan Petrovych", 2);
    req.print = Some(true);
    req.direction = Some(ChartDirection::Left);
    let ids: Vec<u32> = (0..8)
        .map(|_| s.chart_open(&req).unwrap().chart_id)
        .collect();
    assert!(s.chart_window(ids[0], &everything(), 1.0).is_err());
    assert!(s.chart_window(ids[1], &everything(), 1.0).is_err());
    assert!(s.chart_window(ids[7], &everything(), 1.0).is_ok());
}

/// Parent Pavlo Hnat with eighteen childless children (Child01 .. Child18), no partner.
fn wide_family() -> String {
    let mut g = String::from("0 @I1@ INDI\n1 NAME Pavlo /Hnat/\n1 SEX M\n1 FAMS @F1@\n");
    let mut fam = String::from("0 @F1@ FAM\n1 HUSB @I1@\n");
    for i in 1..=18 {
        g.push_str(&format!(
            "0 @I{}@ INDI\n1 NAME Child{i:02} /Hnat/\n1 SEX M\n1 FAMC @F1@\n",
            i + 1
        ));
        fam.push_str(&format!("1 CHIL @I{}@\n", i + 1));
    }
    g + &fam
}

fn child_rows(s: &Session, m: &ChartMeta) -> Vec<i64> {
    let mut ys: Vec<i64> = shown(s, m)
        .into_iter()
        .filter(|(n, _)| n.starts_with("Child"))
        .map(|(_, y)| y.round() as i64)
        .collect();
    ys.sort_unstable();
    ys.dedup();
    ys
}

#[test]
fn fold_absent_equals_before() {
    let (_d, mut s) = open_tree(&tree_of(&wide_family()));
    let plain = request(&s, "Pavlo", 255);
    let mut off = plain.clone();
    off.fold = Some(false);
    let a = s.chart_open(&plain).unwrap();
    let b = s.chart_open(&off).unwrap();
    assert_eq!(shown(&s, &a), shown(&s, &b));
    assert_eq!(child_rows(&s, &a).len(), 1, "unfolded: one row of children");
}

#[test]
fn fold_true_reaches_layout() {
    let (_d, mut s) = open_tree(&tree_of(&wide_family()));
    let mut req = request(&s, "Pavlo", 255);
    req.fold = Some(true);
    let m = s.chart_open(&req).unwrap();
    assert!(child_rows(&s, &m).len() >= 2, "folded: several sub-rows");
    // The same with people chosen (several roots) takes the other layout path.
    let mut many = req.clone();
    many.roots = Some(vec![many.root, find(s.tree(), "Child01")]);
    let m = s.chart_open(&many).unwrap();
    assert!(
        child_rows(&s, &m).len() >= 2,
        "folded around several people"
    );
}

/// A synthetic chart of about 300 people: Root with a partner and 60 children (30 of them with a partner and six
/// children), and Root's own parents with twelve more childless children (the brothers and sisters).
fn big_family() -> String {
    let (mut indi, mut fam) = (String::new(), String::new());
    let mut next = 1;
    let mut person = |indi: &mut String, name: &str, famc: Option<u32>, fams: Option<u32>| {
        let id = next;
        next += 1;
        indi.push_str(&format!("0 @I{id}@ INDI\n1 NAME {name} /Big/\n1 SEX M\n"));
        if let Some(f) = famc {
            indi.push_str(&format!("1 FAMC @F{f}@\n"));
        }
        if let Some(f) = fams {
            indi.push_str(&format!("1 FAMS @F{f}@\n"));
        }
        id
    };
    let mut nf = 0;
    let mut family = |fam: &mut String, h: u32, w: Option<u32>, kids: &[u32]| {
        nf += 1;
        fam.push_str(&format!("0 @F{nf}@ FAM\n1 HUSB @I{h}@\n"));
        if let Some(w) = w {
            fam.push_str(&format!("1 WIFE @I{w}@\n"));
        }
        for k in kids {
            fam.push_str(&format!("1 CHIL @I{k}@\n"));
        }
    };
    // Families are numbered in creation order: F1 = the parents', F2 = Root's own, then one per married child.
    let dad = person(&mut indi, "Dad", None, Some(1));
    let mom = person(&mut indi, "Mom", None, Some(1));
    let root = person(&mut indi, "Root", Some(1), Some(2));
    let sibs: Vec<u32> = (0..12)
        .map(|i| person(&mut indi, &format!("Sib{i:02}"), Some(1), None))
        .collect();
    let mut first = vec![root];
    first.extend(sibs);
    family(&mut fam, dad, Some(mom), &first);
    let wife = person(&mut indi, "Wife", None, Some(2));
    let mut kids = Vec::new();
    let mut later = Vec::new();
    for i in 0..60 {
        let married = i < 30;
        let f = 3 + i;
        let c = person(
            &mut indi,
            &format!("Kid{i:02}"),
            Some(2),
            married.then_some(f),
        );
        kids.push(c);
        if married {
            later.push(c);
        }
    }
    family(&mut fam, root, Some(wife), &kids);
    for (i, c) in later.into_iter().enumerate() {
        let spouse = person(&mut indi, &format!("In{i:02}"), None, Some(3 + i as u32));
        let gk: Vec<u32> = (0..6)
            .map(|j| {
                person(
                    &mut indi,
                    &format!("Gk{i:02}x{j}"),
                    Some(3 + i as u32),
                    None,
                )
            })
            .collect();
        family(&mut fam, c, Some(spouse), &gk);
    }
    indi + &fam
}

#[test]
fn fold_on_a_chart_of_about_300_people_lays_out_in_time() {
    let (_d, mut s) = open_tree(&tree_of(&big_family()));
    let mut req = request(&s, "Root", 255);
    req.kin = Some(AroundKin::All);
    let t = std::time::Instant::now();
    let off = s.chart_open(&req).unwrap();
    let t_off = t.elapsed();
    req.fold = Some(true);
    let t = std::time::Instant::now();
    let on = s.chart_open(&req).unwrap();
    let t_on = t.elapsed();
    eprintln!(
        "TIMING people={} fold off {t_off:?}, fold on {t_on:?}",
        on.person_count
    );
    assert!(on.person_count >= 250, "a big chart: {}", on.person_count);
    assert_eq!(on.person_count, off.person_count);
    assert!(t_on.as_secs() < 20, "folding stays fast: {t_on:?}");
}
