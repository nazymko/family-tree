#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! A blood family stays together in the whole-tree chart: a root couple stands above ALL its children, the one who
//! married into another root family included, and the families joined by that marriage stand next to each other.
//! Synthetic data only: invented people.

use gt_layout::{Metrics, WholeFamily, WholeInput, whole};

const IK: u32 = 1;
const IK_WIFE: u32 = 2;
const SINGLE: u32 = 3;
const SON: u32 = 4;
const SON_WIFE: u32 = 5;
const DAUGHTER: u32 = 11;
const HUSBAND: u32 = 13;

/// 0 IK's father | 1 IK + 2 wife | 3 their single child, 4 their son | 5 the son's wife, daughter of root family G
/// (6 + 7, whose other children are 8 and 19, with 9 married to 8 and a child 10) | 11 the son's daughter, 12 his other
/// child | 13 the daughter's husband, son of root family Z (14 + 15, other children 16, 17, 20) | 18 their child.
fn shape() -> WholeInput {
    let fam = |a: u32, b: u32, kids: &[u32]| WholeFamily {
        partners: [Some(a), Some(b)],
        children: kids.to_vec(),
    };
    let families = vec![
        WholeFamily {
            partners: [Some(0), None],
            children: vec![IK],
        },
        fam(IK, IK_WIFE, &[SINGLE, SON]),
        fam(6, 7, &[SON_WIFE, 8, 19]),
        fam(8, 9, &[10]),
        fam(SON, SON_WIFE, &[12, DAUGHTER]),
        fam(14, 15, &[HUSBAND, 16, 17, 20]),
        fam(HUSBAND, DAUGHTER, &[18]),
        fam(16, 21, &[22, 23]),
        // Other root families tied to G and to Z by marriages: 24 + 25 (children 26, 27), 28 + 29 (children 30, 31).
        fam(24, 25, &[26, 27]),
        fam(19, 26, &[32]),
        fam(28, 29, &[30, 31]),
        fam(17, 30, &[33]),
        fam(22, 31, &[34]),
    ];
    let n = 35;
    let mut birth: Vec<Option<i32>> = (0..n).map(|i| Some(1800 + 3 * i)).collect();
    // The single child is the older one: the married son stands at the end of the row in a plain walk.
    birth[SINGLE as usize] = Some(1900);
    birth[SON as usize] = Some(1905);
    WholeInput {
        count: n as usize,
        families,
        birth,
        surname: vec![String::new(); n as usize],
        include: None,
    }
}

#[test]
fn a_married_son_stays_under_his_parents_and_the_families_he_joins_stand_beside() {
    let l = whole(&shape(), &Metrics::default());
    let w = Metrics::default().node_w;
    let x = |k: u32| {
        let id = l.extras.node_of(k).unwrap();
        l.layout.nodes[id as usize].rect.x
    };
    // The son's box lies within the width of his parents' couple, give or take a card.
    let (lo, hi) = (x(IK).min(x(IK_WIFE)), x(IK).max(x(IK_WIFE)));
    assert!(
        x(SON) >= lo - 1.5 * w && x(SON) <= hi + 1.5 * w,
        "son {} not under parents {lo}..{hi}",
        x(SON)
    );
    // The line from the parents to the son is short, and so is the one from him to his daughter.
    assert!(
        (x(IK) - x(SON)).abs() < 2.5 * w,
        "parents to son: {}",
        (x(IK) - x(SON)).abs()
    );
    assert!(
        (x(SON) - x(DAUGHTER)).abs() < 4.0 * w,
        "son to daughter: {}",
        (x(SON) - x(DAUGHTER)).abs()
    );
    // The single child is under the parents too.
    assert!((x(IK) - x(SINGLE)).abs() < 2.5 * w);
    // His wife's family and the daughter's husband's family stand near, not across the chart.
    assert!(
        (x(SON) - x(6)).abs() < 6.0 * w,
        "son to wife's parents: {}",
        (x(SON) - x(6)).abs()
    );
    assert!(
        (x(DAUGHTER) - x(14)).abs() < 6.0 * w,
        "daughter to husband's parents: {}",
        (x(DAUGHTER) - x(14)).abs()
    );
}
