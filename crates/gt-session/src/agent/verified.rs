//! Verified corrections: the one way an agent changes what someone else wrote. Every call names
//! the record it read (source, page, the record's own words) and says why; the change, the
//! citation of that record on what changed, and the reason in the log are one write the owner
//! can undo with the batch. Whether anyone counts as alive or dead is still checked (step 6),
//! so a correction never makes anyone alive, and makes someone dead only as the rules allow.
//! Nothing here deletes a person.

use super::{AgentCtx, AgentWrite, CORRECTION_REASON_MAX, refused};
use crate::command::OwnerRef;
use crate::error::{ApiError, ErrorCode};
use crate::ops::verified::{
    Cited, EventPatch, NamePatch, plan_child_move, plan_event_correction, plan_name_correction,
    plan_note_correction,
};
use crate::session::Session;
use gt_core::model::*;
use uuid::Uuid;

/// The record a correction rests on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedEvidence {
    pub source: Uuid,
    pub page: String,
    /// The record's own words.
    pub quote: String,
    pub quality: Option<u8>,
}

/// What an event correction changes. Absent fields stay; an empty text clears.
#[derive(Debug, Clone, Default)]
pub struct EventCorrection {
    pub date_raw: Option<String>,
    pub place_text: Option<String>,
    pub place_id: Option<Uuid>,
    pub description: Option<String>,
    pub age: Option<String>,
    pub cause: Option<String>,
}

pub use crate::ops::verified::NamePatch as NameCorrection;

const QUOTE_MAX: usize = 64 * 1024;
const PAGE_MAX: usize = 1000;

