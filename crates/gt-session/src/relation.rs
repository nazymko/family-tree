//! Plain-English names for how a person on the relatives chart is related to the centred one.
//!
//! All the chart's relationship wording lives here, so it can be translated in one place.

use crate::dto::{RoleDto, RoleKind, SexKind};
use gt_layout::{PartnerOf, Rel};

fn pick(sex: SexKind, male: &str, female: &str, neutral: &str) -> String {
    match sex {
        SexKind::Male => male,
        SexKind::Female => female,
        _ => neutral,
    }
    .to_string()
}

/// "great-grandfather" for 3, "2x great-grandfather" for 4, and so on; 1 and 2 use the plain words.
fn generations(steps: u8, male: &str, female: &str, neutral: &str, sex: SexKind) -> String {
    let word = pick(sex, male, female, neutral);
    match steps {
        0 | 1 => word,
        2 => format!("grand{word}"),
        3 => format!("great-grand{word}"),
        n => format!("{}x great-grand{word}", n - 2),
    }
}

/// `rel` as data for the UI, so it can word the relationship in its own language.
pub fn relation_role(rel: Rel, sex: SexKind) -> RoleDto {
    let mut r = RoleDto::new(RoleKind::Focus, 0, sex);
    match rel {
        Rel::Focus => {}
        Rel::Ancestor(n) => (r.kind, r.n) = (RoleKind::Ancestor, i32::from(n)),
        Rel::Descendant(n) => (r.kind, r.n) = (RoleKind::Descendant, i32::from(n)),
        Rel::Sibling { half } => (r.kind, r.half) = (RoleKind::Sibling, half),
        Rel::AuntUncle { half } => (r.kind, r.half) = (RoleKind::AuntUncle, half),
        Rel::NieceNephew => r.kind = RoleKind::NieceNephew,
        Rel::Cousin => r.kind = RoleKind::Cousin,
        Rel::Kin { up, down } => {
            (r.kind, r.n, r.k) = (RoleKind::Kin, i32::from(up), u32::from(down));
        }
        Rel::Partner(of) => match of {
            PartnerOf::Kin { up, down } => {
                (r.kind, r.n, r.k) = (RoleKind::PartnerOfKin, i32::from(up), u32::from(down));
            }
            PartnerOf::Focus => r.kind = RoleKind::Partner,
            PartnerOf::Sibling => r.kind = RoleKind::PartnerOfSibling,
            PartnerOf::AuntUncle => r.kind = RoleKind::PartnerOfAuntUncle,
            PartnerOf::Descendant(n) => {
                (r.kind, r.n) = (RoleKind::PartnerOfDescendant, i32::from(n))
            }
        },
    }
    r
}

/// English words for the blood relative `up` generations above the common ancestor and `down` below it
/// (the screens word it themselves from the role data; this is the plain text fallback).
fn kin_label(up: u8, down: u8, sex: SexKind) -> String {
    let (min, diff) = (up.min(down), i32::from(up) - i32::from(down));
    if min == 1 {
        // A line of brothers and sisters of ancestors, or children of brothers and sisters.
        return if up > down {
            let w = pick(sex, "uncle", "aunt", "aunt or uncle");
            match up {
                2 => w,
                3 => format!("great-{w}"),
                n => format!("{}x great-{w}", n - 2),
            }
        } else {
            let w = pick(sex, "nephew", "niece", "nephew or niece");
            match down {
                2 => w,
                3 => format!("grand{w}"),
                n => format!("{}x great-grand{w}", n - 3),
            }
        };
    }
    let degree = match min - 1 {
        1 => "first".to_string(),
        2 => "second".to_string(),
        3 => "third".to_string(),
        n => format!("{n}th"),
    };
    let w = pick(sex, "cousin", "cousin", "cousin");
    match diff {
        0 => format!("{degree} {w}"),
        d => format!("{degree} {w}, {} removed", d.abs()),
    }
}

