//! GEDCOM -> `Tree` import. Never fails; problems are repaired and reported.

mod fam;
mod header;
mod indi;
mod media_paths;
mod note;
mod obje;
mod repair;
pub mod report;
mod sour;

use crate::idgen::place_id;
use crate::node::GedNode;
use crate::tokenizer::tokenize;
use gt_core::DayOrder;
use gt_core::date::{DateValue, GedcomVersion, GenDate};
use gt_core::ids::new_id;
use gt_core::model::*;
use gt_core::raw::RawNode;
use gt_core::tree::Tree;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use uuid::Uuid;

pub(crate) use indi::compose_name;
pub use report::{ImportReport, MissingMedia, MissingReason, Repair};

#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub day_order: DayOrder,
    pub media_roots: Vec<PathBuf>,
    /// Folder holding the GEDCOM file: absolute and relative `FILE` paths may resolve under it.
    pub gedcom_dir: Option<PathBuf>,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            day_order: DayOrder::Dmy,
            media_roots: Vec::new(),
            gedcom_dir: None,
        }
    }
}

#[derive(Debug)]
pub struct ImportOutcome {
    pub tree: Tree,
    pub report: ImportReport,
    /// `(Media id, absolute source file)`, filled by media path resolution.
    pub resolved_media: Vec<(Uuid, PathBuf)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rec {
    Indi,
    Fam,
    Sour,
    Repo,
    Obje,
    Note,
    Other,
}

/// `(order key, family, child index)` for choosing a person's preferred parent link.
pub(crate) type PrefCand = ((u8, usize), Uuid, usize);

pub(crate) struct Ctx {
    pub tree: Tree,
    pub report: ImportReport,
    pub xref_map: HashMap<String, Uuid>,
    pub xref_kind: HashMap<String, Rec>,
    pub opts: ImportOptions,
    #[allow(dead_code)]
    pub version: GedcomVersion,
    pub place_map: HashMap<String, Uuid>,
    pub evid_map: HashMap<String, Uuid>,
    pub resolved_media: Vec<(Uuid, PathBuf)>,
    pub media_index: Option<HashMap<String, Vec<PathBuf>>>,
    pub id_xref: HashMap<Uuid, String>,
    pub famc: HashMap<(Uuid, Uuid), fam::FamcInfo>,
    pub famc_order: HashMap<(Uuid, Uuid), usize>,
    pub famc_count: HashMap<Uuid, usize>,
    pub famc_by_fam: HashMap<Uuid, Vec<Uuid>>,
    pub fams: HashSet<(Uuid, Uuid)>,
    pub fams_by_fam: HashMap<Uuid, Vec<Uuid>>,
    pub pref_cands: HashMap<Uuid, Vec<PrefCand>>,
    pub fam_seq: usize,
}

/// Value of a node, `None` when absent or empty.
pub(crate) fn val(n: &GedNode) -> Option<&str> {
    n.value.as_deref().filter(|s| !s.is_empty())
}

pub(crate) fn child<'a>(n: &'a GedNode, tag: &str) -> Option<&'a GedNode> {
    n.children.iter().find(|c| c.tag == tag)
}

pub(crate) fn is_pointer(s: &str) -> bool {
    let s = s.trim();
    s.len() >= 3
        && s.starts_with('@')
        && s.ends_with('@')
        && !s.get(1..s.len() - 1).is_some_and(|m| m.contains('@'))
}

pub(crate) fn pos32(pos: usize) -> u32 {
    u32::try_from(pos).unwrap_or(u32::MAX)
}

