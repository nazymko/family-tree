//! The gate on agent writes. An agent adds to the owner's tree and never changes what is there.
//!
//! `apply_as_agent` is the only way an agent's write reaches the tree. In order:
//!
//! 1. The batch exists in this tree, belongs to this connection, and is not closed.
//! 2. The command is on the list of additive commands.
//! 3. A repeat of what this connection already wrote adds nothing (`AlreadyThere`).
//! 4. Facts a person has only once, and parents a child has only one of in each role.
//! 5. Every change is compared in JSON: on what is not the agent's own, only filling an empty value
//!    and adding to a list pass.
//! 6. The owner's rule: the plan is tried on the tree, and if anyone who existed before the write
//!    would count differently as alive or dead, it is refused. This reads the result and not the
//!    command, because the status follows from dates and links in ways no list of commands covers.
//! 7. The write is stored under the agent, outside the owner's undo and redo.
//!
//! The checks that need no plan come first, so that the planner's own errors ("both parents are
//! already recorded") do not answer in their place. Planning changes nothing.
//!
//! "Own" means this connection introduced it and nobody else has changed it since. A write that
//! only moves a `changed` stamp, as adding an event does to its person, changes no fact.

use crate::command::{CitationArg, EditCommand, EditResult, LivingPatch, OwnerKind, OwnerRef};
use crate::dto::ParentRelId;
use crate::error::{ApiError, ErrorCode};
use crate::ops::{agent_number, apply_to_tree_tracked, plan_with};
use crate::revert::{identity, same_ignoring_stamp, stripped};
use crate::session::Session;
use gt_core::model::*;
use gt_store::{Actor, Batch, Change, ChangeOp, ChangeSet, Record, RecordKind};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Who is writing. Comes from the connection's key, never from anything the agent says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCtx {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentWrite {
    Done(EditResult),
    /// This connection already wrote exactly this; nothing was added.
    AlreadyThere {
        record: Uuid,
    },
}

fn refused(msg: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::AgentRefused, msg)
}

const ONLY_ADD: &str = "Agents only add to the tree. Leave what is there as it is, add a note \
                        instead, and list the case in your report.";

/// Step 2. Exhaustive on purpose: a new command is refused to compile until someone decides.
fn check_allowed(cmd: &EditCommand) -> Result<(), ApiError> {
    use EditCommand as C;
    match cmd {
        C::CreatePerson { .. }
        | C::QuickAdd { .. }
        | C::LinkExisting { .. }
        | C::SaveFamily { id: None, .. }
        | C::LinkChild { .. }
        | C::SaveName { .. }
        | C::SaveEvent { .. }
        | C::SaveNote { .. }
        | C::SaveSource { .. }
        | C::UpdateCitation { .. }
        | C::AddNumber { .. } => Ok(()),
        // TODO(merge with Task 3): allow `EditCommand::AddSiblingMarker { .. }` in the arm above.
        C::AddCitation { citation, .. } => match citation {
            CitationArg::Reuse(_) => Err(refused(
                "Reusing the last citation shares the owner's working state. Give the source and \
                 page, or add a note instead.",
            )),
            CitationArg::Clone(_) | CitationArg::New(_) => Ok(()),
        },
        C::UpdatePerson {
            sex,
            living_override,
            private,
            ..
        } => {
            if *living_override != LivingPatch::Keep {
                Err(refused(
                    "Only the owner can say whether someone is alive or dead. Leave a note and \
                     list it in your report.",
                ))
            } else if private.is_some() {
                Err(refused(
                    "Only the owner can mark someone private. Leave a note and list it in your \
                     report.",
                ))
            } else if sex.is_none() {
                Err(refused(format!(
                    "Only sex can be set on a person. {ONLY_ADD}"
                )))
            } else {
                Ok(())
            }
        }
        C::SaveFamily { id: Some(_), .. }
        | C::DeletePerson { .. }
        | C::DeleteName { .. }
        | C::DeleteEvent { .. }
        | C::DeleteFamily { .. }
        | C::UpdateChildLink { .. }
        | C::UnlinkChild { .. }
        | C::ReorderChildren { .. }
        | C::DeleteNote { .. }
        | C::RemoveCitation { .. }
        | C::AttachMedia { .. }
        | C::DetachMedia { .. }
        | C::DeleteMedia { .. }
        | C::SetPrimaryMedia { .. }
        | C::UpdateMedia { .. }
        | C::ImportDraft { .. } => Err(refused(format!(
            "\"{}\" is not open to agents. {ONLY_ADD}",
            cmd.label()
        ))),
    }
}

