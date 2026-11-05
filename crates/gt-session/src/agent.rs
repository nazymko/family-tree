//! The gate on agent writes. An agent adds to the owner's tree and never changes what is there.
//!
//! `apply_as_agent` is the only way an agent's write reaches the tree. In order:
//!
//! 1. The batch exists in this tree, belongs to this connection, and is not closed.
//! 2. The command is on the list of additive commands.
//! 3. A repeat adds nothing (`AlreadyThere`): of a note, event or citation this connection wrote,
//!    of any identical source, name or family, of a link that is there, of a person under their
//!    number, or of an edit that would say what its record says.
//! 4. Facts a person has only once, one birth family, and one parent in each role.
//! 5. Every change is compared in JSON: on what is not the agent's own, only filling an empty value
//!    and adding to a list pass.
//! 6. The owner's rule: the plan is tried on the tree, and if anyone who existed before the write
//!    would count differently as alive or dead, by the owner's mark or by the rule under it, it is
//!    refused. This reads the result and not the command, because the status follows from dates
//!    and links in ways no list of commands covers.
//! 7. The write is stored under the agent, outside the owner's undo and redo.
//!
//! The checks that need no plan come first, so that the planner's own errors ("both parents are
//! already recorded") do not answer in their place. Planning changes nothing.
//!
//! "Own" means this connection introduced it and nobody else has changed it since. A write that
//! only moves a `changed` stamp, as adding an event does to its person, changes no fact.

use crate::command::{
    CitationArg, EditCommand, EditResult, EventOwnerKind, LivingPatch, OwnerKind, OwnerRef,
};
use crate::dto::ParentRelId;
use crate::error::{ApiError, ErrorCode};
use crate::ops::{agent_number, apply_to_tree_tracked, plan_marker_family, plan_with, rel_of};
use crate::revert::{identity, same_ignoring_stamp, stripped};
use crate::session::Session;
use crate::touched::people_reached;
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
    /// Exactly this is already in the tree; nothing was added.
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
        C::SaveFamily {
            id: None,
            partner1: None,
            partner2: None,
            ..
        } => Err(refused(
            "A family needs someone in it. Add a parent or child through the family's people \
             instead. For brothers and sisters nobody named, use add_sibling_marker with \
             sibling_of.",
        )),
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
        | C::AddNumber { .. }
        | C::AddSiblingMarker { .. } => Ok(()),
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

fn blank(t: &Option<String>) -> bool {
    t.as_deref().is_none_or(|t| t.trim().is_empty())
}

/// Step 2b: an agent never creates a person with no name at all, whatever tool or relation made
/// them. Unknown parents and unnamed brothers and sisters have `add_sibling_marker`.
fn check_named(cmd: &EditCommand) -> Result<(), ApiError> {
    let (EditCommand::CreatePerson { draft } | EditCommand::QuickAdd { draft, .. }) = cmd else {
        return Ok(());
    };
    let nameless = blank(&draft.given)
        && blank(&draft.patronymic)
        && draft.surnames.iter().all(|s| s.trim().is_empty());
    if nameless {
        return Err(refused(
            "A person needs a name: give a given name, a surname or a nickname. To record unknown parents and unnamed brothers and sisters use add_sibling_marker with sibling_of; never create a nameless person.",
        ));
    }
    Ok(())
}

/// A name that holds only a nickname (kind nickname, no given name, surname or patronymic): the
/// command to plan, with the nickname in the given slot, and the nickname.
fn nickname_only(cmd: &EditCommand) -> Option<(EditCommand, String)> {
    let EditCommand::SaveName { person, name } = cmd else {
        return None;
    };
    let nick = name.nickname.as_deref()?.trim();
    let bare = blank(&name.given)
        && blank(&name.patronymic)
        && name.surnames.iter().all(|s| s.trim().is_empty());
    if nick.is_empty() || !bare || !matches!(name.kind, crate::NameKindDto::Nickname) {
        return None;
    }
    let mut name = name.clone();
    name.given = Some(nick.to_string());
    Some((
        EditCommand::SaveName {
            person: *person,
            name,
        },
        nick.to_string(),
    ))
}

/// Takes the nickname out of the given slot again, on the name that was planned for it.
fn unfill_nickname(cs: &mut ChangeSet, nick: &str) {
    for ch in &mut cs.changes {
        let before: Vec<Uuid> = match &ch.before {
            Some(Record::Person(p)) => p
                .names
                .iter()
                .filter(|n| n.given.as_deref() == Some(nick))
                .map(|n| n.id)
                .collect(),
            _ => Vec::new(),
        };
        if let Some(Record::Person(p)) = &mut ch.after {
            for n in &mut p.names {
                if n.given.as_deref() == Some(nick)
                    && n.nickname.as_deref() == Some(nick)
                    && !before.contains(&n.id)
                {
                    n.given = None;
                }
            }
        }
    }
}

