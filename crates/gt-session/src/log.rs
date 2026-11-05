//! The change log as the UI reads it: who changed what, on whom, in readable words.

use crate::command::{HistoryOp, OwnerKind, OwnerRef, SkippedFact};
use crate::dto::{CitationDto, NoteDto, Page};
use crate::error::{ApiError, ErrorCode};
use crate::field_diff::{FieldChangeDto, LIST_SEP, REMOVED, Words, record_field_changes};
use crate::names::display_name;
use crate::ops::record_in;
use crate::person_detail::{event_kind_id, event_label};
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
/// Characters of one field value in the change log's pages.
const FIELD_PREVIEW: usize = 200;

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

/// A record in the log as data, so that the UI can word its fixed parts in its own language.
/// Everything here is the user's or the agent's own text, except the event `label` fallback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LogFactDto {
    /// Text as written: a person's name, a note's first words, a source's title.
    Text { text: String },
    /// The names of the partners; empty when the family has none.
    Family { partners: Vec<String> },
    /// `event` is the kind's id (`birth`, `custom:Pilgrimage`), `label` its English name.
    Event {
        event: String,
        label: String,
        date: Option<String>,
        place: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogChangeDto {
    /// `person`, `family`, `event`, `place`, `source`, `repository`, `citation`, `media`, `note`.
    pub kind: String,
    /// The record's id, to open it.
    pub id: Uuid,
    pub op: HistoryOp,
    /// The record in readable words: a note's first words, an event's kind and date.
    pub detail: String,
    /// The same record as data, for the UI to word in its own language.
    pub fact: LogFactDto,
    /// Titles of the sources this change cited.
    pub sources: Vec<String>,
    /// Which fields went from what to what, with ids in words. In the change log's pages a long
    /// value is cut short; the person page's list has it whole.
    pub fields: Vec<FieldChangeDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogPersonDto {
    pub id: Uuid,
    pub name: String,
    /// False once the person has gone from the tree.
    pub exists: bool,
    /// The entry for unnamed brothers and sisters, which has no person page.
    pub sibling_marker: bool,
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
    /// Who ran the batch, by the id that never changes. Labels go by this, not by the name.
    pub actor_id: Uuid,
    pub actor_name: String,
    #[ts(type = "number")]
    pub started: i64,
    #[ts(type = "number | null")]
    pub ended: Option<i64>,
    pub report: Option<String>,
    pub groups: u32,
    pub people: u32,
    pub unreviewed: u32,
    /// Changes of this batch that have not been undone.
    pub standing: u32,
}

/// An agent that has written to the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogAgentDto {
    pub id: Uuid,
    pub name: String,
}

/// A note or citation and the record that holds it, for opening it in its editor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecordHomeDto {
    pub owner: OwnerRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub note: Option<NoteDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub citation: Option<CitationDto>,
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

/// Characters of context kept before the first difference when a long value is clipped.
const CLIP_LEAD: usize = 40;

/// `s` from char `start`, at most `FIELD_PREVIEW` chars, with an ellipsis on a side that was cut.
fn window(s: &str, start: usize) -> String {
    let mut rest = s.chars().skip(start);
    let cut: String = rest.by_ref().take(FIELD_PREVIEW).collect();
    let more = rest.next().is_some();
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.push_str(if more { cut.trim_end() } else { &cut });
    if more {
        out.push('…');
    }
    out
}

/// A field's Before and After cut to `FIELD_PREVIEW` characters each. When both are there and
/// either is too long, both start at the same place: just before they first differ, so an edit
/// far into a long text still shows. A side that was cut carries an ellipsis there.
fn clip_fields(fields: &mut [FieldChangeDto]) {
    for f in fields {
        let long = |v: &Option<String>| {
            v.as_ref()
                .is_some_and(|s| s.chars().count() > FIELD_PREVIEW)
        };
        if !long(&f.before) && !long(&f.after) {
            continue;
        }
        let start = match (&f.before, &f.after) {
            (Some(b), Some(a)) => {
                let same = b.chars().zip(a.chars()).take_while(|(x, y)| x == y).count();
                same.saturating_sub(CLIP_LEAD)
            }
            _ => 0,
        };
        for s in [&mut f.before, &mut f.after].into_iter().flatten() {
            *s = window(s, start);
        }
    }
}

/// Reads ids for field changes: from the group, the tree or the history.
pub(crate) struct LogWords<'a> {
    pub session: &'a Session,
    pub group: &'a [Change],
}

