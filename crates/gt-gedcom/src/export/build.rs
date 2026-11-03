//! `Tree` -> GEDCOM node tree.

use crate::idgen::{derive_id, pair_id};
use crate::import::{DATE_PHRASE_NOTE, canonical_date_text};
use crate::maps::{name_kind_text, parent_rel_text, role_text};
use crate::node::GedNode;
use crate::tags::event_tag;
use gt_core::date::{DateValue, GedcomVersion, GenDate};
use gt_core::model::*;
use gt_core::raw::RawNode;
use gt_core::tree::Tree;
use std::collections::{BTreeSet, HashMap};
use uuid::Uuid;

pub(crate) const EXT_BASE: &str = "https://gene-tree.app/gedcom/ext/";
const OWN_SUBM: &str = "@GTSUBM@";

struct Bld<'a> {
    t: &'a Tree,
    v70: bool,
    ext: BTreeSet<String>,
    px: HashMap<Uuid, String>,
    fx: HashMap<Uuid, String>,
    sx: HashMap<Uuid, String>,
    rx: HashMap<Uuid, String>,
    ox: HashMap<Uuid, String>,
    nx: HashMap<Uuid, String>,
}

fn xrefs<'a, T: 'a>(
    prefix: &str,
    keys: impl Iterator<Item = (&'a Uuid, &'a T)>,
) -> HashMap<Uuid, String> {
    keys.enumerate()
        .map(|(i, (id, _))| (*id, format!("@{prefix}{}@", i + 1)))
        .collect()
}

fn raw_to_ged(r: &RawNode) -> GedNode {
    GedNode {
        line: 0,
        level: 0,
        xref: r.xref.clone(),
        tag: r.tag.clone(),
        value: r.value.clone(),
        ptr: r.value.as_deref().is_some_and(crate::import::is_pointer),
        children: r.children.iter().map(raw_to_ged).collect(),
    }
}

/// Put unknown nodes back among the known children: `pos` is the number of known siblings
/// that preceded the node in the source.
fn insert_unknown(children: &mut Vec<GedNode>, unknown: &[RawNode]) {
    let mut sorted: Vec<&RawNode> = unknown.iter().collect();
    sorted.sort_by_key(|u| u.pos);
    let known = std::mem::take(children);
    let mut pending = sorted.into_iter().peekable();
    let total = known.len();
    for (i, k) in known.into_iter().enumerate() {
        while let Some(u) = pending.next_if(|u| usize::try_from(u.pos).unwrap_or(usize::MAX) <= i) {
            children.push(raw_to_ged(u));
        }
        children.push(k);
    }
    debug_assert!(children.len() >= total);
    children.extend(pending.map(raw_to_ged));
}

/// Days since 1970-01-01 to (year, month, day).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

impl<'a> Bld<'a> {
    fn nd(&mut self, tag: &str, value: Option<String>) -> GedNode {
        if self.v70 && tag.starts_with('_') {
            self.ext.insert(tag.to_string());
        }
        GedNode {
            line: 0,
            level: 0,
            xref: None,
            tag: tag.to_string(),
            value: value.filter(|v| !v.is_empty()),
            ptr: false,
            children: vec![],
        }
    }

    /// A line whose value is a pointer to another record.
    fn ptr(&mut self, tag: &str, xref: String) -> GedNode {
        let mut n = self.nd(tag, Some(xref));
        n.ptr = true;
        n
    }

    fn kv(&mut self, tag: &str, value: &str) -> GedNode {
        self.nd(tag, Some(value.to_string()))
    }

    fn rec(&mut self, xref: Option<String>, tag: &str, value: Option<String>) -> GedNode {
        let mut n = self.nd(tag, value);
        n.xref = xref;
        n
    }

    fn gtid(&mut self, id: Uuid, derived: Option<Uuid>) -> Option<GedNode> {
        (derived != Some(id)).then(|| self.kv("_GTID", &id.to_string()))
    }

    // ---- shared pieces -------------------------------------------------------------------