fn principals(e: &Event) -> impl Iterator<Item = Uuid> + '_ {
    e.participants.iter().filter_map(|p| match p.target {
        ParticipantRef::Person(id) if p.role == Role::Principal => Some(id),
        _ => None,
    })
}

fn one_of_a_kind(k: &EventKind) -> bool {
    matches!(
        k,
        EventKind::Birth | EventKind::Death | EventKind::Burial | EventKind::Cremation
    )
}

/// The new record of this kind in a plan.
fn added<T>(cs: &ChangeSet, f: impl Fn(&Record) -> Option<&T>) -> Option<&T> {
    cs.changes
        .iter()
        .filter(|c| c.before.is_none())
        .find_map(|c| c.after.as_ref().and_then(&f))
}

/// The parents who hold a role for `child`: by birth, or with the link unknown. The second value
/// says it is by birth. A step, adoptive or foster parent does not hold one.
fn role_holders<'a>(
    families: impl Iterator<Item = &'a Family>,
    child: Uuid,
) -> HashMap<Uuid, bool> {
    let mut out = HashMap::new();
    for f in families {
        for link in f.children.iter().filter(|c| c.person == child) {
            for (slot, rel) in [(f.partner1, &link.rel1), (f.partner2, &link.rel2)] {
                if let Some(parent) = slot
                    && matches!(rel, ParentRel::Birth | ParentRel::Unknown)
                {
                    let by_birth = *rel == ParentRel::Birth;
                    let seen = out.entry(parent).or_insert(false);
                    *seen |= by_birth;
                }
            }
        }
    }
    out
}

fn same_role(a: &Sex, b: &Sex) -> bool {
    matches!((a, b), (Sex::Male, Sex::Male) | (Sex::Female, Sex::Female))
}

/// The item of a list with this identity (see `revert::identity`).
fn find_item<'a>(list: &'a [Value], id: &Value) -> Option<&'a Value> {
    list.iter().find(|x| identity(x) == *id)
}

/// One logged write to a record and who made it (`None`: the owner, or before authors were kept).
type Written = (Option<Uuid>, Change);

fn record_is_own(rows: &[Written], me: Uuid) -> bool {
    let Some(((first_by, first), rest)) = rows.split_first() else {
        return false;
    };
    *first_by == Some(me)
        && first.before.is_none()
        && rest.iter().all(|(by, c)| {
            *by == Some(me) || same_ignoring_stamp(c.before.as_ref(), c.after.as_ref())
        })
}

fn at_path<'a>(mut v: &'a Value, path: &[String]) -> Option<&'a Value> {
    for key in path {
        v = v.get(key)?;
    }
    Some(v)
}

/// Whether this connection introduced the item in the list at `path`, and nobody else has
/// changed the item since. The record around it may have had other writes.
fn item_is_own(rows: &[Written], path: &[String], id: &Value, me: Uuid) -> bool {
    let item = |r: &Option<Record>| -> Option<Value> {
        let (json, _) = stripped(r.as_ref()?)?;
        find_item(at_path(&json, path)?.as_array()?, id).cloned()
    };
    let mut own = false;
    for (by, c) in rows {
        let (was, now) = (item(&c.before), item(&c.after));
        if was == now {
            continue;
        }
        if *by != Some(me) || (was.is_some() && !own) {
            return false;
        }
        own = true;
    }
    own
}

struct Compare<'a> {
    rows: &'a [Written],
    me: Uuid,
}

