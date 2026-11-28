//! Empty and parentless families: the list the owner and agents clean up, and the one removal both
//! use. A family is a candidate when it has no partners (with or without children), or exactly one
//! partner and no children. The removal never leaves a person cut off from every parent family,
//! never touches events, and moves the family's notes, citations and pictures to a member.
//!
//! The planning lives here and is shared: the owner's command (`remove_empty_family_as_owner`, one
//! Ctrl+Z step) and the agent tools (`agent/families.rs`, under the agent's batch).

use crate::dto::PersonRef;
use crate::error::{ApiError, ErrorCode};
use crate::ops::{Planner, new_common};
use crate::session::Session;
use gt_core::model::{Family, MediaLink, Note, NoteKind, ParticipantRef, Person};
use gt_core::tree::Tree;
use gt_store::{Actor, ChangeSet};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Which kind of family it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum EmptyFamilyKind {
    /// No partners, with children.
    Parentless,
    /// Exactly one partner and no children.
    SinglePartner,
    /// No partners and no children.
    Empty,
}

impl EmptyFamilyKind {
    /// The word an agent gives to pick one kind.
    pub fn word(self) -> &'static str {
        match self {
            EmptyFamilyKind::Parentless => "parentless",
            EmptyFamilyKind::SinglePartner => "singlePartner",
            EmptyFamilyKind::Empty => "empty",
        }
    }
}

/// A child of a listed family.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EmptyFamilyChildDto {
    pub person: PersonRef,
    /// How many other families the child is in that have at least one partner.
    pub other_parent_families: u32,
    /// `other_parent_families > 0`: the child stays in the родовід without this family.
    pub has_other_family: bool,
}

/// An event of a listed family (marriage and so on).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EmptyFamilyEventDto {
    pub id: Uuid,
    pub kind: String,
    pub date: Option<String>,
}

/// Who made the family and when, from the change log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EmptyFamilyOrigin {
    /// `owner`, `agent` or `copy`.
    pub by: String,
    /// The agent connection's or the copy's name.
    pub name: Option<String>,
    /// Unix ms.
    #[ts(type = "number")]
    pub at: i64,
    /// The log shows the family being created; otherwise this is only its first change.
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EmptyFamilyDto {
    pub id: Uuid,
    pub kind: EmptyFamilyKind,
    pub partners: Vec<PersonRef>,
    pub children: Vec<EmptyFamilyChildDto>,
    pub notes: u32,
    pub citations: u32,
    pub media: u32,
    pub events: Vec<EmptyFamilyEventDto>,
    /// `None` for a family that came with an imported file (no log row).
    pub origin: Option<EmptyFamilyOrigin>,
    /// Whether `remove_empty_family` would go ahead now (given a `notes_to` when needed).
    pub removable: bool,
    /// Why it would be refused, in plain words; empty when `removable`.
    pub blockers: Vec<String>,
    /// The family holds notes, citations or pictures: they move to `notes_to`.
    pub needs_notes_to: bool,
    /// A member to give as `notes_to`: the partner, else the first child.
    pub suggested_notes_to: Option<Uuid>,
}

pub(crate) fn refused(msg: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::AgentRefused, msg)
}

pub(crate) fn kind_of(f: &Family) -> Option<EmptyFamilyKind> {
    let partners = usize::from(f.partner1.is_some()) + usize::from(f.partner2.is_some());
    match (partners, f.children.is_empty()) {
        (0, false) => Some(EmptyFamilyKind::Parentless),
        (0, true) => Some(EmptyFamilyKind::Empty),
        (1, true) => Some(EmptyFamilyKind::SinglePartner),
        _ => None,
    }
}

pub(crate) fn members(f: &Family) -> Vec<Uuid> {
    f.partner1
        .into_iter()
        .chain(f.partner2)
        .chain(f.children.iter().map(|c| c.person))
        .collect()
}

/// How many other families `child` is a child in that have at least one partner.
pub(crate) fn other_parent_families(tree: &Tree, family: Uuid, child: Uuid) -> usize {
    tree.families
        .values()
        .filter(|f| f.common.id != family)
        .filter(|f| f.partner1.is_some() || f.partner2.is_some())
        .filter(|f| f.children.iter().any(|c| c.person == child))
        .count()
}

fn family_events(tree: &Tree, family: Uuid) -> Vec<Uuid> {
    let mut ids: Vec<Uuid> = tree
        .events
        .values()
        .filter(|e| {
            e.participants
                .iter()
                .any(|p| p.target == ParticipantRef::Family(family))
        })
        .map(|e| e.common.id)
        .collect();
    ids.sort();
    ids
}

fn holds_things(f: &Family) -> bool {
    !f.common.notes.is_empty() || !f.common.citations.is_empty() || !f.common.media.is_empty()
}

fn research_note(text: String, now: i64) -> Note {
    Note {
        common: new_common(now),
        kind: NoteKind::Research,
        text,
        lang: None,
        shared: false,
        story: None,
    }
}

