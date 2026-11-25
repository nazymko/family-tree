//! Placement by blood families: every couple hangs below the parents of one of its members (the husband's, the
//! first partner's), so a root couple stands above all its children, whoever they married. The trees this makes
//! (one per top ancestor, "blocks") are then put in a row in the order that makes the lines between them, the
//! marriages that tie one family to another, the shortest; a child who is the bridge to another family goes to the
//! side of its row that faces that family, and a couple turns its member who comes from the other family to the
//! outside. The caller keeps whichever placement, this or the walk, has the shorter lines.

use std::collections::HashMap;

use super::{Scratch, Shape, Shared, Unit, spread};

const ROUNDS: usize = 4;
/// Bridge children are tried at each end of their row in components up to this many units, this many at most.
const TRY_UNITS: usize = 400;
const MAX_SIDE_TRIES: usize = 40;
/// Up to this many blocks every position is tried for each; beyond, only the places beside a linked block.
const MANY_BLOCKS: usize = 150;
const PASSES: usize = 8;

type Home = Option<(usize, usize)>;
/// A family and its children hanging below one unit: (unit, person) of each.
type Downs = (usize, Vec<(usize, usize)>);

/// For every unit: the family it hangs from (a child of it is a member of the unit) and the unit of that family's
/// first partner. Loops (a person who is partner of their own ancestor) are cut by skipping the link.
fn home_forest(units: &[Unit], sh: &Shared, sc: &Scratch) -> Vec<Home> {
    let mut parent: Vec<Home> = vec![None; units.len()];
    for u in 0..units.len() {
        let mut members = units[u].members.clone();
        // Husbands (first partners) first.
        members.sort_by_key(|&m| {
            !sh.partner_in[m]
                .iter()
                .any(|&f| sh.fams[f].partners.first() == Some(&m))
        });
        'found: for m in members {
            for &f in &sh.child_in[m] {
                let Some(&p0) = sh.fams[f].partners.first() else {
                    continue;
                };
                let pu = sc.unit_of[p0];
                if pu == u {
                    continue;
                }
                let mut x = pu;
                let mut cycle = false;
                while let Some((_, q)) = parent[x] {
                    if q == u {
                        cycle = true;
                        break;
                    }
                    x = q;
                }
                if !cycle {
                    parent[u] = Some((f, pu));
                    break 'found;
                }
            }
        }
    }
    parent
}

struct BlockLink {
    a: usize,
    rel_a: f64,
    b: usize,
    rel_b: f64,
}

fn total_cost(
    order: &[usize],
    bw: &[f64],
    gap: f64,
    links: &[BlockLink],
    start: &mut [f64],
) -> f64 {
    start.fill(f64::NAN);
    let mut x = 0.0;
    for &b in order {
        start[b] = x;
        x += bw[b] + gap;
    }
    links
        .iter()
        .filter(|l| !start[l.a].is_nan() && !start[l.b].is_nan())
        .map(|l| ((start[l.a] + l.rel_a) - (start[l.b] + l.rel_b)).abs())
        .sum()
}

/// Order of blocks along the row: built by putting each block where it costs least beside what is already placed,
/// then improved by moving single blocks.
fn order_blocks(bw: &[f64], gap: f64, links: &[BlockLink], init: Option<Vec<usize>>) -> Vec<usize> {
    let nb = bw.len();
    let mut start = vec![f64::NAN; nb];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nb];
    for l in links {
        adj[l.a].push(l.b);
        adj[l.b].push(l.a);
    }
    let candidates = |order: &[usize], b: usize| -> Vec<usize> {
        if nb <= MANY_BLOCKS {
            return (0..=order.len()).collect();
        }
        let mut c = vec![0, order.len()];
        for &n in &adj[b] {
            if let Some(i) = order.iter().position(|&o| o == n) {
                c.push(i);
                c.push(i + 1);
            }
        }
        c.sort_unstable();
        c.dedup();
        c
    };
    let mut order: Vec<usize> = match init {
        Some(o) => o,
        None => {
            let mut order: Vec<usize> = Vec::with_capacity(nb);
            let mut placed = vec![false; nb];
            let mut conn = vec![0usize; nb];
            for step in 0..nb {
                let pick = (0..nb)
                    .filter(|&b| !placed[b])
                    .max_by(|&p, &q| {
                        let key = |b: usize| if step == 0 { adj[b].len() } else { conn[b] };
                        key(p)
                            .cmp(&key(q))
                            .then(bw[p].total_cmp(&bw[q]))
                            .then(q.cmp(&p))
                    })
                    .unwrap_or(0);
                let mut best: Option<(f64, usize)> = None;
                for at in candidates(&order, pick) {
                    let mut trial = order.clone();
                    trial.insert(at, pick);
                    let c = total_cost(&trial, bw, gap, links, &mut start);
                    if best.is_none_or(|(bc, _)| c < bc - 1e-9) {
                        best = Some((c, at));
                    }
                }
                order.insert(best.map_or(0, |b| b.1), pick);
                placed[pick] = true;
                for &n in &adj[pick] {
                    conn[n] += 1;
                }
            }
            order
        }
    };
    let mut cur = total_cost(&order, bw, gap, links, &mut start);
    for _ in 0..PASSES {
        let mut improved = false;
        for b in 0..nb {
            let Some(i) = order.iter().position(|&o| o == b) else {
                continue;
            };
            let mut rest = order.clone();
            rest.remove(i);
            let mut best = (cur, i);
            for at in candidates(&rest, b) {
                let mut trial = rest.clone();
                trial.insert(at, b);
                let c = total_cost(&trial, bw, gap, links, &mut start);
                if c < best.0 - 1e-6 {
                    best = (c, at);
                }
            }
            if best.1 != i {
                rest.insert(best.1, b);
                order = rest;
                cur = best.0;
                improved = true;
            }
        }
        if !improved {
            break;
        }
    }
    order
}

