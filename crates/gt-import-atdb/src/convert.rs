//! The mapping from the Древо Жизни tables to the gt-core model.
//!
//! Table codes (`rec_table`): 13 person, 7 event, 14 place, 16 source, 4 document. Field ids and
//! role ids are the ones seen in real databases of schema version 8; anything else is kept as
//! unknown data on its record and counted in the report.

use crate::AtdbError;
use crate::dates::{self, DateRow};
use crate::db::rows;
use gt_core::date::GenDate;
use gt_core::ids::new_id;
use gt_core::model::*;
use gt_core::raw::RawNode;
use gt_core::tree::Tree;
use gt_gedcom::{ImportOptions, ImportOutcome, ImportReport};
use rusqlite::Connection;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use uuid::Uuid;

const T_EVENT: i64 = 7;
const T_PERSON: i64 = 13;
const T_PLACE: i64 = 14;
const T_SOURCE: i64 = 16;

const F_GIVEN: i64 = 66;
const F_SURNAME: i64 = 64;
const F_PATRONYMIC: i64 = 67;
const F_MAIDEN: i64 = 65;
const F_EVENT_PLACE: i64 = 28;
const F_EVENT_DATE: i64 = 29;
const F_PLACE_FULL: i64 = 93;
const F_PLACE_SHORT: i64 = 94;

const ET_BIRTH: i64 = 1;
const ET_DEATH: i64 = 2;
const ET_MARRIAGE: i64 = 3;

const ER_CHILD: i64 = 1;
const ER_FATHER: i64 = 2;
const ER_MOTHER: i64 = 3;
const ER_DECEASED: i64 = 4;
const ER_SPOUSE_A: i64 = 5;
const ER_SPOUSE_B: i64 = 6;

/// `RawNode::pos` for unknown nodes that follow every known sibling.
const TAIL_POS: u32 = u32::MAX - 1;

struct Str {
    table: i64,
    rec: i64,
    field: i64,
    lang: Option<String>,
    value: String,
}

fn table_name(t: i64) -> String {
    match t {
        T_PERSON => "person".into(),
        T_EVENT => "event".into(),
        T_PLACE => "place".into(),
        T_SOURCE => "source".into(),
        4 => "document".into(),
        9 => "family".into(),
        n => format!("table {n}"),
    }
}

/// What is kept as unknown data (per record) and what is counted for the report.
#[derive(Default)]
struct Unknowns {
    raw: HashMap<(i64, i64), Vec<RawNode>>,
    counts: BTreeMap<String, usize>,
}

impl Unknowns {
    fn count(&mut self, key: String) {
        *self.counts.entry(key).or_insert(0) += 1;
    }

    /// Keep a value on the record `(table, rec)` as an `_ATDB_*` node and count it.
    fn keep(&mut self, table: i64, rec: i64, kind: &str, field: i64, value: String) {
        self.count(format!("{} {kind} {field}", table_name(table)));
        self.raw.entry((table, rec)).or_default().push(RawNode {
            tag: format!("_ATDB_{}_{field}", kind.to_uppercase().replace(' ', "_")),
            xref: None,
            value: Some(value),
            pos: TAIL_POS,
            children: vec![],
        });
    }

    fn take(&mut self, table: i64, rec: i64) -> Vec<RawNode> {
        self.raw.remove(&(table, rec)).unwrap_or_default()
    }
}

fn common(id: Uuid) -> Common {
    Common {
        id,
        private: false,
        changed: None,
        citations: vec![],
        notes: vec![],
        media: vec![],
        ext_ids: vec![],
        unknown: vec![],
    }
}

fn assertion(preferred: bool) -> Assertion {
    Assertion {
        preferred,
        proof: None,
        confidence: None,
    }
}

fn non_empty(s: Option<&String>) -> Option<String> {
    s.map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

// ---------------------------------------------------------------- names

struct NameParts {
    given: Option<String>,
    surname: Option<String>,
    patronymic: Option<String>,
    maiden: Option<String>,
}

fn name_raw(p: &NameParts, surname: &Option<String>) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.extend(p.given.clone());
    parts.extend(p.patronymic.clone());
    if let Some(s) = surname {
        parts.push(format!("/{s}/"));
    }
    parts.join(" ")
}

