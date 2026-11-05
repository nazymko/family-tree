//! Record readers: two passes over the Gramps document (ids first, then content).

use crate::date::{LineIndex, count_key, is_date_tag, read_date};
use gt_core::DayOrder;
use gt_core::model::*;
use gt_core::raw::RawNode;
use gt_core::tree::Tree;
use gt_gedcom::maps::{name_kind_from_text, parent_rel_from_text};
use gt_gedcom::{ImportOptions, ImportReport, MediaLocator, Repair};
use roxmltree::{Document, Node};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum K {
    Person,
    Family,
    Event,
    Place,
    Source,
    Citation,
    Object,
    Note,
    Repo,
}

/// `(section element, record element, kind)`.
const SECTIONS: [(&str, &str, K); 9] = [
    ("people", "person", K::Person),
    ("families", "family", K::Family),
    ("events", "event", K::Event),
    ("places", "placeobj", K::Place),
    ("sources", "source", K::Source),
    ("citations", "citation", K::Citation),
    ("objects", "object", K::Object),
    ("notes", "note", K::Note),
    ("repositories", "repository", K::Repo),
];

/// Order in which kinds are built. Objects come first so links to a broken one can be skipped.
const BUILD_ORDER: [K; 9] = [
    K::Object,
    K::Note,
    K::Repo,
    K::Source,
    K::Citation,
    K::Place,
    K::Event,
    K::Person,
    K::Family,
];

const MAX_RAW_DEPTH: usize = 32;

fn det_id(bytes: &[u8]) -> Uuid {
    fn fnv(seed: u64, bytes: &[u8]) -> u64 {
        let mut h = seed;
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h ^= h >> 29;
        h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        h ^ (h >> 32)
    }
    let a = fnv(0xcbf2_9ce4_8422_2325, bytes);
    let b = fnv(0x8422_2325_cbf2_9ce4, bytes);
    let mut raw = [0u8; 16];
    raw[..8].copy_from_slice(&a.to_be_bytes());
    raw[8..].copy_from_slice(&b.to_be_bytes());
    raw[6] = (raw[6] & 0x0f) | 0x70;
    raw[8] = (raw[8] & 0x3f) | 0x80;
    Uuid::from_bytes(raw)
}

fn handle_id(kind: K, handle: &str) -> Uuid {
    det_id(format!("gramps:{kind:?}:{handle}").as_bytes())
}

fn sub_id(owner: Uuid, what: &str, idx: usize) -> Uuid {
    let mut v = owner.as_bytes().to_vec();
    v.extend_from_slice(what.as_bytes());
    v.extend_from_slice(&(idx as u64).to_be_bytes());
    det_id(&v)
}

fn new_common(id: Uuid) -> Common {
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

fn new_assertion() -> Assertion {
    Assertion {
        preferred: false,
        proof: None,
        confidence: None,
    }
}

fn pos32(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

fn elements<'a, 'i>(n: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children().filter(Node::is_element)
}

fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    elements(n).find(|c| c.tag_name().name() == name)
}

fn text_of(n: Node) -> String {
    n.children()
        .filter(Node::is_text)
        .filter_map(|c| c.text())
        .collect()
}

fn opt_text(n: Node) -> Option<String> {
    let t = text_of(n);
    let t = t.trim();
    (!t.is_empty()).then(|| t.to_string())
}

fn child_text(n: Node, name: &str) -> Option<String> {
    child(n, name).and_then(opt_text)
}

fn nonempty(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|s| !s.is_empty())
}

fn truthy(s: Option<&str>) -> bool {
    matches!(s.map(str::trim), Some("1" | "true"))
}

fn raw_of(n: Node, pos: usize, depth: usize) -> RawNode {
    let mut children: Vec<RawNode> = n
        .attributes()
        .enumerate()
        .map(|(i, a)| RawNode {
            tag: format!("@{}", a.name()),
            xref: None,
            value: Some(a.value().to_string()),
            pos: pos32(i),
            children: vec![],
        })
        .collect();
    if depth < MAX_RAW_DEPTH {
        children.extend(
            elements(n)
                .enumerate()
                .map(|(i, c)| raw_of(c, i, depth + 1)),
        );
    }
    RawNode {
        tag: n.tag_name().name().to_string(),
        xref: None,
        value: opt_text(n),
        pos: pos32(pos),
        children,
    }
}