impl Compare<'_> {
    /// Step 5 on one value: `Err` names the fact that may not change. `in_item`: inside an item of
    /// a list, where ownership has been asked already.
    fn value(
        &self,
        path: &mut Vec<String>,
        b: &Value,
        a: &Value,
        in_item: bool,
    ) -> Result<(), String> {
        if b == a || b.is_null() || b.as_str() == Some("") {
            return Ok(());
        }
        match (b, a) {
            (Value::Array(ba), Value::Array(aa)) => self.array(path, ba, aa, in_item),
            // A date is one fact: its parts mean nothing apart.
            (Value::Object(bo), Value::Object(ao))
                if path.last().is_none_or(|k| k != "date")
                    && bo.len() == ao.len()
                    && bo.keys().all(|k| ao.contains_key(k)) =>
            {
                for (k, bv) in bo {
                    let av = ao.get(k).unwrap_or(&Value::Null);
                    path.push(k.clone());
                    let verdict = self.value(path, bv, av, in_item);
                    path.pop();
                    verdict?;
                }
                Ok(())
            }
            _ => Err(fact_name(path)),
        }
    }

    /// Every old item still there, in the same order; an old item that changed or went must have
    /// passed `value` or be the agent's own.
    fn array(
        &self,
        path: &mut Vec<String>,
        before: &[Value],
        after: &[Value],
        in_item: bool,
    ) -> Result<(), String> {
        let own =
            |path: &[String], id: &Value| !in_item && item_is_own(self.rows, path, id, self.me);
        let mut from = 0;
        for item in before {
            let id = identity(item);
            let found = after
                .iter()
                .enumerate()
                .skip(from)
                .find(|(_, x)| identity(x) == id);
            match found {
                None => {
                    if !own(path, &id) {
                        return Err(fact_name(path));
                    }
                }
                Some((at, now)) => {
                    from = at + 1;
                    if let Err(e) = self.value(path, item, now, true)
                        && !own(path, &id)
                    {
                        return Err(e);
                    }
                }
            }
        }
        Ok(())
    }
}

/// `["Person", "names", "nickname"]` -> `names nickname`.
fn fact_name(path: &[String]) -> String {
    let fact = path.iter().skip(1).cloned().collect::<Vec<_>>().join(" ");
    if fact.is_empty() {
        "this record".into()
    } else {
        fact
    }
}

/// How a child relates to a parent slot nobody holds says nothing, so it is free to change along
/// with filling the slot: a step-father taking the free place beside a mother recorded alone.
fn free_slots_follow(before: &mut Value, after: &Value) {
    let (Some(b), Some(a)) = (before.get_mut("Family"), after.get("Family")) else {
        return;
    };
    let empty: Vec<&str> = [("partner1", "rel1"), ("partner2", "rel2")]
        .into_iter()
        .filter(|(slot, _)| b.get(*slot).is_some_and(Value::is_null))
        .map(|(_, rel)| rel)
        .collect();
    let (Some(kids), Some(new)) = (
        b.get_mut("children").and_then(Value::as_array_mut),
        a.get("children").and_then(Value::as_array),
    ) else {
        return;
    };
    for kid in kids {
        let Some(now) = find_item(new, &identity(kid)).cloned() else {
            continue;
        };
        for rel in &empty {
            if let (Some(slot), Some(v)) = (kid.get_mut(*rel), now.get(*rel)) {
                *slot = v.clone();
            }
        }
    }
}