/// The people a command would give something to, or put in a relationship.
fn people_named(cmd: &EditCommand) -> Vec<Uuid> {
    use EditCommand as C;
    match cmd {
        C::UpdatePerson { id, .. } => vec![*id],
        C::SaveName { person, .. } | C::AddNumber { person, .. } => vec![*person],
        C::SaveNote { target, .. } | C::AddCitation { target, .. }
            if matches!(target.kind, OwnerKind::Person) =>
        {
            vec![target.id]
        }
        C::SaveEvent { owner, .. } if matches!(owner.kind, EventOwnerKind::Person) => {
            vec![owner.id]
        }
        C::QuickAdd { anchor, .. } => vec![*anchor],
        C::LinkExisting { anchor, person, .. } => vec![*anchor, *person],
        C::LinkChild { child, .. } => vec![*child],
        C::SaveFamily {
            partner1, partner2, ..
        } => [*partner1, *partner2].into_iter().flatten().collect(),
        _ => Vec::new(),
    }
}

/// The record an edit names, for telling the agent it already says this.
fn edited_record(cmd: &EditCommand) -> Option<Uuid> {
    use EditCommand as C;
    match cmd {
        C::SaveNote { note, .. } => note.id,
        C::SaveEvent { event, .. } => event.id,
        C::SaveName { name, .. } => name.id,
        C::UpdateCitation { id, .. } | C::UpdatePerson { id, .. } => Some(*id),
        _ => None,
    }
}

/// Step 4 on the plan: a family has no birth or death of its own. What the owner's data already
/// holds stays, so only an event that is new, or new as this kind, counts.
fn check_event_owners(cs: &ChangeSet) -> Result<(), ApiError> {
    for ch in &cs.changes {
        let Some(Record::Event(e)) = &ch.after else {
            continue;
        };
        let of_family = e
            .participants
            .iter()
            .any(|p| matches!(p.target, ParticipantRef::Family(_)));
        let same_kind = matches!(&ch.before, Some(Record::Event(old)) if old.kind == e.kind);
        if of_family && !same_kind && !fits_a_family(&e.kind) {
            return Err(refused(
                "That happens to a person, not a family: add it to the person. A family's events \
                 are marriage, marriage_banns, marriage_contract, marriage_license, \
                 marriage_settlement, engagement, divorce, divorce_filed, annulment, separation, \
                 census, residence, emigration, immigration, or custom:<your words>.",
            ));
        }
    }
    Ok(())
}

fn principals(e: &Event) -> impl Iterator<Item = Uuid> + '_ {
    e.participants.iter().filter_map(|p| match p.target {
        ParticipantRef::Person(id) if p.role == Role::Principal => Some(id),
        _ => None,
    })
}

/// What a couple can have happen to it. The rest happens to a person.
fn fits_a_family(k: &EventKind) -> bool {
    matches!(
        k,
        EventKind::Marriage
            | EventKind::MarriageBanns
            | EventKind::MarriageContract
            | EventKind::MarriageLicense
            | EventKind::MarriageSettlement
            | EventKind::Engagement
            | EventKind::Divorce
            | EventKind::DivorceFiled
            | EventKind::Annulment
            | EventKind::Separation
            | EventKind::Census
            | EventKind::Residence
            | EventKind::Emigration
            | EventKind::Immigration
            | EventKind::Custom(_)
    )
}

fn one_of_a_kind(k: &EventKind) -> bool {
    matches!(
        k,
        EventKind::Birth | EventKind::Death | EventKind::Burial | EventKind::Cremation
    )
}

fn kind_word(k: &EventKind) -> &'static str {
    match k {
        EventKind::Birth => "birth",
        EventKind::Death => "death",
        EventKind::Burial => "burial",
        EventKind::Cremation => "cremation",
        _ => "event",
    }
}

/// The new record of this kind in a plan.
fn added<T>(cs: &ChangeSet, f: impl Fn(&Record) -> Option<&T>) -> Option<&T> {
    cs.changes
        .iter()
        .filter(|c| c.before.is_none())
        .find_map(|c| c.after.as_ref().and_then(&f))
}

