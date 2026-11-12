//! `Project::load_tree`: rebuild the in-memory tree, in saved order (`seq`).

use crate::StoreError;
use crate::codec::*;
use crate::project::Project;
use gt_core::Uuid;
use gt_core::model::*;
use gt_core::tree::{Tree, TreeHeader};
use rusqlite::{Connection, Row};
use std::collections::HashMap;

type Key = (String, String);

/// All polymorphic link rows, grouped per target in stored order.
struct Links {
    cites: HashMap<Key, Vec<Uuid>>,
    notes: HashMap<Key, Vec<Uuid>>,
    media: HashMap<Key, Vec<MediaLink>>,
    ext: HashMap<Key, Vec<ExternalId>>,
}

/// The clip columns of a `media_link` row (8 to 11): none when all are empty, else a whole clip.
/// Any column of the wrong type or a stretch that is not `start < end` is an error.
fn read_clip(r: &Row) -> Result<Option<Clip>, String> {
    use rusqlite::types::Value as V;
    let col = |i: usize| r.get::<_, V>(i).map_err(|e| e.to_string());
    let (id, start, end, label) = (col(8)?, col(9)?, col(10)?, col(11)?);
    if [&id, &start, &end, &label]
        .iter()
        .all(|v| matches!(v, V::Null))
    {
        return Ok(None);
    }
    let id = match id {
        V::Text(t) => Uuid::parse_str(&t).map_err(|_| "has a bad id".to_string())?,
        _ => return Err("has no id".into()),
    };
    let ms = |v: V| match v {
        V::Integer(n) => u64::try_from(n).map_err(|_| "has a negative time".to_string()),
        _ => Err("has a time that is not a number".to_string()),
    };
    let (start_ms, end_ms) = (ms(start)?, ms(end)?);
    if start_ms >= end_ms {
        return Err("ends before it starts".into());
    }
    let label = match label {
        V::Text(t) if !t.trim().is_empty() => Some(t),
        V::Null | V::Text(_) => None,
        _ => return Err("has a label that is not text".into()),
    };
    Ok(Some(Clip {
        id,
        start_ms,
        end_ms,
        label,
    }))
}

impl Links {
    fn load(conn: &Connection) -> Result<Links, StoreError> {
        let mut l = Links {
            cites: HashMap::new(),
            notes: HashMap::new(),
            media: HashMap::new(),
            ext: HashMap::new(),
        };
        let mut st = conn.prepare(
            "SELECT target_type, target_id, citation_id FROM citation_link ORDER BY pos, rowid",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let c = parse_uuid(&r.get::<_, String>(2)?)?;
            l.cites.entry((r.get(0)?, r.get(1)?)).or_default().push(c);
        }
        let mut st = conn
            .prepare("SELECT target_type, target_id, note_id FROM note_link ORDER BY pos, rowid")?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let n = parse_uuid(&r.get::<_, String>(2)?)?;
            l.notes.entry((r.get(0)?, r.get(1)?)).or_default().push(n);
        }
        let mut st = conn.prepare(
            "SELECT target_type, target_id, media_id, \"primary\", region_x, region_y, region_w, \
             region_h, clip_id, clip_start_ms, clip_end_ms, clip_label FROM media_link \
             ORDER BY pos, rowid",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let x: Option<f64> = r.get(4)?;
            let y: Option<f64> = r.get(5)?;
            let w: Option<f64> = r.get(6)?;
            let h: Option<f64> = r.get(7)?;
            let region = match (x, y, w, h) {
                (Some(x), Some(y), Some(w), Some(h)) => Some(Region {
                    x: x as f32,
                    y: y as f32,
                    w: w as f32,
                    h: h as f32,
                }),
                _ => None,
            };
            let media = parse_uuid(&r.get::<_, String>(2)?)?;
            let clip = match read_clip(r) {
                Ok(clip) => clip,
                Err(why) => {
                    // A clip that cannot be read is left out with its link: shown without its
                    // times, the link would claim the whole recording.
                    eprintln!("gt-store: a clip of media {media} {why}; left out");
                    continue;
                }
            };
            let link = MediaLink {
                media,
                region,
                primary: r.get(3)?,
                clip,
            };
            l.media
                .entry((r.get(0)?, r.get(1)?))
                .or_default()
                .push(link);
        }
        let mut st = conn.prepare(
            "SELECT target_type, target_id, kind, scheme, value FROM ext_id ORDER BY pos, rowid",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let kind = ext_kind_parse(&r.get::<_, String>(2)?, r.get(3)?)?;
            let e = ExternalId {
                kind,
                value: r.get(4)?,
            };
            l.ext.entry((r.get(0)?, r.get(1)?)).or_default().push(e);
        }
        Ok(l)
    }

    fn cites_notes(&mut self, ty: &str, id: &str) -> (Vec<Uuid>, Vec<Uuid>) {
        let k = (ty.to_string(), id.to_string());
        (
            self.cites.remove(&k).unwrap_or_default(),
            self.notes.remove(&k).unwrap_or_default(),
        )
    }

    /// Common fields from the first four columns (`id, private, changed, unknown_json`).
    fn common(&mut self, ty: &str, r: &Row) -> Result<Common, StoreError> {
        let id_text: String = r.get(0)?;
        let (citations, notes) = self.cites_notes(ty, &id_text);
        let k = (ty.to_string(), id_text.clone());
        Ok(Common {
            id: parse_uuid(&id_text)?,
            private: r.get(1)?,
            changed: r.get(2)?,
            citations,
            notes,
            media: self.media.remove(&k).unwrap_or_default(),
            ext_ids: self.ext.remove(&k).unwrap_or_default(),
            unknown: unknown_parse(r.get(3)?)?,
        })
    }
}