fn one_name(
    p: &NameParts,
    kind: NameKind,
    surname: Option<String>,
    lang: Option<&str>,
    preferred: bool,
) -> Name {
    Name {
        id: new_id(),
        kind,
        assertion: assertion(preferred),
        private: false,
        prefix: None,
        given: p.given.clone(),
        call: None,
        surname_prefix: None,
        surnames: surname.iter().cloned().collect(),
        patronymic: p.patronymic.clone(),
        suffix: None,
        nickname: None,
        lang: lang.map(str::to_string),
        date: None,
        raw: Some(name_raw(p, &surname)),
        citations: vec![],
        notes: vec![],
        unknown: vec![],
    }
}

/// Female patronymic endings (Ukrainian and Russian). Unlike the male -ович/-евич, which are also common
/// surnames, these are never surnames.
const FEMALE_PATRONYMIC_ENDINGS: [&str; 5] = ["івна", "ївна", "овна", "евна", "ична"];

/// A female patronymic typed into the surname field (the patronymic field left empty) is read as the
/// patronymic. Otherwise, with a maiden name beside it, it would come out as a "married surname".
fn patronymic_out_of_surname(mut p: NameParts) -> NameParts {
    let looks_patronymic = |s: &str| {
        let lower = s.trim().to_lowercase();
        !lower.contains(' ')
            && FEMALE_PATRONYMIC_ENDINGS
                .iter()
                .any(|e| lower.ends_with(e) && lower.chars().count() > e.chars().count() + 1)
    };
    if p.patronymic.is_none() && p.surname.as_deref().is_some_and(looks_patronymic) {
        p.patronymic = p.surname.take();
    }
    p
}