pub(crate) fn new_common(id: Uuid) -> Common {
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

pub(crate) fn new_assertion() -> Assertion {
    Assertion {
        preferred: false,
        proof: None,
        confidence: None,
    }
}

/// Keep a node nobody understands, and count it.
/// `RawNode::pos` of an unknown node that follows every known sibling.
pub(crate) const TAIL_POS: u32 = u32::MAX - 1;

pub(crate) fn push_unknown(
    ctx: &mut Ctx,
    into: &mut Vec<RawNode>,
    c: &GedNode,
    pos: usize,
    siblings: usize,
) {
    count_dropped(ctx, &c.tag);
    // `pos` counts the known siblings that precede the node (earlier unknown siblings are in
    // `into`), so it stays stable when the exporter emits unknown nodes among its own children.
    // A node after which no known sibling follows is anchored to the tail (`TAIL_POS`).
    let known_before = if pos == usize::MAX {
        pos
    } else {
        pos.saturating_sub(into.len())
    };
    let tail = pos != usize::MAX && pos + 1 == siblings;
    if tail {
        // The run of unknown nodes at the very end (same known count) is anchored to the tail.
        let k = pos32(known_before);
        for u in into.iter_mut().rev().take_while(|u| u.pos == k) {
            u.pos = TAIL_POS;
        }
    }
    let at = if tail { TAIL_POS } else { pos32(known_before) };
    into.push(c.to_raw(at));
}

/// Count a node that was not mapped (also used for nodes that had no place to be kept).
pub(crate) fn count_dropped(ctx: &mut Ctx, tag: &str) {
    *ctx.report.unmapped_tags.entry(tag.to_string()).or_insert(0) += 1;
}

/// Resolve a pointer to a record of kind `want`; reports dangling pointers.
/// `@VOID@` resolves to nothing without a repair.
pub(crate) fn resolve(ctx: &mut Ctx, c: &GedNode, want: Rec) -> Option<Uuid> {
    let p = val(c)?.trim();
    if p.eq_ignore_ascii_case("@VOID@") {
        return None;
    }
    match (ctx.xref_map.get(p), ctx.xref_kind.get(p)) {
        (Some(id), Some(k)) if *k == want => Some(*id),
        _ => {
            ctx.report.repairs.push(Repair::DanglingPointer {
                line: c.line,
                xref: p.to_string(),
            });
            None
        }
    }
}

pub(crate) fn parse_uuid(s: &str) -> Option<Uuid> {
    Uuid::parse_str(s.trim()).ok()
}

/// A date node (value plus 7.0 `PHRASE`), with `raw` canonicalised so exports re-import equal.
pub(crate) fn read_date(ctx: &mut Ctx, node: &GedNode) -> Option<GenDate> {
    let order = ctx.opts.day_order;
    let text = node.value.as_deref().unwrap_or("").trim().to_string();
    let phrase = child(node, "PHRASE").and_then(val).map(str::to_string);
    let mut g = match (text.is_empty(), phrase) {
        (true, None) => return None,
        (true, Some(p)) => GenDate::parse(&format!("({p})"), order),
        (false, None) => GenDate::parse(&text, order),
        (false, Some(p)) => {
            let alt = GenDate::parse(&p, order);
            let usable = !matches!(alt.value, DateValue::Unparsed | DateValue::PhraseOnly)
                && alt.to_gedcom(GedcomVersion::V70).0 == text;
            if usable {
                alt
            } else {
                GenDate::parse(&format!("{text} ({p})"), order)
            }
        }
    };
    // Normal forms, so that export and re-import agree:
    // - a phrase next to an exact date is an interpreted date;
    // - a phrase that is not itself a date is kept as unparsed text.
    match g.value.clone() {
        DateValue::Exact(d) if g.phrase.is_some() => g.value = DateValue::Interpreted(d),
        DateValue::PhraseOnly => {
            let p = g.phrase.clone()?;
            let alt = GenDate::parse(&p, order);
            if matches!(alt.value, DateValue::Unparsed) {
                g = alt;
            }
        }
        _ => {}
    }
    if matches!(g.value, DateValue::Unparsed) {
        ctx.report.unparsed_dates.push((node.line, g.raw.clone()));
    } else {
        g.raw = canonical_date_text(&g);
    }
    Some(g)
}

/// Prefix of the note the 5.5.1 exporter writes for a date phrase it cannot express.
pub const DATE_PHRASE_NOTE: &str = "Date phrase: ";

/// The 5.5.1 text for a date, with any phrase the format would drop appended in parentheses.
pub fn canonical_date_text(g: &GenDate) -> String {
    let (t, _) = g.to_gedcom(GedcomVersion::V551);
    match (&g.value, &g.phrase) {
        (DateValue::Interpreted(_) | DateValue::PhraseOnly, _) | (_, None) => t,
        (_, Some(p)) => format!("{t} ({p})"),
    }
}

fn chan_ms(node: &GedNode, order: DayOrder) -> Option<i64> {
    let dn = child(node, "DATE")?;
    let g = GenDate::parse(val(dn)?, order);
    let jdn = match &g.value {
        DateValue::Exact(d) if d.day.is_some() && d.month.is_some() => d.jdn_range()?.0,
        _ => return None,
    };
    let days = i64::from(jdn) - 2_440_588;
    let mut ms = 0i64;
    if let Some(t) = child(dn, "TIME").and_then(val) {
        let mut it = t.split(':');
        let h: f64 = it.next()?.trim().parse().ok()?;
        let m: f64 = it.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);
        let s: f64 = it.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);
        let total = (h * 3600.0 + m * 60.0 + s) * 1000.0;
        if !total.is_finite() || !(0.0..=90_000_000.0).contains(&total) {
            return None;
        }
        ms = total as i64;
    }
    days.checked_mul(86_400_000)?.checked_add(ms)
}

