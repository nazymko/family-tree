//! Plain-English names for how a person on the relatives chart is related to the centred one.
//!
//! All the chart's relationship wording lives here, so it can be translated in one place.

use crate::dto::SexKind;
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
        Rel::Partner(of) => match of {
            PartnerOf::Focus => "partner".to_string(),
            PartnerOf::Sibling => "sibling's partner".to_string(),
            PartnerOf::AuntUncle => "aunt or uncle's partner".to_string(),
            PartnerOf::Descendant(n) => {
                format!(
                    "{}'s partner",
                    generations(n, "child", "child", "child", SexKind::Unknown)
                )
            }
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
        assert_eq!(p(PartnerOf::Sibling), "sibling's partner");
        assert_eq!(p(PartnerOf::AuntUncle), "aunt or uncle's partner");
        assert_eq!(p(PartnerOf::Descendant(1)), "child's partner");
        assert_eq!(p(PartnerOf::Descendant(2)), "grandchild's partner");
    }
}
