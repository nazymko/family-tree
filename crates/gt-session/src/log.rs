//! The change log as the UI reads it: who changed what, on whom, in readable words.

use crate::command::{HistoryOp, SkippedFact};
use crate::dto::Page;
use crate::error::{ApiError, ErrorCode};
use crate::names::display_name;
use crate::ops::record_in;
use crate::person_detail::event_label;
use crate::revert::common_of;
use crate::session::Session;
use gt_core::model::{ParticipantRef, Role};
use gt_store::{
    Actor, Batch, Change, ChangeOp, ChangeSet, LogFilter, LogGroup, Record, RecordKind, ReviewScope,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use ts_rs::TS;
use uuid::Uuid;

/// Longest log page.
const MAX_LOG_PAGE: usize = 200;
/// Characters of a note's text shown in the log.
const NOTE_PREVIEW: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ActorDto {
    User,
    Agent { id: Uuid, name: String },
    BeforeTracking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BatchRef {
    pub id: Uuid,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogChangeDto {
    /// `person`, `family`, `event`, `place`, `source`, `repository`, `citation`, `media`, `note`.
    pub kind: String,
    pub op: HistoryOp,
    /// The record in readable words: a note's first words, an event's kind and date.
    pub detail: String,
    /// Titles of the sources this change cited.
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogPersonDto {
    pub id: Uuid,
    pub name: String,
    /// False once the person has gone from the tree.
    pub exists: bool,
    /// When the owner reviewed this change for this person.
    #[ts(type = "number | null")]
    pub reviewed: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogRowDto {
    pub group: Uuid,
    #[ts(type = "number")]
    pub ts: i64,
    pub actor: ActorDto,
    pub batch: Option<BatchRef>,
    pub label: String,
    pub changes: Vec<LogChangeDto>,
    pub people: Vec<LogPersonDto>,
    /// A later change undid this one.
    pub reverted: bool,
    /// An agent's change that has not been undone.
    pub can_undo: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BatchDto {
    pub id: Uuid,
    pub title: String,
    pub source_file: Option<String>,
    pub actor_name: String,
    #[ts(type = "number")]
    pub started: i64,
    #[ts(type = "number | null")]
    pub ended: Option<i64>,
    pub report: Option<String>,
    pub groups: u32,
    pub people: u32,
    pub unreviewed: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogFilterDto {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub actor_id: Option<Uuid>,
    /// `user` or `agent`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub actor_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub batch: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub person: Option<Uuid>,
    #[serde(default)]
    pub unreviewed_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ReviewScopeDto {
    Person { id: Uuid },
    Batch { id: Uuid },
    Group { id: Uuid },
}

pub(crate) fn kind_name(k: RecordKind) -> &'static str {
    match k {
        RecordKind::Person => "person",
        RecordKind::Family => "family",
        RecordKind::Event => "event",
        RecordKind::Place => "place",
        RecordKind::Source => "source",
        RecordKind::Repository => "repository",
        RecordKind::Citation => "citation",
        RecordKind::Media => "media",
        RecordKind::Note => "note",
    }
}

/// Every citation a record lists, with those on its names and child links.
fn citations_of(r: &Record) -> Vec<Uuid> {
    let mut out = common_of(r).citations.clone();
    match r {
        Record::Person(p) => out.extend(p.names.iter().flat_map(|n| n.citations.iter().copied())),
        Record::Family(f) => {
            out.extend(f.children.iter().flat_map(|c| c.citations.iter().copied()));
        }
        _ => {}
    }
    out
}

fn preview(text: &str) -> String {
    let t = text.trim();
    if t.chars().count() <= NOTE_PREVIEW {
        return t.to_string();
    }
    let cut: String = t.chars().take(NOTE_PREVIEW).collect();
    format!("{}…", cut.trim_end())
}

impl Session {
    /// The people `cs` is about: persons changed, partners and children of changed families,
    /// principals of changed events (a family taking part counts as its partners). Reads both
    /// sides of every change, so a deleted record's people are still named.
    pub fn people_of(&self, cs: &ChangeSet) -> Vec<Uuid> {
        let mut out: Vec<Uuid> = Vec::new();
        let mut seen: HashSet<Uuid> = HashSet::new();
        let mut add = |id: Uuid| {
            if seen.insert(id) {
                out.push(id);
            }
        };
        let mut cs_families: HashMap<Uuid, Vec<&gt_core::model::Family>> = HashMap::new();
        for ch in &cs.changes {
            for rec in [&ch.before, &ch.after].into_iter().flatten() {
                if let Record::Family(f) = rec {
                    cs_families.entry(f.common.id).or_default().push(f);
                }
            }
        }
        for ch in &cs.changes {
            if ch.kind == RecordKind::Person {
                add(ch.id);
            }
            for rec in [&ch.before, &ch.after].into_iter().flatten() {
                match rec {
                    Record::Family(f) => {
                        f.partner1.iter().chain(&f.partner2).for_each(|p| add(*p));
                        f.children.iter().for_each(|c| add(c.person));
                    }
                    Record::Event(e) => {
                        for p in e.participants.iter().filter(|p| p.role == Role::Principal) {
                            match p.target {
                                ParticipantRef::Person(id) => add(id),
                                ParticipantRef::Family(id) => {
                                    let mut partners = |f: &gt_core::model::Family| {
                                        f.partner1.iter().chain(&f.partner2).for_each(|x| add(*x));
                                    };
                                    if let Some(list) = cs_families.get(&id) {
                                        list.iter().for_each(|f| partners(f));
                                    } else if let Some(f) = self.tree.families.get(&id) {
                                        partners(f);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        out
    }

    /// A record as the log can still find it: in the group, in the tree, or in its history.
    fn find_logged(&self, kind: RecordKind, id: Uuid, group: &[Change]) -> Option<Record> {
        let here = group.iter().find(|c| c.id == id);
        if let Some(r) = here.and_then(|c| c.after.clone()) {
            return Some(r);
        }
        if let Some(r) = record_in(&self.tree, kind, id) {
            return Some(r);
        }
        if let Some(r) = here.and_then(|c| c.before.clone()) {
            return Some(r);
        }
        let rows = self.project.rows_of(id).ok()?;
        rows.into_iter()
            .rev()
            .find_map(|c| c.after.or(c.before))
            .filter(|r| r.kind() == kind)
    }

    fn logged_name(&self, id: Uuid, group: &[Change]) -> String {
        match self.find_logged(RecordKind::Person, id, group) {
            Some(Record::Person(p)) => display_name(&p, self.settings.name_order),
            _ => "Unknown".to_string(),
        }
    }

    fn source_title(&self, id: Uuid, group: &[Change]) -> Option<String> {
        match self.find_logged(RecordKind::Source, id, group)? {
            Record::Source(s) => s.title.or(s.abbreviation),
            _ => None,
        }
    }

    /// One record in readable words.
    pub(crate) fn describe(&self, rec: &Record, group: &[Change]) -> String {
        match rec {
            Record::Person(p) => display_name(p, self.settings.name_order),
            Record::Family(f) => {
                let names: Vec<String> = f
                    .partner1
                    .iter()
                    .chain(&f.partner2)
                    .map(|id| self.logged_name(*id, group))
                    .collect();
                if names.is_empty() {
                    "Family".to_string()
                } else {
                    names.join(" and ")
                }
            }
            Record::Event(e) => {
                let mut parts = vec![event_label(&e.kind)];
                parts.extend(e.date.as_ref().map(|d| d.display()));
                let place = e
                    .place
                    .and_then(|id| self.find_logged(RecordKind::Place, id, group));
                parts.extend(place.and_then(|r| match r {
                    Record::Place(p) => p.names.first().map(|n| n.name.clone()).or(p.raw),
                    _ => None,
                }));
                parts.join(", ")
            }
            Record::Place(p) => p
                .names
                .first()
                .map(|n| n.name.clone())
                .or_else(|| p.raw.clone())
                .unwrap_or_default(),
            Record::Source(s) => s
                .title
                .clone()
                .or_else(|| s.abbreviation.clone())
                .unwrap_or_default(),
            Record::Repository(r) => r.name.clone(),
            Record::Citation(c) => {
                let mut parts: Vec<String> = c
                    .source
                    .and_then(|id| self.source_title(id, group))
                    .into_iter()
                    .collect();
                parts.extend(c.page.clone());
                parts.join(", ")
            }
            Record::Media(m) => m
                .title
                .clone()
                .or_else(|| m.original_name.clone())
                .unwrap_or_else(|| m.path.clone()),
            Record::Note(n) => preview(&n.text),
        }
    }

    /// Titles of the sources a change added: the citation it created, or those it attached.
    fn sources_of_change(&self, ch: &Change, group: &[Change]) -> Vec<String> {
        let Some(rec) = ch.after.as_ref().or(ch.before.as_ref()) else {
            return Vec::new();
        };
        let mut sources: Vec<Uuid> = Vec::new();
        if let Record::Citation(c) = rec {
            sources.extend(c.source);
        }
        let had: Vec<Uuid> = match (&ch.before, &ch.after) {
            (Some(b), Some(_)) => citations_of(b),
            _ => Vec::new(),
        };
        for id in citations_of(rec).into_iter().filter(|c| !had.contains(c)) {
            if let Some(Record::Citation(c)) = self.find_logged(RecordKind::Citation, id, group) {
                sources.extend(c.source);
            }
        }
        let mut out: Vec<String> = Vec::new();
        for s in sources {
            if let Some(title) = self.source_title(s, group)
                && !out.contains(&title)
            {
                out.push(title);
            }
        }
        out
    }

    fn log_row(
        &self,
        g: LogGroup,
        batches: &mut HashMap<Uuid, Option<Batch>>,
    ) -> Result<LogRowDto, ApiError> {
        let is_agent = matches!(g.actor, Some(Actor::Agent { .. }));
        let actor = match g.actor {
            None => ActorDto::BeforeTracking,
            Some(Actor::User) => ActorDto::User,
            Some(Actor::Agent { id, name }) => ActorDto::Agent { id, name },
        };
        let batch = match g.batch {
            None => None,
            Some(id) => {
                let found = match batches.get(&id) {
                    Some(b) => b.clone(),
                    None => {
                        let b = self.project.batch_get(id)?;
                        batches.insert(id, b.clone());
                        b
                    }
                };
                Some(BatchRef {
                    id,
                    title: found.map(|b| b.title).unwrap_or_default(),
                })
            }
        };
        let changes = g
            .changes
            .iter()
            .map(|ch| LogChangeDto {
                kind: kind_name(ch.kind).to_string(),
                op: match (&ch.before, &ch.after) {
                    (None, _) => HistoryOp::Create,
                    (Some(_), Some(_)) => HistoryOp::Update,
                    (Some(_), None) => HistoryOp::Delete,
                },
                detail: ch
                    .after
                    .as_ref()
                    .or(ch.before.as_ref())
                    .map(|r| self.describe(r, &g.changes))
                    .unwrap_or_default(),
                sources: self.sources_of_change(ch, &g.changes),
            })
            .collect();
        let people = g
            .people
            .iter()
            .map(|p| LogPersonDto {
                id: p.id,
                name: self.logged_name(p.id, &g.changes),
                exists: self.tree.persons.contains_key(&p.id),
                reviewed: p.reviewed,
            })
            .collect();
        Ok(LogRowDto {
            group: g.group,
            ts: g.ts,
            actor,
            batch,
            label: g.label,
            changes,
            people,
            reverted: g.reverted,
            can_undo: is_agent && !g.reverted,
        })
    }

    /// One page of the log, newest first; `limit` is clamped to 200.
    pub fn log(
        &self,
        f: LogFilterDto,
        offset: usize,
        limit: usize,
    ) -> Result<Page<LogRowDto>, ApiError> {
        let filter = LogFilter {
            actor_id: f.actor_id,
            actor_kind: f.actor_kind,
            batch: f.batch,
            person: f.person,
            unreviewed_only: f.unreviewed_only,
        };
        let (groups, total) = self
            .project
            .log_page(&filter, offset, limit.min(MAX_LOG_PAGE))?;
        let mut batches = HashMap::new();
        let items = groups
            .into_iter()
            .map(|g| self.log_row(g, &mut batches))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Page {
            total,
            offset: u32::try_from(offset).unwrap_or(u32::MAX),
            items,
            letters: None,
        })
    }

    /// All batches, newest first.
    pub fn batches(&self) -> Result<Vec<BatchDto>, ApiError> {
        self.project
            .batch_list()?
            .into_iter()
            .map(|b| {
                let (groups, people, unreviewed) = self.project.batch_counts(b.id)?;
                Ok(BatchDto {
                    id: b.id,
                    title: b.title,
                    source_file: b.source_file,
                    actor_name: b.actor_name,
                    started: b.started,
                    ended: b.ended,
                    report: b.report,
                    groups,
                    people,
                    unreviewed,
                })
            })
            .collect()
    }

    pub fn mark_reviewed(&mut self, scope: ReviewScopeDto) -> Result<(), ApiError> {
        let (_, now) = (self.clock.0)();
        let scope = match scope {
            ReviewScopeDto::Person { id } => ReviewScope::Person(id),
            ReviewScopeDto::Batch { id } => ReviewScope::Batch(id),
            ReviewScopeDto::Group { id } => ReviewScope::Group(id),
        };
        self.project.mark_reviewed(scope, now)?;
        Ok(())
    }

    /// The people with an agent's change the owner has not reviewed, sorted by id.
    pub fn unreviewed_people(&self) -> Result<Vec<Uuid>, ApiError> {
        let mut ids: Vec<Uuid> = self.project.unreviewed_people()?.into_iter().collect();
        ids.sort();
        Ok(ids)
    }

    /// What agents changed on this person and still stands, newest first, reviewed or not.
    pub fn person_agent_changes(&self, id: Uuid) -> Result<Vec<LogRowDto>, ApiError> {
        let filter = LogFilter {
            person: Some(id),
            actor_kind: Some("agent".into()),
            ..Default::default()
        };
        let (groups, _) = self.project.log_page(&filter, 0, usize::MAX)?;
        let mut batches = HashMap::new();
        groups
            .into_iter()
            .filter(|g| !g.reverted)
            .map(|g| self.log_row(g, &mut batches))
            .collect()
    }

    /// A change that could not be undone in full, in the owner's terms.
    pub(crate) fn skipped_fact(&self, ch: &Change) -> SkippedFact {
        let group = std::slice::from_ref(ch);
        let about = match ch.kind {
            RecordKind::Note => self.note_owner(ch.id),
            _ => {
                let one = ChangeSet {
                    group: Uuid::nil(),
                    reverts: None,
                    label: String::new(),
                    ts: 0,
                    changes: vec![ch.clone()],
                };
                self.people_of(&one).into_iter().next()
            }
        };
        SkippedFact {
            kind: kind_name(ch.kind).to_string(),
            person: about.and_then(|id| self.person_ref(id)),
            detail: ch
                .after
                .as_ref()
                .or(ch.before.as_ref())
                .map(|r| self.describe(r, group))
                .unwrap_or_default(),
        }
    }

    /// Refuses anything but an agent's group: the owner's changes are undone with Ctrl+Z.
    pub(crate) fn refuse_unless_agent(g: &LogGroup) -> Result<(), ApiError> {
        if matches!(g.actor, Some(Actor::Agent { .. })) {
            return Ok(());
        }
        Err(ApiError::new(
            ErrorCode::Invalid,
            "Only an agent's change can be undone from the change log.",
        ))
    }
}

/// `ChangeOp` on the wire.
pub(crate) fn history_op(op: ChangeOp) -> HistoryOp {
    match op {
        ChangeOp::Create => HistoryOp::Create,
        ChangeOp::Update => HistoryOp::Update,
        ChangeOp::Delete => HistoryOp::Delete,
    }
}