fn ext_id(common: &mut Common, kind: ExtIdKind, c: &GedNode) -> bool {
    match val(c) {
        Some(v) => {
            common.ext_ids.push(ExternalId {
                kind,
                value: v.to_string(),
            });
            true
        }
        None => false,
    }
}

/// Handle the children every record-like structure shares. Returns true when consumed.
pub(crate) fn common_child(ctx: &mut Ctx, c: &GedNode, common: &mut Common) -> bool {
    match c.tag.as_str() {
        "_GTID" => true,
        "RESN" => {
            let v = val(c).unwrap_or("").trim().to_ascii_uppercase();
            if v == "PRIVACY" || v == "CONFIDENTIAL" {
                common.private = true;
                true
            } else {
                false
            }
        }
        "_UID" | "UID" => ext_id(common, ExtIdKind::Uid, c),
        "_FSFTID" => ext_id(common, ExtIdKind::FamilySearch, c),
        "EXID" => {
            let scheme = child(c, "TYPE").and_then(val).unwrap_or("").to_string();
            ext_id(common, ExtIdKind::Exid { scheme }, c)
        }
        "CHAN" => match chan_ms(c, ctx.opts.day_order) {
            Some(ms) => {
                common.changed = Some(ms);
                true
            }
            None => false,
        },
        _ => {
            let id = common.id;
            let mut cit = std::mem::take(&mut common.citations);
            let mut notes = std::mem::take(&mut common.notes);
            let mut media = std::mem::take(&mut common.media);
            let done = link_child(ctx, id, c, &mut cit, &mut notes, Some(&mut media));
            common.citations = cit;
            common.notes = notes;
            common.media = media;
            done
        }
    }
}

/// Citations, notes and media links under a structure.
pub(crate) fn link_child(
    ctx: &mut Ctx,
    owner: Uuid,
    c: &GedNode,
    citations: &mut Vec<Uuid>,
    notes: &mut Vec<Uuid>,
    media: Option<&mut Vec<MediaLink>>,
) -> bool {
    match c.tag.as_str() {
        "SOUR" => sour::link_citation(ctx, owner, c, citations),
        "NOTE" | "SNOTE" => note::link_note(ctx, owner, c, notes),
        "OBJE" => match media {
            Some(m) => obje::link_media(ctx, owner, c, m),
            None => false,
        },
        _ => false,
    }
}

/// Assertion children (`_PREF`, `_PROOF`, `QUAY`).
pub(crate) fn assertion_child(a: &mut Assertion, c: &GedNode) -> bool {
    let v = val(c).unwrap_or("").trim().to_ascii_uppercase();
    match c.tag.as_str() {
        "_PREF" if v == "Y" => {
            a.preferred = true;
            true
        }
        "_PROOF" if v == "PROVEN" => {
            a.proof = Some(Proof::Proven);
            true
        }
        "_PROOF" if v == "DISPROVEN" => {
            a.proof = Some(Proof::Disproven);
            true
        }
        "QUAY" => match v.parse::<u8>() {
            Ok(q) if q <= 3 => {
                a.confidence = Some(q);
                true
            }
            _ => false,
        },
        _ => false,
    }
}

