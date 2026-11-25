#![allow(clippy::unwrap_used)]
//! Family-shaped synthetic fixtures for the "sheets by branch" print (plan 2026-11-22-sheets-packing, Task 1): all-kin
//! folded print layouts of four synthetic trees in the four directions, written gzip-compressed into the app's test
//! fixtures (`app/src/lib/chart/testing/fam`). Only keys and geometry: the app's loader gives the names.

mod common;

use std::fmt::Write as _;
use std::io::Write as _;

use common::MapSource;
use gt_layout::{AroundBase, Direction, EdgeKind, Key, Layout, Metrics, around_open};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
    fn chance(&mut self, pct: u64) -> bool {
        self.below(100) < pct
    }
}

struct Gen {
    s: MapSource,
    next: Key,
    r: Rng,
    people: usize,
    marry: u64,
}

impl Gen {
    fn new_key(&mut self) -> Key {
        self.next += 1;
        self.people += 1;
        self.next
    }
    /// Descendants of `p` (already placed) down `depth` more generations; `big` makes a fertile branch.
    fn family_below(&mut self, p: Key, depth: u32, big: bool) {
        if depth == 0 {
            return;
        }
        let marry = if big { 85 } else { self.marry };
        if !self.r.chance(marry) {
            return;
        }
        let sp = self.new_key();
        let n = if big {
            2 + self.r.below(5)
        } else {
            self.r.below(5)
        };
        let kids: Vec<Key> = (0..n).map(|_| self.new_key()).collect();
        self.s.family(Some(p), Some(sp), &kids);
        for &k in &kids {
            self.family_below(k, depth - 1, big);
        }
    }
}

/// Root 1; Ahnentafel ancestors down to `gens` generations, some lines ending early; each ancestor with a sibling group
/// of uneven size (a few large leaf groups that fold, most small), siblings with families of uneven depth.
fn tree(
    seed: u64,
    gens: u32,
    sib_max: u64,
    big_branches: &[Key],
    marry: u64,
) -> (MapSource, usize) {
    let mut g = Gen {
        s: MapSource::default(),
        next: 100_000,
        r: Rng(seed),
        people: 1,
        marry,
    };
    let last: Key = (1 << gens) - 1;
    // Lines that stop early: a few ancestors without parents.
    let mut has_parents = vec![false; (last + 1) as usize];
    for n in 1..=last {
        if 2 * n + 1 > last {
            continue;
        }
        let parent_known = n == 1 || has_parents[(n / 2) as usize];
        if !parent_known {
            continue;
        }
        // Stop some lines early (not at the first two generations).
        if n >= 4 && g.r.chance(18) {
            continue;
        }
        has_parents[n as usize] = true;
    }
    for n in 1..=last {
        if !has_parents[n as usize] {
            continue;
        }
        g.people += 2;
        let gen_level = 31 - n.leading_zeros(); // 0 for root
        let sibs = if n == 1 {
            3
        } else if g.r.chance(15) {
            10 + g.r.below(sib_max)
        } else {
            g.r.below(6)
        };
        let mut kids: Vec<Key> = (0..sibs).map(|_| g.new_key()).collect();
        let at = g.r.below(kids.len() as u64 + 1) as usize;
        kids.insert(at, n);
        g.s.family(Some(2 * n), Some(2 * n + 1), &kids);
        let big = big_branches.contains(&n);
        for &k in &kids {
            if k == n {
                continue;
            }
            let depth = gen_level + 1;
            g.family_below(k, depth.min(3), big);
        }
    }
    // Root's own family: spouse, 3 children, grandchildren.
    let sp = g.new_key();
    let kids: Vec<Key> = (0..3).map(|_| g.new_key()).collect();
    g.s.family(Some(1), Some(sp), &kids);
    for &k in &kids {
        g.family_below(k, 1, true);
    }
    let n = g.people;
    (g.s, n)
}

/// A coordinate rounded to 0.1 (keeps the fixtures small).
fn r1(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

fn json(l: &Layout) -> String {
    let mut out = format!("{{\"root\":{},\"nodes\":[", l.root);
    for (i, n) in l.nodes.iter().enumerate() {
        let sep = if i == 0 { "" } else { "," };
        let dup = n.dup_of.map_or("null".to_string(), |d| d.to_string());
        let _ = write!(
            out,
            "{sep}{{\"id\":{},\"key\":{},\"x\":{},\"y\":{},\"w\":{},\"h\":{},\"gen\":{},\"dupOf\":{dup}}}",
            n.id,
            n.key,
            r1(n.rect.x),
            r1(n.rect.y),
            r1(n.rect.w),
            r1(n.rect.h),
            n.r#gen
        );
    }
    out.push_str("],\"edges\":[");
    for (i, e) in l.edges.iter().enumerate() {
        let sep = if i == 0 { "" } else { "," };
        let kind = match e.kind {
            EdgeKind::Parent => "parent",
            EdgeKind::Partner => "partner",
            EdgeKind::Sibling => "sibling",
            EdgeKind::Dup => "dup",
        };
        let pts: Vec<String> = e.pts.iter().map(|&v| r1(v).to_string()).collect();
        let _ = write!(
            out,
            "{sep}{{\"from\":{},\"to\":{},\"kind\":\"{kind}\",\"pts\":[{}]}}",
            e.from,
            e.to,
            pts.join(",")
        );
    }
    out.push_str("]}");
    out
}

#[test]
#[ignore = "writes app fixtures: FAM_FIXTURES_DIR=<dir>"]
fn write_fam_fixtures() {
    let Ok(dir) = std::env::var("FAM_FIXTURES_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    // (name, seed, generations, siblings, big-family generations, marry percent)
    type Case = (&'static str, u64, u32, u64, &'static [Key], u64);
    let cases: [Case; 4] = [
        ("fam-a", 11, 6, 8, &[5], 55),
        ("fam-b", 23, 6, 10, &[4], 30),
        ("fam-c", 37, 5, 12, &[2, 3], 40),
        ("fam-d", 51, 7, 12, &[6], 20),
    ];
    for (name, seed, gens, sib, big, marry) in cases {
        let (s, n) = tree(seed, gens, sib, big, marry);
        for (dn, d) in [
            ("top", Direction::Top),
            ("bottom", Direction::Bottom),
            ("left", Direction::Left),
            ("right", Direction::Right),
        ] {
            let m = Metrics::compact().for_direction(Direction::Top, d);
            let mut l = around_open(&s, 1, 255, AroundBase::All, &[], true, &m, 100_000);
            l.orient(Direction::Top, d);
            println!("{name} {dn}: tree people ~{n}, nodes {}", l.nodes.len());
            let file = std::fs::File::create(format!("{dir}/{name}-{dn}.json.gz")).unwrap();
            let mut gz = flate2::write::GzEncoder::new(file, flate2::Compression::best());
            gz.write_all(json(&l).as_bytes()).unwrap();
            gz.finish().unwrap();
        }
    }
}