    fn chan(&mut self, ms: i64) -> GedNode {
        let days = ms.div_euclid(86_400_000);
        let secs = ms.rem_euclid(86_400_000) / 1000;
        let (y, m, d) = civil_from_days(days);
        let month = usize::try_from(m - 1)
            .ok()
            .and_then(|i| MONTHS.get(i))
            .copied()
            .unwrap_or("JAN");
        let mut date = self.kv("DATE", &format!("{d} {month} {y}"));
        date.children.push(self.kv(
            "TIME",
            &format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60),
        ));
        let mut c = self.nd("CHAN", None);
        c.children.push(date);
        c
    }

    fn common_children(&mut self, c: &Common, out: &mut Vec<GedNode>) {
        if c.private {
            out.push(self.kv("RESN", "PRIVACY"));
        }
        for e in &c.ext_ids {
            let n = match &e.kind {
                ExtIdKind::Xref => continue,
                ExtIdKind::Uid => {
                    let tag = if self.v70 { "UID" } else { "_UID" };
                    self.kv(tag, &e.value)
                }
                ExtIdKind::FamilySearch => self.kv("_FSFTID", &e.value),
                ExtIdKind::AncestryApid => self.kv("_APID", &e.value),
                ExtIdKind::Exid { scheme } => {
                    let mut n = self.kv("EXID", &e.value);
                    if !scheme.is_empty() {
                        n.children.push(self.kv("TYPE", scheme));
                    }
                    n
                }
                ExtIdKind::Other(tag) => self.kv(tag, &e.value),
            };
            out.push(n);
        }
        for (i, id) in c.notes.iter().enumerate() {
            if let Some(n) = self.note_ref(c.id, i, *id) {
                out.push(n);
            }
        }
        for (i, id) in c.citations.iter().enumerate() {
            if let Some(n) = self.cit_ref(c.id, i, *id) {
                out.push(n);
            }
        }
        for l in &c.media {
            if let Some(n) = self.media_ref(l) {
                out.push(n);
            }
        }
        if let Some(ms) = c.changed {
            out.push(self.chan(ms));
        }
    }

    fn note_ref(&mut self, owner: Uuid, idx: usize, nid: Uuid) -> Option<GedNode> {
        let t = self.t;
        let n = t.notes.get(&nid)?;
        if n.shared {
            let x = self.nx.get(&nid)?.clone();
            let tag = if self.v70 { "SNOTE" } else { "NOTE" };
            return Some(self.ptr(tag, x));
        }
        let mut node = self.nd("NOTE", Some(n.text.clone()));
        self.note_body(n, &mut node, Some(derive_id(owner, "note", idx)));
        Some(node)
    }

    fn note_body(&mut self, n: &Note, node: &mut GedNode, derived: Option<Uuid>) {
        if let Some(l) = &n.lang {
            node.children.push(self.kv("LANG", l));
        }
        let mut kids = Vec::new();
        self.common_children(&n.common, &mut kids);
        kids.extend(self.gtid(n.common.id, derived));
        node.children.append(&mut kids);
        insert_unknown(&mut node.children, &n.common.unknown);
    }

    fn cit_ref(&mut self, owner: Uuid, idx: usize, cid: Uuid) -> Option<GedNode> {
        let t = self.t;
        let c = t.citations.get(&cid)?;
        let value = match c.source {
            Some(s) if self.sx.contains_key(&s) => self.sx.get(&s).cloned(),
            _ if self.v70 => Some("@VOID@".to_string()),
            _ => None,
        };
        let is_ptr = value.is_some();
        let mut n = self.nd("SOUR", value);
        n.ptr = is_ptr;
        if let Some(p) = &c.page {
            n.children.push(self.kv("PAGE", p));
        }
        if c.date.is_some() || c.text.is_some() {
            let mut d = self.nd("DATA", None);
            if let Some(g) = &c.date {
                let dn = self.date_node(g, None);
                d.children.push(dn);
            }
            if let Some(t) = &c.text {
                d.children.push(self.kv("TEXT", t));
            }
            n.children.push(d);
        }
        if let Some(q) = c.quality {
            n.children.push(self.kv("QUAY", &q.to_string()));
        }
        let mut kids = Vec::new();
        self.common_children(&c.common, &mut kids);
        kids.extend(self.gtid(cid, Some(derive_id(owner, "cit", idx))));
        n.children.append(&mut kids);
        insert_unknown(&mut n.children, &c.common.unknown);
        Some(n)
    }

    fn media_ref(&mut self, l: &MediaLink) -> Option<GedNode> {
        let x = self.ox.get(&l.media)?.clone();
        let mut n = self.ptr("OBJE", x);
        if l.primary {
            n.children.push(self.kv("_PRIM", "Y"));
        }
        if let Some(r) = &l.region {
            n.children
                .push(self.kv("_REGION", &format!("{} {} {} {}", r.x, r.y, r.w, r.h)));
        }
        Some(n)
    }

    /// A `DATE` node. In 5.5.1 a phrase the format cannot hold goes into `phrase_note` when the
    /// caller can place a note, else it is appended in parentheses.
    fn date_node(&mut self, g: &GenDate, phrase_note: Option<&mut Option<String>>) -> GedNode {
        if self.v70 {
            let (t, ph) = match g.value {
                DateValue::Unparsed => (String::new(), Some(g.raw.clone())),
                _ => g.to_gedcom(GedcomVersion::V70),
            };
            let mut n = self.nd("DATE", Some(t));
            if let Some(p) = ph {
                n.children.push(self.kv("PHRASE", &p));
            }
            return n;
        }
        let text = match (&g.value, &g.phrase, phrase_note) {
            (DateValue::Unparsed, _, _) => g.raw.clone(),
            (DateValue::Interpreted(_) | DateValue::PhraseOnly, _, _) | (_, None, _) => {
                g.to_gedcom(GedcomVersion::V551).0
            }
            (_, Some(p), Some(slot)) => {
                *slot = Some(p.clone());
                g.to_gedcom(GedcomVersion::V551).0
            }
            (_, Some(_), None) => canonical_date_text(g),
        };
        self.nd("DATE", Some(text))
    }

    fn place_node(&mut self, pid: Uuid) -> Option<GedNode> {
        let t = self.t;
        let leaf = t.places.get(&pid)?;
        let mut names: Vec<&str> = Vec::new();
        let mut cur = Some(leaf);
        let mut guard = 0;
        while let Some(p) = cur {
            names.push(p.names.first().map_or("", |n| n.name.as_str()));
            cur = p.parent.and_then(|id| t.places.get(&id));
            guard += 1;
            if guard > 64 {
                break;
            }
        }
        let mut n = self.nd("PLAC", Some(names.join(", ")));
        if let (Some(lat), Some(lon)) = (leaf.lat, leaf.lon) {
            let mut map = self.nd("MAP", None);
            let lat_s = format!("{}{}", if lat < 0.0 { 'S' } else { 'N' }, lat.abs());
            let lon_s = format!("{}{}", if lon < 0.0 { 'W' } else { 'E' }, lon.abs());
            map.children.push(self.kv("LATI", &lat_s));
            map.children.push(self.kv("LONG", &lon_s));
            n.children.push(map);
        }
        let mut kids = Vec::new();
        self.common_children(&leaf.common, &mut kids);
        n.children.append(&mut kids);
        insert_unknown(&mut n.children, &leaf.common.unknown);
        Some(n)
    }

    fn assertion_children(&mut self, a: &Assertion, skip_pref: bool, out: &mut Vec<GedNode>) {
        if a.preferred && !skip_pref {
            out.push(self.kv("_PREF", "Y"));
        }
        match a.proof {
            Some(Proof::Proven) => out.push(self.kv("_PROOF", "PROVEN")),
            Some(Proof::Disproven) => out.push(self.kv("_PROOF", "DISPROVEN")),
            None => {}
        }
        if let Some(q) = a.confidence {
            out.push(self.kv("QUAY", &q.to_string()));
        }
    }

    // ---- events --------------------------------------------------------------------------

    fn event_node(&mut self, ev: &Event) -> GedNode {
        let code = event_tag(&ev.kind, self.v70);
        let (tag, value) = if ev.negative {
            (if self.v70 { "NO" } else { "_NO" }, Some(code.to_string()))
        } else {
            (code, ev.description.clone())
        };
        let mut n = self.nd(tag, value);
        if let EventKind::Custom(t) = &ev.kind
            && !t.is_empty()
        {
            n.children.push(self.kv("TYPE", t));
        }
        let mut phrase_note = None;
        if let Some(g) = &ev.date {
            let d = self.date_node(g, Some(&mut phrase_note));
            n.children.push(d);
        }
        if let Some(note) = phrase_note {
            n.children
                .push(self.kv("NOTE", &format!("{DATE_PHRASE_NOTE}{note}")));
        }
        if let Some(p) = ev.place.and_then(|p| self.place_node(p)) {
            n.children.push(p);
        }
        if let Some(a) = &ev.age {
            n.children.push(self.kv("AGE", a));
        }
        if let Some(c) = &ev.cause {
            n.children.push(self.kv("CAUS", c));
        }
        for p in &ev.participants {
            if p.role == Role::Principal {
                continue;
            }
            let ParticipantRef::Person(pid) = p.target else {
                continue;
            };
            let Some(x) = self.px.get(&pid).cloned() else {
                continue;
            };
            let mut asso = self.ptr("ASSO", x);
            let (text, phrase) = role_text(&p.role, self.v70);
            if !text.is_empty() && p.role != Role::Other(String::new()) {
                let tag = if self.v70 { "ROLE" } else { "RELA" };
                let mut r = self.kv(tag, &text);
                if let Some(ph) = phrase {
                    r.children.push(self.kv("PHRASE", &ph));
                }
                asso.children.push(r);
            }
            n.children.push(asso);
        }
        let mut kids = Vec::new();
        self.assertion_children(&ev.assertion, false, &mut kids);
        kids.push(self.kv("_EVID", &ev.common.id.to_string()));
        self.common_children(&ev.common, &mut kids);
        n.children.append(&mut kids);
        insert_unknown(&mut n.children, &ev.common.unknown);
        n
    }

    // ---- records -------------------------------------------------------------------------

    fn name_node(&mut self, person: Uuid, idx: usize, nm: &Name) -> GedNode {
        let value = nm
            .raw
            .clone()
            .or_else(|| Some(crate::import::compose_name(nm)));
        let mut n = self.nd("NAME", value);
        if let Some((t, ph)) = name_kind_text(&nm.kind, self.v70) {
            let mut ty = self.kv("TYPE", &t);
            if let Some(p) = ph {
                ty.children.push(self.kv("PHRASE", &p));
            }
            n.children.push(ty);
        }
        let parts = [
            ("NPFX", nm.prefix.clone()),
            ("GIVN", nm.given.clone()),
            ("NICK", nm.nickname.clone()),
            ("SPFX", nm.surname_prefix.clone()),
            (
                "SURN",
                (!nm.surnames.is_empty()).then(|| nm.surnames.join(", ")),
            ),
            ("NSFX", nm.suffix.clone()),
        ];
        for (tag, v) in parts {
            if let Some(v) = v {
                n.children.push(self.kv(tag, &v));
            }
        }
        if let Some(g) = &nm.date {
            let d = self.date_node(g, None);
            n.children.push(d);
        }
        if nm.private {
            n.children.push(self.kv("RESN", "PRIVACY"));
        }
        let mut kids = Vec::new();
        self.assertion_children(&nm.assertion, idx == 0, &mut kids);
        for (i, id) in nm.notes.iter().enumerate() {
            if let Some(x) = self.note_ref(nm.id, i, *id) {
                kids.push(x);
            }
        }
        for (i, id) in nm.citations.iter().enumerate() {
            if let Some(x) = self.cit_ref(nm.id, i, *id) {
                kids.push(x);
            }
        }
        kids.extend(self.gtid(nm.id, Some(derive_id(person, "name", idx))));
        n.children.append(&mut kids);
        insert_unknown(&mut n.children, &nm.unknown);
        n
    }

    fn sex_text(s: &Sex) -> String {
        match s {
            Sex::Male => "M".into(),
            Sex::Female => "F".into(),
            Sex::Intersex => "X".into(),
            Sex::Unknown => "U".into(),
            Sex::Other(t) => t.clone(),
        }
    }
}