/// Names of one person; `by_lang` has the main language first.
fn names_of(by_lang: &[(Option<String>, NameParts)]) -> Vec<Name> {
    let mut out: Vec<Name> = Vec::new();
    for (lang, p) in by_lang {
        let first = out.is_empty();
        let l = lang.as_deref();
        let translation = l.is_some() && !first;
        let base = if translation {
            NameKind::Aka
        } else {
            NameKind::Birth
        };
        match (&p.surname, &p.maiden) {
            (Some(s), Some(m)) if s != m => {
                let married = if translation {
                    NameKind::Aka
                } else {
                    NameKind::Married
                };
                out.push(one_name(p, married, Some(s.clone()), l, first));
                out.push(one_name(p, NameKind::Birth, Some(m.clone()), l, false));
            }
            (Some(s), _) | (None, Some(s)) => {
                out.push(one_name(p, base, Some(s.clone()), l, first));
            }
            (None, None) => {
                if p.given.is_some() || p.patronymic.is_some() {
                    out.push(one_name(p, base, None, l, first));
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------- the conversion

struct Detail {
    person: i64,
    event: i64,
    role: i64,
}

type PairKey = (Option<Uuid>, Option<Uuid>);

struct Builder {
    tree: Tree,
    unknowns: Unknowns,
    person_ids: HashMap<i64, Uuid>,
    place_ids: HashMap<i64, Uuid>,
    event_ids: HashMap<i64, Uuid>,
    families: HashMap<PairKey, Uuid>,
    /// Children already linked to a family.
    placed: std::collections::HashSet<Uuid>,
    resolved_media: Vec<(Uuid, PathBuf)>,
}

impl Builder {
    fn sex(&self, p: Option<Uuid>) -> Option<Sex> {
        p.and_then(|p| self.tree.persons.get(&p))
            .map(|p| p.sex.clone())
    }

    /// The family of this pair of partners (either may be missing), created on first use.
    fn family(&mut self, a: Option<Uuid>, b: Option<Uuid>) -> Uuid {
        let key = (a.min(b), a.max(b));
        if let Some(id) = self.families.get(&key) {
            return *id;
        }
        let (mut p1, mut p2) = (a, b);
        if self.sex(p1) == Some(Sex::Female) && self.sex(p2) == Some(Sex::Male) {
            std::mem::swap(&mut p1, &mut p2);
        }
        let id = new_id();
        self.tree.families.insert(
            id,
            Family {
                common: common(id),
                partner1: p1,
                partner2: p2,
                kind: UnionKind::Unknown,
                children: vec![],
            },
        );
        self.families.insert(key, id);
        id
    }

    fn add_child(&mut self, fam: Uuid, child: Uuid) {
        let first_family = self.placed.insert(child);
        if let Some(f) = self.tree.families.get_mut(&fam)
            && !f.children.iter().any(|c| c.person == child)
        {
            f.children.push(ChildLink {
                person: child,
                rel1: ParentRel::Birth,
                rel2: ParentRel::Birth,
                preferred: first_family,
                assertion: assertion(false),
                citations: vec![],
                unknown: vec![],
            });
        }
    }

    /// The `Common` of a record of the given atdb table, when it was imported.
    fn common_of(&mut self, table: i64, rec: i64) -> Option<&mut Common> {
        match table {
            T_PERSON => {
                let id = self.person_ids.get(&rec)?;
                self.tree.persons.get_mut(id).map(|p| &mut p.common)
            }
            T_EVENT => {
                let id = self.event_ids.get(&rec)?;
                self.tree.events.get_mut(id).map(|e| &mut e.common)
            }
            T_PLACE => {
                let id = self.place_ids.get(&rec)?;
                self.tree.places.get_mut(id).map(|p| &mut p.common)
            }
            _ => None,
        }
    }
}

fn guid_map(conn: &Connection, table: i64) -> Result<HashMap<i64, Vec<ExternalId>>, AtdbError> {
    let sql = format!("SELECT rec_id, guid FROM Recs WHERE rec_table = {table}");
    Ok(rows(conn, "Recs", &sql, |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, Option<Vec<u8>>>(1)?))
    })?
    .into_iter()
    .map(|(id, g)| {
        let ext = match g.as_deref().and_then(|g| <[u8; 16]>::try_from(g).ok()) {
            Some(b) => vec![ExternalId {
                kind: ExtIdKind::Uid,
                value: Uuid::from_bytes(b).to_string(),
            }],
            None => vec![],
        };
        (id, ext)
    })
    .collect())
}

pub fn convert(conn: &Connection, opts: &ImportOptions) -> Result<ImportOutcome, AtdbError> {
    let mut report = ImportReport {
        encoding: "SQLite database".into(),
        ..ImportReport::default()
    };
    let mut b = Builder {
        tree: Tree::new(),
        unknowns: Unknowns::default(),
        person_ids: HashMap::new(),
        place_ids: HashMap::new(),
        event_ids: HashMap::new(),
        families: HashMap::new(),
        placed: Default::default(),
        resolved_media: Vec::new(),
    };

    let global: Vec<(Option<i64>, Option<String>)> = rows(
        conn,
        "Global",
        "SELECT version, mainlang FROM Global ORDER BY id",
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let (version, mainlang) = global.into_iter().next().unwrap_or((None, None));
    report.producer = Some(match version {
        Some(v) => format!("Древо Жизни (Genery), database version {v}"),
        None => "Древо Жизни (Genery)".into(),
    });
    b.tree.header.producer = report.producer.clone();

    let strs: Vec<Str> = rows(
        conn,
        "ValuesStr",
        "SELECT rec_table, rec_id, f_id, lang, vstr FROM ValuesStr ORDER BY id",
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        },
    )?
    .into_iter()
    .filter_map(|(table, rec, field, lang, v)| {
        let value = v?.trim().to_string();
        (!value.is_empty()).then_some(Str {
            table,
            rec,
            field,
            lang: lang.map(|l| l.trim().to_string()).filter(|l| !l.is_empty()),
            value,
        })
    })
    .collect();

    people(conn, &mut b, &strs, mainlang.as_deref())?;
    places(conn, &mut b, &strs)?;
    other_values(conn, &mut b)?;
    // String fields of events, documents and any other table: kept on the record, counted by
    // table and field id (the values never go into the report).
    for s in strs
        .iter()
        .filter(|s| !matches!(s.table, T_PERSON | T_PLACE | T_SOURCE))
    {
        b.unknowns
            .keep(s.table, s.rec, "field", s.field, s.value.clone());
    }
    let event_dates = event_dates(conn, &mut b)?;
    let event_places = event_places(conn, &mut b)?;
    events(conn, &mut b, &event_dates, &event_places)?;
    sources_and_media(conn, &mut b, &strs, opts, &mut report)?;
    attach_person_unknowns(&mut b);

    report.unmapped_tags = std::mem::take(&mut b.unknowns.counts);
    let counts = &mut report.counts;
    counts.insert("persons".into(), b.tree.persons.len());
    counts.insert("families".into(), b.tree.families.len());
    counts.insert("events".into(), b.tree.events.len());
    counts.insert("places".into(), b.tree.places.len());
    if !b.tree.sources.is_empty() {
        counts.insert("sources".into(), b.tree.sources.len());
    }
    if !b.tree.media.is_empty() {
        counts.insert("media".into(), b.tree.media.len());
    }
    Ok(ImportOutcome {
        tree: b.tree,
        report,
        resolved_media: b.resolved_media,
    })
}

fn people(
    conn: &Connection,
    b: &mut Builder,
    strs: &[Str],
    mainlang: Option<&str>,
) -> Result<(), AtdbError> {
    let persons: Vec<(i64, Option<i64>)> = rows(
        conn,
        "Persons",
        "SELECT id, sex FROM Persons ORDER BY id",
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let guids = guid_map(conn, T_PERSON)?;
    // person -> language ("" = main) -> field -> value
    let mut names: HashMap<i64, BTreeMap<String, HashMap<i64, String>>> = HashMap::new();
    for s in strs.iter().filter(|s| s.table == T_PERSON) {
        if matches!(s.field, F_GIVEN | F_SURNAME | F_PATRONYMIC | F_MAIDEN) {
            let lang = s
                .lang
                .clone()
                .filter(|l| Some(l.as_str()) != mainlang)
                .unwrap_or_default();
            let bucket = names.entry(s.rec).or_default().entry(lang).or_default();
            if let Entry::Vacant(e) = bucket.entry(s.field) {
                e.insert(s.value.clone());
            } else {
                b.unknowns
                    .keep(T_PERSON, s.rec, "repeated field", s.field, s.value.clone());
            }
        } else {
            b.unknowns
                .keep(T_PERSON, s.rec, "field", s.field, s.value.clone());
        }
    }
    for (pid, sex) in persons {
        let id = new_id();
        b.person_ids.insert(pid, id);
        let sex = match sex {
            Some(1) => Sex::Male,
            Some(2) => Sex::Female,
            Some(n) => {
                b.unknowns
                    .count(format!("person sex {n} (read as unknown)"));
                Sex::Unknown
            }
            None => Sex::Unknown,
        };
        let by_lang: Vec<(Option<String>, NameParts)> = names
            .remove(&pid)
            .unwrap_or_default()
            .into_iter()
            .map(|(lang, f)| {
                (
                    (!lang.is_empty()).then_some(lang),
                    patronymic_out_of_surname(NameParts {
                        given: non_empty(f.get(&F_GIVEN)),
                        surname: non_empty(f.get(&F_SURNAME)),
                        patronymic: non_empty(f.get(&F_PATRONYMIC)),
                        maiden: non_empty(f.get(&F_MAIDEN)),
                    }),
                )
            })
            .collect();
        let mut c = common(id);
        c.ext_ids = guids.get(&pid).cloned().unwrap_or_default();
        b.tree.persons.insert(
            id,
            Person {
                common: c,
                sex,
                names: names_of(&by_lang),
                living_override: None,
            },
        );
    }
    Ok(())
}

fn places(conn: &Connection, b: &mut Builder, strs: &[Str]) -> Result<(), AtdbError> {
    let list: Vec<(i64, Option<i64>)> = rows(
        conn,
        "Places",
        "SELECT id, parent_id FROM Places ORDER BY id",
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut full: HashMap<i64, String> = HashMap::new();
    let mut short: HashMap<i64, String> = HashMap::new();
    for s in strs.iter().filter(|s| s.table == T_PLACE) {
        match s.field {
            F_PLACE_FULL => {
                full.entry(s.rec).or_insert_with(|| s.value.clone());
            }
            F_PLACE_SHORT => {
                short.entry(s.rec).or_insert_with(|| s.value.clone());
            }
            f => b.unknowns.keep(T_PLACE, s.rec, "field", f, s.value.clone()),
        }
    }
    for (pid, _) in &list {
        b.place_ids.insert(*pid, new_id());
    }
    let parents: HashMap<i64, i64> = list.iter().filter_map(|(i, p)| Some((*i, (*p)?))).collect();
    let name_of = |id: i64| short.get(&id).or_else(|| full.get(&id)).cloned();
    for (pid, parent) in &list {
        let Some(id) = b.place_ids.get(pid).copied() else {
            continue;
        };
        let mut names: Vec<PlaceName> = Vec::new();
        for n in [name_of(*pid), full.get(pid).cloned()]
            .into_iter()
            .flatten()
        {
            if !names.iter().any(|x| x.name == n) {
                names.push(PlaceName {
                    name: n,
                    lang: None,
                    date: None,
                });
            }
        }
        // "Village, District, Region": own name, then the ancestors'. A cycle stops the walk.
        let mut chain: Vec<String> = Vec::new();
        let mut cur = Some(*pid);
        for _ in 0..64 {
            let Some(c) = cur else { break };
            chain.extend(name_of(c));
            cur = parents.get(&c).copied();
        }
        let mut c = common(id);
        c.unknown = b.unknowns.take(T_PLACE, *pid);
        let mut parent_id = parent.and_then(|p| b.place_ids.get(&p).copied());
        if parent.is_some() && parent_id.is_none() {
            b.unknowns
                .count("place whose parent is not in the file".into());
        }
        // A parent chain that comes back to this place is cut here.
        let mut up = *parent;
        for _ in 0..64 {
            match up {
                Some(u) if u == *pid => {
                    parent_id = None;
                    b.unknowns
                        .count("place parent loop (parent dropped)".into());
                    break;
                }
                Some(u) => up = parents.get(&u).copied(),
                None => break,
            }
        }
        b.tree.places.insert(
            id,
            Place {
                common: c,
                names,
                parent: parent_id,
                kind: None,
                lat: None,
                lon: None,
                raw: (!chain.is_empty()).then(|| chain.join(", ")),
            },
        );
    }
    Ok(())
}

/// Numeric values and links that nothing maps: kept on their record and counted.
fn other_values(conn: &Connection, b: &mut Builder) -> Result<(), AtdbError> {
    type Num = (i64, i64, i64, Option<f64>, Option<f64>);
    let nums: Vec<Num> = rows(
        conn,
        "ValuesNum",
        "SELECT rec_table, rec_id, f_id, vnum, vnum2 FROM ValuesNum ORDER BY id",
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )?;
    for (t, rec, f, a, c) in nums {
        let text = match (a, c) {
            (Some(a), Some(c)) => format!("{a} {c}"),
            (Some(a), None) | (None, Some(a)) => a.to_string(),
            (None, None) => continue,
        };
        b.unknowns.keep(t, rec, "number", f, text);
    }
    let links: Vec<(i64, i64, i64, i64, i64)> = rows(
        conn,
        "ValuesLinks",
        "SELECT rec_table, rec_id, f_id, vlink_table, vlink_id FROM ValuesLinks ORDER BY id",
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )?;
    for (t, rec, f, vt, vid) in links {
        if t == T_EVENT && f == F_EVENT_PLACE && vt == T_PLACE {
            continue;
        }
        b.unknowns
            .keep(t, rec, "link", f, format!("{} {vid}", table_name(vt)));
    }
    Ok(())
}

fn event_dates(conn: &Connection, b: &mut Builder) -> Result<HashMap<i64, GenDate>, AtdbError> {
    type Row = (i64, i64, i64, DateRow);
    let list: Vec<Row> = rows(
        conn,
        "ValuesDates",
        "SELECT rec_table, rec_id, f_id, d, m, y, d2, m2, y2, calendar, calendar2, type FROM ValuesDates ORDER BY id",
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                DateRow {
                    d: r.get(3)?,
                    m: r.get(4)?,
                    y: r.get(5)?,
                    d2: r.get(6)?,
                    m2: r.get(7)?,
                    y2: r.get(8)?,
                    calendar: r.get(9)?,
                    calendar2: r.get(10)?,
                    ty: r.get(11)?,
                },
            ))
        },
    )?;
    let mut out: HashMap<i64, GenDate> = HashMap::new();
    for (t, rec, f, row) in list {
        let Some((g, unparsed)) = dates::to_date(&row) else {
            continue;
        };
        if t == T_EVENT && f == F_EVENT_DATE && !out.contains_key(&rec) {
            for c in dates::caveats(&row) {
                b.unknowns.count(c);
            }
            if unparsed {
                b.unknowns
                    .count("date kept as text (not a valid date)".into());
            }
            out.insert(rec, g);
        } else {
            b.unknowns.keep(t, rec, "date", f, g.raw);
        }
    }
    Ok(out)
}

fn event_places(conn: &Connection, b: &mut Builder) -> Result<HashMap<i64, Uuid>, AtdbError> {
    let links: Vec<(i64, i64)> = rows(
        conn,
        "ValuesLinks",
        "SELECT rec_id, vlink_id FROM ValuesLinks WHERE rec_table = 7 AND f_id = 28 AND vlink_table = 14 ORDER BY id",
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut out = HashMap::new();
    for (ev, place) in links {
        match b.place_ids.get(&place) {
            Some(p) => {
                if let Some(first) = out.insert(ev, *p) {
                    out.insert(ev, first);
                    b.unknowns
                        .count("second place on an event (the first is kept)".into());
                }
            }
            None => b
                .unknowns
                .count("event place that is not in the file".into()),
        }
    }
    Ok(out)
}

fn events(
    conn: &Connection,
    b: &mut Builder,
    dates: &HashMap<i64, GenDate>,
    places: &HashMap<i64, Uuid>,
) -> Result<(), AtdbError> {
    let evs: Vec<(i64, i64)> = rows(
        conn,
        "Events",
        "SELECT id, et_id FROM Events ORDER BY id",
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let details: Vec<Detail> = rows(
        conn,
        "EventDetails",
        "SELECT p_id, e_id, er_id FROM EventDetails ORDER BY e_id, p_ord, id",
        |r| {
            Ok(Detail {
                person: r.get(0)?,
                event: r.get(1)?,
                role: r.get(2)?,
            })
        },
    )?;
    // Which role is a "main" one, for event types this importer does not know.
    let mains: HashMap<i64, bool> = rows(
        conn,
        "EventRoles",
        "SELECT id, ismain FROM EventRoles",
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)? == Some(1))),
    )?
    .into_iter()
    .collect();
    let mut by_event: HashMap<i64, Vec<&Detail>> = HashMap::new();
    let mut lost_roles = 0usize;
    for d in &details {
        if b.person_ids.contains_key(&d.person) {
            by_event.entry(d.event).or_default().push(d);
        } else {
            lost_roles += 1;
        }
    }
    if lost_roles > 0 {
        *b.unknowns
            .counts
            .entry("event role of a person that is not in the file".into())
            .or_insert(0) += lost_roles;
    }
    let guids = guid_map(conn, T_EVENT)?;
    let principal = |t| Participant {
        target: t,
        role: Role::Principal,
    };
    for (eid, et) in evs {
        let ds: &[&Detail] = by_event.get(&eid).map_or(&[], Vec::as_slice);
        let with_role = |role: i64| -> Vec<Uuid> {
            ds.iter()
                .filter(|d| d.role == role)
                .filter_map(|d| b.person_ids.get(&d.person).copied())
                .collect()
        };
        let mut participants: Vec<Participant> = Vec::new();
        let kind = match et {
            ET_BIRTH => {
                let children = with_role(ER_CHILD);
                let father = with_role(ER_FATHER).first().copied();
                let mother = with_role(ER_MOTHER).first().copied();
                if father.is_some() || mother.is_some() {
                    if children.is_empty() {
                        b.unknowns
                            .count("birth event with parents but no child".into());
                    } else {
                        let fam = b.family(father, mother);
                        for c in &children {
                            b.add_child(fam, *c);
                        }
                    }
                }
                participants.extend(
                    children
                        .into_iter()
                        .map(|c| principal(ParticipantRef::Person(c))),
                );
                EventKind::Birth
            }
            ET_DEATH => {
                participants.extend(
                    with_role(ER_DECEASED)
                        .into_iter()
                        .map(|c| principal(ParticipantRef::Person(c))),
                );
                EventKind::Death
            }
            ET_MARRIAGE => {
                let a = with_role(ER_SPOUSE_A).first().copied();
                let c = with_role(ER_SPOUSE_B).first().copied();
                if a.is_some() || c.is_some() {
                    let fam = b.family(a, c);
                    if let Some(f) = b.tree.families.get_mut(&fam) {
                        f.kind = UnionKind::Married;
                    }
                    participants.push(principal(ParticipantRef::Family(fam)));
                }
                EventKind::Marriage
            }
            n => {
                b.unknowns
                    .count(format!("event type {n} (kept as a custom event)"));
                for d in ds {
                    let Some(p) = b.person_ids.get(&d.person).copied() else {
                        continue;
                    };
                    let role = if mains.get(&d.role).copied().unwrap_or(false) {
                        Role::Principal
                    } else {
                        Role::Other(format!("role {}", d.role))
                    };
                    participants.push(Participant {
                        target: ParticipantRef::Person(p),
                        role,
                    });
                }
                EventKind::Custom(format!("Event type {n}"))
            }
        };
        // Known event types: keep every person whose role was not used above (an unknown role, or
        // a second father / mother / spouse) as a plain participant, and count it.
        let used: &[(i64, bool)] = match et {
            ET_BIRTH => &[(ER_CHILD, true), (ER_FATHER, false), (ER_MOTHER, false)],
            ET_DEATH => &[(ER_DECEASED, true)],
            ET_MARRIAGE => &[(ER_SPOUSE_A, false), (ER_SPOUSE_B, false)],
            _ => &[],
        };
        if !used.is_empty() {
            let mut seen: Vec<i64> = Vec::new();
            for d in ds {
                let Some(p) = b.person_ids.get(&d.person).copied() else {
                    continue;
                };
                let multi = used.iter().find(|(r, _)| *r == d.role).map(|(_, m)| *m);
                let handled = match multi {
                    Some(true) => true,
                    Some(false) => {
                        let first = !seen.contains(&d.role);
                        seen.push(d.role);
                        first
                    }
                    None => false,
                };
                if !handled {
                    b.unknowns.count(format!(
                        "role {} on event type {et} (kept as a participant)",
                        d.role
                    ));
                    participants.push(Participant {
                        target: ParticipantRef::Person(p),
                        role: Role::Other(format!("role {}", d.role)),
                    });
                }
            }
        }
        if participants.is_empty() {
            b.unknowns.count("event with no people (left out)".into());
            continue;
        }
        let id = new_id();
        let mut c = common(id);
        c.ext_ids = guids.get(&eid).cloned().unwrap_or_default();
        c.unknown = b.unknowns.take(T_EVENT, eid);
        b.event_ids.insert(eid, id);
        b.tree.events.insert(
            id,
            Event {
                common: c,
                kind,
                date: dates.get(&eid).cloned(),
                place: places.get(&eid).copied(),
                description: None,
                age: None,
                cause: None,
                negative: false,
                assertion: assertion(false),
                participants,
            },
        );
    }
    Ok(())
}