/// Build the place chain for `text` and return the leaf place id (deduplicated by chain text).
pub(crate) fn place_from_text(ctx: &mut Ctx, text: &str, node: &GedNode) -> Uuid {
    let parts: Vec<&str> = text.split(',').map(str::trim).collect();
    let mut parent: Option<Uuid> = None;
    let mut created = false;
    for i in (0..parts.len()).rev() {
        let key = parts.get(i..).unwrap_or(&[]).join(", ");
        created = false;
        let id = match ctx.place_map.get(&key) {
            Some(id) => *id,
            None => {
                let id = place_id(&key);
                ctx.tree.places.insert(
                    id,
                    Place {
                        common: new_common(id),
                        names: vec![PlaceName {
                            name: parts.get(i).copied().unwrap_or("").to_string(),
                            lang: None,
                            date: None,
                        }],
                        parent,
                        kind: None,
                        lat: None,
                        lon: None,
                        raw: Some(key.clone()),
                    },
                );
                ctx.place_map.insert(key, id);
                created = true;
                id
            }
        };
        parent = Some(id);
    }
    let leaf = parent.unwrap_or_else(new_id);
    let mut common = match ctx.tree.places.get_mut(&leaf) {
        Some(p) => std::mem::replace(&mut p.common, new_common(leaf)),
        None => return leaf,
    };
    for (pos, c) in node.children.iter().enumerate() {
        if c.tag == "MAP" {
            if let Some(p) = ctx.tree.places.get_mut(&leaf)
                && p.lat.is_none()
                && apply_map(c, p)
            {
                continue;
            }
            if !created {
                continue;
            }
        } else if !created {
            continue;
        }
        if !common_child(ctx, c, &mut common) {
            push_unknown(ctx, &mut common.unknown, c, pos, node.children.len());
        }
    }
    if let Some(p) = ctx.tree.places.get_mut(&leaf) {
        p.common = common;
    }
    leaf
}

fn coord(s: &str, pos: char, neg: char) -> Option<f64> {
    let s = s.trim();
    let first = s.chars().next()?.to_ascii_uppercase();
    let (sign, rest) = if first == pos {
        (1.0, s.get(1..)?)
    } else if first == neg {
        (-1.0, s.get(1..)?)
    } else {
        (1.0, s)
    };
    let v: f64 = rest.trim().parse().ok()?;
    v.is_finite().then_some(sign * v)
}

fn apply_map(map: &GedNode, p: &mut Place) -> bool {
    let lat = child(map, "LATI")
        .and_then(val)
        .and_then(|v| coord(v, 'N', 'S'));
    let lon = child(map, "LONG")
        .and_then(val)
        .and_then(|v| coord(v, 'E', 'W'));
    match (lat, lon) {
        (Some(a), Some(b)) => {
            p.lat = Some(a);
            p.lon = Some(b);
            true
        }
        _ => false,
    }
}

fn enc_name(e: &crate::encoding::DetectedEncoding) -> String {
    use crate::encoding::DetectedEncoding::*;
    match e {
        Utf8 { bom: true } => "UTF-8 (BOM)".into(),
        Utf8 { bom: false } => "UTF-8".into(),
        Utf16Le => "UTF-16LE".into(),
        Utf16Be => "UTF-16BE".into(),
        Ansel => "ANSEL".into(),
        Windows1252 => "Windows-1252".into(),
        Other(e) => e.name().to_string(),
    }
}

fn rec_kind(tag: &str) -> Rec {
    match tag {
        "INDI" => Rec::Indi,
        "FAM" => Rec::Fam,
        "SOUR" => Rec::Sour,
        "REPO" => Rec::Repo,
        "OBJE" => Rec::Obje,
        "NOTE" | "SNOTE" => Rec::Note,
        _ => Rec::Other,
    }
}