impl Session {
    /// Opens a batch for this connection; every write names one.
    pub fn begin_batch(
        &mut self,
        ctx: &AgentCtx,
        title: &str,
        source_file: Option<&str>,
    ) -> Result<Uuid, ApiError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "Give the batch a title, for example the name of the file you are working from.",
            ));
        }
        let (_, now) = (self.clock.0)();
        let id = gt_core::ids::new_id();
        self.project.batch_create(&Batch {
            id,
            title: title.to_string(),
            source_file: source_file
                .map(str::trim)
                .filter(|f| !f.is_empty())
                .map(str::to_string),
            actor_id: ctx.id,
            actor_name: ctx.name.clone(),
            started: now,
            ended: None,
            report: None,
        })?;
        Ok(id)
    }

    /// Closes a batch and stores its report. What was written stays.
    pub fn end_batch(&mut self, ctx: &AgentCtx, batch: Uuid, report: &str) -> Result<(), ApiError> {
        self.check_batch(ctx, batch)?;
        let (_, now) = (self.clock.0)();
        self.project.batch_close(batch, report, now)?;
        Ok(())
    }

    /// The person an agent recorded under this number in this source file.
    pub fn find_by_number(&self, source_file: &str, number: &str) -> Option<Uuid> {
        let id = agent_number(source_file, number)?;
        self.tree
            .persons
            .values()
            .find(|p| p.common.ext_ids.contains(&id))
            .map(|p| p.common.id)
    }

    /// Writes one command for an agent, if the gate lets it through (see the module comment).
    pub fn apply_as_agent(
        &mut self,
        cmd: EditCommand,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<AgentWrite, ApiError> {
        self.check_batch(ctx, batch)?;
        check_allowed(&cmd)?;
        let cmd = self.keep_preferred(cmd);
        self.check_links_and_numbers(&cmd)?;
        let (_, now) = (self.clock.0)();
        // No last citation: that is the owner's, and `check_allowed` refused asking for it.
        let (cs, focus) = plan_with(&self.tree, &cmd, now, self.day_order(), None)?;
        if let Some(record) = self.repeat_of(&cmd, &cs, ctx.id)? {
            return Ok(AgentWrite::AlreadyThere { record });
        }
        self.check_one_of_a_kind(&cs)?;
        self.check_parent_roles(&cs)?;
        for ch in &cs.changes {
            self.check_change(ch, ctx.id)?;
        }
        self.check_living(&cs)?;
        if !cs.changes.is_empty() {
            let actor = Actor::Agent {
                id: ctx.id,
                name: ctx.name.clone(),
            };
            self.commit(&cs, actor, Some(batch))?;
        }
        Ok(AgentWrite::Done(self.result(&cs, cs.label.clone(), focus)))
    }

    fn check_batch(&self, ctx: &AgentCtx, batch: Uuid) -> Result<(), ApiError> {
        let Some(b) = self.project.batch_get(batch)? else {
            return Err(refused(
                "This batch is not in the tree that is open now. Open a new batch and try again.",
            ));
        };
        if b.actor_id != ctx.id {
            return Err(refused(
                "This batch belongs to another connection. Open your own batch and try again.",
            ));
        }
        if b.ended.is_some() {
            return Err(refused(
                "This batch is closed. Open a new batch and try again.",
            ));
        }
        Ok(())
    }

    /// A name an agent adds never takes over as preferred, and one it edits stays as it is:
    /// a change of preferred name would change the old name too.
    fn keep_preferred(&self, mut cmd: EditCommand) -> EditCommand {
        if let EditCommand::SaveName { person, name } = &mut cmd {
            let current = name.id.and_then(|id| {
                self.tree
                    .persons
                    .get(person)?
                    .names
                    .iter()
                    .find(|n| n.id == id)
                    .map(|n| n.assertion.preferred)
            });
            name.preferred = current.unwrap_or(false);
        }
        cmd
    }

    fn person_name(&self, id: Uuid) -> String {
        self.person_ref(id)
            .map(|p| p.name)
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "this person".into())
    }

    /// Step 4, the checks the command alone can answer.
    fn check_links_and_numbers(&self, cmd: &EditCommand) -> Result<(), ApiError> {
        use crate::command::Relation as R;
        match cmd {
            EditCommand::QuickAdd {
                anchor,
                relation: relation @ (R::Father | R::Mother),
                parent_rel: None | Some(ParentRelId::Birth),
                ..
            }
            | EditCommand::LinkExisting {
                anchor,
                relation: relation @ (R::Father | R::Mother),
                parent_rel: None | Some(ParentRelId::Birth),
                ..
            } => {
                let holders = role_holders(self.tree.families.values(), *anchor);
                let (role, word) = match relation {
                    R::Father => (Sex::Male, "father"),
                    _ => (Sex::Female, "mother"),
                };
                let taken = holders.keys().any(|id| {
                    self.tree
                        .persons
                        .get(id)
                        .is_some_and(|p| same_role(&p.sex, &role))
                });
                if taken || holders.len() >= 2 {
                    return Err(refused(format!(
                        "{} already has a {word}. A second one cannot be added by birth. Add a \
                         note giving both versions and the source, or link as a step, adoptive \
                         or foster parent if that is what the source says.",
                        self.person_name(*anchor)
                    )));
                }
                Ok(())
            }
            EditCommand::LinkChild { family, child, .. } => {
                let elsewhere = self.tree.families.values().any(|f| {
                    f.common.id != *family
                        && f.children.iter().any(|l| {
                            l.person == *child
                                && (l.rel1 == ParentRel::Birth || l.rel2 == ParentRel::Birth)
                        })
                });
                if elsewhere {
                    return Err(refused(format!(
                        "{} already has a birth family. Add a note giving both versions and the \
                         source.",
                        self.person_name(*child)
                    )));
                }
                Ok(())
            }
            EditCommand::AddNumber {
                person,
                source_file,
                number,
            } => match self.find_by_number(source_file, number) {
                Some(other) if other != *person => Err(refused(format!(
                    "Number {} of {} already points at {}. A number points at one person within \
                     a file. Use that person, or add a note if you think they are wrong.",
                    number.trim(),
                    source_file.trim(),
                    self.person_name(other)
                ))),
                _ => Ok(()),
            },
            _ => Ok(()),
        }
    }

    /// Whether this connection wrote the record in the first place.
    fn introduced_by(&self, id: Uuid, me: Uuid) -> Result<bool, ApiError> {
        Ok(self
            .project
            .last_writers(id)?
            .first()
            .is_some_and(|(by, op)| *by == Some(me) && *op == ChangeOp::Create))
    }

    /// The notes and citations held by a target.
    fn lists_of(&self, target: OwnerRef) -> Option<(&[Uuid], &[Uuid])> {
        fn of(c: &Common) -> (&[Uuid], &[Uuid]) {
            (&c.notes, &c.citations)
        }
        let t = &self.tree;
        match target.kind {
            OwnerKind::Person => t.persons.get(&target.id).map(|x| of(&x.common)),
            OwnerKind::Family => t.families.get(&target.id).map(|x| of(&x.common)),
            OwnerKind::Event => t.events.get(&target.id).map(|x| of(&x.common)),
            OwnerKind::Media => t.media.get(&target.id).map(|x| of(&x.common)),
            OwnerKind::Citation => t.citations.get(&target.id).map(|x| of(&x.common)),
            OwnerKind::Name => t
                .persons
                .values()
                .flat_map(|p| &p.names)
                .find(|n| n.id == target.id)
                .map(|n| (&n.notes[..], &n.citations[..])),
            OwnerKind::ChildLink => None,
        }
    }

    /// Step 3: the record that already says what this addition says, written by this connection.
    /// Only additions: an edit names its record and is never a repeat.
    fn repeat_of(
        &self,
        cmd: &EditCommand,
        cs: &ChangeSet,
        me: Uuid,
    ) -> Result<Option<Uuid>, ApiError> {
        let mine = |candidates: Vec<Uuid>| -> Result<Option<Uuid>, ApiError> {
            for id in candidates {
                if self.introduced_by(id, me)? {
                    return Ok(Some(id));
                }
            }
            Ok(None)
        };
        match cmd {
            EditCommand::SaveNote { target, note } if note.id.is_none() => {
                let (notes, _) = self.lists_of(*target).unwrap_or_default();
                mine(
                    notes
                        .iter()
                        .copied()
                        .filter(|id| self.tree.notes.get(id).is_some_and(|n| n.text == note.text))
                        .collect(),
                )
            }
            EditCommand::SaveEvent { owner, event } if event.id.is_none() => {
                let Some(new) = added(cs, |r| match r {
                    Record::Event(e) => Some(e),
                    _ => None,
                }) else {
                    return Ok(None);
                };
                let target = match owner.kind {
                    crate::command::EventOwnerKind::Person => ParticipantRef::Person(owner.id),
                    crate::command::EventOwnerKind::Family => ParticipantRef::Family(owner.id),
                };
                mine(
                    self.tree
                        .events
                        .values()
                        .filter(|e| {
                            e.kind == new.kind
                                && e.date == new.date
                                && e.place == new.place
                                && e.participants.iter().any(|p| p.target == target)
                        })
                        .map(|e| e.common.id)
                        .collect(),
                )
            }
            EditCommand::AddCitation { target, .. } => {
                let Some(new) = added(cs, |r| match r {
                    Record::Citation(c) => Some(c),
                    _ => None,
                }) else {
                    return Ok(None);
                };
                let (_, citations) = self.lists_of(*target).unwrap_or_default();
                mine(
                    citations
                        .iter()
                        .copied()
                        .filter(|id| {
                            self.tree
                                .citations
                                .get(id)
                                .is_some_and(|c| c.source == new.source && c.page == new.page)
                        })
                        .collect(),
                )
            }
            EditCommand::AddNumber {
                person,
                source_file,
                number,
            } => Ok(self
                .find_by_number(source_file, number)
                .filter(|found| found == person)),
            _ => Ok(None),
        }
    }

    /// Step 4 on the plan: nobody gets a second birth, death, burial or cremation, however the
    /// event names them and whether it is new or an own event turned into one. An event that
    /// already was this for them is no second one: a note on it adds no fact.
    fn check_one_of_a_kind(&self, cs: &ChangeSet) -> Result<(), ApiError> {
        for ch in &cs.changes {
            let Some(Record::Event(e)) = &ch.after else {
                continue;
            };
            if !one_of_a_kind(&e.kind) {
                continue;
            }
            let held_before = match &ch.before {
                Some(Record::Event(old)) if old.kind == e.kind => principals(old).collect(),
                _ => Vec::new(),
            };
            for person in principals(e).filter(|p| !held_before.contains(p)) {
                let has_one = self.tree.events.values().any(|other| {
                    other.common.id != e.common.id
                        && other.kind == e.kind
                        && principals(other).any(|p| p == person)
                });
                if has_one {
                    return Err(refused(format!(
                        "{} already has this event, and a person has only one. Add a note giving \
                         both versions and the source.",
                        self.person_name(person)
                    )));
                }
            }
        }
        Ok(())
    }

    /// Step 4 on the plan: a link by birth where the child has a parent in that role, by any route:
    /// as a parent, from the parent's side as a child, or a partner joining a family with
    /// children. Someone whose sex is not known takes whichever role is free, so at most two.
    fn check_parent_roles(&self, cs: &ChangeSet) -> Result<(), ApiError> {
        let changed: HashMap<Uuid, Option<&Family>> = cs
            .changes
            .iter()
            .filter(|c| c.kind == RecordKind::Family)
            .map(|c| {
                let after = match &c.after {
                    Some(Record::Family(f)) => Some(f),
                    _ => None,
                };
                (c.id, after)
            })
            .collect();
        let sex_of = |id: Uuid| -> Option<&Sex> {
            let planned = cs.changes.iter().find_map(|c| match &c.after {
                Some(Record::Person(p)) if c.id == id => Some(&p.sex),
                _ => None,
            });
            planned.or_else(|| self.tree.persons.get(&id).map(|p| &p.sex))
        };
        let children: HashSet<Uuid> = changed
            .values()
            .flatten()
            .flat_map(|f| f.children.iter().map(|c| c.person))
            .collect();
        for child in children {
            let before = role_holders(self.tree.families.values(), child);
            let untouched = self
                .tree
                .families
                .values()
                .filter(|f| !changed.contains_key(&f.common.id));
            let after = role_holders(untouched.chain(changed.values().copied().flatten()), child);
            for (parent, by_birth) in after {
                if !by_birth || before.contains_key(&parent) {
                    continue;
                }
                let clash = before
                    .keys()
                    .any(|held| match (sex_of(parent), sex_of(*held)) {
                        (Some(a), Some(b)) => same_role(a, b),
                        _ => false,
                    });
                if clash || before.len() >= 2 {
                    return Err(refused(format!(
                        "{} already has a parent in that role. A second one cannot be added by \
                         birth. Add a note giving both versions and the source.",
                        self.person_name(child)
                    )));
                }
            }
        }
        Ok(())
    }

    /// Step 5 on one change of the plan.
    fn check_change(&self, ch: &Change, me: Uuid) -> Result<(), ApiError> {
        // A new record takes nothing from anyone.
        let Some(before) = &ch.before else {
            return Ok(());
        };
        if ch.after.is_some() && same_ignoring_stamp(Some(before), ch.after.as_ref()) {
            return Ok(());
        }
        let writers = self.project.last_writers(ch.id)?;
        let rows: Vec<Written> = writers
            .into_iter()
            .map(|(by, _)| by)
            .zip(self.project.rows_of(ch.id)?)
            .collect();
        if record_is_own(&rows, me) {
            return Ok(());
        }
        let refuse = |fact: String| {
            refused(format!(
                "This would change {fact}, on a record you did not create or that someone has \
                 changed since. {ONLY_ADD}"
            ))
        };
        let Some(after) = &ch.after else {
            return Err(refuse("this record".into()));
        };
        // Records that cannot be compared are not let through.
        let (Some((mut b, _)), Some((a, _))) = (stripped(before), stripped(after)) else {
            return Err(refuse("this record".into()));
        };
        free_slots_follow(&mut b, &a);
        Compare { rows: &rows, me }
            .value(&mut Vec::new(), &b, &a, false)
            .map_err(refuse)
    }

    /// Step 6: tries the plan on the tree and takes it off again. The test is whether anyone who
    /// exists before this write counts differently after it, whatever the command was.
    fn check_living(&mut self, cs: &ChangeSet) -> Result<(), ApiError> {
        let mut removed_at = HashMap::new();
        apply_to_tree_tracked(&mut self.tree, cs, &mut removed_at);
        let (effective, _) = self.living_maps();
        apply_to_tree_tracked(&mut self.tree, &cs.inverse(), &mut removed_at);
        let flipped = self
            .tree
            .persons
            .keys()
            .find(|id| self.living.get(id) != effective.get(id));
        match flipped {
            Some(id) => Err(refused(format!(
                "This would change whether {} counts as alive or dead. Only the owner can do \
                 that. Leave a note and list it in your report.",
                self.person_name(*id)
            ))),
            None => Ok(()),
        }
    }
}
