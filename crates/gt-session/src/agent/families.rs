//! Cleaning up empty and parentless families, by an agent: a family of any author is removed
//! inside the agent's batch (so the batch undo restores the family, its links and its notes), and a
//! child link of any author is removed when the child keeps another family with a parent.
//!
//! The refusals are the point: a real couple, a real single-parent family, a person who would be
//! left with no parent family, events, and a missing reason are never removed. Whether anyone is
//! alive or dead is never touched (`check_living`).

use super::{AgentCtx, CORRECTION_REASON_MAX, people_reached, refused};
use crate::empty_families::{EmptyFamilyDto, EmptyFamilyKind};
use crate::error::{ApiError, ErrorCode};
use crate::session::Session;
use gt_store::Actor;
use uuid::Uuid;

/// What a removal did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmptyFamilyRemoval {
    pub removed: Uuid,
    /// Children whose link went with the family.
    pub children_unlinked: Vec<Uuid>,
    /// Who took the notes, citations and pictures over.
    pub moved_to: Option<Uuid>,
    pub notes: usize,
    pub citations: usize,
    pub media: usize,
}

fn check_reason(reason: &str, what: &str) -> Result<String, ApiError> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(refused(format!("Give the reason: {what}.")));
    }
    if reason.chars().count() > CORRECTION_REASON_MAX {
        return Err(ApiError::new(
            ErrorCode::Invalid,
            "The reason holds up to 1000 characters: put the rest in a note.",
        ));
    }
    Ok(reason.to_string())
}

impl Session {
    /// The empty and parentless families, for an agent to look through. Read only.
    pub fn list_empty_families_as_agent(
        &self,
        kind: Option<EmptyFamilyKind>,
    ) -> Result<Vec<EmptyFamilyDto>, ApiError> {
        self.list_empty_families(kind)
    }

    /// Removes the family `family` (whoever made it) in one change set under `batch`. See the
    /// module comment and `plan_empty_family_removal` for what it checks and moves.
    pub fn remove_empty_family_as_agent(
        &mut self,
        family: Uuid,
        reason: &str,
        notes_to: Option<Uuid>,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(EmptyFamilyRemoval, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        let reason = check_reason(
            reason,
            "why this family is empty or wrong, and who or what showed it",
        )?;
        let who = format!("agent {}", ctx.name);
        let fam = self
            .tree
            .families
            .get(&family)
            .ok_or_else(|| ApiError::not_found("family", family))?;
        let (notes, citations, media) = (
            fam.common.notes.len(),
            fam.common.citations.len(),
            fam.common.media.len(),
        );
        let children_unlinked: Vec<Uuid> = fam.children.iter().map(|c| c.person).collect();
        let (cs, touched) = self.plan_empty_family_removal(family, &reason, notes_to, &who)?;
        self.check_living(&cs)?;
        let changed: Vec<Uuid> = cs.changes.iter().map(|c| c.id).collect();
        let mut reached = people_reached(&self.tree, &changed);
        let actor = Actor::Agent {
            id: ctx.id,
            name: ctx.name.clone(),
        };
        self.commit(&cs, actor, Some(batch))?;
        for id in people_reached(&self.tree, &changed)
            .into_iter()
            .chain(touched.iter().copied())
        {
            if !reached.contains(&id) {
                reached.push(id);
            }
        }
        Ok((
            EmptyFamilyRemoval {
                removed: family,
                children_unlinked,
                moved_to: notes_to,
                notes,
                citations,
                media,
            },
            reached,
        ))
    }

    /// Removes the link of `child` in `family` (whoever made it), in one change set under `batch`,
    /// when the child keeps another family that has at least one partner.
    pub fn remove_child_from_family_as_agent(
        &mut self,
        family: Uuid,
        child: Uuid,
        reason: &str,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<Vec<Uuid>, ApiError> {
        self.check_batch(ctx, batch)?;
        let reason = check_reason(reason, "why this child does not belong in this family")?;
        let who = format!("agent {}", ctx.name);
        let cs = self.plan_child_removal_anywhere(family, child, &reason, &who)?;
        self.check_living(&cs)?;
        let changed: Vec<Uuid> = cs.changes.iter().map(|c| c.id).collect();
        let mut reached = people_reached(&self.tree, &changed);
        let actor = Actor::Agent {
            id: ctx.id,
            name: ctx.name.clone(),
        };
        self.commit(&cs, actor, Some(batch))?;
        for id in people_reached(&self.tree, &changed) {
            if !reached.contains(&id) {
                reached.push(id);
            }
        }
        if !reached.contains(&child) {
            reached.push(child);
        }
        Ok(reached)
    }
}