type EventsBy<'a> = HashMap<Uuid, Vec<&'a Event>>;

/// Build all top-level records for `t`.
pub(crate) fn build(t: &Tree, version: GedcomVersion) -> Vec<GedNode> {
    let v70 = version == GedcomVersion::V70;
    let mut b = Bld {
        t,
        v70,
        ext: BTreeSet::new(),
        px: xrefs("I", t.persons.iter()),
        fx: xrefs("F", t.families.iter()),
        sx: xrefs("S", t.sources.iter()),
        rx: xrefs("R", t.repositories.iter()),
        ox: xrefs("O", t.media.iter()),
        nx: xrefs("N", t.notes.iter().filter(|(_, n)| n.shared)),
    };

    // Events by principal.
    let mut by_person: EventsBy = HashMap::new();
    let mut by_family: EventsBy = HashMap::new();
    for ev in t.events.values() {
        for p in ev.participants.iter().filter(|p| p.role == Role::Principal) {
            let map = match p.target {
                ParticipantRef::Person(_) => &mut by_person,
                ParticipantRef::Family(_) => &mut by_family,
            };
            let id = match p.target {
                ParticipantRef::Person(i) | ParticipantRef::Family(i) => i,
            };
            let list = map.entry(id).or_default();
            if !list.last().is_some_and(|l| std::ptr::eq(*l, ev)) {
                list.push(ev);
            }
        }
    }
    // FAMC / FAMS per person.
    let mut famc: HashMap<Uuid, Vec<(Uuid, usize)>> = HashMap::new();
    let mut fams: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for (fid, f) in &t.families {
        for p in [f.partner1, f.partner2].into_iter().flatten() {
            let l = fams.entry(p).or_default();
            if !l.contains(fid) {
                l.push(*fid);
            }
        }
        for (i, c) in f.children.iter().enumerate() {
            famc.entry(c.person).or_default().push((*fid, i));
        }
    }

    let mut out: Vec<GedNode> = Vec::new();
    let mut tail: Vec<GedNode> = Vec::new();

    // INDI
    for (pid, p) in &t.persons {
        let mut n = b.rec(b.px.get(pid).cloned(), "INDI", None);
        for (i, nm) in p.names.iter().enumerate() {
            let nn = b.name_node(*pid, i, nm);
            n.children.push(nn);
        }
        if p.sex == Sex::Intersex && !b.v70 {
            // 5.5.1 has no X (invalid value): keep it in an extension tag.
            n.children.push(b.kv("SEX", "U"));
            n.children.push(b.kv("_SEX", "X"));
        } else {
            n.children.push(b.kv("SEX", &Bld::sex_text(&p.sex)));
        }
        let mut common = Vec::new();
        // ext ids and privacy first, then events, links, notes/citations/media.
        let mut c0 = p.common.clone();
        c0.notes.clear();
        c0.citations.clear();
        c0.media.clear();
        c0.changed = None;
        b.common_children(&c0, &mut common);
        n.children.append(&mut common);
        for ev in by_person.get(pid).into_iter().flatten() {
            let en = b.event_node(ev);
            n.children.push(en);
        }
        for (fid, idx) in famc.get(pid).into_iter().flatten() {
            let Some(f) = t.families.get(fid) else {
                continue;
            };
            let Some(l) = f.children.get(*idx) else {
                continue;
            };
            let Some(x) = b.fx.get(fid).cloned() else {
                continue;
            };
            let mut fc = b.ptr("FAMC", x);
            famc_children(&mut b, l, *pid, *fid, &mut fc);
            n.children.push(fc);
        }
        for fid in fams.get(pid).into_iter().flatten() {
            if let Some(x) = b.fx.get(fid).cloned() {
                let s = b.ptr("FAMS", x);
                n.children.push(s);
            }
        }
        let mut rest = Vec::new();
        let mut c1 = Common {
            private: false,
            ext_ids: vec![],
            ..p.common.clone()
        };
        c1.unknown.clear();
        b.common_children(&c1, &mut rest);
        rest.extend(b.gtid(*pid, None));
        n.children.append(&mut rest);
        insert_unknown(&mut n.children, &p.common.unknown);
        out.push(n);
    }

    // FAM
    for (fid, f) in &t.families {
        let mut n = b.rec(b.fx.get(fid).cloned(), "FAM", None);
        let sex = |id: Option<Uuid>| id.and_then(|i| t.persons.get(&i)).map(|p| p.sex.clone());
        let (mut p1, mut p2) = (f.partner1, f.partner2);
        if sex(p1) == Some(Sex::Female) && sex(p2) == Some(Sex::Male) {
            std::mem::swap(&mut p1, &mut p2);
        }
        if let Some(x) = p1.and_then(|p| b.px.get(&p).cloned()) {
            n.children.push(b.ptr("HUSB", x));
        }
        if let Some(x) = p2.and_then(|p| b.px.get(&p).cloned()) {
            n.children.push(b.ptr("WIFE", x));
        }
        for c in &f.children {
            if let Some(x) = b.px.get(&c.person).cloned() {
                n.children.push(b.ptr("CHIL", x));
            }
        }
        for ev in by_family.get(fid).into_iter().flatten() {
            let en = b.event_node(ev);
            n.children.push(en);
        }
        let mut rest = Vec::new();
        b.common_children(&f.common, &mut rest);
        rest.extend(b.gtid(*fid, None));
        n.children.append(&mut rest);
        insert_unknown(&mut n.children, &f.common.unknown);
        out.push(n);
    }

    // SOUR
    for (sid, s) in &t.sources {
        let mut n = b.rec(b.sx.get(sid).cloned(), "SOUR", None);
        for (tag, v) in [
            ("TITL", &s.title),
            ("AUTH", &s.author),
            ("PUBL", &s.publication),
            ("ABBR", &s.abbreviation),
            ("TEXT", &s.text),
        ] {
            if let Some(v) = v {
                n.children.push(b.kv(tag, v));
            }
        }
        for r in &s.repositories {
            if let Some(x) = b.rx.get(&r.repository).cloned() {
                let mut rn = b.ptr("REPO", x);
                if let Some(c) = &r.call_number {
                    rn.children.push(b.kv("CALN", c));
                }
                n.children.push(rn);
            }
        }
        let mut rest = Vec::new();
        b.common_children(&s.common, &mut rest);
        rest.extend(b.gtid(*sid, None));
        n.children.append(&mut rest);
        insert_unknown(&mut n.children, &s.common.unknown);
        out.push(n);
    }

    // REPO
    for (rid, r) in &t.repositories {
        let mut n = b.rec(b.rx.get(rid).cloned(), "REPO", None);
        n.children.push(b.nd("NAME", Some(r.name.clone())));
        if let Some(a) = &r.address {
            n.children.push(b.kv("ADDR", a));
        }
        for u in &r.urls {
            n.children.push(b.kv("WWW", u));
        }
        let mut rest = Vec::new();
        b.common_children(&r.common, &mut rest);
        rest.extend(b.gtid(*rid, None));
        n.children.append(&mut rest);
        insert_unknown(&mut n.children, &r.common.unknown);
        out.push(n);
    }

    // OBJE
    for (oid, m) in &t.media {
        let mut n = b.rec(b.ox.get(oid).cloned(), "OBJE", None);
        let mut file = b.kv("FILE", &m.path);
        if let Some(mime) = &m.mime {
            file.children.push(b.kv("FORM", mime));
        }
        if v70 {
            if let Some(t) = &m.title {
                file.children.push(b.kv("TITL", t));
            }
            n.children.push(file);
        } else {
            n.children.push(file);
            if let Some(t) = &m.title {
                n.children.push(b.kv("TITL", t));
            }
        }
        let mut rest = Vec::new();
        b.common_children(&m.common, &mut rest);
        rest.extend(b.gtid(*oid, None));
        n.children.append(&mut rest);
        insert_unknown(&mut n.children, &m.common.unknown);
        out.push(n);
    }

    // Shared notes.
    for (nid, note) in t.notes.iter().filter(|(_, n)| n.shared) {
        let tag = if v70 { "SNOTE" } else { "NOTE" };
        let mut n = b.rec(b.nx.get(nid).cloned(), tag, Some(note.text.clone()));
        b.note_body(note, &mut n, None);
        out.push(n);
    }

    // Unknown records travel with the header (pos == u32::MAX marks a record).
    let mut head_unknown: Vec<RawNode> = Vec::new();
    for u in &t.header.unknown {
        if u.pos == u32::MAX {
            tail.push(raw_to_ged(u));
        } else {
            head_unknown.push(u.clone());
        }
    }
    let has_subm = tail.iter().any(|r| r.tag == "SUBM");

    // HEAD
    let mut head = b.nd("HEAD", None);
    let mut src = b.kv("SOUR", "GENE-TREE");
    src.children.push(b.kv("VERS", env!("CARGO_PKG_VERSION")));
    if let Some(p) = &t.header.producer {
        let mut d = b.kv("DATA", p);
        if let Some(v) = &t.header.source_version {
            d.children.push(b.kv("VERS", v));
        }
        src.children.push(d);
    }
    head.children.push(src);
    let mut gedc = b.nd("GEDC", None);
    gedc.children
        .push(b.kv("VERS", if v70 { "7.0" } else { "5.5.1" }));
    if !v70 {
        gedc.children.push(b.kv("FORM", "LINEAGE-LINKED"));
    }
    head.children.push(gedc);
    if !v70 {
        head.children.push(b.kv("CHAR", "UTF-8"));
        if !has_subm {
            head.children.push(b.ptr("SUBM", OWN_SUBM.to_string()));
            let mut s = b.rec(Some(OWN_SUBM.to_string()), "SUBM", None);
            s.children.push(b.kv("NAME", "gene-tree"));
            tail.push(s);
        }
    } else if !b.ext.is_empty() {
        let mut schma = b.nd("SCHMA", None);
        let tags: Vec<String> = b.ext.iter().cloned().collect();
        for tag in tags {
            let uri = format!("{EXT_BASE}{}", tag.trim_start_matches('_'));
            schma.children.push(b.kv("TAG", &format!("{tag} {uri}")));
        }
        head.children.push(schma);
    }
    head.children.extend(head_unknown.iter().map(raw_to_ged));

    let mut all = vec![head];
    all.append(&mut out);
    all.append(&mut tail);
    all.push(b.nd("TRLR", None));
    all
}

