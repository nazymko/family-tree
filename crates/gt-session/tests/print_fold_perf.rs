#![allow(clippy::unwrap_used)]
//! How long a "best fit" print of the compact layout takes on a big "all kin" chart: one folded print chart per
//! direction. `cargo test --release -p gt-session --test print_fold_perf -- --ignored --nocapture`.

mod common;

use std::fmt::Write as _;
use std::time::Instant;

use common::*;
use gt_session::*;

/// Person 1 (Osoba Testenko) with `gens` generations of ancestors (Ahnentafel numbers as record ids); every ancestor
/// below the top row and person 1 have 12 to 20 brothers and sisters, and every fifth of them is married with eight
/// children. Invented names only.
fn all_kin_gedcom(gens: u32) -> String {
    let mut ged = String::new();
    let mut fams = String::new();
    let mut next = 1_000_000u32;
    let mut fam = 0u32;
    let person = |ged: &mut String, id: u32, sex: char, links: &str| {
        let _ = write!(
            ged,
            "0 @I{id}@ INDI\n1 NAME Osoba{id} /Testenko/\n1 SEX {sex}\n{links}"
        );
    };
    let top = 1u32 << (gens + 1);
    let mut links: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
    for n in 1..(1u32 << gens) {
        fam += 1;
        let f = fam;
        let _ = write!(
            fams,
            "0 @F{f}@ FAM\n1 HUSB @I{}@\n1 WIFE @I{}@\n",
            2 * n,
            2 * n + 1
        );
        links
            .entry(2 * n)
            .or_default()
            .push_str(&format!("1 FAMS @F{f}@\n"));
        links
            .entry(2 * n + 1)
            .or_default()
            .push_str(&format!("1 FAMS @F{f}@\n"));
        let count = 12 + (n * 7) % 9;
        let mut kids: Vec<u32> = Vec::new();
        for i in 0..count {
            let k = next;
            next += 1;
            kids.push(k);
            if i % 5 == 2 {
                fam += 1;
                let g = fam;
                let partner = next;
                next += 1;
                let _ = write!(fams, "0 @F{g}@ FAM\n1 HUSB @I{k}@\n1 WIFE @I{partner}@\n");
                links
                    .entry(k)
                    .or_default()
                    .push_str(&format!("1 FAMS @F{g}@\n"));
                person(&mut ged, partner, 'F', &format!("1 FAMS @F{g}@\n"));
                for _ in 0..8 {
                    let c = next;
                    next += 1;
                    let _ = writeln!(fams, "1 CHIL @I{c}@");
                    person(&mut ged, c, 'M', &format!("1 FAMC @F{g}@\n"));
                }
            }
        }
        kids.insert(kids.len() / 2, n);
        for &k in &kids {
            let _ = writeln!(fams, "1 CHIL @I{k}@");
            links
                .entry(k)
                .or_default()
                .push_str(&format!("1 FAMC @F{f}@\n"));
        }
    }
    let mut ids: Vec<_> = links.into_iter().collect();
    ids.sort();
    for (id, l) in ids {
        let sex = if id < top && id % 2 == 1 && id > 1 {
            'F'
        } else {
            'M'
        };
        person(&mut ged, id, sex, &l);
    }
    // The children of the married brothers and sisters were written with their family; the families come last.
    ged.push_str(&fams);
    ged
}

fn request(root: uuid::Uuid, direction: ChartDirection) -> ChartRequest {
    ChartRequest {
        open: None,
        kind: ChartKind::Around,
        root,
        generations: 255,
        aunts_uncles: None,
        kin: Some(AroundKin::All),
        roots: None,
        direction: Some(direction),
        print: Some(true),
        fold: Some(true),
    }
}

/// "Best fit" opens one compact print chart per direction: about 3,000 people take a few seconds at most (release).
#[test]
#[ignore = "timing test: run with --release -- --ignored"]
fn best_fit_print_of_3k_all_kin() {
    let tree = tree_of(&all_kin_gedcom(6));
    let (_dir, mut s) = open_tree(&tree);
    let root = find(&tree, "Osoba1");
    let t = Instant::now();
    let mut cards = 0;
    for d in [
        ChartDirection::Top,
        ChartDirection::Bottom,
        ChartDirection::Left,
        ChartDirection::Right,
    ] {
        let one = Instant::now();
        let meta = s.chart_open(&request(root, d)).unwrap();
        eprintln!("{d:?}: {} cards in {:?}", meta.node_count, one.elapsed());
        cards = meta.node_count;
        s.chart_close(meta.chart_id);
    }
    let el = t.elapsed();
    eprintln!("best fit, 4 directions: {el:?}");
    assert!(cards >= 2_500, "{cards} cards");
    assert!(el.as_secs_f64() < 4.0, "took {el:?}");
}