fn event_kind(t: &str) -> EventKind {
    use EventKind::*;
    match t.trim().to_ascii_lowercase().as_str() {
        "birth" => Birth,
        "death" => Death,
        "marriage" => Marriage,
        "marriage settlement" => MarriageSettlement,
        "marriage license" => MarriageLicense,
        "marriage contract" => MarriageContract,
        "marriage banns" => MarriageBanns,
        "engagement" => Engagement,
        "divorce" => Divorce,
        "divorce filing" => DivorceFiled,
        "annulment" => Annulment,
        "adopted" => Adoption,
        "baptism" => Baptism,
        "burial" => Burial,
        "census" => Census,
        "christening" => Christening,
        "cremation" => Cremation,
        "education" => Education,
        "emigration" => Emigration,
        "graduation" => Graduation,
        "immigration" => Immigration,
        "military service" => Military,
        "naturalization" => Naturalization,
        "nobility title" | "title" => Title,
        "nationality" => Nationality,
        "occupation" => Occupation,
        "physical description" => PhysicalDescription,
        "probate" => Probate,
        "religion" => Religion,
        "residence" => Residence,
        "retirement" => Retirement,
        "will" => Will,
        _ => Custom(t.trim().to_string()),
    }
}

/// Role of an `eventref`; a missing attribute means `Primary`.
fn role_of(role: Option<&str>) -> Role {
    match role.map(str::trim) {
        None | Some("" | "Primary" | "Family") => Role::Principal,
        Some("Witness") => Role::Witness,
        Some("Clergy") => Role::Clergy,
        Some("Celebrant") => Role::Officiator,
        Some("Informant") => Role::Informant,
        Some("Bride") => Role::Wife,
        Some("Groom") => Role::Husband,
        Some(other) => Role::Other(other.to_string()),
    }
}

fn gramps_parent_rel(s: Option<&str>) -> ParentRel {
    match s.map(str::trim) {
        None | Some("" | "Birth") => ParentRel::Birth,
        Some("Adopted") => ParentRel::Adopted,
        Some("Stepchild") => ParentRel::Step,
        Some("Foster") => ParentRel::Foster,
        Some("Unknown") => ParentRel::Unknown,
        Some(other) => parent_rel_from_text(other),
    }
}

fn note_kind(t: &str) -> NoteKind {
    match t.trim() {
        "" | "General" => NoteKind::General,
        "Research" => NoteKind::Research,
        "Transcript" => NoteKind::Transcription,
        other if other.ends_with(" Note") => NoteKind::General,
        other => NoteKind::Other(other.to_string()),
    }
}

/// Decimal degrees from `52.5`, `-0.12`, `N52.5` or `52°30'0"N`.
fn coord(s: &str) -> Option<f64> {
    let s = s.trim();
    if let Ok(v) = s.parse::<f64>() {
        return v.is_finite().then_some(v);
    }
    let hemi = s
        .chars()
        .find(|c| matches!(c.to_ascii_uppercase(), 'N' | 'S' | 'E' | 'W'))?
        .to_ascii_uppercase();
    let nums: Vec<f64> = s
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.parse().ok())
        .collect();
    let (d, m, sec) = (
        *nums.first()?,
        nums.get(1).copied().unwrap_or(0.0),
        nums.get(2).copied().unwrap_or(0.0),
    );
    let v = d + m / 60.0 + sec / 3600.0;
    let sign = if matches!(hemi, 'S' | 'W') || s.starts_with('-') {
        -1.0
    } else {
        1.0
    };
    v.is_finite().then_some(sign * v)
}

fn split_pct(v: Option<&str>) -> Option<f32> {
    v?.trim().parse::<f32>().ok().map(|n| n / 100.0)
}

pub(crate) struct Ctx<'a> {
    lines: LineIndex<'a>,
    marriages: HashSet<Uuid>,
    parentin: Vec<(Uuid, Uuid)>,
    order: DayOrder,
    tree: Tree,
    pub(crate) report: ImportReport,
    ids: HashMap<(K, String), Uuid>,
    tags: HashMap<String, String>,
    locator: MediaLocator,
    resolved: Vec<(Uuid, PathBuf)>,
    note_refs: HashMap<Uuid, usize>,
    childof: HashMap<Uuid, Vec<Uuid>>,
    bad_objects: HashSet<Uuid>,
}

impl<'a> Ctx<'a> {
    pub(crate) fn new(doc: &Document<'a>, opts: &ImportOptions) -> Self {
        Self {
            lines: LineIndex::new(doc.input_text()),
            marriages: HashSet::new(),
            parentin: Vec::new(),
            order: opts.day_order,
            tree: Tree::new(),
            report: ImportReport::default(),
            ids: HashMap::new(),
            tags: HashMap::new(),
            locator: MediaLocator::new(opts),
            resolved: Vec::new(),
            note_refs: HashMap::new(),
            childof: HashMap::new(),
            bad_objects: HashSet::new(),
        }
    }

    pub(crate) fn finish(mut self) -> (Tree, ImportReport, Vec<(Uuid, PathBuf)>) {
        self.finish_parentin();
        self.finish_notes();
        self.finish_places();
        self.finish_preferred();
        (self.tree, self.report, self.resolved)
    }