impl LogWords<'_> {
    fn find(&self, kind: RecordKind, id: Uuid) -> Option<Record> {
        self.session.find_logged(kind, id, self.group)
    }

    /// The record in words; [`REMOVED`] when it cannot be found any more.
    fn text_of(&self, kind: RecordKind, id: Uuid) -> String {
        self.find(kind, id).map_or_else(
            || REMOVED.to_string(),
            |r| self.session.record_text(&r, self.group),
        )
    }
}

impl Words for LogWords<'_> {
    fn person(&self, id: Uuid) -> String {
        self.session.logged_name(id, self.group)
    }
    fn family(&self, id: Uuid) -> String {
        match self.find(RecordKind::Family, id) {
            Some(Record::Family(f)) => f
                .partner1
                .iter()
                .chain(&f.partner2)
                .map(|p| self.person(*p))
                .collect::<Vec<_>>()
                .join(&LIST_SEP.to_string()),
            _ => "Unknown".to_string(),
        }
    }
    fn place(&self, id: Uuid) -> String {
        self.text_of(RecordKind::Place, id)
    }
    fn source(&self, id: Uuid) -> String {
        match self.find(RecordKind::Source, id) {
            Some(_) => self
                .session
                .source_title(id, self.group)
                .unwrap_or_default(),
            None => REMOVED.to_string(),
        }
    }
    fn repository(&self, id: Uuid) -> String {
        self.text_of(RecordKind::Repository, id)
    }
    fn citation(&self, id: Uuid) -> String {
        self.text_of(RecordKind::Citation, id)
    }
    fn note(&self, id: Uuid) -> String {
        self.text_of(RecordKind::Note, id)
    }
    fn media(&self, id: Uuid) -> String {
        self.text_of(RecordKind::Media, id)
    }
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
            // A note's own text can change without its owner's record changing.
            if ch.kind == RecordKind::Note
                && let Some(owner) = self.note_owner(ch.id)
            {
                add(owner);
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
        match self.fact(rec, group) {
            LogFactDto::Text { text } => text,
            LogFactDto::Family { partners } if partners.is_empty() => "Family".to_string(),
            LogFactDto::Family { partners } => partners.join(" and "),
            LogFactDto::Event {
                label, date, place, ..
            } => std::iter::once(label)
                .chain(date)
                .chain(place)
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    /// One record as data.
    pub(crate) fn fact(&self, rec: &Record, group: &[Change]) -> LogFactDto {
        match rec {
            Record::Family(f) => LogFactDto::Family {
                partners: f
                    .partner1
                    .iter()
                    .chain(&f.partner2)
                    .map(|id| self.logged_name(*id, group))
                    .collect(),
            },
            Record::Event(e) => LogFactDto::Event {
                event: event_kind_id(&e.kind),
                label: event_label(&e.kind),
                date: e.date.as_ref().map(|d| d.display()),
                place: e
                    .place
                    .and_then(|id| self.find_logged(RecordKind::Place, id, group))
                    .and_then(|r| match r {
                        Record::Place(p) => p.names.first().map(|n| n.name.clone()).or(p.raw),
                        _ => None,
                    }),
            },
            other => LogFactDto::Text {
                text: self.record_text(other, group),
            },
        }
    }

    fn record_text(&self, rec: &Record, group: &[Change]) -> String {
        match rec {
            Record::Person(p) => display_name(p, self.settings.name_order),
            Record::Family(_) | Record::Event(_) => String::new(),
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

    /// `whole`: field values in full, as the person page shows them; otherwise cut short.
    fn log_row(
        &self,
        g: LogGroup,
        batches: &mut HashMap<Uuid, Option<Batch>>,
        whole: bool,
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
        let words = LogWords {
            session: self,
            group: &g.changes,
        };
        let changes = g
            .changes
            .iter()
            .map(|ch| LogChangeDto {
                kind: kind_name(ch.kind).to_string(),
                id: ch.id,
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
                fact: self.change_fact(ch, &g.changes),
                sources: self.sources_of_change(ch, &g.changes),
                fields: {
                    let mut f = record_field_changes(ch.before.as_ref(), ch.after.as_ref(), &words);
                    if !whole {
                        clip_fields(&mut f);
                    }
                    f
                },
            })
            .collect();
        let people = g
            .people
            .iter()
            .map(|p| LogPersonDto {
                id: p.id,
                name: self.logged_name(p.id, &g.changes),
                exists: self.tree.persons.contains_key(&p.id),
                sibling_marker: matches!(
                    self.find_logged(RecordKind::Person, p.id, &g.changes),
                    Some(Record::Person(r)) if r.sibling_marker
                ),
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
            standing_only: false,
        };
        let (groups, total) = self
            .project
            .log_page(&filter, offset, limit.min(MAX_LOG_PAGE))?;
        let mut batches = HashMap::new();
        let items = groups
            .into_iter()
            .map(|g| self.log_row(g, &mut batches, false))
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
                    actor_id: b.actor_id,
                    actor_name: b.actor_name,
                    started: b.started,
                    ended: b.ended,
                    report: b.report,
                    groups,
                    people,
                    unreviewed,
                    standing: self.project.batch_standing(b.id)?,
                })
            })
            .collect()
    }

    /// Every agent in the log, sorted by name.
    pub fn log_agents(&self) -> Result<Vec<LogAgentDto>, ApiError> {
        Ok(self
            .project
            .log_agents()?
            .into_iter()
            .map(|(id, name)| LogAgentDto { id, name })
            .collect())
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

    /// One page of what agents changed on this person and still stands, newest first, reviewed
    /// or not. Values are whole.
    pub fn person_agent_changes(
        &self,
        id: Uuid,
        offset: usize,
        limit: usize,
    ) -> Result<Page<LogRowDto>, ApiError> {
        let filter = LogFilter {
            person: Some(id),
            actor_kind: Some("agent".into()),
            standing_only: true,
            ..Default::default()
        };
        self.whole_page(&filter, offset, limit)
    }

    /// The note or citation `id` (`kind` is `note` or `citation`) with the record that holds it;
    /// `None` when it is gone or nothing holds it.
    pub fn record_home(&self, kind: &str, id: Uuid) -> Result<Option<RecordHomeDto>, ApiError> {
        let note = match kind {
            "note" => true,
            "citation" => false,
            other => {
                return Err(ApiError::new(
                    ErrorCode::Invalid,
                    format!("Only a note or a citation has a holder, not {other}."),
                ));
            }
        };
        let t = &self.tree;
        let held = |notes: &[Uuid], citations: &[Uuid]| {
            if note {
                notes.contains(&id)
            } else {
                citations.contains(&id)
            }
        };
        let mut owner = None;
        for (pid, p) in &t.persons {
            if held(&p.common.notes, &p.common.citations) {
                owner = Some(OwnerRef {
                    kind: OwnerKind::Person,
                    id: *pid,
                });
            } else if let Some(n) = p.names.iter().find(|n| held(&n.notes, &n.citations)) {
                owner = Some(OwnerRef {
                    kind: OwnerKind::Name,
                    id: n.id,
                });
            }
            if owner.is_some() {
                break;
            }
        }
        let found = |c: &gt_core::model::Common, kind: OwnerKind, owner: &mut Option<OwnerRef>| {
            if owner.is_none() && held(&c.notes, &c.citations) {
                *owner = Some(OwnerRef { kind, id: c.id });
            }
        };
        for x in t.families.values() {
            found(&x.common, OwnerKind::Family, &mut owner);
        }
        for x in t.events.values() {
            found(&x.common, OwnerKind::Event, &mut owner);
        }
        for x in t.media.values() {
            found(&x.common, OwnerKind::Media, &mut owner);
        }
        for x in t.citations.values() {
            found(&x.common, OwnerKind::Citation, &mut owner);
        }
        let Some(owner) = owner else {
            return Ok(None);
        };
        Ok(Some(RecordHomeDto {
            owner,
            note: if note {
                self.note_dtos(&[id]).pop()
            } else {
                None
            },
            citation: if note { None } else { self.citation_dto(id) },
        }))
    }

    /// Whether the log holds groups from before authors were tracked; their notes and events may
    /// not reach the person they belong to.
    pub fn has_untracked_changes(&self) -> Result<bool, ApiError> {
        Ok(self.project.has_untracked_groups()?)
    }

    fn whole_page(
        &self,
        filter: &LogFilter,
        offset: usize,
        limit: usize,
    ) -> Result<Page<LogRowDto>, ApiError> {
        let (groups, total) = self
            .project
            .log_page(filter, offset, limit.min(MAX_LOG_PAGE))?;
        let mut batches = HashMap::new();
        let items = groups
            .into_iter()
            .map(|g| self.log_row(g, &mut batches, true))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Page {
            total,
            offset: u32::try_from(offset).unwrap_or(u32::MAX),
            items,
            letters: None,
        })
    }

    /// One page of every change group that reached this person, by anyone, newest first. Values
    /// are whole. Groups from before authors were tracked reach only the person record itself.
    pub fn person_history(
        &self,
        id: Uuid,
        offset: usize,
        limit: usize,
    ) -> Result<Page<LogRowDto>, ApiError> {
        let filter = LogFilter {
            person: Some(id),
            ..Default::default()
        };
        self.whole_page(&filter, offset, limit)
    }

    fn change_fact(&self, ch: &Change, group: &[Change]) -> LogFactDto {
        ch.after.as_ref().or(ch.before.as_ref()).map_or(
            LogFactDto::Text {
                text: String::new(),
            },
            |r| self.fact(r, group),
        )
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
            fact: self.change_fact(ch, group),
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

#[cfg(test)]
mod clip_tests {
    use super::*;

    fn clipped(before: &str, after: &str) -> (String, String) {
        let mut f = vec![FieldChangeDto {
            field: "text".into(),
            of: None,
            name_number: None,
            name_kind: None,
            name_id: None,
            before: Some(before.into()),
            after: Some(after.into()),
        }];
        clip_fields(&mut f);
        (
            f[0].before.clone().unwrap_or_default(),
            f[0].after.clone().unwrap_or_default(),
        )
    }

    fn body(s: &str) -> &str {
        s.trim_start_matches('…').trim_end_matches('…')
    }

    fn within_limit(s: &str) {
        assert!(body(s).chars().count() <= FIELD_PREVIEW, "{}", s.len());
    }

    #[test]
    fn short_values_stay_as_they_are() {
        let (b, a) = clipped("Ivan Koval", "Olena Koval");
        assert_eq!((b.as_str(), a.as_str()), ("Ivan Koval", "Olena Koval"));
        let exact = "x".repeat(FIELD_PREVIEW);
        let (b, a) = clipped(&exact, &format!("{}y", &exact[1..]));
        assert_eq!(b, exact);
        assert!(a.ends_with('y'));
    }

    #[test]
    fn an_append_to_a_long_note_shows_the_added_text() {
        let old = "word ".repeat(180);
        let new = format!("{old}Added line about Ivan Koval.");
        let (b, a) = clipped(&old, &new);
        assert_ne!(b, a);
        assert!(a.contains("Added line about Ivan Koval."), "{a}");
        assert!(b.starts_with('…') && a.starts_with('…'));
        assert!(!a.ends_with('…'), "the after side reaches its end: {a}");
        within_limit(&b);
        within_limit(&a);
    }

    #[test]
    fn an_insert_in_the_middle_shows_it_with_context_and_marks_both_cuts() {
        let old = format!("{}{}", "a".repeat(500), "b".repeat(500));
        let new = format!("{}INSERTED{}", "a".repeat(500), "b".repeat(500));
        let (b, a) = clipped(&old, &new);
        assert!(a.contains("INSERTED"), "{a}");
        assert!(!b.contains("INSERTED"));
        assert!(a.starts_with('…') && a.ends_with('…'));
        assert!(b.starts_with('…') && b.ends_with('…'));
        within_limit(&b);
        within_limit(&a);
    }

    #[test]
    fn a_change_at_the_start_keeps_the_head_and_marks_only_the_end() {
        let old = format!("Old start {}", "z".repeat(600));
        let new = format!("New start {}", "z".repeat(600));
        let (b, a) = clipped(&old, &new);
        assert!(b.starts_with("Old start") && a.starts_with("New start"));
        assert!(b.ends_with('…') && a.ends_with('…'));
        within_limit(&b);
        within_limit(&a);
    }

    #[test]
    fn a_common_prefix_longer_than_the_limit_is_skipped() {
        let prefix = "p".repeat(FIELD_PREVIEW * 3);
        let (b, a) = clipped(&format!("{prefix}one"), &format!("{prefix}two"));
        assert!(b.contains("one") && a.contains("two"), "{b} | {a}");
        assert!(b.starts_with('…') && a.starts_with('…'));
        within_limit(&b);
        within_limit(&a);
    }

    #[test]
    fn multi_byte_text_is_cut_on_char_boundaries() {
        let prefix = "Родовід Ковалів 👪 ".repeat(40);
        let (b, a) = clipped(
            &format!("{prefix}кінець"),
            &format!("{prefix}кінець і ще 🌳 рядок"),
        );
        assert!(a.contains("ще 🌳 рядок"), "{a}");
        within_limit(&b);
        within_limit(&a);
        let (b, a) = clipped(
            &format!("{prefix}😀{prefix}"),
            &format!("{prefix}🌳{prefix}"),
        );
        assert!(b.contains('😀') && a.contains('🌳'));
    }

    #[test]
    fn a_side_that_is_missing_is_clipped_on_its_own() {
        let long = "q".repeat(500);
        let mut f = vec![FieldChangeDto {
            field: "text".into(),
            of: None,
            name_number: None,
            name_kind: None,
            name_id: None,
            before: None,
            after: Some(long),
        }];
        clip_fields(&mut f);
        let a = f[0].after.clone().unwrap_or_default();
        assert!(a.ends_with('…') && !a.starts_with('…'));
        within_limit(&a);
    }
}
