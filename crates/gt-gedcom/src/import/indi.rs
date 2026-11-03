//! INDI records: names, events (shared with FAM), sex.

use super::{
    Ctx, Rec, assertion_child, child, common_child, link_child, new_assertion, new_common,
    parse_uuid, place_from_text, push_unknown, read_date, resolve, val,
};
use crate::idgen::derive_id;
use crate::maps::{name_kind_from_text, role_from_text};
use crate::node::GedNode;
use crate::tags::event_kind;
use gt_core::ids::new_id;
use gt_core::model::*;
use uuid::Uuid;

fn sex_of(v: Option<&str>) -> Sex {
    match v.map(str::trim) {
        None => Sex::Unknown,
        Some(s) => match s.to_ascii_uppercase().as_str() {
            "M" => Sex::Male,
            "F" => Sex::Female,
            "X" => Sex::Intersex,
            "U" | "" => Sex::Unknown,
            _ => Sex::Other(s.to_string()),
        },
    }
}

pub(crate) fn read_indi(ctx: &mut Ctx, node: &GedNode, id: Uuid) {
    let mut common = new_common(id);
    if let Some(x) = &node.xref {
        common.ext_ids.push(ExternalId {
            kind: ExtIdKind::Xref,
            value: x.clone(),
        });
    }
    let mut sex = Sex::Unknown;
    let mut names: Vec<Name> = Vec::new();
    for (pos, c) in node.children.iter().enumerate() {
        match c.tag.as_str() {
            "NAME" => {
                let more = read_name(ctx, id, names.len(), c, names.is_empty());
                names.extend(more);
            }
            "SEX" => sex = sex_of(val(c)),
            "FAMC" => super::fam::note_famc(ctx, id, c),
            "FAMS" => super::fam::note_fams(ctx, id, c),
            _ if read_event_child(ctx, ParticipantRef::Person(id), c) => {}
            _ => {
                if !common_child(ctx, c, &mut common) {
                    push_unknown(ctx, &mut common.unknown, c, pos);
                }
            }
        }
    }
    ctx.tree.persons.insert(
        id,
        Person {
            common,
            sex,
            names,
            living_override: None,
        },
    );
}

/// If `c` is an event structure (`BIRT`, `EVEN`, `NO MARR`, ...), read it and return true.
pub(crate) fn read_event_child(ctx: &mut Ctx, owner: ParticipantRef, c: &GedNode) -> bool {
    let tag = c.tag.as_str();
    if matches!(tag, "EVEN" | "FACT") {
        read_event(ctx, owner, c, custom_kind(c), false);
        return true;
    }
    if let Some(k) = event_kind(tag) {
        read_event(ctx, owner, c, k, false);
        return true;
    }
    if matches!(tag, "NO" | "_NO") {
        let which = val(c).unwrap_or("").trim();
        let kind = if matches!(which, "EVEN" | "FACT") {
            Some(custom_kind(c))
        } else {
            event_kind(which)
        };
        if let Some(k) = kind {
            read_event(ctx, owner, c, k, true);
            return true;
        }
    }
    false
}

fn custom_kind(c: &GedNode) -> EventKind {
    EventKind::Custom(child(c, "TYPE").and_then(val).unwrap_or("").to_string())
}

fn some(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// Split `Given /Surname/ Suffix`.
fn split_name(v: &str) -> (Option<String>, Vec<String>, Option<String>) {
    match v.find('/') {
        None => (some(v), vec![], None),
        Some(a) => {
            let given = v.get(..a).and_then(some);
            let rest = v.get(a + 1..).unwrap_or("");
            let (sur, suffix) = match rest.find('/') {
                Some(b) => (
                    rest.get(..b).unwrap_or(""),
                    rest.get(b + 1..).and_then(some),
                ),
                None => (rest, None),
            };
            (given, some(sur).into_iter().collect(), suffix)
        }
    }
}

/// `Given /Surname/ Suffix` from the parts.
pub(crate) fn compose_name(n: &Name) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(g) = &n.given {
        parts.push(g.clone());
    }
    if !n.surnames.is_empty() || n.suffix.is_some() {
        parts.push(format!("/{}/", n.surnames.join(" ")));
    }
    if let Some(s) = &n.suffix {
        parts.push(s.clone());
    }
    parts.join(" ")
}

fn blank_name(id: Uuid, first: bool) -> Name {
    Name {
        id,
        kind: NameKind::Birth,
        assertion: Assertion {
            preferred: first,
            ..new_assertion()
        },
        private: false,
        prefix: None,
        given: None,
        call: None,
        surname_prefix: None,
        surnames: vec![],
        patronymic: None,
        suffix: None,
        nickname: None,
        lang: None,
        date: None,
        raw: None,
        citations: vec![],
        notes: vec![],
        unknown: vec![],
    }
}