/// Import GEDCOM bytes. Never fails: problems become [`Repair`]s in the report.
pub fn import(bytes: &[u8], opts: &ImportOptions) -> ImportOutcome {
    let tok = tokenize(bytes);
    let head = tok.records.iter().find(|r| r.tag == "HEAD");
    let (version, vers) = header::detect_version(head);
    let mut ctx = Ctx {
        tree: Tree::new(),
        report: ImportReport {
            encoding: enc_name(&tok.encoding),
            version: vers,
            ..ImportReport::default()
        },
        xref_map: HashMap::new(),
        xref_kind: HashMap::new(),
        opts: opts.clone(),
        version,
        place_map: HashMap::new(),
        evid_map: HashMap::new(),
        resolved_media: Vec::new(),
        media_index: None,
        id_xref: HashMap::new(),
        famc: HashMap::new(),
        famc_order: HashMap::new(),
        famc_count: HashMap::new(),
        famc_by_fam: HashMap::new(),
        fams: HashSet::new(),
        fams_by_fam: HashMap::new(),
        pref_cands: HashMap::new(),
        fam_seq: 0,
    };
    ctx.report
        .repairs
        .extend(tok.issues.iter().cloned().map(Repair::Token));
    if head.is_none() {
        ctx.report.repairs.push(Repair::MissingHeader);
    }
    if !tok.records.iter().any(|r| r.tag == "TRLR") {
        ctx.report.repairs.push(Repair::MissingTrailer);
    }

    // Pass 1: an id for every xref.
    let mut used: HashSet<Uuid> = HashSet::new();
    let mut ids: Vec<Uuid> = Vec::with_capacity(tok.records.len());
    for r in &tok.records {
        let mut id = child(r, "_GTID")
            .and_then(val)
            .and_then(parse_uuid)
            .filter(|u| !used.contains(u))
            .unwrap_or_else(new_id);
        if let Some(x) = &r.xref {
            if ctx.xref_map.contains_key(x) {
                ctx.report.repairs.push(Repair::DuplicateXref {
                    line: r.line,
                    xref: x.clone(),
                });
                id = new_id();
            } else {
                ctx.xref_map.insert(x.clone(), id);
                ctx.id_xref.insert(id, x.clone());
                ctx.xref_kind.insert(x.clone(), rec_kind(&r.tag));
            }
        }
        used.insert(id);
        ids.push(id);
    }

    // Pass 2.
    if let Some(h) = head {
        header::read_head(&mut ctx, h);
    }
    for r in &tok.records {
        if !matches!(r.tag.as_str(), "HEAD" | "TRLR") {
            *ctx.report.counts.entry(r.tag.clone()).or_insert(0) += 1;
        }
    }
    let recs: Vec<(&GedNode, Uuid, usize)> = tok
        .records
        .iter()
        .zip(ids.iter().copied())
        .enumerate()
        .map(|(i, (r, id))| (r, id, i))
        .collect();
    for (r, id, _) in &recs {
        if r.tag == "INDI" {
            indi::read_indi(&mut ctx, r, *id);
        }
    }
    for (r, id, _) in &recs {
        if r.tag == "FAM" {
            fam::read_fam(&mut ctx, r, *id);
        }
    }
    repair::finish_preferred_links(&mut ctx);
    for (r, id, _) in &recs {
        match r.tag.as_str() {
            "SOUR" => sour::read_source(&mut ctx, r, *id),
            "REPO" => sour::read_repository(&mut ctx, r, *id),
            "OBJE" => obje::read_media_record(&mut ctx, r, *id),
            "NOTE" | "SNOTE" => note::read_note_record(&mut ctx, r, *id),
            "HEAD" | "TRLR" | "INDI" | "FAM" => {}
            "SUBM" if r.xref.as_deref() == Some(header::OWN_SUBM) => {}
            _ => {
                // Records we do not model (SUBM, SUBN, vendor records) travel with the header.
                let mut v = std::mem::take(&mut ctx.tree.header.unknown);
                push_unknown(&mut ctx, &mut v, r, usize::MAX, 0);
                ctx.tree.header.unknown = v;
            }
        }
    }
    ImportOutcome {
        tree: ctx.tree,
        report: ctx.report,
        resolved_media: ctx.resolved_media,
    }
}