/// By birth, or not known to be anything else. A step, adoptive or foster link is neither.
fn birth_like(rel: &ParentRel) -> bool {
    matches!(rel, ParentRel::Birth | ParentRel::Unknown)
}

/// The parents who hold a role for `child`. A step, adoptive or foster parent does not hold one.
fn role_holders<'a>(families: impl Iterator<Item = &'a Family>, child: Uuid) -> HashSet<Uuid> {
    let mut out = HashSet::new();
    for f in families {
        for link in f.children.iter().filter(|c| c.person == child) {
            for (slot, rel) in [(f.partner1, &link.rel1), (f.partner2, &link.rel2)] {
                if let Some(parent) = slot
                    && birth_like(rel)
                {
                    out.insert(parent);
                }
            }
        }
    }
    out
}

/// Whether this link makes the family the child's birth family: through a parent who holds a role,
/// or, with no parent recorded yet, by what the link says.
fn is_birth_link(f: &Family, link: &ChildLink) -> bool {
    if f.partner1.is_none() && f.partner2.is_none() {
        return birth_like(&link.rel1) || birth_like(&link.rel2);
    }
    (f.partner1.is_some() && birth_like(&link.rel1))
        || (f.partner2.is_some() && birth_like(&link.rel2))
}

fn has_birth_link(f: &Family, child: Uuid) -> bool {
    f.children
        .iter()
        .any(|l| l.person == child && is_birth_link(f, l))
}

/// Whether a link says what was asked for: the same kind, with birth and unknown counted as one.
fn says_link(have: &ParentRel, asked: Option<ParentRelId>) -> bool {
    match asked.map(|rel| rel_of(rel, None)) {
        None | Some(ParentRel::Birth | ParentRel::Unknown) => birth_like(have),
        Some(kind) => *have == kind,
    }
}

/// How this family's child is linked to `partner`, one of its partners.
fn link_to<'a>(f: &Family, link: &'a ChildLink, partner: Uuid) -> Option<&'a ParentRel> {
    if f.partner1 == Some(partner) {
        Some(&link.rel1)
    } else if f.partner2 == Some(partner) {
        Some(&link.rel2)
    } else {
        None
    }
}

/// What a record says, as a whole value, so that a new field is compared without anyone having to
/// remember it. Left out: its id and stamp, and what is attached to it later, since a citation on
/// an event does not make the event say something else. Who took part is compared whatever the
/// order they were named in, and a text whatever the space around it.
fn fact_value(r: &Record) -> Option<Value> {
    let (mut v, _) = stripped(r)?;
    let body = v.as_object_mut()?.values_mut().next()?;
    let common = body.get_mut("common")?.as_object_mut()?;
    for attached in ["id", "citations", "notes", "media", "ext_ids", "unknown"] {
        common.remove(attached);
    }
    if let Some(people) = body.get_mut("participants").and_then(Value::as_array_mut) {
        people.sort_by_key(Value::to_string);
    }
    if let Some(Value::String(text)) = body.get_mut("text") {
        *text = text.trim().to_string();
    }
    Some(v)
}