fn add_media(into: &mut Vec<MediaLink>, from: &[MediaLink]) {
    let has_primary = into.iter().any(|l| l.primary);
    for l in from {
        if into
            .iter()
            .any(|k| k.media == l.media && k.region == l.region && k.clip == l.clip)
        {
            continue;
        }
        let mut l = l.clone();
        if has_primary {
            l.primary = false;
        }
        into.push(l);
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl Session {
    fn plain_name(&self, id: Uuid) -> String {
        self.person_ref(id)
            .map_or_else(|| id.to_string(), |p| p.name)
    }

    fn whose_family(&self, fam: &Family) -> String {
        match fam.partner1.or(fam.partner2) {
            Some(x) => format!("the family of {}", self.plain_name(x)),
            None => "a family with no partners".to_string(),
        }
    }

    /// The reasons `fam` cannot be removed, whatever `notes_to` is; empty when it can.
    pub(crate) fn removal_blockers(&self, fam: &Family) -> Vec<String> {
        let mut out = Vec::new();
        let partners = usize::from(fam.partner1.is_some()) + usize::from(fam.partner2.is_some());
        if partners == 2 {
            out.push(
                "The family has two partners: it is a real couple, not an empty family.".into(),
            );
        }
        if partners == 1 && !fam.children.is_empty() {
            out.push(
                "The family has one partner and children: it is a real single-parent family."
                    .into(),
            );
        }
        if !family_events(&self.tree, fam.common.id).is_empty() {
            out.push(
                "The family has events (a marriage or the like). Move them to a person first, or \
                 leave the family."
                    .into(),
            );
        }
        if partners == 0 {
            for c in &fam.children {
                if other_parent_families(&self.tree, fam.common.id, c.person) == 0 {
                    out.push(format!(
                        "{} would be left with no parent family: removing the link would cut \
                         them off from the родовід.",
                        self.plain_name(c.person)
                    ));
                }
            }
        }
        if holds_things(fam) && members(fam).is_empty() {
            out.push(
                "The family holds notes, citations or pictures and has no partner or child to \
                 move them to."
                    .into(),
            );
        }
        out
    }

    /// The families that are empty or parentless, with what each holds, who made it and when,
    /// and whether it can be removed. `kind` narrows to one kind.
    pub fn list_empty_families(
        &self,
        kind: Option<EmptyFamilyKind>,
    ) -> Result<Vec<EmptyFamilyDto>, ApiError> {
        let mut rows: Vec<EmptyFamilyDto> = Vec::new();
        for fam in self.tree.families.values() {
            let Some(k) = kind_of(fam) else { continue };
            if kind.is_some_and(|want| want != k) {
                continue;
            }
            let id = fam.common.id;
            let partners: Vec<PersonRef> = [fam.partner1, fam.partner2]
                .into_iter()
                .flatten()
                .filter_map(|p| self.person_ref(p))
                .collect();
            let children: Vec<EmptyFamilyChildDto> = fam
                .children
                .iter()
                .filter_map(|c| {
                    let n = other_parent_families(&self.tree, id, c.person);
                    Some(EmptyFamilyChildDto {
                        person: self.person_ref(c.person)?,
                        other_parent_families: count(n),
                        has_other_family: n > 0,
                    })
                })
                .collect();
            let events = family_events(&self.tree, id)
                .into_iter()
                .filter_map(|e| self.tree.events.get(&e))
                .map(|e| EmptyFamilyEventDto {
                    id: e.common.id,
                    kind: format!("{:?}", e.kind),
                    date: e.date.as_ref().map(|d| d.display()),
                })
                .collect();
            let origin = self.project.first_write(id)?.and_then(|w| {
                let (by, name) = match w.actor? {
                    Actor::User => ("owner".to_string(), None),
                    Actor::Agent { name, .. } => ("agent".to_string(), Some(name)),
                    Actor::Copy { name } => ("copy".to_string(), Some(name)),
                };
                Some(EmptyFamilyOrigin {
                    by,
                    name,
                    at: w.ts,
                    created: w.created,
                })
            });
            let blockers = self.removal_blockers(fam);
            rows.push(EmptyFamilyDto {
                id,
                kind: k,
                partners,
                children,
                notes: count(fam.common.notes.len()),
                citations: count(fam.common.citations.len()),
                media: count(fam.common.media.len()),
                events,
                origin,
                removable: blockers.is_empty(),
                blockers,
                needs_notes_to: holds_things(fam),
                suggested_notes_to: members(fam).first().copied(),
            });
        }
        let order = |k: EmptyFamilyKind| match k {
            EmptyFamilyKind::Parentless => 0,
            EmptyFamilyKind::SinglePartner => 1,
            EmptyFamilyKind::Empty => 2,
        };
        rows.sort_by_key(|r| (order(r.kind), r.id));
        Ok(rows)
    }

    /// All the refusals, checked against the tree.
    pub(crate) fn check_empty_family_removal(
        &self,
        fam: &Family,
        notes_to: Option<Uuid>,
    ) -> Result<(), ApiError> {
        let blockers = self.removal_blockers(fam);
        if !blockers.is_empty() {
            return Err(refused(blockers.join(" ")));
        }
        if let Some(to) = notes_to
            && !members(fam).contains(&to)
        {
            return Err(refused(
                "notes_to must be a partner or a child of the family being removed.",
            ));
        }
        if holds_things(fam) && notes_to.is_none() {
            return Err(refused(
                "The family holds notes, citations or pictures. Give notes_to: a partner or a \
                 child of the family, who takes them over.",
            ));
        }
        Ok(())
    }

    /// The change set that removes `family`: child links go with it (each child gets a research
    /// note), its notes, citations and pictures move to `notes_to`. `who` names the actor in the
    /// notes. Also answers the people it touched.
    pub(crate) fn plan_empty_family_removal(
        &self,
        family: Uuid,
        reason: &str,
        notes_to: Option<Uuid>,
        who: &str,
    ) -> Result<(ChangeSet, Vec<Uuid>), ApiError> {
        let Some(fam) = self.tree.families.get(&family) else {
            return Err(ApiError::not_found("family", family));
        };
        self.check_empty_family_removal(fam, notes_to)?;
        let (_, now) = (self.clock.0)();
        let mut p = Planner::new(&self.tree, now, self.day_order());
        let whose = self.whose_family(fam);
        let mut touched: Vec<Uuid> = Vec::new();
        for c in &fam.children {
            p.abandon(&c.citations, &[], true);
            let mut child: Person = p.get(c.person)?;
            let note = research_note(
                format!(
                    "Removed from {whose} (family {family}) as a child by {who}: that family had \
                     no partners and the child stays in another family. Reason: {reason}"
                ),
                now,
            );
            child.common.notes.push(note.common.id);
            child.common.changed = Some(now);
            p.put(note);
            p.put(child);
            touched.push(c.person);
        }
        if let Some(to) = notes_to {
            let mut person: Person = p.get(to)?;
            let (n, c, m) = (
                fam.common.notes.len(),
                fam.common.citations.len(),
                fam.common.media.len(),
            );
            for x in &fam.common.notes {
                if !person.common.notes.contains(x) {
                    person.common.notes.push(*x);
                }
            }
            for x in &fam.common.citations {
                if !person.common.citations.contains(x) {
                    person.common.citations.push(*x);
                }
            }
            add_media(&mut person.common.media, &fam.common.media);
            let note = research_note(
                format!(
                    "{whose} (family {family}) was removed by {who} as empty. Its {n} notes, {c} \
                     citations and {m} pictures moved here. Reason: {reason}"
                ),
                now,
            );
            person.common.notes.push(note.common.id);
            person.common.changed = Some(now);
            p.put(note);
            p.put(person);
            if !touched.contains(&to) {
                touched.push(to);
            }
        }
        p.remove::<Family>(family);
        let cs = p.finish(format!("Remove empty family: {reason}"));
        Ok((cs, touched))
    }

    /// The change set that takes one child out of one family; the child must keep another family
    /// that has at least one partner. A research note on the child records why.
    pub(crate) fn plan_child_removal_anywhere(
        &self,
        family: Uuid,
        child: Uuid,
        reason: &str,
        who: &str,
    ) -> Result<ChangeSet, ApiError> {
        let Some(fam) = self.tree.families.get(&family) else {
            return Err(ApiError::not_found("family", family));
        };
        if !self.tree.persons.contains_key(&child) {
            return Err(ApiError::not_found("person", child));
        }
        if !fam.children.iter().any(|c| c.person == child) {
            return Err(refused(format!(
                "{} is not a child in this family, so there is no link to remove.",
                self.plain_name(child)
            )));
        }
        if other_parent_families(&self.tree, family, child) == 0 {
            return Err(refused(format!(
                "{} has no other family with a parent: removing this link would cut them off from \
                 the родовід. Link them to their real parents first.",
                self.plain_name(child)
            )));
        }
        let whose = self.whose_family(fam);
        let note = format!(
            "Removed from {whose} (family {family}) as a child by {who}; the child stays in \
             another family. Reason: {reason}"
        );
        let (_, now) = (self.clock.0)();
        let (cs, _) = crate::ops::plan_child_link_removal(
            &self.tree,
            family,
            child,
            &note,
            now,
            self.day_order(),
        )?;
        Ok(cs)
    }

    /// The owner removes an empty family: one undo step (Ctrl+Z brings back the family, its links
    /// and its notes). Same refusals as the agent tool.
    pub fn remove_empty_family_as_owner(
        &mut self,
        family: Uuid,
        notes_to: Option<Uuid>,
    ) -> Result<crate::command::EditResult, ApiError> {
        let (cs, touched) = self
            .plan_empty_family_removal(
                family,
                "removed by the owner from the empty families list",
                notes_to,
                "the owner",
            )
            .map_err(|e| match e.code {
                ErrorCode::AgentRefused => ApiError::new(ErrorCode::Invalid, e.message),
                _ => e,
            })?;
        self.commit(&cs, Actor::User, None)?;
        self.push_undo(cs.clone());
        Ok(self.result(&cs, cs.label.clone(), touched.first().copied()))
    }
}