/// Places every unit of a component by families; returns the left edge of each unit and the sum of the distances
/// from the children to their parents. The order of the members of units is left as this placement wants it.
#[allow(clippy::needless_range_loop)]
pub(super) fn place_families(
    units: &mut [Unit],
    rows_n: usize,
    sh: &Shared,
    sc: &mut Scratch,
) -> (Vec<f64>, f64) {
    let m = sh.m;
    let node_w = f64::from(m.node_w);
    let step = node_w + f64::from(m.couple_gap);
    let gap_sibling = f64::from(m.gap_x) * super::GAP_SIBLINGS;
    let gap_group = f64::from(m.gap_x) * super::GAP_GROUPS;
    let nu = units.len();

    let parent = home_forest(units, sh, sc);
    // The families of this component only (the tables are shared by all of them).
    let mut comp_fams: Vec<usize> = units
        .iter()
        .flat_map(|u| u.members.iter())
        .flat_map(|&p| sh.partner_in[p].iter().chain(&sh.child_in[p]).copied())
        .collect();
    comp_fams.sort_unstable();
    comp_fams.dedup();
    // Children by unit and family, with the person that is the child.
    let mut downs: Vec<Vec<Downs>> = vec![Vec::new(); nu];
    for &f in &comp_fams {
        let fam = &sh.fams[f];
        let Some(&p0) = fam.partners.first() else {
            continue;
        };
        let pu = sc.unit_of[p0];
        let mut kids: Vec<(usize, usize)> = Vec::new();
        for &c in &fam.kids {
            let cu = sc.unit_of[c];
            if parent[cu] == Some((f, pu)) && !kids.iter().any(|k| k.0 == cu) {
                kids.push((cu, c));
            }
        }
        if !kids.is_empty() {
            downs[pu].push((f, kids));
        }
    }
    // Walk order (parents before children) and the block of each unit.
    let mut order: Vec<usize> = Vec::with_capacity(nu);
    let mut block_of = vec![usize::MAX; nu];
    let mut roots: Vec<usize> = Vec::new();
    for r in 0..nu {
        if parent[r].is_some() {
            continue;
        }
        let b = roots.len();
        roots.push(r);
        block_of[r] = b;
        let mut stack = vec![r];
        while let Some(u) = stack.pop() {
            order.push(u);
            for (_, ks) in &downs[u] {
                for &(k, _) in ks.iter().rev() {
                    block_of[k] = b;
                    stack.push(k);
                }
            }
        }
    }
    let nb = roots.len();
    // Lines between blocks: (parent person, child person).
    let mut links: Vec<(usize, usize)> = Vec::new();
    for &f in &comp_fams {
        let fam = &sh.fams[f];
        if let Some(&p0) = fam.partners.first() {
            for &c in &fam.kids {
                if block_of[sc.unit_of[p0]] != block_of[sc.unit_of[c]] {
                    links.push((p0, c));
                }
            }
        }
        if fam.partners.is_empty() {
            for w in fam.kids.windows(2) {
                if block_of[sc.unit_of[w[0]]] != block_of[sc.unit_of[w[1]]] {
                    links.push((w[0], w[1]));
                }
            }
        }
    }

    let snapshot =
        |units: &[Unit]| -> Vec<Vec<usize>> { units.iter().map(|u| u.members.clone()).collect() };
    let round = |units: &mut [Unit],
                 sc: &mut Scratch,
                 prev: Option<&Vec<f64>>,
                 prev_order: Option<Vec<usize>>,
                 forced: &HashMap<usize, i8>|
     -> Round {
        let px = |sc: &Scratch, xs: &[f64], p: usize| {
            xs[sc.unit_of[p]] + sc.pos_in_unit[p] as f64 * step + node_w / 2.0
        };
        // Couples turn the member whose own parents stand elsewhere towards them.
        if let Some(xs) = &prev {
            for u in 0..nu {
                if units[u].members.len() != 2 {
                    continue;
                }
                let key = |sc: &Scratch, p: usize| {
                    sh.child_in[p]
                        .iter()
                        .find_map(|&f| sh.fams[f].partners.first().map(|&p0| px(sc, xs, p0)))
                        .unwrap_or_else(|| px(sc, xs, p))
                };
                let (a, b) = (units[u].members[0], units[u].members[1]);
                if key(sc, b) < key(sc, a) {
                    units[u].members.swap(0, 1);
                    sc.pos_in_unit[units[u].members[0]] = 0;
                    sc.pos_in_unit[units[u].members[1]] = 1;
                }
            }
        }
        // Where each unit's descendants are linked to: the mean x of the other ends of the lines out of its subtree.
        let mut tgt: Vec<Option<f64>> = vec![None; nu];
        if let Some(xs) = &prev {
            let mut sum = vec![0.0f64; nu];
            let mut cnt = vec![0.0f64; nu];
            for &(a, b) in &links {
                let (ua, ub) = (sc.unit_of[a], sc.unit_of[b]);
                sum[ua] += px(sc, xs, b);
                cnt[ua] += 1.0;
                sum[ub] += px(sc, xs, a);
                cnt[ub] += 1.0;
            }
            for &u in order.iter().rev() {
                if let Some((_, pu)) = parent[u] {
                    let (s, c) = (sum[u], cnt[u]);
                    sum[pu] += s;
                    cnt[pu] += c;
                }
            }
            for u in 0..nu {
                if cnt[u] > 0.0 {
                    tgt[u] = Some(sum[u] / cnt[u]);
                }
            }
        }

        let mut shapes: Vec<Option<Shape>> = (0..nu).map(|_| None).collect();
        let pos = &sc.pos_in_unit;
        let unit_of = &sc.unit_of;
        let centre = |p: usize| pos[p] as f64 * step + node_w / 2.0;
        for &u in order.iter().rev() {
            let mut c = Shape::empty(rows_n);
            c.spans[units[u].row] = (0.0, units[u].w);
            c.items.push((u, 0.0));
            let anchor = |f: usize| {
                let inside: Vec<f64> = sh.fams[f]
                    .partners
                    .iter()
                    .filter(|&&q| unit_of[q] == u)
                    .map(|&q| centre(q))
                    .collect();
                inside.iter().sum::<f64>() / inside.len().max(1) as f64
            };
            let mut fams: Vec<(f64, usize)> = downs[u]
                .iter()
                .enumerate()
                .map(|(i, (f, _))| (anchor(*f), i))
                .collect();
            fams.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (at, i) in fams {
                let kids = &downs[u][i].1;
                // Bridges to other families go to the side of the row facing them.
                let ordered: Vec<(usize, usize)> = match &prev {
                    None => kids.clone(),
                    Some(xs) => {
                        let here = px(sc, xs, sh.fams[downs[u][i].0].partners[0]);
                        let (mut left, mut mid, mut right) = (Vec::new(), Vec::new(), Vec::new());
                        for &k in kids {
                            let side = forced.get(&k.0).copied();
                            match (tgt[k.0], side) {
                                (_, Some(0)) => mid.push(k),
                                (t, Some(-1)) => left.push((t.unwrap_or(here), k)),
                                (t, Some(_)) => right.push((t.unwrap_or(here), k)),
                                (Some(t), None) if t < here => left.push((t, k)),
                                (Some(t), None) => right.push((t, k)),
                                (None, None) => mid.push(k),
                            }
                        }
                        left.sort_by(|a, b| a.0.total_cmp(&b.0));
                        right.sort_by(|a, b| a.0.total_cmp(&b.0));
                        left.into_iter()
                            .map(|t| t.1)
                            .chain(mid)
                            .chain(right.into_iter().map(|t| t.1))
                            .collect()
                    }
                };
                let mut mid = Shape::empty(rows_n);
                let mut ends: Vec<f64> = Vec::new();
                for (ku, kp) in ordered {
                    if let Some(t) = shapes[ku].take() {
                        let row = units[ku].row;
                        let dx = mid.clear_shift(&t, None, true, |r| {
                            if r == row { gap_sibling } else { gap_group }
                        });
                        mid.add(t, dx);
                        ends.push(dx + centre(kp));
                    }
                }
                if ends.is_empty() {
                    continue;
                }
                let middle = f64::midpoint(ends[0], ends[ends.len() - 1]);
                let dx = c.clear_shift(&mid, Some(at - middle), true, |_| gap_group);
                c.add(mid, dx);
            }
            shapes[u] = Some(c);
        }

        // Blocks: their widths and where the linked people stand in them.
        let mut bshape: Vec<Shape> = Vec::with_capacity(nb);
        let mut bx0 = vec![0.0f64; nb];
        let mut bw = vec![0.0f64; nb];
        let mut rel = vec![0.0f64; nu];
        for (b, &r) in roots.iter().enumerate() {
            let s = shapes[r].take().unwrap_or_else(|| Shape::empty(rows_n));
            let used = s.spans.iter().filter(|&&t| Shape::is_used(t));
            let (lo, hi) = used.fold((f64::INFINITY, f64::NEG_INFINITY), |a, t| {
                (a.0.min(t.0), a.1.max(t.1))
            });
            bx0[b] = lo;
            bw[b] = hi - lo;
            for &(u, x) in &s.items {
                rel[u] = x - lo;
            }
            bshape.push(s);
        }
        let blinks: Vec<BlockLink> = links
            .iter()
            .map(|&(a, b)| BlockLink {
                a: block_of[sc.unit_of[a]],
                rel_a: rel[sc.unit_of[a]] + sc.pos_in_unit[a] as f64 * step + node_w / 2.0,
                b: block_of[sc.unit_of[b]],
                rel_b: rel[sc.unit_of[b]] + sc.pos_in_unit[b] as f64 * step + node_w / 2.0,
            })
            .collect();
        let border = order_blocks(&bw, gap_group, &blinks, prev_order);
        let mut total = Shape::empty(rows_n);
        let mut last_dx = f64::NEG_INFINITY;
        let mut shapes_by_block: Vec<Option<Shape>> = bshape.into_iter().map(Some).collect();
        for &b in &border {
            let Some(s) = shapes_by_block[b].take() else {
                continue;
            };
            let dx = total
                .clear_shift(&s, None, true, |_| gap_group)
                .max(last_dx);
            last_dx = dx;
            total.add(s, dx);
        }
        let mut xs = vec![0.0f64; nu];
        for &(u, x) in &total.items {
            xs[u] = x;
        }
        let score = spread(&xs, units, sh, sc);
        Round {
            xs,
            order: border,
            score,
            members: snapshot(units),
            tgt,
        }
    };
    let restore = |units: &mut [Unit], sc: &mut Scratch, mem: &[Vec<usize>]| {
        for (u, m) in units.iter_mut().zip(mem) {
            for (k, &p) in m.iter().enumerate() {
                sc.pos_in_unit[p] = k;
            }
            u.members.clone_from(m);
        }
    };
    let mut forced: HashMap<usize, i8> = HashMap::new();
    let mut best = round(units, sc, None, None, &forced);
    for _ in 1..ROUNDS {
        restore(units, sc, &best.members);
        let next = round(units, sc, Some(&best.xs), Some(best.order.clone()), &forced);
        if next.score < best.score - 1e-6 {
            best = next;
        } else {
            break;
        }
    }
    // Every child that is a bridge to another family: try it at each end of its row and in the middle.
    if nu <= TRY_UNITS {
        let mut cands: Vec<usize> = (0..nu)
            .filter(|&u| best.tgt[u].is_some() && parent[u].is_some())
            .collect();
        cands.truncate(MAX_SIDE_TRIES);
        for u in cands {
            let mut improved = true;
            for side in [-1i8, 1, 0] {
                if !improved {
                    break;
                }
                let old = forced.get(&u).copied();
                if old == Some(side) {
                    continue;
                }
                forced.insert(u, side);
                restore(units, sc, &best.members);
                let next = round(units, sc, Some(&best.xs), Some(best.order.clone()), &forced);
                if next.score < best.score - 1e-6 {
                    best = next;
                    improved = false;
                } else {
                    match old {
                        Some(o) => forced.insert(u, o),
                        None => forced.remove(&u),
                    };
                }
            }
        }
    }
    restore(units, sc, &best.members);
    (best.xs, best.score)
}

struct Round {
    xs: Vec<f64>,
    order: Vec<usize>,
    score: f64,
    members: Vec<Vec<usize>>,
    tgt: Vec<Option<f64>>,
}
