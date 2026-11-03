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

/// Latest JDN by which the person was born, if a birth-like event gives an upper bound.
/// An open-ended date (`AFT`) has no `sort_max` and so proves nothing.
fn birth_bound(tree: &Tree, person: Uuid) -> Option<i32> {
    tree.events_of(person)
        .filter(|(e, role)| {
            !e.negative
                && **role == Role::Principal
                && matches!(
                    e.kind,
                    EventKind::Birth | EventKind::Christening | EventKind::Baptism
                )
        })
        .filter_map(|(e, _)| e.date.as_ref().and_then(|d| d.sort_max))
        .max()
}

fn proven_dead(tree: &Tree, id: Uuid, rule: &LivingRule, today_jdn: i32) -> bool {
    let max_age = rule.max_age_years;
    if tree.events_of(id).any(|(e, role)| {
        !e.negative
            && *role == Role::Principal
            && matches!(
                e.kind,
                EventKind::Death | EventKind::Burial | EventKind::Cremation | EventKind::Probate
            )
    }) {
        return true;
    }
    let older_than = |p: Uuid, years: u32| {
        birth_bound(tree, p).is_some_and(|b| b < gregorian_minus_years(today_jdn, years))
    };
    if older_than(id, max_age) {
        return true;
    }
    let child_years = max_age.saturating_sub(12);
    let child_dead = tree
        .families_as_partner(id)
        .flat_map(|f| f.children.iter())
        .any(|c| older_than(c.person, child_years));
    if child_dead {
        return true;
    }
    tree.families_as_child(id).any(|(f, _)| {
        [f.partner1, f.partner2]
            .into_iter()
            .flatten()
            .any(|p| older_than(p, max_age.saturating_add(70)))
    })
}

/// `true` = living. Every person in the tree gets an entry.
pub fn compute_living(tree: &Tree, rule: &LivingRule, today_jdn: i32) -> HashMap<Uuid, bool> {
    tree.persons
        .iter()
        .map(|(id, p)| {
            let living = match p.living_override {
                Some(LivingOverride::Living) => true,
                Some(LivingOverride::Deceased) => false,
                None => !proven_dead(tree, *id, rule, today_jdn),
            };
            (*id, living)
        })
        .collect()
}