fn same_fact(a: &Record, b: &Record) -> bool {
    match (fact_value(a), fact_value(b)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// `fact_value` for a name, which is no record of its own. Its preferred flag is left out too: an
/// agent's name never has it.
fn name_value(n: &Name) -> Option<Value> {
    let mut v = serde_json::to_value(n).ok()?;
    let body = v.as_object_mut()?;
    for attached in ["id", "assertion", "raw", "citations", "notes", "unknown"] {
        body.remove(attached);
    }
    Some(v)
}

/// The families changed by a plan, each with what it is after (`None`: removed).
fn changed_families(cs: &ChangeSet) -> HashMap<Uuid, Option<&Family>> {
    cs.changes
        .iter()
        .filter(|c| c.kind == RecordKind::Family)
        .map(|c| {
            let after = match &c.after {
                Some(Record::Family(f)) => Some(f),
                _ => None,
            };
            (c.id, after)
        })
        .collect()
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
        self.apply_as_agent_reaching(cmd, ctx, batch)
            .map(|(write, _)| write)
    }

    /// [`Session::apply_as_agent`], and whom the write reached: the changed ids and the people they
    /// belonged to before it or belong to after it, so that someone it unlinked is told too. Empty
    /// when nothing was written. Pass the list to `Host::changed` once the session lock is released.
    pub fn apply_as_agent_reaching(
        &mut self,
        cmd: EditCommand,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        self.gated(cmd, None, ctx, batch)
    }

    /// [`Session::apply_as_agent_reaching`] for a new person with their number in a source file:
    /// one write, so that no failure can leave them without it. With someone under that number
    /// already, that is who this is, and nothing is written.
    pub fn apply_numbered_as_agent(
        &mut self,
        cmd: EditCommand,
        source_file: &str,
        number: &str,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        let Some(id) = agent_number(source_file, number) else {
            return Err(refused("Give both the source file and the number."));
        };
        if !matches!(
            cmd,
            EditCommand::CreatePerson { .. } | EditCommand::QuickAdd { .. }
        ) {
            return Err(refused("Only a new person is added with a number."));
        }
        if let Some(record) = self.find_by_number(source_file, number) {
            return Ok((AgentWrite::AlreadyThere { record }, Vec::new()));
        }
        self.gated(cmd, Some(id), ctx, batch)
    }

    /// Takes away a family this connection made in this open batch, when nothing hangs on it: no
    /// children, events, notes, citations or media. A refused link can leave such a family behind.
    /// Anything else is refused. It goes through the same checks and the same log as any write.
    pub fn remove_own_empty_family(
        &mut self,
        family: Uuid,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        let Some(fam) = self.tree.families.get(&family) else {
            return Err(ApiError::not_found("family", family));
        };
        let mine = self.introduced_by(family, ctx.id)?
            && self.project.created_in_batch(family)? == Some(batch);
        if !mine {
            return Err(refused(
                "Only a family you made yourself in this open batch can be removed. Leave this one and list it in your report.",
            ));
        }
        let has_event = self.tree.events.values().any(|e| {
            e.participants
                .iter()
                .any(|p| p.target == ParticipantRef::Family(family))
        });
        if !fam.children.is_empty()
            || has_event
            || !fam.common.notes.is_empty()
            || !fam.common.citations.is_empty()
            || !fam.common.media.is_empty()
        {
            return Err(refused(
                "This family has children, events, notes or citations, so it is not empty and stays. Leave it and list it in your report.",
            ));
        }
        let (_, now) = (self.clock.0)();
        let cmd = EditCommand::DeleteFamily { id: family };
        let (cs, focus) = plan_with(&self.tree, &cmd, now, self.day_order(), None)?;
        self.pass_and_store(&cmd, cs, focus, ctx, batch)
    }

    /// Adds the entry for brothers and sisters nobody named beside `person`: in their birth family,
    /// or in a new family with no partners when they have none, which is one write with its marker.
    pub fn add_sibling_marker_beside(
        &mut self,
        person: Uuid,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        if !self.tree.persons.contains_key(&person) {
            return Err(ApiError::not_found("person", person));
        }
        self.check_no_marker(std::slice::from_ref(&person))?;
        let births: Vec<Uuid> = self
            .tree
            .families
            .values()
            .filter(|f| has_birth_link(f, person))
            .map(|f| f.common.id)
            .collect();
        match births.as_slice() {
            [] => {
                let (_, now) = (self.clock.0)();
                let (family, cs, focus) =
                    plan_marker_family(&self.tree, person, now, self.day_order());
                let cmd = EditCommand::AddSiblingMarker { family };
                self.pass_and_store(&cmd, cs, focus, ctx, batch)
            }
            [family] => self.gated(
                EditCommand::AddSiblingMarker { family: *family },
                None,
                ctx,
                batch,
            ),
            _ => Err(refused(format!(
                "{} has more than one birth family. Say which with family: get_relatives gives \
                 their ids.",
                self.person_name(person)
            ))),
        }
    }

    /// Everything after the batch. `number` goes on the person the write creates.
    fn gated(
        &mut self,
        cmd: EditCommand,
        number: Option<ExternalId>,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        check_allowed(&cmd)?;
        check_named(&cmd)?;
        let cmd = self.keep_preferred(cmd);
        self.check_no_marker(&people_named(&cmd))?;
        // Before the checks below: they would answer a repeat with a conflict that is not there.
        if let Some(record) = self.link_repeat(&cmd) {
            return Ok((AgentWrite::AlreadyThere { record }, Vec::new()));
        }
        self.check_links_and_numbers(&cmd)?;
        // The planner refuses a second marker; for an agent's rerun that is a repeat, not an error.
        if let EditCommand::AddSiblingMarker { family } = &cmd
            && let Some(record) = self.marker_of(*family)
        {
            return Ok((AgentWrite::AlreadyThere { record }, Vec::new()));
        }
        let (_, now) = (self.clock.0)();
        // No last citation: that is the owner's, and `check_allowed` refused asking for it.
        // The owner's editor wants a given name or a surname on every name, and the planner is the
        // one code path for both. A name that is only a nickname is planned with the nickname in
        // the given slot, which the planner accepts, and the slot is emptied again below: the
        // agent's record says exactly what the agent gave, with no invented given name.
        let nick = nickname_only(&cmd);
        let planned = match &nick {
            Some((with_given, _)) => with_given,
            None => &cmd,
        };
        let (mut cs, focus) = plan_with(&self.tree, planned, now, self.day_order(), None)?;
        if let Some((_, nick)) = &nick {
            unfill_nickname(&mut cs, nick);
        }
        if let Some(number) = number {
            let new = cs.changes.iter_mut().find(|c| Some(c.id) == focus);
            if let Some(Change {
                after: Some(Record::Person(person)),
                ..
            }) = new
            {
                person.common.ext_ids.push(number);
            }
        }
        if let Some(record) = self.repeat_of(&cmd, &cs, ctx.id)? {
            return Ok((AgentWrite::AlreadyThere { record }, Vec::new()));
        }
        // Saying again what a record says is no write, and leaves no row in the log.
        let says_the_same = |c: &Change| same_ignoring_stamp(c.before.as_ref(), c.after.as_ref());
        if let Some(first) = cs.changes.first()
            && cs.changes.iter().all(says_the_same)
        {
            let record = edited_record(&cmd).unwrap_or(first.id);
            return Ok((AgentWrite::AlreadyThere { record }, Vec::new()));
        }
        self.pass_and_store(&cmd, cs, focus, ctx, batch)
    }

    /// The checks on a plan, in the order of the module comment, then the write.
    fn pass_and_store(
        &mut self,
        cmd: &EditCommand,
        cs: ChangeSet,
        focus: Option<Uuid>,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        check_event_owners(&cs)?;
        self.check_one_of_a_kind(&cs)?;
        self.check_birth_family(&cs)?;
        self.check_parent_roles(&cs)?;
        self.check_partner_is_no_parent(cmd, &cs)?;
        for ch in &cs.changes {
            self.check_change(ch, ctx.id)?;
        }
        self.check_living(&cs)?;
        let mut reached = Vec::new();
        if !cs.changes.is_empty() {
            let actor = Actor::Agent {
                id: ctx.id,
                name: ctx.name.clone(),
            };
            let changed: Vec<Uuid> = cs.changes.iter().map(|c| c.id).collect();
            reached = people_reached(&self.tree, &changed);
            self.commit(&cs, actor, Some(batch))?;
            for id in people_reached(&self.tree, &changed) {
                if !reached.contains(&id) {
                    reached.push(id);
                }
            }
        }
        let result = self.result(&cs, cs.label.clone(), focus);
        Ok((AgentWrite::Done(result), reached))
    }

    fn marker_of(&self, family: Uuid) -> Option<Uuid> {
        let family = self.tree.families.get(&family)?;
        family
            .children
            .iter()
            .map(|c| c.person)
            .find(|id| self.tree.persons.get(id).is_some_and(|p| p.sibling_marker))
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

    /// Whether any of `people` is the entry for brothers and sisters nobody named. The planner has
    /// its own refusals for this, which speak to the owner, who can delete the entry.
    fn check_no_marker(&self, people: &[Uuid]) -> Result<(), ApiError> {
        let is_marker = |id: &Uuid| self.tree.persons.get(id).is_some_and(|p| p.sibling_marker);
        if people.iter().any(is_marker) {
            return Err(refused(
                "That entry stands for brothers and sisters nobody named, and holds nothing else. \
                 Write on a named person or the family instead.",
            ));
        }
        Ok(())
    }

    /// Step 3 for a link: the person it would link, when the tree already says the same.
    fn link_repeat(&self, cmd: &EditCommand) -> Option<Uuid> {
        use crate::command::Relation as R;
        let families = || self.tree.families.values();
        let child_of = |f: &Family, child: Uuid, parent: Uuid, asked| {
            f.children
                .iter()
                .filter(|l| l.person == child)
                .filter_map(|l| link_to(f, l, parent))
                .any(|have| says_link(have, asked))
        };
        match cmd {
            EditCommand::LinkExisting {
                anchor,
                relation,
                person,
                family,
                parent_rel,
                ..
            } => {
                let chosen = |f: &&Family| family.is_none_or(|id| f.common.id == id);
                let there = match relation {
                    R::Father | R::Mother => {
                        families().any(|f| child_of(f, *anchor, *person, *parent_rel))
                    }
                    R::Child => families()
                        .filter(chosen)
                        .any(|f| child_of(f, *person, *anchor, *parent_rel)),
                    R::Partner => families().any(|f| {
                        (f.partner1 == Some(*anchor) && f.partner2 == Some(*person))
                            || (f.partner1 == Some(*person) && f.partner2 == Some(*anchor))
                    }),
                    R::Sibling => families().any(|f| {
                        f.children.iter().any(|l| l.person == *anchor)
                            && f.children.iter().any(|l| l.person == *person)
                    }),
                };
                there.then_some(*person)
            }
            EditCommand::LinkChild {
                family, child, rel, ..
            } => {
                let f = self.tree.families.get(family)?;
                let link = f.children.iter().find(|l| l.person == *child)?;
                let have = if f.partner1.is_some() {
                    &link.rel1
                } else {
                    &link.rel2
                };
                says_link(have, Some(rel.0)).then_some(*child)
            }
            _ => None,
        }
    }

    /// Step 4, the checks the command alone can answer.
    fn check_links_and_numbers(&self, cmd: &EditCommand) -> Result<(), ApiError> {
        use crate::command::Relation as R;
        match cmd {
            EditCommand::QuickAdd {
                anchor,
                relation: relation @ (R::Father | R::Mother),
                parent_rel: None | Some(ParentRelId::Birth | ParentRelId::Unknown),
                ..
            }
            | EditCommand::LinkExisting {
                anchor,
                relation: relation @ (R::Father | R::Mother),
                parent_rel: None | Some(ParentRelId::Birth | ParentRelId::Unknown),
                ..
            } => {
                let holders = role_holders(self.tree.families.values(), *anchor);
                let (role, word) = match relation {
                    R::Father => (Sex::Male, "father"),
                    _ => (Sex::Female, "mother"),
                };
                let taken = holders.iter().any(|id| {
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
            // Whatever the link: `check_birth_family` covers the other routes, for birth links.
            EditCommand::LinkChild { family, child, .. } => {
                let elsewhere = self
                    .tree
                    .families
                    .values()
                    .any(|f| f.common.id != *family && has_birth_link(f, *child));
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

    /// Step 3: the record that already says what this addition says, in every field the agent can
    /// give (`fact_value`). Notes, events and citations count when this connection wrote them; a
    /// source, name or family counts whoever wrote it, since a copy of it adds nothing. Only
    /// additions: an edit names its record and is never a repeat.
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
                let Some(new) = added(cs, |r| match r {
                    Record::Note(n) => Some(n),
                    _ => None,
                }) else {
                    return Ok(None);
                };
                let new = Record::Note(new.clone());
                let (notes, _) = self.lists_of(*target).unwrap_or_default();
                mine(
                    notes
                        .iter()
                        .copied()
                        .filter(|id| {
                            self.tree
                                .notes
                                .get(id)
                                .is_some_and(|n| same_fact(&Record::Note(n.clone()), &new))
                        })
                        .collect(),
                )
            }
            EditCommand::SaveEvent { event, .. } if event.id.is_none() => {
                let Some(new) = added(cs, |r| match r {
                    Record::Event(e) => Some(e),
                    _ => None,
                }) else {
                    return Ok(None);
                };
                let whole = Record::Event(new.clone());
                mine(
                    self.tree
                        .events
                        .values()
                        .filter(|e| {
                            e.kind == new.kind
                                && e.date == new.date
                                && same_fact(&Record::Event((*e).clone()), &whole)
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
                let new = Record::Citation(new.clone());
                let (_, citations) = self.lists_of(*target).unwrap_or_default();
                mine(
                    citations
                        .iter()
                        .copied()
                        .filter(|id| {
                            self.tree
                                .citations
                                .get(id)
                                .is_some_and(|c| same_fact(&Record::Citation(c.clone()), &new))
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
            EditCommand::SaveSource { source } if source.id.is_none() => {
                let Some(new) = added(cs, |r| match r {
                    Record::Source(x) => Some(x),
                    _ => None,
                }) else {
                    return Ok(None);
                };
                let whole = Record::Source(new.clone());
                Ok(self
                    .tree
                    .sources
                    .values()
                    .find(|x| {
                        x.title == new.title && same_fact(&Record::Source((*x).clone()), &whole)
                    })
                    .map(|x| x.common.id))
            }
            EditCommand::SaveName { person, name } if name.id.is_none() => {
                let Some(existing) = self.tree.persons.get(person) else {
                    return Ok(None);
                };
                let planned = cs.changes.iter().find_map(|c| match &c.after {
                    Some(Record::Person(p)) if c.id == *person => Some(p),
                    _ => None,
                });
                let new = planned.and_then(|p| {
                    p.names
                        .iter()
                        .find(|n| !existing.names.iter().any(|old| old.id == n.id))
                });
                let Some(new) = new.and_then(name_value) else {
                    return Ok(None);
                };
                Ok(existing
                    .names
                    .iter()
                    .find(|old| name_value(old).as_ref() == Some(&new))
                    .map(|old| old.id))
            }
            EditCommand::SaveFamily { id: None, .. } => {
                let Some(new) = added(cs, |r| match r {
                    Record::Family(f) => Some(f),
                    _ => None,
                }) else {
                    return Ok(None);
                };
                let one_partner = new.partner1.is_none() || new.partner2.is_none();
                for f in self.tree.families.values() {
                    let same_people = (f.partner1 == new.partner1 && f.partner2 == new.partner2)
                        || (f.partner1 == new.partner2 && f.partner2 == new.partner1);
                    if !same_people || f.kind != new.kind {
                        continue;
                    }
                    // One person with children is someone's parent, not yet a couple: handing out
                    // such a family would invite a partner into the owner's children's parents.
                    if !one_partner
                        || f.children.is_empty()
                        || self.introduced_by(f.common.id, me)?
                    {
                        return Ok(Some(f.common.id));
                    }
                }
                Ok(None)
            }
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
                        "{} already has a {}, and a person has only one. Add a note giving both \
                         versions and the source.",
                        self.person_name(person),
                        kind_word(&e.kind)
                    )));
                }
            }
        }
        Ok(())
    }

    /// Step 4 on the plan: a child gets one birth family, by any route. A child already in two in
    /// the owner's data stays there: only a link that was not a birth link before counts. So does
    /// a birth parent's new couple with a step-parent, which brings the child no new parent.
    fn check_birth_family(&self, cs: &ChangeSet) -> Result<(), ApiError> {
        let changed = changed_families(cs);
        for ch in &cs.changes {
            let Some(Record::Family(f)) = &ch.after else {
                continue;
            };
            let was = match &ch.before {
                Some(Record::Family(old)) => Some(old),
                _ => None,
            };
            for link in f.children.iter().filter(|l| is_birth_link(f, l)) {
                if was.is_some_and(|old| has_birth_link(old, link.person)) {
                    continue;
                }
                let held = role_holders(self.tree.families.values(), link.person);
                let parents: Vec<Uuid> = [(f.partner1, &link.rel1), (f.partner2, &link.rel2)]
                    .into_iter()
                    .filter_map(|(slot, rel)| slot.filter(|_| birth_like(rel)))
                    .collect();
                if !parents.is_empty() && parents.iter().all(|p| held.contains(p)) {
                    continue;
                }
                let untouched = self
                    .tree
                    .families
                    .values()
                    .filter(|o| !changed.contains_key(&o.common.id));
                let elsewhere = untouched
                    .chain(changed.values().copied().flatten())
                    .any(|o| o.common.id != f.common.id && has_birth_link(o, link.person));
                if elsewhere {
                    return Err(refused(format!(
                        "{} already has a birth family. Add a note giving both versions and the \
                         source, or link as a step, adoptive or foster parent if that is what the \
                         source says.",
                        self.person_name(link.person)
                    )));
                }
            }
        }
        Ok(())
    }

    /// Step 4 on the plan: a parent by birth or with the link unknown, where the child has a parent
    /// in that role, by any route: as a parent, from the parent's side as a child, a partner
    /// joining a family with children, or a change of sex afterwards. Someone whose sex is not
    /// known takes whichever role is free, so at most two. What the owner's data already holds
    /// stays: only a clash this plan brings counts.
    fn check_parent_roles(&self, cs: &ChangeSet) -> Result<(), ApiError> {
        let changed = changed_families(cs);
        let sex_before = |id: Uuid| self.tree.persons.get(&id).map(|p| &p.sex);
        let sex_after = |id: &Uuid| {
            let planned = cs.changes.iter().find_map(|c| match &c.after {
                Some(Record::Person(p)) if c.id == *id => Some(&p.sex),
                _ => None,
            });
            planned.or_else(|| sex_before(*id))
        };
        let after_families = || {
            self.tree
                .families
                .values()
                .filter(|f| !changed.contains_key(&f.common.id))
                .chain(changed.values().copied().flatten())
        };
        let new_sex: HashSet<Uuid> = cs
            .changes
            .iter()
            .filter_map(|c| match (&c.before, &c.after) {
                (Some(Record::Person(b)), Some(Record::Person(a))) if b.sex != a.sex => Some(c.id),
                _ => None,
            })
            .collect();
        let of_new_sex = after_families().filter(|f| {
            [f.partner1, f.partner2]
                .into_iter()
                .flatten()
                .any(|p| new_sex.contains(&p))
        });
        let children: HashSet<Uuid> = changed
            .values()
            .copied()
            .flatten()
            .chain(of_new_sex)
            .flat_map(|f| f.children.iter().map(|c| c.person))
            .collect();
        for child in children {
            let before = role_holders(self.tree.families.values(), child);
            let after = role_holders(after_families(), child);
            let clashes_now = |a: &Uuid, b: &Uuid| match (sex_after(a), sex_after(b)) {
                (Some(x), Some(y)) => same_role(x, y),
                _ => false,
            };
            let clashed_before = |a: &Uuid, b: &Uuid| {
                before.contains(a)
                    && before.contains(b)
                    && match (sex_before(*a), sex_before(*b)) {
                        (Some(x), Some(y)) => same_role(x, y),
                        _ => false,
                    }
            };
            let too_many = before.len() >= 2 && after.iter().any(|p| !before.contains(p));
            // Only a role someone held, or a sex that changed: a child new to a couple takes no
            // role from anyone, whatever sexes that couple has.
            let involved = |p: &Uuid| before.contains(p) || new_sex.contains(p);
            let clash = after.iter().any(|a| {
                after.iter().any(|b| {
                    a != b
                        && (involved(a) || involved(b))
                        && clashes_now(a, b)
                        && !clashed_before(a, b)
                })
            });
            if too_many || clash {
                return Err(refused(format!(
                    "{} already has a parent in that role. A second one cannot be added by birth. \
                     Add a note giving both versions and the source.",
                    self.person_name(child)
                )));
            }
        }
        Ok(())
    }

    /// Step 4 on the plan: joining a family as a partner says nothing about its children, but the
    /// slot would make them that person's children by birth. Whichever way the agent got the
    /// family's id, that has to be said outright. Adding a parent to a child does say it.
    fn check_partner_is_no_parent(
        &self,
        cmd: &EditCommand,
        cs: &ChangeSet,
    ) -> Result<(), ApiError> {
        use crate::command::Relation as R;
        let (EditCommand::QuickAdd {
            relation: R::Partner,
            ..
        }
        | EditCommand::LinkExisting {
            relation: R::Partner,
            ..
        }) = cmd
        else {
            return Ok(());
        };
        for ch in &cs.changes {
            let (Some(Record::Family(old)), Some(Record::Family(f))) = (&ch.before, &ch.after)
            else {
                continue;
            };
            for link in &f.children {
                let before = role_holders(std::iter::once(old), link.person);
                let after = role_holders(std::iter::once(f), link.person);
                if after.iter().any(|p| !before.contains(p)) {
                    let child = self.person_name(link.person);
                    return Err(refused(format!(
                        "A partner in this family would become a parent of {child} by birth. If \
                         the source says so, add them as a parent of {child}, or as a step \
                         parent. Otherwise add them as a partner in a family of their own, or \
                         add a note."
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
    /// exists before this write counts differently after it, whatever the command was. Both maps:
    /// the owner's mark hides what the rule says, and taking the mark off would show the change.
    /// Both sides are read now, so a date the rule passes overnight is not blamed on this write.
    // A birth date just inside the age limit that later passes it is time passing, not this write.
    fn check_living(&mut self, cs: &ChangeSet) -> Result<(), ApiError> {
        let (effective_before, computed_before) = self.living_maps();
        let mut removed_at = HashMap::new();
        apply_to_tree_tracked(&mut self.tree, cs, &mut removed_at);
        let (effective, computed) = self.living_maps();
        apply_to_tree_tracked(&mut self.tree, &cs.inverse(), &mut removed_at);
        // Someone the owner marked dead may turn out dead by the rule too: that changes no status,
        // now or once the mark comes off. A death date is what transcripts most often add.
        let agrees_with_mark = |id: &Uuid| {
            computed_before.get(id) == Some(&true)
                && computed.get(id) == Some(&false)
                && self
                    .tree
                    .persons
                    .get(id)
                    .is_some_and(|p| p.living_override == Some(LivingOverride::Deceased))
        };
        let flipped = self.tree.persons.keys().find(|id| {
            effective_before.get(id) != effective.get(id)
                || (computed_before.get(id) != computed.get(id) && !agrees_with_mark(id))
        });
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