/// The label for `rel`. `sex` is the sex of the person being labelled.
pub fn relation_label(rel: Rel, sex: SexKind) -> String {
    match rel {
        Rel::Focus => "starting person".to_string(),
        Rel::Ancestor(n) => generations(n, "father", "mother", "parent", sex),
        Rel::Descendant(n) => generations(n, "son", "daughter", "child", sex),
        Rel::Sibling { half } => {
            let w = pick(sex, "brother", "sister", "sibling");
            if half { format!("half {w}") } else { w }
        }
        Rel::AuntUncle { half } => {
            let w = pick(sex, "uncle", "aunt", "aunt or uncle");
            if half { format!("half {w}") } else { w }
        }
        Rel::NieceNephew => pick(sex, "nephew", "niece", "nephew or niece"),
        Rel::Cousin => "cousin".to_string(),
        Rel::Kin { up, down } => kin_label(up, down, sex),
        Rel::Partner(of) => match of {
            PartnerOf::Kin { .. } => "relative by marriage".to_string(),
            PartnerOf::Focus => "partner".to_string(),
            PartnerOf::Sibling => pick(sex, "brother-in-law", "sister-in-law", "in-law"),
            PartnerOf::AuntUncle => pick(sex, "uncle by marriage", "aunt by marriage", "in-law"),
            PartnerOf::Descendant(n) => match sex {
                SexKind::Male | SexKind::Female => {
                    format!("{}-in-law", generations(n, "son", "daughter", "child", sex))
                }
                _ => "in-law".to_string(),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use SexKind::{Female, Male, Unknown};

    #[test]
    fn ancestors() {
        let l = |n, s| relation_label(Rel::Ancestor(n), s);
        assert_eq!(l(1, Male), "father");
        assert_eq!(l(1, Female), "mother");
        assert_eq!(l(1, Unknown), "parent");
        assert_eq!(l(2, Female), "grandmother");
        assert_eq!(l(3, Male), "great-grandfather");
        assert_eq!(l(4, Female), "2x great-grandmother");
    }

    #[test]
    fn descendants() {
        let l = |n, s| relation_label(Rel::Descendant(n), s);
        assert_eq!(l(1, Male), "son");
        assert_eq!(l(1, Unknown), "child");
        assert_eq!(l(2, Female), "granddaughter");
        assert_eq!(l(3, Unknown), "great-grandchild");
        assert_eq!(l(5, Male), "3x great-grandson");
    }

    #[test]
    fn siblings_and_their_children() {
        let s = |half, x| relation_label(Rel::Sibling { half }, x);
        assert_eq!(s(false, Male), "brother");
        assert_eq!(s(true, Female), "half sister");
        assert_eq!(s(true, Unknown), "half sibling");
        assert_eq!(relation_label(Rel::NieceNephew, Female), "niece");
        assert_eq!(relation_label(Rel::NieceNephew, Male), "nephew");
        assert_eq!(relation_label(Rel::NieceNephew, Unknown), "nephew or niece");
        assert_eq!(relation_label(Rel::Cousin, Female), "cousin");
    }

    #[test]
    fn distant_relatives_by_generations_up_and_down() {
        let k = |up, down, x| relation_label(Rel::Kin { up, down }, x);
        assert_eq!(k(3, 1, Male), "great-uncle");
        assert_eq!(k(4, 1, Female), "2x great-aunt");
        assert_eq!(k(1, 3, Female), "grandniece");
        assert_eq!(k(2, 2, Female), "first cousin");
        assert_eq!(k(3, 3, Male), "second cousin");
        assert_eq!(k(3, 2, Male), "first cousin, 1 removed");
        let r = relation_role(Rel::Kin { up: 3, down: 2 }, Male);
        assert_eq!((r.kind, r.n, r.k), (RoleKind::Kin, 3, 2));
        let p = relation_role(Rel::Partner(PartnerOf::Kin { up: 2, down: 2 }), Female);
        assert_eq!((p.kind, p.n, p.k), (RoleKind::PartnerOfKin, 2, 2));
    }

    #[test]
    fn aunts_and_uncles() {
        let a = |half, x| relation_label(Rel::AuntUncle { half }, x);
        assert_eq!(a(false, Female), "aunt");
        assert_eq!(a(false, Male), "uncle");
        assert_eq!(a(true, Male), "half uncle");
        assert_eq!(a(false, Unknown), "aunt or uncle");
    }

    #[test]
    fn partners_and_the_focus() {
        assert_eq!(relation_label(Rel::Focus, Male), "starting person");
        let p = |of| relation_label(Rel::Partner(of), Female);
        assert_eq!(p(PartnerOf::Focus), "partner");
        assert_eq!(p(PartnerOf::Sibling), "sister-in-law");
        assert_eq!(p(PartnerOf::AuntUncle), "aunt by marriage");
        assert_eq!(p(PartnerOf::Descendant(1)), "daughter-in-law");
        assert_eq!(p(PartnerOf::Descendant(2)), "granddaughter-in-law");
        assert_eq!(p(PartnerOf::Descendant(3)), "great-granddaughter-in-law");
        let m = |of| relation_label(Rel::Partner(of), Male);
        assert_eq!(m(PartnerOf::Sibling), "brother-in-law");
        assert_eq!(m(PartnerOf::AuntUncle), "uncle by marriage");
        assert_eq!(m(PartnerOf::Descendant(1)), "son-in-law");
        assert_eq!(m(PartnerOf::Descendant(2)), "grandson-in-law");
        let u = |of| relation_label(Rel::Partner(of), Unknown);
        assert_eq!(u(PartnerOf::Sibling), "in-law");
        assert_eq!(u(PartnerOf::AuntUncle), "in-law");
        assert_eq!(u(PartnerOf::Descendant(1)), "in-law");
    }
}
