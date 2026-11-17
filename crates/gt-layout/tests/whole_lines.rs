#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The whole-tree chart of a tree with many top ancestor clusters tied together by marriages must not draw
//! lines across the whole chart. Synthetic data only: invented clusters, no real names.

use std::collections::HashMap;
use std::time::Instant;

use gt_layout::{Metrics, Quality, WholeFamily, WholeInput, WholeLayout, quality, whole};

struct Builder {
    count: usize,
    ids: HashMap<(usize, String), u32>,
    surname: Vec<String>,
    birth: Vec<Option<i32>>,
}

impl Builder {
    fn person(&mut self, c: usize, slot: &str, year: i32) -> u32 {
        let id = self.count as u32;
        self.count += 1;
        self.ids.insert((c, slot.to_string()), id);
        self.surname.push(format!("Clan{c}"));
        self.birth.push(Some(year));
        id
    }
    fn get(&self, c: usize, slot: &str) -> u32 {
        self.ids[&(c, slot.to_string())]
    }
}

type Slot = (usize, &'static str);

/// Clusters `0..n`, each: founders `a`+`b`; children `k0`,`k1` (sons) and `k2` (daughter) with a spouse each;
/// two grandchildren per couple (`g{i}0` a son, `g{i}1` a daughter); `g00`, `g11`, `g20` marry and have one child.
/// `cross`: (a person who marries, the spouse from another cluster who takes the place of the plain in-law). The
/// children of the couple are the first person's; whatever hangs off the spouse's own slot is dropped.
pub fn clusters(n: usize, cross: &[(Slot, Slot)]) -> WholeInput {
    let mut b = Builder {
        count: 0,
        ids: HashMap::new(),
        surname: Vec::new(),
        birth: Vec::new(),
    };
    let spouse_slot = |slot: &str| match slot {
        "k0" => "w0",
        "k1" => "w1",
        "k2" => "h2",
        "g00" => "x0",
        "g11" => "x1",
        "g20" => "x2",
        _ => unreachable!(),
    };
    let marries_in = |c: usize, k: &str| cross.iter().find(|&&(_, (c2, s2))| c2 == c && s2 == k);
    let marries_out = |c: usize, k: &str| cross.iter().find(|&&((c1, s1), _)| c1 == c && s1 == k);
    for c in 0..n {
        b.person(c, "a", 1800);
        b.person(c, "b", 1802);
        for i in 0..3 {
            b.person(c, &format!("k{i}"), 1825 + i as i32);
        }
        for i in 0..3 {
            for j in 0..2 {
                b.person(c, &format!("g{i}{j}"), 1850 + (i * 2 + j) as i32);
            }
        }
        for i in 0..3 {
            b.person(c, &format!("y{i}"), 1880);
        }
        for k in ["k0", "k1", "k2", "g00", "g11", "g20"] {
            if marries_out(c, k).is_none() && marries_in(c, k).is_none() {
                b.person(c, spouse_slot(k), 1826);
            }
        }
    }
    let spouse = |b: &Builder, c: usize, k: &str| -> Option<u32> {
        if let Some(&(_, (c2, s2))) = marries_out(c, k) {
            return Some(b.get(c2, s2));
        }
        if marries_in(c, k).is_some() {
            return None; // listed with the other side
        }
        Some(b.get(c, spouse_slot(k)))
    };
    let mut fams: Vec<WholeFamily> = Vec::new();
    for c in 0..n {
        let kid = |s: &str| b.get(c, s);
        fams.push(WholeFamily {
            partners: [Some(kid("a")), Some(kid("b"))],
            children: vec![kid("k0"), kid("k1"), kid("k2")],
        });
        for (i, k) in ["k0", "k1", "k2"].into_iter().enumerate() {
            if let Some(sp) = spouse(&b, c, k) {
                fams.push(WholeFamily {
                    partners: [Some(kid(k)), Some(sp)],
                    children: vec![kid(&format!("g{i}0")), kid(&format!("g{i}1"))],
                });
            }
        }
        for (i, g) in ["g00", "g11", "g20"].into_iter().enumerate() {
            if let Some(sp) = spouse(&b, c, g) {
                fams.push(WholeFamily {
                    partners: [Some(kid(g)), Some(sp)],
                    children: vec![kid(&format!("y{i}"))],
                });
            }
        }
    }
    prune(b, fams)
}

/// Drops the people who ended up with no family of any size (the spouse's own slots that were replaced).
fn prune(b: Builder, fams: Vec<WholeFamily>) -> WholeInput {
    let n = b.count;
    let mut dsu: Vec<usize> = (0..n).collect();
    fn find(d: &mut [usize], mut x: usize) -> usize {
        while d[x] != x {
            d[x] = d[d[x]];
            x = d[x];
        }
        x
    }
    for f in &fams {
        let all: Vec<usize> = f
            .partners
            .iter()
            .flatten()
            .chain(&f.children)
            .map(|&p| p as usize)
            .collect();
        for &p in &all[1..] {
            let (x, y) = (find(&mut dsu, all[0]), find(&mut dsu, p));
            dsu[x] = y;
        }
    }
    let mut size: HashMap<usize, usize> = HashMap::new();
    for p in 0..n {
        let r = find(&mut dsu, p);
        *size.entry(r).or_default() += 1;
    }
    let mut remap: Vec<Option<u32>> = vec![None; n];
    let (mut surname, mut birth) = (Vec::new(), Vec::new());
    for p in 0..n {
        let r = find(&mut dsu, p);
        if size[&r] >= 6 {
            remap[p] = Some(surname.len() as u32);
            surname.push(b.surname[p].clone());
            birth.push(b.birth[p]);
        }
    }
    let families = fams
        .iter()
        .filter(|f| f.partners[0].is_some_and(|p| remap[p as usize].is_some()))
        .map(|f| WholeFamily {
            partners: f.partners.map(|p| p.and_then(|p| remap[p as usize])),
            children: f
                .children
                .iter()
                .filter_map(|&c| remap[c as usize])
                .collect(),
        })
        .collect();
    WholeInput {
        count: surname.len(),
        families,
        birth,
        surname,
        include: None,
    }
}

/// Nine clusters R1..R9 (0..8) in the order the user added them, with these marriages: a son of R1 and a daughter
/// of R9; a daughter of that couple and a grandson of R2; another grandson of R2 and a granddaughter of R7; a
/// grandson of R7 and a granddaughter of R4. R3 and R5 marry each other and link to nothing else; R6 and R8 link
/// to nothing.
pub fn nine_clusters() -> WholeInput {
    clusters(
        9,
        &[
            ((0, "k0"), (8, "k2")),
            ((1, "g11"), (0, "g01")),
            ((1, "g20"), (6, "g11")),
            ((6, "g00"), (3, "g01")),
            ((2, "k0"), (4, "k2")),
        ],
    )
}

pub fn measure(i: &WholeInput) -> (WholeLayout, Quality) {
    let l = whole(i, &Metrics::default());
    let q = quality(&l.layout);
    (l, q)
}

/// Where each cluster's people stand (x of the left edge), for reading a layout.
pub fn describe(i: &WholeInput, l: &WholeLayout) -> String {
    let mut out = String::new();
    for c in 0..9 {
        let name = format!("Clan{c}");
        let xs: Vec<(i32, u32)> = l
            .layout
            .nodes
            .iter()
            .filter(|n| i.surname[n.key as usize] == name)
            .map(|n| (n.rect.x as i32, n.key))
            .collect();
        let min = xs.iter().map(|t| t.0).min().unwrap_or(0);
        let max = xs.iter().map(|t| t.0).max().unwrap_or(0);
        let mut rows: Vec<(u32, i32, u32)> = l
            .layout
            .nodes
            .iter()
            .filter(|n| i.surname[n.key as usize] == name)
            .map(|n| (n.r#gen as u32, n.rect.x as i32, n.key))
            .collect();
        rows.sort();
        out += &format!("R{} x {min}..{max} n={} {rows:?}\n", c + 1, xs.len());
    }
    out
}

#[test]
fn print_clusters() {
    let i = nine_clusters();
    let (l, _) = measure(&i);
    eprintln!("{}", describe(&i, &l));
}

/// The shape the owner reported: a root couple P+Q with a married son A and an unmarried son B. A married the
/// daughter of another family F, whose own tree reaches on to a third and a fourth family. P+Q must stand beside
/// F's root couple, above their sons, and the line from P+Q to A must be short.
fn in_law_shape() -> WholeInput {
    // 0 P, 1 Q, 2 A, 3 B | 4 F1, 5 F2, 6 Fd (A's wife), 7 Fs (her brother), 8 Fw (his wife)
    // | 9 G1, 10 G2 (Fw's parents), 11 Gb (Fw's brother), 12 Gw (his wife) | 13 Ac (A's daughter), 14 Ah (her husband)
    // | 15 H1, 16 H2 (Ah's parents), 17 Hb (Ah's brother, unmarried) | 18 Ak (child of Ac+Ah) | 19 Fk (child of Fs+Fw)
    let fam = |a: u32, b: u32, kids: &[u32]| WholeFamily {
        partners: [Some(a), Some(b)],
        children: kids.to_vec(),
    };
    let families = vec![
        fam(0, 1, &[2, 3]),
        fam(4, 5, &[6, 7]),
        fam(2, 6, &[13]),
        fam(7, 8, &[19]),
        fam(9, 10, &[8, 11]),
        fam(11, 12, &[]),
        fam(13, 14, &[18]),
        fam(15, 16, &[14, 17]),
    ];
    WholeInput {
        count: 20,
        families,
        birth: (0..20).map(|i| Some(1800 + 3 * i)).collect(),
        surname: vec![String::new(); 20],
        include: None,
    }
}

#[test]
fn a_root_couple_stands_beside_the_family_their_married_son_joined() {
    let i = in_law_shape();
    let (l, q) = measure(&i);
    let x = |k: u32| {
        let id = l.extras.node_of(k).unwrap();
        l.layout.nodes[id as usize].rect.x
    };
    let w = Metrics::default().node_w;
    // P+Q above their sons: within a card or two of the middle of A and B.
    let sons = f32::midpoint(x(2), x(3));
    assert!((x(0) - sons).abs() < 3.0 * w, "P+Q far from their sons");
    // The line from P+Q to A is short, and P+Q stand next to F's root couple, not across the chart.
    assert!(
        (x(0) - x(2)).abs() < 6.0 * w,
        "P to A: {}",
        (x(0) - x(2)).abs()
    );
    assert!(
        (x(0) - x(4)).abs() < 8.0 * w,
        "P to F: {}",
        (x(0) - x(4)).abs()
    );
    eprintln!(
        "shape: P-A {} P-F {} total_h {:.0} longest_h {:.0}",
        (x(0) - x(2)).abs(),
        (x(0) - x(4)).abs(),
        q.horizontal_connectors,
        q.longest_horizontal_connector
    );
}

#[test]
fn fixture_is_about_160_people() {
    let i = nine_clusters();
    assert!((120..=175).contains(&i.count), "{} people", i.count);
}

#[test]
fn print_numbers() {
    let i = nine_clusters();
    let t = Instant::now();
    let (l, q) = measure(&i);
    eprintln!(
        "people={} total_h={:.0} longest_h={:.0} crossings={} width={:.0} height={:.0} blocks={} ms={}",
        i.count,
        q.horizontal_connectors,
        q.longest_horizontal_connector,
        q.line_crossings,
        q.width,
        q.height,
        l.extras.blocks.len(),
        t.elapsed().as_millis()
    );
}