fn sources_and_media(
    conn: &Connection,
    b: &mut Builder,
    strs: &[Str],
    opts: &ImportOptions,
    report: &mut ImportReport,
) -> Result<(), AtdbError> {
    // Sources: the lowest string field is the title, the others go to the source text.
    let srcs: Vec<i64> = rows(conn, "Sources", "SELECT id FROM Sources ORDER BY id", |r| {
        r.get(0)
    })?;
    let mut source_ids: HashMap<i64, Uuid> = HashMap::new();
    for sid in srcs {
        let mut vals: Vec<&Str> = strs
            .iter()
            .filter(|s| s.table == T_SOURCE && s.rec == sid)
            .collect();
        vals.sort_by_key(|s| s.field);
        let title = vals.first().map(|s| s.value.clone());
        let mut text: Vec<String> = Vec::new();
        for s in vals.iter().skip(1) {
            text.push(format!("field {}: {}", s.field, s.value));
            b.unknowns.count(format!(
                "source field {} (kept in the source text)",
                s.field
            ));
        }
        let id = new_id();
        source_ids.insert(sid, id);
        b.tree.sources.insert(
            id,
            Source {
                common: common(id),
                title,
                author: None,
                publication: None,
                abbreviation: None,
                text: (!text.is_empty()).then(|| text.join("\n")),
                repositories: vec![],
            },
        );
    }
    let links: Vec<(i64, i64, i64)> = rows(
        conn,
        "SourceDetails",
        "SELECT s_id, rec_table, rec_id FROM SourceDetails ORDER BY id",
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    for (sid, t, rec) in links {
        let Some(src) = source_ids.get(&sid).copied() else {
            continue;
        };
        let cid = new_id();
        match b.common_of(t, rec) {
            Some(c) => {
                c.citations.push(cid);
                b.tree.citations.insert(
                    cid,
                    Citation {
                        common: common(cid),
                        source: Some(src),
                        page: None,
                        date: None,
                        text: None,
                        quality: None,
                        evidence: Evidence {
                            original: None,
                            primary_info: None,
                            direct: None,
                        },
                    },
                );
            }
            None => b.unknowns.count(format!(
                "source attached to a {} (not imported)",
                table_name(t)
            )),
        }
    }

    // Documents become media. The files are located by the GEDCOM importer's resolver (same
    // folder and media-root rules), by importing a tiny generated GEDCOM that lists the paths.
    let docs: Vec<(i64, String)> = rows(
        conn,
        "Documents",
        "SELECT id, path FROM Documents ORDER BY id",
        |r| {
            Ok((
                r.get(0)?,
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            ))
        },
    )?
    .into_iter()
    .filter(|(_, p)| !p.trim().is_empty())
    .collect();
    if docs.is_empty() {
        return Ok(());
    }
    let mut ged = String::from("0 HEAD\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n");
    let mut media_ids: HashMap<i64, Uuid> = HashMap::new();
    for (i, (did, path)) in docs.iter().enumerate() {
        let id = new_id();
        media_ids.insert(*did, id);
        let clean: String = path
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        ged.push_str(&format!("0 @M{i}@ OBJE\n1 _GTID {id}\n1 FILE {clean}\n"));
    }
    ged.push_str("0 TRLR\n");
    let sub = gt_gedcom::import(ged.as_bytes(), opts);
    for (id, mut m) in sub.tree.media {
        if let Some((did, _)) = media_ids.iter().find(|(_, v)| **v == id) {
            m.common.unknown.extend(b.unknowns.take(4, *did));
        }
        b.tree.media.insert(id, m);
    }
    b.resolved_media.extend(sub.resolved_media);
    report.missing_media.extend(sub.report.missing_media);
    report.media_urls.extend(sub.report.media_urls);

    type DocLink = (i64, i64, i64, Option<i64>, bool);
    let details: Vec<DocLink> = rows(
        conn,
        "DocumentDetails",
        "SELECT d_id, rec_table, rec_id, ismain, x1 IS NOT NULL FROM DocumentDetails ORDER BY d_ord, id",
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )?;
    for (did, t, rec, ismain, region) in details {
        let Some(media) = media_ids.get(&did).copied() else {
            continue;
        };
        if region {
            b.unknowns
                .count("picture region of a document (shown whole)".into());
        }
        let link = MediaLink {
            media,
            region: None,
            primary: ismain == Some(1),
        };
        match b.common_of(t, rec) {
            Some(c) => c.media.push(link),
            None => b.unknowns.count(format!(
                "document attached to a {} (not imported)",
                table_name(t)
            )),
        }
    }
    Ok(())
}

/// Unknown values of persons are attached last; events and places took theirs earlier.
fn attach_person_unknowns(b: &mut Builder) {
    let ids: Vec<(i64, Uuid)> = b.person_ids.iter().map(|(k, v)| (*k, *v)).collect();
    for (pid, id) in ids {
        let raw = b.unknowns.take(T_PERSON, pid);
        if let Some(p) = b.tree.persons.get_mut(&id) {
            p.common.unknown = raw;
        }
    }
}

#[cfg(test)]
mod patronymic_tests {
    use super::{NameParts, names_of, patronymic_out_of_surname};
    use gt_core::model::NameKind;

    fn parts(
        given: &str,
        surname: Option<&str>,
        patronymic: Option<&str>,
        maiden: Option<&str>,
    ) -> NameParts {
        NameParts {
            given: Some(given.into()),
            surname: surname.map(Into::into),
            patronymic: patronymic.map(Into::into),
            maiden: maiden.map(Into::into),
        }
    }

    #[test]
    fn female_patronymic_in_the_surname_field_becomes_the_patronymic() {
        let p = patronymic_out_of_surname(parts("Olena", Some("Ivanivna"), None, Some("Koval")));
        // Latin test words do not end in Cyrillic: nothing moves.
        assert_eq!(p.surname.as_deref(), Some("Ivanivna"));
        let p = patronymic_out_of_surname(parts("Олена", Some("Іванівна"), None, Some("Коваль")));
        assert_eq!(p.surname, None);
        assert_eq!(p.patronymic.as_deref(), Some("Іванівна"));
        let names = names_of(&[(None, p)]);
        assert_eq!(names.len(), 1, "no made-up married name");
        assert_eq!(names[0].kind, NameKind::Birth);
        assert_eq!(names[0].surnames, vec!["Коваль".to_string()]);
        assert_eq!(names[0].patronymic.as_deref(), Some("Іванівна"));
    }

    #[test]
    fn real_surnames_and_filled_patronymics_are_left_alone() {
        // Male -ович is also a surname.
        let p = patronymic_out_of_surname(parts("Петро", Some("Антонович"), None, None));
        assert_eq!(p.surname.as_deref(), Some("Антонович"));
        // The patronymic field is already filled: the surname stays a surname.
        let p = patronymic_out_of_surname(parts("Олена", Some("Петрівна"), Some("Іванівна"), None));
        assert_eq!(p.surname.as_deref(), Some("Петрівна"));
        // An ordinary surname.
        let p = patronymic_out_of_surname(parts("Олена", Some("Коваль"), None, Some("Мельник")));
        assert_eq!(p.surname.as_deref(), Some("Коваль"));
        // Married name stays when both are real surnames.
        let names = names_of(&[(None, p)]);
        assert_eq!(
            names.iter().filter(|n| n.kind == NameKind::Married).count(),
            1
        );
    }
}