fn assertion(
    preferred: bool,
    proof: Option<String>,
    confidence: Option<u8>,
) -> Result<Assertion, StoreError> {
    Ok(Assertion {
        preferred,
        proof: opt_enum_parse(proof)?,
        confidence,
    })
}

impl Project {
    pub fn load_tree(&self) -> Result<Tree, StoreError> {
        let conn = &self.conn;
        let mut links = Links::load(conn)?;
        let mut tree = Tree::new();

        let mut st =
            conn.prepare("SELECT producer, source_version, unknown_json FROM tree_header")?;
        let mut rows = st.query([])?;
        if let Some(r) = rows.next()? {
            tree.header = TreeHeader {
                producer: r.get(0)?,
                source_version: r.get(1)?,
                unknown: unknown_parse(r.get(2)?)?,
            };
        }

        // Names, grouped by person.
        let mut names: HashMap<String, Vec<Name>> = HashMap::new();
        let mut st = conn.prepare(
            "SELECT id, private, unknown_json, person_id, kind, preferred, proof, confidence, \
             prefix, given, call, surname_prefix, surnames_json, patronymic, suffix, nickname, \
             lang, date_json, raw FROM name ORDER BY pos, rowid",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let id_text: String = r.get(0)?;
            let (citations, notes) = links.cites_notes("name", &id_text);
            let surnames: String = r.get(12)?;
            let n = Name {
                id: parse_uuid(&id_text)?,
                kind: enum_parse(&r.get::<_, String>(4)?)?,
                assertion: assertion(r.get(5)?, r.get(6)?, r.get(7)?)?,
                private: r.get(1)?,
                prefix: r.get(8)?,
                given: r.get(9)?,
                call: r.get(10)?,
                surname_prefix: r.get(11)?,
                surnames: serde_json::from_str(&surnames).map_err(|e| corrupt(e.to_string()))?,
                patronymic: r.get(13)?,
                suffix: r.get(14)?,
                nickname: r.get(15)?,
                lang: r.get(16)?,
                date: date_parse(r.get(17)?)?,
                raw: r.get(18)?,
                citations,
                notes,
                unknown: unknown_parse(r.get(2)?)?,
            };
            names.entry(r.get(3)?).or_default().push(n);
        }

        let mut st = conn.prepare(
            "SELECT id, private, changed, unknown_json, sex, living_override, sibling_marker              FROM person ORDER BY seq",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let common = links.common("person", r)?;
            let id = common.id;
            let p = Person {
                sex: enum_parse(&r.get::<_, String>(4)?)?,
                names: names.remove(&id.to_string()).unwrap_or_default(),
                living_override: opt_enum_parse(r.get(5)?)?,
                sibling_marker: r.get(6)?,
                common,
            };
            tree.persons.insert(id, p);
        }

        // Child links, grouped by family.
        let mut children: HashMap<String, Vec<ChildLink>> = HashMap::new();
        let mut st = conn.prepare(
            "SELECT id, unknown_json, family_id, person_id, rel1, rel2, preferred, assertion_preferred, proof, \
             confidence FROM child_link ORDER BY pos, rowid",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let id_text: String = r.get(0)?;
            let (citations, _) = links.cites_notes("child_link", &id_text);
            let c = ChildLink {
                person: parse_uuid(&r.get::<_, String>(3)?)?,
                rel1: enum_parse(&r.get::<_, String>(4)?)?,
                rel2: enum_parse(&r.get::<_, String>(5)?)?,
                preferred: r.get(6)?,
                assertion: assertion(r.get(7)?, r.get(8)?, r.get(9)?)?,
                citations,
                unknown: unknown_parse(r.get(1)?)?,
            };
            children.entry(r.get(2)?).or_default().push(c);
        }

        let mut st = conn.prepare(
            "SELECT id, private, changed, unknown_json, partner1_id, partner2_id, kind \
             FROM family ORDER BY seq",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let common = links.common("family", r)?;
            let id = common.id;
            let f = Family {
                partner1: opt_uuid(r.get(4)?)?,
                partner2: opt_uuid(r.get(5)?)?,
                kind: enum_parse(&r.get::<_, String>(6)?)?,
                children: children.remove(&id.to_string()).unwrap_or_default(),
                common,
            };
            tree.families.insert(id, f);
        }

        let mut parts: HashMap<String, Vec<Participant>> = HashMap::new();
        let mut st = conn.prepare(
            "SELECT event_id, target_type, target_id, role FROM participant ORDER BY pos, rowid",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let target = parse_uuid(&r.get::<_, String>(2)?)?;
            let target = match r.get::<_, String>(1)?.as_str() {
                "person" => ParticipantRef::Person(target),
                "family" => ParticipantRef::Family(target),
                other => return Err(corrupt(format!("bad participant type {other:?}"))),
            };
            let p = Participant {
                target,
                role: enum_parse(&r.get::<_, String>(3)?)?,
            };
            parts.entry(r.get(0)?).or_default().push(p);
        }

        let mut st = conn.prepare(
            "SELECT id, private, changed, unknown_json, kind, date_json, place_id, description, \
             age, cause, negative, preferred, proof, confidence FROM event ORDER BY seq",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let common = links.common("event", r)?;
            let id = common.id;
            let e = Event {
                kind: enum_parse(&r.get::<_, String>(4)?)?,
                date: date_parse(r.get(5)?)?,
                place: opt_uuid(r.get(6)?)?,
                description: r.get(7)?,
                age: r.get(8)?,
                cause: r.get(9)?,
                negative: r.get(10)?,
                assertion: assertion(r.get(11)?, r.get(12)?, r.get(13)?)?,
                participants: parts.remove(&id.to_string()).unwrap_or_default(),
                common,
            };
            tree.events.insert(id, e);
        }

        let mut place_names: HashMap<String, Vec<PlaceName>> = HashMap::new();
        let mut st = conn.prepare(
            "SELECT place_id, name, lang, date_json FROM place_name ORDER BY pos, rowid",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let n = PlaceName {
                name: r.get(1)?,
                lang: r.get(2)?,
                date: date_parse(r.get(3)?)?,
            };
            place_names.entry(r.get(0)?).or_default().push(n);
        }
        let mut st = conn.prepare(
            "SELECT id, private, changed, unknown_json, parent_id, kind, lat, lon, raw \
             FROM place ORDER BY seq",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let common = links.common("place", r)?;
            let id = common.id;
            let p = Place {
                names: place_names.remove(&id.to_string()).unwrap_or_default(),
                parent: opt_uuid(r.get(4)?)?,
                kind: r.get(5)?,
                lat: r.get(6)?,
                lon: r.get(7)?,
                raw: r.get(8)?,
                common,
            };
            tree.places.insert(id, p);
        }

        let mut repo_refs: HashMap<String, Vec<RepoRef>> = HashMap::new();
        let mut st = conn.prepare(
            "SELECT source_id, repository_id, call_number FROM repo_ref ORDER BY pos, rowid",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let rr = RepoRef {
                repository: parse_uuid(&r.get::<_, String>(1)?)?,
                call_number: r.get(2)?,
            };
            repo_refs.entry(r.get(0)?).or_default().push(rr);
        }
        let mut st = conn.prepare(
            "SELECT id, private, changed, unknown_json, title, author, publication, abbreviation, \
             text FROM source ORDER BY seq",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let common = links.common("source", r)?;
            let id = common.id;
            let s = Source {
                title: r.get(4)?,
                author: r.get(5)?,
                publication: r.get(6)?,
                abbreviation: r.get(7)?,
                text: r.get(8)?,
                repositories: repo_refs.remove(&id.to_string()).unwrap_or_default(),
                common,
            };
            tree.sources.insert(id, s);
        }

        let mut st = conn.prepare(
            "SELECT id, private, changed, unknown_json, name, address, urls_json \
             FROM repository ORDER BY seq",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let common = links.common("repository", r)?;
            let id = common.id;
            let urls: String = r.get(6)?;
            let repo = Repository {
                name: r.get(4)?,
                address: r.get(5)?,
                urls: serde_json::from_str(&urls).map_err(|e| corrupt(e.to_string()))?,
                common,
            };
            tree.repositories.insert(id, repo);
        }

        let mut st = conn.prepare(
            "SELECT id, private, changed, unknown_json, source_id, page, date_json, text, quality, \
             ev_original, ev_primary, ev_direct FROM citation ORDER BY seq",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let common = links.common("citation", r)?;
            let id = common.id;
            let c = Citation {
                source: opt_uuid(r.get(4)?)?,
                page: r.get(5)?,
                date: date_parse(r.get(6)?)?,
                text: r.get(7)?,
                quality: r.get(8)?,
                evidence: Evidence {
                    original: r.get(9)?,
                    primary_info: r.get(10)?,
                    direct: r.get(11)?,
                },
                common,
            };
            tree.citations.insert(id, c);
        }

        let mut st = conn.prepare(
            "SELECT id, private, changed, unknown_json, path, original_name, mime, title, \
             date_json, description, sha256, transcription_json, rotation, place_id FROM media ORDER BY seq",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let common = links.common("media", r)?;
            let id = common.id;
            let transcription = match r.get::<_, Option<String>>(11)? {
                Some(j) => mended_transcription(id, &j),
                None => Vec::new(),
            };
            let rotation = mended_rotation(id, r.get_ref(12)?);
            let m = Media {
                transcription,
                rotation,
                // A place that is gone reads as none.
                place: match r.get::<_, Option<String>>(13)? {
                    Some(t) => Some(parse_uuid(&t)?).filter(|p| tree.places.contains_key(p)),
                    None => None,
                },
                path: r.get(4)?,
                original_name: r.get(5)?,
                mime: r.get(6)?,
                title: r.get(7)?,
                date: date_parse(r.get(8)?)?,
                description: r.get(9)?,
                sha256: r.get(10)?,
                common,
            };
            tree.media.insert(id, m);
        }

        let mut st = conn.prepare(
            "SELECT id, private, changed, unknown_json, kind, text, lang, shared \
             FROM note ORDER BY seq",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let common = links.common("note", r)?;
            let id = common.id;
            let n = Note {
                kind: enum_parse(&r.get::<_, String>(4)?)?,
                text: r.get(5)?,
                lang: r.get(6)?,
                shared: r.get(7)?,
                common,
            };
            tree.notes.insert(id, n);
        }
        Ok(tree)
    }
}