/// A NAME node, plus the extra `Married` name for each `_MARNM` child.
fn read_name(ctx: &mut Ctx, person: Uuid, idx: usize, node: &GedNode, first: bool) -> Vec<Name> {
    let id = child(node, "_GTID")
        .and_then(val)
        .and_then(parse_uuid)
        .unwrap_or_else(|| derive_id(person, "name", idx));
    let mut n = blank_name(id, first);
    let mut extra: Vec<Name> = Vec::new();
    if let Some(v) = val(node) {
        let (g, s, x) = split_name(v);
        n.given = g;
        n.surnames = s;
        n.suffix = x;
        n.raw = Some(v.to_string());
    }
    let mut had_parts = false;
    for (pos, c) in node.children.iter().enumerate() {
        let v = val(c).map(str::trim).filter(|s| !s.is_empty());
        match (c.tag.as_str(), v) {
            ("NPFX", Some(v)) => n.prefix = Some(v.to_string()),
            ("GIVN", Some(v)) => {
                n.given = Some(v.to_string());
                had_parts = true;
            }
            ("NICK", Some(v)) => n.nickname = Some(v.to_string()),
            ("SPFX", Some(v)) => n.surname_prefix = Some(v.to_string()),
            ("SURN", Some(v)) => {
                n.surnames = v
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
                had_parts = true;
            }
            ("NSFX", Some(v)) => n.suffix = Some(v.to_string()),
            ("TYPE", Some(v)) => {
                let t = if v.eq_ignore_ascii_case("OTHER") {
                    child(c, "PHRASE").and_then(val).unwrap_or(v)
                } else {
                    v
                };
                n.kind = name_kind_from_text(t);
            }
            ("DATE", _) => n.date = read_date(ctx, c),
            ("RESN", Some(r))
                if r.eq_ignore_ascii_case("PRIVACY") || r.eq_ignore_ascii_case("CONFIDENTIAL") =>
            {
                n.private = true;
            }
            ("_GTID", _) => {}
            ("_MARNM", Some(v)) => {
                let mut m = blank_name(derive_id(person, "name", idx + 1 + extra.len()), false);
                m.kind = NameKind::Married;
                let raw = if v.contains('/') {
                    v.to_string()
                } else {
                    format!("/{v}/")
                };
                let (g, s, x) = split_name(&raw);
                m.given = g;
                m.surnames = s;
                m.suffix = x;
                m.raw = Some(raw);
                extra.push(m);
            }
            _ => {
                if assertion_child(&mut n.assertion, c) {
                    continue;
                }
                let mut cit = std::mem::take(&mut n.citations);
                let mut notes = std::mem::take(&mut n.notes);
                let done = link_child(ctx, id, c, &mut cit, &mut notes, None);
                n.citations = cit;
                n.notes = notes;
                if !done {
                    push_unknown(ctx, &mut n.unknown, c, pos);
                }
            }
        }
    }
    if n.raw.is_none() && (had_parts || n.given.is_some() || !n.surnames.is_empty()) {
        n.raw = Some(compose_name(&n));
    }
    let mut out = vec![n];
    out.extend(extra);
    out
}

fn add_participant(list: &mut Vec<Participant>, p: Participant) {
    if !list.contains(&p) {
        list.push(p);
    }
}

fn read_asso(ctx: &mut Ctx, c: &GedNode) -> Option<Participant> {
    let target = resolve(ctx, c, Rec::Indi)?;
    let role = if let Some(rn) = child(c, "ROLE")
        && let Some(r) = val(rn)
    {
        let text = if r.eq_ignore_ascii_case("OTHER") {
            child(rn, "PHRASE").and_then(val).unwrap_or(r)
        } else {
            r
        };
        role_from_text(text)
    } else if let Some(r) = child(c, "RELA").and_then(val) {
        role_from_text(r)
    } else {
        Role::Other(String::new())
    };
    Some(Participant {
        target: ParticipantRef::Person(target),
        role,
    })
}

/// Read one event structure belonging to `owner`, merging with an existing event on `_EVID`.
pub(crate) fn read_event(
    ctx: &mut Ctx,
    owner: ParticipantRef,
    node: &GedNode,
    kind: EventKind,
    negative: bool,
) {
    let custom = matches!(kind, EventKind::Custom(_));
    let evid = child(node, "_EVID")
        .and_then(val)
        .map(|s| s.trim().to_string());
    let id = match &evid {
        Some(k) => *ctx
            .evid_map
            .entry(k.clone())
            .or_insert_with(|| parse_uuid(k).unwrap_or_else(new_id)),
        None => new_id(),
    };
    let principal = Participant {
        target: owner,
        role: Role::Principal,
    };
    if ctx.tree.events.contains_key(&id) {
        // Another copy of a shared event: only add the participants.
        let mut extra = vec![principal];
        for c in node.children.iter().filter(|c| c.tag == "ASSO") {
            if let Some(p) = read_asso(ctx, c) {
                extra.push(p);
            }
        }
        if let Some(ev) = ctx.tree.events.get_mut(&id) {
            for p in extra {
                add_participant(&mut ev.participants, p);
            }
            ev.participants.sort_by_key(|p| p.role != Role::Principal);
        }
        return;
    }
    let mut common = new_common(id);
    let mut ev = Event {
        common: new_common(id),
        kind,
        date: None,
        place: None,
        description: if negative {
            None
        } else {
            val(node).map(str::to_string)
        },
        age: None,
        cause: None,
        negative,
        assertion: new_assertion(),
        participants: vec![principal],
    };
    for (pos, c) in node.children.iter().enumerate() {
        let v = val(c).map(str::to_string);
        match c.tag.as_str() {
            "DATE" if ev.date.is_none() => ev.date = read_date(ctx, c),
            "PLAC" if ev.place.is_none() && v.is_some() => {
                ev.place = v.map(|t| place_from_text(ctx, &t, c));
            }
            "AGE" if ev.age.is_none() && v.is_some() => ev.age = v,
            "CAUS" if ev.cause.is_none() && v.is_some() => ev.cause = v,
            "TYPE" if custom => {}
            "_EVID" => {}
            "ASSO" => {
                if let Some(p) = read_asso(ctx, c) {
                    add_participant(&mut ev.participants, p);
                }
            }
            _ => {
                if !assertion_child(&mut ev.assertion, c) && !common_child(ctx, c, &mut common) {
                    push_unknown(ctx, &mut common.unknown, c, pos);
                }
            }
        }
    }
    ev.common = common;
    ctx.tree.events.insert(id, ev);
}
