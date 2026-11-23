//! "Events of a place": one chronological list of what happened in a place and the places under
//! it. Two sources meet here: the events of people in the tree (births, marriages, deaths and the
//! rest whose place is this one), and the place records, which are about other people and come from
//! church books, confession lists, revisions and censuses (see `gt_store::place_records`).

use crate::dto::PlaceOption;
use crate::error::{ApiError, ErrorCode};
use crate::map::year_of;
use crate::names::display_name;
use crate::person_detail::{event_kind_id, role_text};
use crate::session::Session;
use gt_core::model::{EventKind, ParticipantRef};
use gt_store::{
    AddOutcome, PLACE_RECORD_KINDS, PlaceRecordFilter, PlaceRecordInput, PlaceRecordRow,
    fold_search,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use ts_rs::TS;
use uuid::Uuid;

/// The most rows one page of events holds.
pub const PLACE_EVENTS_PAGE_MAX: u32 = 500;
const PLACE_EVENTS_PAGE_DEFAULT: u32 = 100;
/// The most records one `add_place_records` call takes.
pub const PLACE_RECORDS_BATCH_MAX: usize = 1000;
/// The most people one record names.
pub const PLACE_RECORD_PEOPLE_MAX: usize = 200;

/// Whose events a list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum PlaceEventsWho {
    /// Everyone: people of the tree and the people of the place records.
    #[default]
    All,
    /// Events of people in the tree, and records that name one of them.
    Ours,
    /// Records about other people only.
    Others,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaceEventsQuery {
    pub place: Uuid,
    /// Kinds to show (`birth`, `marriage`, `death`, `confession`, `revision`, `census`, `other`); all when absent or empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub kinds: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub year_from: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub year_to: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub who: Option<PlaceEventsWho>,
    /// Part of a name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub offset: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<u32>,
}

/// A person in an event row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaceEventPersonDto {
    /// The name to show: the tree's name for a relative, else what the record says.
    pub name: String,
    /// The name as the book wrote it, when that adds anything.
    pub as_written: String,
    pub role: String,
    pub age: String,
    pub estate: String,
    pub note: String,
    /// The person in the tree, when a relative.
    pub person: Option<Uuid>,
    pub living: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaceEventDto {
    /// Stable within one answer: `e:<event>` or `r:<record>`.
    pub key: String,
    /// `tree` (an event of the tree) or `record` (a place record).
    pub source: String,
    /// A relative's event, or a record that names a relative.
    pub ours: bool,
    /// `birth`, `marriage`, `death`, `confession`, `revision`, `census` or `other`.
    pub kind: String,
    /// The tree's own event kind for a tree event (`baptism`, `burial`, `residence`...).
    pub kind_detail: String,
    pub date: String,
    pub year: Option<i32>,
    /// The place, when it is one under the place asked about.
    pub place: Option<String>,
    pub people: Vec<PlaceEventPersonDto>,
    pub book: String,
    pub page: String,
    pub rec_id: String,
    pub note: String,
    pub unsure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct KindCount {
    pub kind: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DecadeCount {
    pub decade: i32,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaceEventsPage {
    pub place: PlaceOption,
    /// Rows that match every filter.
    pub total: u32,
    pub offset: u32,
    pub items: Vec<PlaceEventDto>,
    /// Counts by kind and by decade of the rows that match the who and name filters, ignoring the
    /// kinds and years asked for, so the overview stays whole while one is chosen.
    pub overview_total: u32,
    pub by_kind: Vec<KindCount>,
    pub by_decade: Vec<DecadeCount>,
}

/// What a tree event is, in the six kinds a place record has, and `other`.
pub fn coarse_kind(k: &EventKind) -> &'static str {
    use EventKind::*;
    match k {
        Birth | Christening | Baptism => "birth",
        Marriage | MarriageBanns | MarriageContract | MarriageLicense | MarriageSettlement
        | Engagement => "marriage",
        Death | Burial | Cremation => "death",
        Census => "census",
        _ => "other",
    }
}

struct TreeEvent {
    id: Uuid,
    year: Option<i32>,
    kind: &'static str,
    place: Uuid,
    people: Vec<Uuid>,
    search: String,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SortKey {
    undated: bool,
    year: i32,
    /// Tree events before records of the same year.
    rank: u8,
    tie: u128,
}

enum Src {
    Tree(usize),
    Record(i64),
}

fn invalid(m: &str) -> ApiError {
    ApiError::new(ErrorCode::Invalid, m)
}

impl Session {
    /// The place and every place below it.
    pub fn place_subtree(&self, root: Uuid) -> Vec<Uuid> {
        let mut children: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for (id, p) in &self.tree.places {
            if let Some(parent) = p.parent {
                children.entry(parent).or_default().push(*id);
            }
        }
        let mut seen: HashSet<Uuid> = HashSet::from([root]);
        let mut out = vec![root];
        let mut i = 0;
        while let Some(&next) = out.get(i) {
            i += 1;
            for c in children.get(&next).into_iter().flatten() {
                if seen.insert(*c) {
                    out.push(*c);
                }
            }
        }
        out
    }

    fn place_leaf_name(&self, id: Uuid) -> Option<String> {
        self.place_option(id).map(|o| o.name)
    }

    /// Events of the tree whose place is in `places`.
    fn tree_events_in(&self, places: &HashSet<Uuid>) -> Vec<TreeEvent> {
        let order = self.settings.name_order;
        let mut out = Vec::new();
        for (id, e) in &self.tree.events {
            let Some(place) = e.place.filter(|p| places.contains(p)) else {
                continue;
            };
            if e.negative || e.common.private {
                continue;
            }
            let mut people: Vec<Uuid> = Vec::new();
            for part in &e.participants {
                let ids: Vec<Uuid> = match part.target {
                    ParticipantRef::Person(p) => vec![p],
                    ParticipantRef::Family(f) => self
                        .tree
                        .families
                        .get(&f)
                        .map(|f| [f.partner1, f.partner2].into_iter().flatten().collect())
                        .unwrap_or_default(),
                };
                for p in ids {
                    let visible = self
                        .tree
                        .persons
                        .get(&p)
                        .is_some_and(|x| !x.common.private && !x.sibling_marker);
                    if visible && !people.contains(&p) {
                        people.push(p);
                    }
                }
            }
            if people.is_empty() {
                continue;
            }
            let search = fold_search(
                &people
                    .iter()
                    .filter_map(|p| self.tree.persons.get(p))
                    .map(|p| display_name(p, order))
                    .collect::<Vec<_>>()
                    .join(" "),
            );
            out.push(TreeEvent {
                id: *id,
                year: e.date.as_ref().and_then(year_of),
                kind: coarse_kind(&e.kind),
                place,
                people,
                search,
            });
        }
        out
    }

    fn tree_event_dto(&self, t: &TreeEvent, root: Uuid) -> Option<PlaceEventDto> {
        let e = self.tree.events.get(&t.id)?;
        let order = self.settings.name_order;
        let role_of = |p: Uuid| -> String {
            e.participants
                .iter()
                .find(|x| match x.target {
                    ParticipantRef::Person(q) => q == p,
                    ParticipantRef::Family(f) => self
                        .tree
                        .families
                        .get(&f)
                        .is_some_and(|f| f.partner1 == Some(p) || f.partner2 == Some(p)),
                })
                .map(|x| role_text(&x.role).to_lowercase())
                .unwrap_or_default()
        };
        let people = t
            .people
            .iter()
            .filter_map(|p| {
                let person = self.tree.persons.get(p)?;
                Some(PlaceEventPersonDto {
                    name: display_name(person, order),
                    as_written: String::new(),
                    role: role_of(*p),
                    age: if e.participants.len() == 1 {
                        e.age.clone().unwrap_or_default()
                    } else {
                        String::new()
                    },
                    estate: String::new(),
                    note: String::new(),
                    person: Some(*p),
                    living: self.living.get(p).copied().unwrap_or(true),
                })
            })
            .collect();
        Some(PlaceEventDto {
            key: format!("e:{}", t.id),
            source: "tree".into(),
            ours: true,
            kind: t.kind.into(),
            kind_detail: event_kind_id(&e.kind),
            date: e.date.as_ref().map(|d| d.display()).unwrap_or_default(),
            year: t.year,
            place: (t.place != root)
                .then(|| self.place_leaf_name(t.place))
                .flatten(),
            people,
            book: String::new(),
            page: String::new(),
            rec_id: String::new(),
            note: e.description.clone().unwrap_or_default(),
            unsure: false,
        })
    }

    fn record_dto(&self, r: &PlaceRecordRow, root: Uuid) -> PlaceEventDto {
        let order = self.settings.name_order;
        let mut ours = false;
        let people = r
            .people
            .iter()
            .map(|p| {
                let relative = p.person.filter(|id| self.tree.persons.contains_key(id));
                ours |= relative.is_some();
                let written = [p.given.trim(), p.patronymic.trim(), p.surname.trim()]
                    .iter()
                    .filter(|s| !s.is_empty())
                    .copied()
                    .collect::<Vec<_>>()
                    .join(" ");
                let name = match relative.and_then(|id| self.tree.persons.get(&id)) {
                    Some(person) => display_name(person, order),
                    None if written.is_empty() => p.as_written.clone(),
                    None => written.clone(),
                };
                PlaceEventPersonDto {
                    as_written: if p.as_written.trim() == name || p.as_written.trim().is_empty() {
                        String::new()
                    } else {
                        p.as_written.clone()
                    },
                    name,
                    role: p.role.clone(),
                    age: p.age.clone(),
                    estate: p.estate.clone(),
                    note: p.note.clone(),
                    person: relative,
                    living: relative
                        .map(|id| self.living.get(&id).copied().unwrap_or(true))
                        .unwrap_or(false),
                }
            })
            .collect();
        PlaceEventDto {
            key: format!("r:{}", r.id),
            source: "record".into(),
            ours,
            kind: r.kind.clone(),
            kind_detail: String::new(),
            date: r.date.clone(),
            year: r.year,
            place: (r.place != root)
                .then(|| self.place_leaf_name(r.place))
                .flatten(),
            people,
            book: r.book.clone(),
            page: r.page.clone(),
            rec_id: r.rec_id.clone(),
            note: r.note.clone(),
            unsure: r.unsure,
        }
    }

    /// One page of the events of a place and the places under it, oldest first, undated last.
    pub fn place_events(&self, q: &PlaceEventsQuery) -> Result<PlaceEventsPage, ApiError> {
        let root = self
            .place_option(q.place)
            .ok_or_else(|| ApiError::not_found("place", q.place))?;
        let who = q.who.unwrap_or_default();
        let limit = q
            .limit
            .unwrap_or(PLACE_EVENTS_PAGE_DEFAULT)
            .clamp(1, PLACE_EVENTS_PAGE_MAX) as usize;
        let offset = q.offset.unwrap_or(0) as usize;
        let kinds: Vec<String> = q
            .kinds
            .iter()
            .flatten()
            .map(|k| k.trim().to_lowercase())
            .filter(|k| !k.is_empty())
            .collect();
        let query = fold_search(q.query.as_deref().unwrap_or(""));
        let query = query.trim().to_string();

        let places = self.place_subtree(q.place);
        let place_set: HashSet<Uuid> = places.iter().copied().collect();

        // The events of people in the tree (none when only the others are wanted).
        let mut tree_events: Vec<TreeEvent> = if who == PlaceEventsWho::Others {
            Vec::new()
        } else {
            self.tree_events_in(&place_set)
        };
        if !query.is_empty() {
            tree_events.retain(|t| t.search.contains(&query));
        }

        let f = PlaceRecordFilter {
            places,
            kinds: kinds.clone(),
            year_from: q.year_from,
            year_to: q.year_to,
            relatives: match who {
                PlaceEventsWho::All => None,
                PlaceEventsWho::Ours => Some(true),
                PlaceEventsWho::Others => Some(false),
            },
            query: query.clone(),
        };

        // The overview ignores the kinds and years asked for.
        let mut by_kind: BTreeMap<String, u32> = BTreeMap::new();
        let mut by_decade: BTreeMap<i32, u32> = BTreeMap::new();
        let mut overview_total = 0u32;
        for t in &tree_events {
            overview_total += 1;
            *by_kind.entry(t.kind.to_string()).or_default() += 1;
            if let Some(y) = t.year {
                *by_decade.entry(y.div_euclid(10) * 10).or_default() += 1;
            }
        }
        let counts = self.project.place_record_counts(&f)?;
        overview_total += counts.total;
        for (k, n) in counts.by_kind {
            *by_kind.entry(k).or_default() += n;
        }
        for (d, n) in counts.by_decade {
            *by_decade.entry(d).or_default() += n;
        }

        // The rows that pass every filter, from both sources, in one order.
        let mut keyed: Vec<(SortKey, Src)> = Vec::new();
        for (i, t) in tree_events.iter().enumerate() {
            if !kinds.is_empty() && !kinds.iter().any(|k| k == t.kind) {
                continue;
            }
            if q.year_from.is_some() || q.year_to.is_some() {
                let Some(y) = t.year else { continue };
                if q.year_from.is_some_and(|f| y < f) || q.year_to.is_some_and(|to| y > to) {
                    continue;
                }
            }
            keyed.push((
                SortKey {
                    undated: t.year.is_none(),
                    year: t.year.unwrap_or(0),
                    rank: 0,
                    tie: t.id.as_u128(),
                },
                Src::Tree(i),
            ));
        }
        for (id, year) in self.project.place_record_keys(&f)? {
            keyed.push((
                SortKey {
                    undated: year.is_none(),
                    year: year.unwrap_or(0),
                    rank: 1,
                    tie: id as u128,
                },
                Src::Record(id),
            ));
        }
        keyed.sort_by_key(|(k, _)| *k);
        let total = keyed.len() as u32;

        let window: Vec<&(SortKey, Src)> = keyed.iter().skip(offset).take(limit).collect();
        let record_ids: Vec<i64> = window
            .iter()
            .filter_map(|(_, s)| match s {
                Src::Record(id) => Some(*id),
                Src::Tree(_) => None,
            })
            .collect();
        let rows: HashMap<i64, PlaceRecordRow> = self
            .project
            .place_record_rows(&record_ids)?
            .into_iter()
            .map(|r| (r.id, r))
            .collect();
        let mut items = Vec::with_capacity(window.len());
        for (_, s) in window {
            match s {
                Src::Tree(i) => {
                    if let Some(dto) = tree_events
                        .get(*i)
                        .and_then(|t| self.tree_event_dto(t, q.place))
                    {
                        items.push(dto);
                    }
                }
                Src::Record(id) => {
                    if let Some(r) = rows.get(id) {
                        items.push(self.record_dto(r, q.place));
                    }
                }
            }
        }
        Ok(PlaceEventsPage {
            place: root,
            total,
            offset: offset as u32,
            items,
            overview_total,
            by_kind: by_kind
                .into_iter()
                .map(|(kind, count)| KindCount { kind, count })
                .collect(),
            by_decade: by_decade
                .into_iter()
                .map(|(decade, count)| DecadeCount { decade, count })
                .collect(),
        })
    }

    /// Adds records to a place (an existing one of the tree), in one transaction. A record whose
    /// book and record id are there already is skipped. `batch` is the agent batch, if an agent
    /// adds them.
    pub fn add_place_records(
        &mut self,
        place: Uuid,
        records: &[PlaceRecordInput],
        batch: Option<Uuid>,
    ) -> Result<AddOutcome, ApiError> {
        if !self.tree.places.contains_key(&place) {
            return Err(ApiError::not_found("place", place));
        }
        for r in records {
            if r.people.len() > PLACE_RECORD_PEOPLE_MAX {
                return Err(invalid("A record names too many people."));
            }
            for p in r.people.iter().filter_map(|p| p.person) {
                if !self.tree.persons.contains_key(&p) {
                    return Err(ApiError::not_found("person", p));
                }
            }
        }
        let (_, now) = (self.clock.0)();
        Ok(self.project.add_place_records(place, records, batch, now)?)
    }

    /// The same for an agent: its open batch must be this connection's. The records carry the
    /// batch, so the owner can tell which ones an agent added.
    pub fn add_place_records_as_agent(
        &mut self,
        ctx: &crate::agent::AgentCtx,
        batch: Uuid,
        place: Uuid,
        records: &[PlaceRecordInput],
    ) -> Result<AddOutcome, ApiError> {
        self.check_batch_open(ctx, batch)?;
        self.add_place_records(place, records, Some(batch))
    }

    /// The people of the tree that records of this batch call: for a refresh of what shows them.
    pub fn place_record_people(records: &[PlaceRecordInput]) -> Vec<Uuid> {
        let mut out: Vec<Uuid> = records
            .iter()
            .flat_map(|r| r.people.iter().filter_map(|p| p.person))
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// How many of the records are stored already (an import preview).
    pub fn count_place_records_present(
        &self,
        book: &str,
        rec_ids: &[&str],
    ) -> Result<u32, ApiError> {
        Ok(self.project.count_place_records_present(book, rec_ids)?)
    }

    /// Records the place and the places under it hold.
    pub fn place_record_total(&self, place: Uuid) -> Result<u32, ApiError> {
        Ok(self
            .project
            .count_place_records(&self.place_subtree(place))?)
    }
}

/// The kinds a place record can have, for a tool's schema.
pub fn place_record_kinds() -> &'static [&'static str] {
    &PLACE_RECORD_KINDS
}
