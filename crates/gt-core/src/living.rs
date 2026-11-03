//! Fail-safe living-person rule (spec §3.3): a person is dead only when
//! something proves it; missing or open-ended dates never do.
use crate::date::calendar::gregorian_minus_years;
use crate::model::*;
use crate::tree::Tree;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LivingRule {
    pub max_age_years: u32,
}

impl Default for LivingRule {
    fn default() -> Self {
        Self { max_age_years: 100 }
    }
}

/// Per-person facts gathered in one pass over the events.
#[derive(Default)]
struct Facts {
    /// Principal of a non-negated death-like event.
    dead_event: bool,
    /// Latest JDN by which the person was born, if a birth-like event gives an upper bound.
    /// An open-ended date (`AFT`) has no `sort_max` and so proves nothing.
    birth_bound: Option<i32>,
}

fn gather_facts(tree: &Tree) -> HashMap<Uuid, Facts> {
    let mut facts: HashMap<Uuid, Facts> = HashMap::new();
    for e in tree.events.values() {
        if e.negative {
            continue;
        }
        let is_death = matches!(
            e.kind,
            EventKind::Death | EventKind::Burial | EventKind::Cremation | EventKind::Probate
        );
        let is_birth = matches!(
            e.kind,
            EventKind::Birth | EventKind::Christening | EventKind::Baptism
        );
        if !is_death && !is_birth {
            continue;
        }
        let bound = e.date.as_ref().and_then(|d| d.sort_max);
        for p in &e.participants {
            if p.role != Role::Principal {
                continue;
            }
            let ParticipantRef::Person(id) = p.target else {
                continue;
            };
            let f = facts.entry(id).or_default();
            if is_death {
                f.dead_event = true;
            }
            if is_birth {
                f.birth_bound = f.birth_bound.max(bound);
            }
        }
    }
    facts
}

/// `true` = living. Every person in the tree gets an entry.
///
/// Linear in persons + events + family links: facts and parent/child
/// indexes are built once, then each person is checked against them.
pub fn compute_living(tree: &Tree, rule: &LivingRule, today_jdn: i32) -> HashMap<Uuid, bool> {
    compute(tree, rule, today_jdn, true)
}

/// As `compute_living` but ignoring every manual `living_override`: the rule alone. Applying the
/// overrides on top of this map gives exactly `compute_living`.
pub fn compute_living_ignoring_overrides(
    tree: &Tree,
    rule: &LivingRule,
    today_jdn: i32,
) -> HashMap<Uuid, bool> {
    compute(tree, rule, today_jdn, false)
}

fn compute(tree: &Tree, rule: &LivingRule, today_jdn: i32, overrides: bool) -> HashMap<Uuid, bool> {
    let max_age = rule.max_age_years;
    let self_cut = gregorian_minus_years(today_jdn, max_age);
    let child_cut = gregorian_minus_years(today_jdn, max_age.saturating_sub(12));
    let parent_cut = gregorian_minus_years(today_jdn, max_age.saturating_add(70));

    let facts = gather_facts(tree);
    let born_before = |p: Uuid, cut: i32| {
        facts
            .get(&p)
            .and_then(|f| f.birth_bound)
            .is_some_and(|b| b < cut)
    };

    // A child older than `child_years` proves each of its parents dead; a parent
    // older than `max_age + 70` proves each of its children dead.
    let mut proven: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
    for f in tree.families.values() {
        let partners = [f.partner1, f.partner2];
        let old_partner = partners
            .iter()
            .flatten()
            .any(|&p| born_before(p, parent_cut));
        let old_child = f.children.iter().any(|c| born_before(c.person, child_cut));
        if old_child {
            proven.extend(partners.iter().flatten().copied());
        }
        if old_partner {
            proven.extend(f.children.iter().map(|c| c.person));
        }
    }

    tree.persons
        .iter()
        .map(|(id, p)| {
            let living = match p.living_override.as_ref().filter(|_| overrides) {
                Some(LivingOverride::Living) => true,
                Some(LivingOverride::Deceased) => false,
                None => {
                    let dead = facts.get(id).is_some_and(|f| f.dead_event)
                        || born_before(*id, self_cut)
                        || proven.contains(id);
                    !dead
                }
            };
            (*id, living)
        })
        .collect()
}