fn famc_children(b: &mut Bld, l: &ChildLink, person: Uuid, fam: Uuid, fc: &mut GedNode) {
    let pedi = |r: &ParentRel| match r {
        ParentRel::Birth => Some("birth"),
        ParentRel::Adopted => Some("adopted"),
        ParentRel::Foster => Some("foster"),
        ParentRel::Sealed => Some("sealing"),
        _ => None,
    };
    if l.rel1 == l.rel2
        && let Some(p) = pedi(&l.rel1)
    {
        if l.rel1 != ParentRel::Birth {
            let v = if b.v70 {
                p.to_uppercase()
            } else {
                p.to_string()
            };
            fc.children.push(b.kv("PEDI", &v));
        }
    } else {
        let f = parent_rel_text(&l.rel1);
        let m = parent_rel_text(&l.rel2);
        fc.children.push(b.kv("_FREL", &f));
        fc.children.push(b.kv("_MREL", &m));
    }
    let mut kids = Vec::new();
    if l.preferred {
        kids.push(b.kv("_PREF", "Y"));
    }
    let a = Assertion {
        preferred: false,
        ..l.assertion.clone()
    };
    b.assertion_children(&a, true, &mut kids);
    let owner = pair_id(person, fam);
    for (i, id) in l.citations.iter().enumerate() {
        if let Some(x) = b.cit_ref(owner, i, *id) {
            kids.push(x);
        }
    }
    fc.children.append(&mut kids);
    insert_unknown(&mut fc.children, &l.unknown);
}