/// How a picture is turned. A value that is no quarter turn reads as upright, and is reported on
/// stderr; it never stops the tree from opening.
fn mended_rotation(media: Uuid, v: rusqlite::types::ValueRef<'_>) -> Option<u16> {
    use rusqlite::types::ValueRef;
    match v {
        ValueRef::Null => None,
        ValueRef::Integer(n) if [0, 90, 180, 270].contains(&n) => Some(n as u16),
        _ => {
            eprintln!("gt-store: media {media}: an unknown rotation reads as upright");
            Some(0)
        }
    }
}

/// A picture's stored transcription, read as far as it can be: a damaged piece is mended or
/// left out, never a reason the tree cannot open. What was changed is reported on stderr, as the
/// store reports other damage it works around.
fn mended_transcription(media: Uuid, json: &str) -> Vec<TextRegion> {
    let raw: Vec<serde_json::Value> = match serde_json::from_str(json) {
        Ok(list) => list,
        Err(e) => {
            eprintln!("gt-store: media {media}: the transcription cannot be read ({e}); left out");
            return Vec::new();
        }
    };
    let total = raw.len();
    let mut seen = std::collections::HashSet::new();
    let mut mended = 0;
    let out: Vec<TextRegion> = raw
        .into_iter()
        .filter_map(|v| serde_json::from_value::<TextRegion>(v).ok())
        .filter(|r| seen.insert(r.id))
        .map(|mut r| {
            if mend_text_region(&mut r) {
                mended += 1;
            }
            r
        })
        .collect();
    let dropped = total - out.len();
    if dropped + mended > 0 {
        eprintln!(
            "gt-store: media {media}: {dropped} damaged transcription piece(s) left out, {mended} mended"
        );
    }
    out
}