impl Session {
    /// Checks the evidence and the reason: a real source, a page, the record's words, a reason.
    fn check_verified(&self, e: &VerifiedEvidence, reason: &str) -> Result<String, ApiError> {
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(refused(
                "Give the reason: what the record shows that the tree has wrong.",
            ));
        }
        if reason.chars().count() > CORRECTION_REASON_MAX {
            return Err(refused(
                "The reason holds up to 1000 characters: say what the record shows, and put \
                 the rest in a note.",
            ));
        }
        if !self.tree.sources.contains_key(&e.source) {
            return Err(refused(
                "The evidence names a source that is not in the tree. Add the source first \
                 (add_source), then correct with the record you read in it.",
            ));
        }
        if e.page.trim().is_empty() || e.page.chars().count() > PAGE_MAX {
            return Err(refused(
                "The evidence needs the page (or entry) of the record you read.",
            ));
        }
        if e.quote.trim().is_empty() {
            return Err(refused(
                "The evidence needs the record's own words (quote). Without a record you \
                 actually read, leave a note and list the case in your report.",
            ));
        }
        if e.quote.len() > QUOTE_MAX {
            return Err(refused("The quote holds up to 64 KiB."));
        }
        if e.quality.is_some_and(|q| q > 3) {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "quality must be from 0 to 3.",
            ));
        }
        Ok(reason.to_string())
    }

    fn cited<'a>(e: &'a VerifiedEvidence) -> Cited<'a> {
        Cited {
            source: e.source,
            page: e.page.trim(),
            quote: e.quote.trim(),
            quality: e.quality,
        }
    }

    fn finish_verified(
        &mut self,
        planned: Option<(gt_store::ChangeSet, Option<Uuid>)>,
        record: Uuid,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        let Some((cs, focus)) = planned else {
            return Ok((AgentWrite::AlreadyThere { record }, Vec::new()));
        };
        self.check_living(&cs)?;
        self.store(cs, focus, ctx, batch)
    }

    /// Corrects a name of any person: its given name, patronymic, surnames or kind, and/or makes
    /// it the main name. When the main name's text changes, its old text stays as an alternative
    /// name (as recorded), so nothing is lost.
    #[allow(clippy::too_many_arguments)]
    pub fn correct_name_as_agent(
        &mut self,
        person: Uuid,
        name: Option<Uuid>,
        patch: &NamePatch,
        evidence: &VerifiedEvidence,
        reason: &str,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        let reason = self.check_verified(evidence, reason)?;
        let (_, now) = (self.clock.0)();
        let planned = plan_name_correction(
            &self.tree,
            person,
            name,
            patch,
            &Self::cited(evidence),
            &reason,
            now,
            self.day_order(),
        )?;
        self.finish_verified(planned, name.unwrap_or(person), ctx, batch)
    }

    /// Corrects an event of any person or family: date, place, description, age or cause.
    pub fn correct_event_as_agent(
        &mut self,
        event: Uuid,
        patch: &EventCorrection,
        evidence: &VerifiedEvidence,
        reason: &str,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        let reason = self.check_verified(evidence, reason)?;
        let e = self
            .tree
            .events
            .get(&event)
            .ok_or_else(|| ApiError::not_found("event", event))?;
        let order = self.day_order();
        let date = match patch.date_raw.as_deref().map(str::trim) {
            None => None,
            Some("") => {
                return Err(refused(
                    "A correction gives a date, not an empty one. To say a date is unknown, \
                     leave a note.",
                ));
            }
            Some(raw) => {
                if raw.chars().count() > super::CORRECTION_DATE_MAX {
                    return Err(refused("The date holds up to 100 characters."));
                }
                let d = crate::ops::parse_date(Some(raw), order)
                    .ok_or_else(|| ApiError::new(ErrorCode::Invalid, "date must be a date."))?;
                let is_birth = e.kind == EventKind::Birth;
                if is_birth || e.kind == EventKind::Death {
                    self.check_life_order(event, is_birth, &d)?;
                }
                Some(d)
            }
        };
        if let Some(id) = patch.place_id
            && !self.tree.places.contains_key(&id)
        {
            return Err(ApiError::not_found("place", id));
        }
        let ev = EventPatch {
            date,
            place_text: patch.place_text.clone(),
            place_id: patch.place_id,
            description: patch.description.clone(),
            age: patch.age.clone(),
            cause: patch.cause.clone(),
        };
        let (_, now) = (self.clock.0)();
        let planned = plan_event_correction(
            &self.tree,
            event,
            &ev,
            &Self::cited(evidence),
            &reason,
            now,
            order,
        )?;
        self.finish_verified(planned, event, ctx, batch)
    }

    /// Replaces the text of any note (and its story, which must then be given anew or cleared).
    /// The citation goes on what the note is attached to (`holder`).
    #[allow(clippy::too_many_arguments)]
    pub fn correct_note_as_agent(
        &mut self,
        note: Uuid,
        holder: OwnerRef,
        text: &str,
        story: Option<&str>,
        clear_story: bool,
        evidence: &VerifiedEvidence,
        reason: &str,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        let reason = self.check_verified(evidence, reason)?;
        let Some(current) = self.tree.notes.get(&note) else {
            return Err(ApiError::not_found("note", note));
        };
        if text.trim().is_empty() {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "text must be the new text of the note.",
            ));
        }
        if text.len() > QUOTE_MAX {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "text is too long: a note holds up to 64 KiB.",
            ));
        }
        if clear_story && story.is_some() {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "Give the new story or clear_story, not both.",
            ));
        }
        if current
            .story
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty())
            && story.is_none()
            && !clear_story
            && current.text != text
        {
            return Err(refused(
                "This note has a story (a readable retelling) that would no longer match. Give \
                 the new story too, or clear_story: true to take it away.",
            ));
        }
        let (_, now) = (self.clock.0)();
        let planned = plan_note_correction(
            &self.tree,
            note,
            holder,
            text,
            story,
            clear_story,
            &Self::cited(evidence),
            &reason,
            now,
            self.day_order(),
        )?;
        self.finish_verified(planned, note, ctx, batch)
    }

    /// Moves a child's link from one family to another that shares a parent with it (a family of
    /// the father alone to the family of the father and the mother). The link kind stays. A source
    /// family this connection made in this batch and that is left empty is removed.
    #[allow(clippy::too_many_arguments)]
    pub fn move_child_as_agent(
        &mut self,
        child: Uuid,
        from: Uuid,
        to: Uuid,
        evidence: &VerifiedEvidence,
        reason: &str,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        let reason = self.check_verified(evidence, reason)?;
        if from == to {
            return Err(refused("The two families are the same."));
        }
        if !self.tree.persons.contains_key(&child) {
            return Err(ApiError::not_found("person", child));
        }
        if self.tree.persons[&child].sibling_marker {
            return Err(refused(
                "The entry for brothers and sisters nobody named is not moved. Leave it and \
                 list it in your report.",
            ));
        }
        let source = self
            .tree
            .families
            .get(&from)
            .ok_or_else(|| ApiError::not_found("family", from))?;
        let target = self
            .tree
            .families
            .get(&to)
            .ok_or_else(|| ApiError::not_found("family", to))?;
        let in_source = source.children.iter().any(|l| l.person == child);
        let in_target = target.children.iter().any(|l| l.person == child);
        if !in_source {
            if in_target {
                return Ok((AgentWrite::AlreadyThere { record: child }, Vec::new()));
            }
            return Err(refused(format!(
                "{} is not a child in the first family, so there is no link to move.",
                self.person_name(child)
            )));
        }
        if in_target {
            return Err(refused(
                "The child is a child in both families already. Leave it and list it in your \
                 report.",
            ));
        }
        let parents = |f: &Family| {
            [f.partner1, f.partner2]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
        };
        let target_parents = parents(target);
        if !parents(source).iter().any(|p| target_parents.contains(p)) {
            return Err(refused(
                "The two families share no parent, so this is not the same child's parents \
                 found more fully. Leave a note and list it in your report.",
            ));
        }
        if target_parents.contains(&child) {
            return Err(refused("A person cannot be their own child."));
        }
        let name_of = |s: &Self, f: &Family| {
            let names: Vec<String> = parents(f).into_iter().map(|p| s.person_name(p)).collect();
            if names.is_empty() {
                "a family with no partners".to_string()
            } else {
                format!("the family of {}", names.join(" and "))
            }
        };
        let note = format!(
            "Moved from {} (family {from}) to {} (family {to}) as a child. Reason: {reason}",
            name_of(self, source),
            name_of(self, target),
        );
        let (_, now) = (self.clock.0)();
        let (cs, focus) = plan_child_move(
            &self.tree,
            child,
            from,
            to,
            &note,
            &Self::cited(evidence),
            &reason,
            now,
            self.day_order(),
        )?;
        self.check_birth_family(&cs)?;
        self.check_parent_roles(&cs)?;
        self.check_living(&cs)?;
        let (write, mut reached) = self.store(cs, focus, ctx, batch)?;
        // A family this batch made, left with nothing: take it away (anything else stays).
        if let Ok((_, more)) = self.remove_own_empty_family(from, ctx, batch) {
            reached.extend(more);
        }
        Ok((write, reached))
    }
}