    fn line(&self, n: Node) -> u32 {
        self.lines.row(n.range().start)
    }

    fn count(&mut self, key: &str) {
        count_key(&mut self.report, key);
    }

    fn lookup(&mut self, n: Node, kind: K) -> Option<Uuid> {
        let Some(h) = nonempty(n.attribute("hlink")) else {
            self.count(&format!("{}/@hlink(missing)", n.tag_name().name()));
            return None;
        };
        match self.ids.get(&(kind, h.to_string())) {
            Some(id) => Some(*id),
            None => {
                let line = self.line(n);
                self.report.repairs.push(Repair::DanglingPointer {
                    line,
                    xref: h.to_string(),
                });
                None
            }
        }
    }

    /// Count attributes of `n` that are not in the space-separated `allowed` list.
    fn check_attrs(&mut self, n: Node, label: &str, allowed: &str) {
        for a in n.attributes() {
            if !allowed.split(' ').any(|k| k == a.name()) {
                self.count(&format!("{label}/@{}", a.name()));
            }
        }
    }

    fn keep(&mut self, into: &mut Vec<RawNode>, n: Node, pos: usize, key: &str) {
        self.count(key);
        into.push(raw_of(n, pos, 0));
    }

    // ---- passes ----

    pub(crate) fn run<'d>(&mut self, root: Node<'d, 'a>) {
        let mut records: Vec<(K, Node<'d, 'a>, Uuid)> = Vec::new();
        let mut header_unknown: Vec<RawNode> = Vec::new();
        // Pass 1: an id for every handle.
        for (pos, sec) in elements(root).enumerate() {
            let name = sec.tag_name().name();
            if let Some((_, rec, kind)) = SECTIONS.iter().find(|(s, _, _)| *s == name) {
                for r in elements(sec) {
                    let rname = r.tag_name().name();
                    if rname != *rec {
                        self.keep(&mut header_unknown, r, pos, &format!("{name}/{rname}"));
                        continue;
                    }
                    let Some(h) = nonempty(r.attribute("handle")) else {
                        self.count(&format!("{rec}/@handle(missing)"));
                        continue;
                    };
                    if self.ids.contains_key(&(*kind, h.to_string())) {
                        let line = self.line(r);
                        self.report.repairs.push(Repair::DuplicateXref {
                            line,
                            xref: h.to_string(),
                        });
                        continue;
                    }
                    let id = handle_id(*kind, h);
                    self.ids.insert((*kind, h.to_string()), id);
                    *self.report.counts.entry((*rec).to_string()).or_insert(0) += 1;
                    records.push((*kind, r, id));
                }
            } else {
                match name {
                    "header" => self.read_header(sec, &mut header_unknown),
                    "tags" => self.read_tags(sec),
                    _ => self.keep(&mut header_unknown, sec, pos, &format!("database/{name}")),
                }
            }
        }
        // Pass 2: content.
        for kind in BUILD_ORDER {
            for (k, node, id) in &records {
                if *k == kind {
                    match kind {
                        K::Object => self.object(*node, *id),
                        K::Note => self.note(*node, *id),
                        K::Repo => self.repository(*node, *id),
                        K::Source => self.source(*node, *id),
                        K::Citation => self.citation(*node, *id),
                        K::Place => self.place(*node, *id),
                        K::Event => self.event(*node, *id),
                        K::Person => self.person(*node, *id),
                        K::Family => self.family(*node, *id),
                    }
                }
            }
        }
        let orphans = self
            .tree
            .events
            .values()
            .filter(|e| e.participants.is_empty())
            .count();
        if orphans > 0 {
            *self
                .report
                .unmapped_tags
                .entry("event(unreferenced)".to_string())
                .or_insert(0) += orphans;
        }
        self.tree.header.unknown.extend(header_unknown);
    }

    fn read_header(&mut self, h: Node, unknown: &mut Vec<RawNode>) {
        for (pos, c) in elements(h).enumerate() {
            match c.tag_name().name() {
                "created" => {
                    let v = c.attribute("version").map(str::to_string);
                    self.report.producer = Some(match &v {
                        Some(v) => format!("Gramps {v}"),
                        None => "Gramps".to_string(),
                    });
                    self.tree.header.producer = Some("Gramps".to_string());
                    self.tree.header.source_version = v;
                }
                // The media base path is untrusted and the user chooses the media roots instead.
                other => self.keep(unknown, c, pos, &format!("header/{other}")),
            }
        }
    }

    fn read_tags(&mut self, tags: Node) {
        for t in elements(tags).filter(|t| t.tag_name().name() == "tag") {
            if let (Some(h), Some(n)) = (t.attribute("handle"), t.attribute("name")) {
                self.tags.insert(h.to_string(), n.to_string());
            }
        }
    }

    // ---- shared structure ----

    fn link_note(&mut self, n: Node, into: &mut Vec<Uuid>) {
        if let Some(id) = self.lookup(n, K::Note)
            && !into.contains(&id)
        {
            into.push(id);
            *self.note_refs.entry(id).or_insert(0) += 1;
        }
    }

    fn link_citation(&mut self, n: Node, into: &mut Vec<Uuid>) {
        if let Some(id) = self.lookup(n, K::Citation)
            && !into.contains(&id)
        {
            into.push(id);
        }
    }

    fn link_object(&mut self, n: Node, into: &mut Vec<MediaLink>, primary: bool) {
        self.check_attrs(n, "objref", "hlink");
        for c in elements(n) {
            if c.tag_name().name() != "region" {
                self.count(&format!("objref/{}", c.tag_name().name()));
            }
        }
        let Some(id) = self.lookup(n, K::Object) else {
            return;
        };
        if self.bad_objects.contains(&id) {
            self.count("objref(file missing)");
            return;
        }
        let region = child(n, "region").and_then(|r| {
            let x1 = split_pct(r.attribute("corner1_x"))?;
            let y1 = split_pct(r.attribute("corner1_y"))?;
            let x2 = split_pct(r.attribute("corner2_x"))?;
            let y2 = split_pct(r.attribute("corner2_y"))?;
            let ok = |v: f32| (0.0..=1.0).contains(&v);
            (ok(x1) && ok(y1) && ok(x2) && ok(y2) && x2 > x1 && y2 > y1).then_some(Region {
                x: x1,
                y: y1,
                w: x2 - x1,
                h: y2 - y1,
            })
        });
        if child(n, "region").is_some() && region.is_none() {
            self.count("objref/region(invalid)");
        }
        if !into.iter().any(|m| m.media == id) {
            into.push(MediaLink {
                media: id,
                region,
                primary,
            });
        }
    }

    /// Id/change/priv, note, citation, media and tag links, and everything not in `known`
    /// (kept as raw nodes and counted). `known` and `known_attrs` are space-separated; the word
    /// `dates` stands for the four Gramps date elements.
    fn common(
        &mut self,
        node: Node,
        id: Uuid,
        label: &str,
        known: &str,
        known_attrs: &str,
        first_media_primary: bool,
    ) -> Common {
        let mut c = new_common(id);
        if let Some(x) = nonempty(node.attribute("id")) {
            c.ext_ids.push(ExternalId {
                kind: ExtIdKind::Xref,
                value: x.to_string(),
            });
        }
        c.changed = node
            .attribute("change")
            .and_then(|v| v.trim().parse::<i64>().ok())
            .and_then(|s| s.checked_mul(1000));
        c.private = truthy(node.attribute("priv"));
        for (i, a) in node.attributes().enumerate() {
            let n = a.name();
            if matches!(n, "handle" | "id" | "change" | "priv")
                || known_attrs.split(' ').any(|k| k == n)
            {
                continue;
            }
            self.count(&format!("{label}/@{n}"));
            c.unknown.push(RawNode {
                tag: format!("@{n}"),
                xref: None,
                value: Some(a.value().to_string()),
                pos: pos32(i),
                children: vec![],
            });
        }
        let dates_known = known.split(' ').any(|k| k == "dates");
        let mut first_obj = true;
        for (pos, ch) in elements(node).enumerate() {
            let name = ch.tag_name().name();
            match name {
                "noteref" => {
                    let mut v = std::mem::take(&mut c.notes);
                    self.link_note(ch, &mut v);
                    c.notes = v;
                }
                "citationref" => {
                    let mut v = std::mem::take(&mut c.citations);
                    self.link_citation(ch, &mut v);
                    c.citations = v;
                }
                "objref" => {
                    let mut v = std::mem::take(&mut c.media);
                    self.link_object(ch, &mut v, first_media_primary && first_obj);
                    c.media = v;
                    first_obj = false;
                }
                "tagref" => {
                    let hl = ch.attribute("hlink").unwrap_or("");
                    let tag = match self.tags.get(hl) {
                        Some(t) => t.clone(),
                        None => {
                            let line = self.line(ch);
                            self.report.repairs.push(Repair::DanglingPointer {
                                line,
                                xref: hl.chars().take(100).collect(),
                            });
                            String::new()
                        }
                    };
                    self.count("tagref");
                    c.unknown.push(RawNode {
                        tag: "tagref".to_string(),
                        xref: None,
                        value: Some(tag),
                        pos: pos32(pos),
                        children: vec![],
                    });
                }
                n if known.split(' ').any(|k| k == n) || (dates_known && is_date_tag(n)) => {}
                _ => {
                    let mut v = std::mem::take(&mut c.unknown);
                    self.keep(&mut v, ch, pos, &format!("{label}/{name}"));
                    c.unknown = v;
                }
            }
        }
        c
    }

    fn date_of(&mut self, node: Node) -> Option<gt_core::date::GenDate> {
        read_date(&self.lines, node, self.order, &mut self.report)
    }

    // ---- records ----

    fn note(&mut self, node: Node, id: Uuid) {
        let c = self.common(node, id, "note", "text", "type format", false);
        if truthy(node.attribute("format")) {
            self.count("note/@format(preformatted)");
        }
        self.tree.notes.insert(
            id,
            Note {
                common: c,
                kind: note_kind(node.attribute("type").unwrap_or("")),
                text: child(node, "text").map(text_of).unwrap_or_default(),
                lang: None,
                shared: false,
            },
        );
    }

    /// A `parentin` link the family does not confirm is an inconsistent file.
    fn finish_parentin(&mut self) {
        let bad = self
            .parentin
            .iter()
            .filter(|(p, f)| {
                self.tree
                    .families
                    .get(f)
                    .is_none_or(|f| f.partner1 != Some(*p) && f.partner2 != Some(*p))
            })
            .count();
        for _ in 0..bad {
            self.count("parentin(not confirmed by family)");
        }
    }

    fn finish_notes(&mut self) {
        for (id, n) in &mut self.tree.notes {
            n.shared = self.note_refs.get(id).copied().unwrap_or(0) != 1;
        }
    }

    fn repository(&mut self, node: Node, id: Uuid) {
        let c = self.common(node, id, "repository", "rname address url", "", false);
        let mut address = None;
        for (i, a) in elements(node)
            .filter(|c| c.tag_name().name() == "address")
            .enumerate()
        {
            if i > 0 {
                self.count("repository/address(extra)");
                continue;
            }
            let parts: Vec<String> = [
                "street", "locality", "city", "county", "state", "postal", "country", "phone",
            ]
            .iter()
            .filter_map(|n| child_text(a, n))
            .collect();
            if !parts.is_empty() {
                address = Some(parts.join(", "));
            }
        }
        let urls = elements(node)
            .filter(|c| c.tag_name().name() == "url")
            .filter_map(|u| nonempty(u.attribute("href")).map(str::to_string))
            .collect();
        self.tree.repositories.insert(
            id,
            Repository {
                common: c,
                name: child_text(node, "rname").unwrap_or_default(),
                address,
                urls,
            },
        );
    }

    fn source(&mut self, node: Node, id: Uuid) {
        let c = self.common(
            node,
            id,
            "source",
            "stitle sauthor spubinfo sabbrev reporef",
            "",
            false,
        );
        let mut repositories = Vec::new();
        for r in elements(node).filter(|c| c.tag_name().name() == "reporef") {
            if nonempty(r.attribute("medium")).is_some() {
                self.count("reporef/@medium");
            }
            self.check_attrs(r, "reporef", "hlink callno medium");
            for cc in elements(r) {
                self.count(&format!("reporef/{}", cc.tag_name().name()));
            }
            if let Some(repo) = self.lookup(r, K::Repo) {
                repositories.push(RepoRef {
                    repository: repo,
                    call_number: nonempty(r.attribute("callno")).map(str::to_string),
                });
            }
        }
        self.tree.sources.insert(
            id,
            Source {
                common: c,
                title: child_text(node, "stitle"),
                author: child_text(node, "sauthor"),
                publication: child_text(node, "spubinfo"),
                abbreviation: child_text(node, "sabbrev"),
                text: None,
                repositories,
            },
        );
    }

    fn citation(&mut self, node: Node, id: Uuid) {
        let c = self.common(
            node,
            id,
            "citation",
            "dates page confidence sourceref",
            "",
            false,
        );
        let source = child(node, "sourceref").and_then(|s| self.lookup(s, K::Source));
        // Gramps confidence 0..4 (very low .. very high) onto the GEDCOM 0..3 scale.
        let quality = match child_text(node, "confidence") {
            None => None,
            Some(v) => match v.parse::<u8>() {
                Ok(q) if q <= 4 => {
                    if q == 4 {
                        self.count("citation/confidence(4 folded to 3)");
                    }
                    Some(q.min(3))
                }
                _ => {
                    self.count("citation/confidence(invalid)");
                    None
                }
            },
        };
        let date = self.date_of(node);
        self.tree.citations.insert(
            id,
            Citation {
                common: c,
                source,
                page: child_text(node, "page"),
                date,
                text: None,
                quality,
                evidence: Evidence {
                    original: None,
                    primary_info: None,
                    direct: None,
                },
            },
        );
    }

    fn object(&mut self, node: Node, id: Uuid) {
        let c = self.common(node, id, "object", "file dates", "", false);
        let Some(file) = child(node, "file") else {
            self.count("object(no file)");
            self.bad_objects.insert(id);
            self.tree.header.unknown.push(raw_of(node, 0, 0));
            return;
        };
        if elements(node)
            .filter(|c| c.tag_name().name() == "file")
            .count()
            > 1
        {
            self.count("object/file(extra)");
        }
        self.check_attrs(file, "file", "src mime description checksum");
        if file.attribute("checksum").is_some() {
            self.count("file/@checksum");
        }
        let path = file
            .attribute("src")
            .unwrap_or("")
            .trim()
            .replace('\\', "/");
        if path.is_empty() {
            self.count("object(no file)");
            self.bad_objects.insert(id);
            return;
        }
        let date = self.date_of(node);
        let original_name = self
            .locator
            .resolve(id, &path, &mut self.report, &mut self.resolved);
        self.tree.media.insert(
            id,
            Media {
                common: c,
                path,
                original_name,
                mime: nonempty(file.attribute("mime")).map(str::to_string),
                title: nonempty(file.attribute("description")).map(str::to_string),
                date,
                description: None,
                sha256: None,
            },
        );
    }

    fn place(&mut self, node: Node, id: Uuid) {
        let c = self.common(
            node,
            id,
            "place",
            "ptitle pname coord placeref",
            "type",
            false,
        );
        let mut names: Vec<PlaceName> = Vec::new();
        for p in elements(node).filter(|c| c.tag_name().name() == "pname") {
            let name = p.attribute("value").unwrap_or("").trim().to_string();
            if name.is_empty() {
                self.count("pname(empty)");
            }
            names.push(PlaceName {
                name,
                lang: nonempty(p.attribute("lang")).map(str::to_string),
                date: self.date_of(p),
            });
        }
        if !names.is_empty() && child(node, "ptitle").is_some() {
            self.count("ptitle(ignored, pname present)");
        }
        if names.is_empty() {
            // Older files carry only the title.
            names.push(PlaceName {
                name: child_text(node, "ptitle").unwrap_or_default(),
                lang: None,
                date: None,
            });
        }
        let (mut lat, mut lon) = (None, None);
        if let Some(co) = child(node, "coord") {
            lat = co.attribute("lat").and_then(coord);
            lon = co.attribute("long").and_then(coord);
            if lat.is_none() || lon.is_none() {
                self.count("coord(unparsed)");
                lat = None;
                lon = None;
            }
        }
        let mut parent = None;
        let mut extra = Vec::new();
        for (i, r) in elements(node)
            .filter(|c| c.tag_name().name() == "placeref")
            .enumerate()
        {
            if self.date_present(r) {
                self.count("placeref/date");
            }
            if i == 0 {
                parent = self.lookup(r, K::Place);
            } else {
                // A place enclosed by several places: only the first is the parent.
                self.keep(&mut extra, r, i, "placeref(extra)");
            }
        }
        let mut common = c;
        common.unknown.extend(extra);
        self.tree.places.insert(
            id,
            Place {
                common,
                names,
                parent,
                kind: nonempty(node.attribute("type")).map(str::to_string),
                lat,
                lon,
                raw: None,
            },
        );
    }

    fn date_present(&self, n: Node) -> bool {
        elements(n).any(|c| is_date_tag(c.tag_name().name()))
    }

    /// Break parent cycles, then give every place the full chain text GEDCOM places carry.
    fn finish_places(&mut self) {
        let ids: Vec<Uuid> = self.tree.places.keys().copied().collect();
        let mut cut = Vec::new();
        for id in &ids {
            let mut seen = HashSet::new();
            let mut cur = *id;
            seen.insert(cur);
            while let Some(p) = self.tree.places.get(&cur).and_then(|p| p.parent) {
                if !seen.insert(p) {
                    cut.push(cur);
                    break;
                }
                cur = p;
            }
        }
        for id in cut {
            if let Some(p) = self.tree.places.get_mut(&id) {
                p.parent = None;
            }
            self.count("placeref(cycle)");
        }
        for id in ids {
            let mut parts: Vec<String> = Vec::new();
            let mut cur = Some(id);
            while let Some(c) = cur {
                let Some(p) = self.tree.places.get(&c) else {
                    break;
                };
                if let Some(n) = p.names.first() {
                    parts.push(n.name.clone());
                }
                cur = p.parent;
            }
            if let Some(p) = self.tree.places.get_mut(&id) {
                p.raw = Some(parts.join(", "));
            }
        }
    }

    fn event(&mut self, node: Node, id: Uuid) {
        let mut c = self.common(
            node,
            id,
            "event",
            "type dates place cause description attribute",
            "",
            false,
        );
        let kind = event_kind(&child_text(node, "type").unwrap_or_default());
        let place = child(node, "place").and_then(|p| self.lookup(p, K::Place));
        let mut age = None;
        for (pos, a) in elements(node)
            .filter(|c| c.tag_name().name() == "attribute")
            .enumerate()
        {
            let ty = a.attribute("type").unwrap_or("");
            let val = nonempty(a.attribute("value"));
            if ty.eq_ignore_ascii_case("age") && age.is_none() && val.is_some() {
                age = val.map(str::to_string);
            } else {
                self.keep(&mut c.unknown, a, pos, "event/attribute");
            }
        }
        let date = self.date_of(node);
        self.tree.events.insert(
            id,
            Event {
                common: c,
                kind,
                date,
                place,
                description: child_text(node, "description"),
                age,
                cause: child_text(node, "cause"),
                negative: false,
                assertion: new_assertion(),
                participants: vec![],
            },
        );
    }

    fn add_event_refs(&mut self, node: Node, target: ParticipantRef) {
        for r in elements(node).filter(|c| c.tag_name().name() == "eventref") {
            self.check_attrs(r, "eventref", "hlink role");
            for c in elements(r) {
                self.count(&format!("eventref/{}", c.tag_name().name()));
            }
            let Some(eid) = self.lookup(r, K::Event) else {
                continue;
            };
            let p = Participant {
                target: target.clone(),
                role: role_of(r.attribute("role")),
            };
            if let Some(ev) = self.tree.events.get_mut(&eid) {
                if ev.kind == EventKind::Marriage
                    && let ParticipantRef::Family(f) = &target
                {
                    self.marriages.insert(*f);
                }
                if !ev.participants.contains(&p) {
                    ev.participants.push(p);
                }
                ev.participants.sort_by_key(|p| p.role != Role::Principal);
            }
        }
    }

    fn person(&mut self, node: Node, id: Uuid) {
        let c = self.common(
            node,
            id,
            "person",
            "gender name eventref childof parentin",
            "",
            true,
        );
        let sex = match child_text(node, "gender").as_deref() {
            None | Some("U") => Sex::Unknown,
            Some("M") => Sex::Male,
            Some("F") => Sex::Female,
            Some("X") => Sex::Intersex,
            Some(o) => Sex::Other(o.to_string()),
        };
        let all: Vec<Node> = elements(node)
            .filter(|c| c.tag_name().name() == "name")
            .collect();
        let (primary, alts): (Vec<Node>, Vec<Node>) =
            all.into_iter().partition(|n| !truthy(n.attribute("alt")));
        let names: Vec<Name> = primary
            .into_iter()
            .chain(alts)
            .enumerate()
            .map(|(i, n)| self.name(n, id, i))
            .collect();
        self.add_event_refs(node, ParticipantRef::Person(id));
        for r in elements(node).filter(|c| c.tag_name().name() == "childof") {
            if let Some(f) = self.lookup(r, K::Family) {
                self.childof.entry(id).or_default().push(f);
            }
        }
        // `parentin` repeats the partner links the family records carry: checked in `finish`.
        for r in elements(node).filter(|c| c.tag_name().name() == "parentin") {
            if let Some(f) = self.lookup(r, K::Family) {
                self.parentin.push((id, f));
            }
        }
        self.tree.persons.insert(
            id,
            Person {
                common: c,
                sex,
                names,
                living_override: None,
                sibling_marker: false,
            },
        );
    }

    fn name(&mut self, node: Node, person: Uuid, idx: usize) -> Name {
        let nid = sub_id(person, "name", idx);
        let c = self.common(
            node,
            nid,
            "name",
            "first call nick title suffix surname dates",
            "type alt",
            false,
        );
        if !c.media.is_empty() {
            self.count("name/objref");
        }
        let ty = node.attribute("type").unwrap_or("").trim();
        let kind = match ty {
            "Birth Name" => NameKind::Birth,
            "Also Known As" => NameKind::Aka,
            "Married Name" => NameKind::Married,
            "Other Name" => NameKind::Other("Other Name".to_string()),
            "" | "Unknown" if idx == 0 => NameKind::Birth,
            "" | "Unknown" => NameKind::Aka,
            other => name_kind_from_text(other),
        };
        let mut unknown = c.unknown;
        let mut surnames: Vec<(String, bool)> = Vec::new();
        let mut prefix = None;
        let mut patronymic = None;
        for (pos, s) in elements(node)
            .filter(|c| c.tag_name().name() == "surname")
            .enumerate()
        {
            let Some(text) = opt_text(s) else { continue };
            let pre = nonempty(s.attribute("prefix")).map(str::to_string);
            let is_patronymic = s
                .attribute("derivation")
                .is_some_and(|d| d.trim().eq_ignore_ascii_case("patronymic"));
            if is_patronymic && patronymic.is_none() {
                patronymic = Some(text);
                continue;
            }
            if nonempty(s.attribute("connector")).is_some() {
                self.keep(&mut unknown, s, pos, "surname/@connector");
            }
            let text = match (&prefix, pre) {
                (None, Some(p)) => {
                    prefix = Some(p);
                    text
                }
                (_, Some(p)) => format!("{p} {text}"),
                (_, None) => text,
            };
            surnames.push((text, truthy(s.attribute("prim"))));
        }
        // Primary surname first.
        surnames.sort_by_key(|(_, prim)| !*prim);
        let surnames: Vec<String> = surnames.into_iter().map(|(t, _)| t).collect();
        let given = child_text(node, "first");
        let suffix = child_text(node, "suffix");
        let raw = {
            let mut parts: Vec<String> = Vec::new();
            parts.extend(given.clone());
            parts.extend(patronymic.clone());
            if !surnames.is_empty() || suffix.is_some() {
                let sur = surnames.join(" ");
                parts.push(match &prefix {
                    Some(p) => format!("/{p} {sur}/"),
                    None => format!("/{sur}/"),
                });
            }
            parts.extend(suffix.clone());
            let r = parts.join(" ");
            (!r.is_empty()).then_some(r)
        };
        let date = self.date_of(node);
        Name {
            id: nid,
            kind,
            assertion: Assertion {
                preferred: idx == 0,
                ..new_assertion()
            },
            private: c.private,
            prefix: child_text(node, "title"),
            given,
            call: child_text(node, "call"),
            surname_prefix: prefix,
            surnames,
            patronymic,
            suffix,
            nickname: child_text(node, "nick"),
            lang: None,
            date,
            raw,
            citations: c.citations,
            notes: c.notes,
            unknown,
        }
    }

    fn family(&mut self, node: Node, id: Uuid) {
        let c = self.common(
            node,
            id,
            "family",
            "rel father mother eventref childref",
            "",
            false,
        );
        let mut p1 = child(node, "father").and_then(|n| self.lookup(n, K::Person));
        let mut p2 = child(node, "mother").and_then(|n| self.lookup(n, K::Person));
        let sex = |s: &Self, p: Option<Uuid>| {
            p.and_then(|p| s.tree.persons.get(&p))
                .map(|p| p.sex.clone())
        };
        if sex(self, p1) == Some(Sex::Female) && sex(self, p2) == Some(Sex::Male) {
            std::mem::swap(&mut p1, &mut p2);
        }
        let mut kind = match child(node, "rel").and_then(|r| nonempty(r.attribute("type"))) {
            None | Some("Unknown") => UnionKind::Unknown,
            Some("Married") => UnionKind::Married,
            Some("Unmarried") => UnionKind::Unmarried,
            Some("Civil Union") => UnionKind::CivilUnion,
            Some(o) => UnionKind::Other(o.to_string()),
        };
        self.add_event_refs(node, ParticipantRef::Family(id));
        if self.marriages.contains(&id) && kind == UnionKind::Unknown {
            kind = UnionKind::Married;
        }
        let mut children: Vec<ChildLink> = Vec::new();
        for r in elements(node).filter(|c| c.tag_name().name() == "childref") {
            self.check_attrs(r, "childref", "hlink frel mrel");
            let Some(person) = self.lookup(r, K::Person) else {
                continue;
            };
            if children.iter().any(|l| l.person == person) {
                continue;
            }
            let mut citations = Vec::new();
            let mut unknown = Vec::new();
            for (pos, cc) in elements(r).enumerate() {
                match cc.tag_name().name() {
                    "citationref" => self.link_citation(cc, &mut citations),
                    other => self.keep(&mut unknown, cc, pos, &format!("childref/{other}")),
                }
            }
            children.push(ChildLink {
                person,
                rel1: gramps_parent_rel(r.attribute("frel")),
                rel2: gramps_parent_rel(r.attribute("mrel")),
                preferred: false,
                assertion: new_assertion(),
                citations,
                unknown,
            });
        }
        self.tree.families.insert(
            id,
            Family {
                common: c,
                partner1: p1,
                partner2: p2,
                kind,
                children,
            },
        );
    }

    /// A person's preferred parent link: the first `childof` family that lists them, else the
    /// first family (document order) that does.
    fn finish_preferred(&mut self) {
        let mut first: HashMap<Uuid, Uuid> = HashMap::new();
        let mut links: HashSet<(Uuid, Uuid)> = HashSet::new();
        for (fid, f) in &self.tree.families {
            for l in &f.children {
                first.entry(l.person).or_insert(*fid);
                links.insert((l.person, *fid));
            }
        }
        for (person, default) in first {
            let fam = self
                .childof
                .get(&person)
                .and_then(|l| l.iter().find(|f| links.contains(&(person, **f))))
                .copied()
                .unwrap_or(default);
            if let Some(l) = self
                .tree
                .families
                .get_mut(&fam)
                .and_then(|f| f.children.iter_mut().find(|l| l.person == person))
            {
                l.preferred = true;
            }
        }
    }
}
