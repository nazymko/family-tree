//! importDraft: everything traced from a picture (new people, their birth and death events, notes,
//! families, links to people already in the tree and an optional picture) as one change set.
//! Every check runs before the first record is written, so a refused draft leaves nothing behind.

use super::{Planner, assertion, conflict, invalid, new_common, parse_date, sex_of, text};
use crate::command::{DraftFamily, DraftPerson};
use crate::error::ApiError;
use gt_core::ids::new_id;
use gt_core::model::*;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Most people (and families) one draft may add.
pub const MAX_DRAFT_PEOPLE: usize = 500;

/// Title of the source created for a traced picture.
pub const PICTURE_SOURCE_TITLE: &str = "Family tree picture";

/// The picture, already stored in the project, that goes with the draft.
pub(crate) struct StagedPicture {
    pub media: Media,
}

fn name_of(d: &DraftPerson) -> (Option<String>, Vec<String>, Option<String>) {
    let given = text(d.given.as_deref());
    let surnames: Vec<String> = text(d.surname.as_deref()).into_iter().collect();
    (given, surnames, text(d.patronymic.as_deref()))
}

fn event(p: &Planner, kind: EventKind, raw: &str, person: Uuid) -> Event {
    Event {
        common: new_common(p.now),
        kind,
        date: parse_date(Some(raw), p.order),
        place: None,
        description: None,
        age: None,
        cause: None,
        negative: false,
        assertion: assertion(true),
        participants: vec![Participant {
            target: ParticipantRef::Person(person),
            role: Role::Principal,
        }],
    }
}

struct Fam {
    p1: Option<Uuid>,
    p2: Option<Uuid>,
    /// A family already in the tree that the kids join.
    into: Option<Uuid>,
    kids: Vec<Uuid>,
}

pub(super) fn import_draft(
    p: &mut Planner,
    persons: &[DraftPerson],
    families: &[DraftFamily],
    picture: Option<&StagedPicture>,
) -> Result<Option<Uuid>, ApiError> {
    if persons.len() > MAX_DRAFT_PEOPLE || families.len() > MAX_DRAFT_PEOPLE {
        return Err(invalid(format!(
            "A traced picture can add at most {MAX_DRAFT_PEOPLE} people at a time. Remove some and try again."
        )));
    }
    if persons.is_empty() {
        return Err(invalid("There is nobody to add. Mark a person first."));
    }

    // ---- check everything first
    let mut ids: HashMap<&str, Uuid> = HashMap::new();
    for (i, d) in persons.iter().enumerate() {
        let key = d.key.trim();
        if key.is_empty() || ids.contains_key(key) {
            return Err(invalid(format!(
                "Card {} has a missing or repeated label. Close the trace and open it again.",
                i + 1
            )));
        }
        let id = match d.existing {
            Some(e) => {
                p.get::<Person>(e)?;
                e
            }
            None => {
                let (given, surnames, _) = name_of(d);
                if given.is_none() && surnames.is_empty() {
                    return Err(invalid(format!(
                        "Card {} has no name. Enter a given name or a surname.",
                        i + 1
                    )));
                }
                new_id()
            }
        };
        ids.insert(key, id);
    }
    let existing: Vec<Uuid> = persons.iter().filter_map(|d| d.existing).collect();
    let distinct: HashSet<Uuid> = existing.iter().copied().collect();
    if distinct.len() != existing.len() {
        return Err(invalid(
            "The same person from the tree is used on two cards. Merge the cards first.",
        ));
    }
    let resolve = |k: &str| -> Result<Uuid, ApiError> {
        ids.get(k.trim())
            .copied()
            .ok_or_else(|| invalid("A link points at a card that is not in the draft."))
    };

    let mut fams: Vec<Fam> = Vec::new();
    for f in families {
        let p1 = f.partner1.as_deref().map(&resolve).transpose()?;
        let p2 = f.partner2.as_deref().map(&resolve).transpose()?;
        let kids = f
            .children
            .iter()
            .map(|k| resolve(k))
            .collect::<Result<Vec<_>, _>>()?;
        if p1.is_none() && p2.is_none() {
            return Err(invalid("A family needs at least one parent or partner."));
        }
        if p1.is_some() && p1 == p2 {
            return Err(invalid("A family needs two different people."));
        }
        let mut seen = HashSet::new();
        for k in &kids {
            if Some(*k) == p1 || Some(*k) == p2 {
                return Err(invalid("A person cannot be their own child."));
            }
            if !seen.insert(*k) {
                return Err(conflict("A child is listed twice in one family."));
            }
        }
        // Children of ONE parent from the tree join that parent's family; they do not start a second one.
        let mut into = None;
        let (mut p1, mut p2) = (p1, p2);
        let single = match (p1, p2) {
            (Some(a), None) | (None, Some(a)) => Some(a),
            _ => None,
        };
        if let Some(fid) = f.existing_family {
            let fam = p.tree.families.get(&fid).ok_or_else(|| {
                invalid("The family chosen for the new people is not in the tree any more.")
            })?;
            let ok = single.is_some_and(|a| fam.partner1 == Some(a) || fam.partner2 == Some(a));
            if !ok {
                return Err(invalid(
                    "The family chosen for the new people does not include that parent.",
                ));
            }
            into = Some(fid);
        } else if let Some(a) = single {
            let mine: Vec<Uuid> = p.tree.families_as_partner(a).map(|x| x.common.id).collect();
            match mine.as_slice() {
                [] => {}
                [only] => into = Some(*only),
                _ => {
                    return Err(conflict(
                        "A parent on the picture is in more than one family in the tree. Choose which family the new people belong to.",
                    ));
                }
            }
        }
        if let Some(fid) = into {
            // Both parents of that family count for the loop check below.
            if let Some(fam) = p.tree.families.get(&fid) {
                (p1, p2) = (fam.partner1, fam.partner2);
            }
        }
        fams.push(Fam { p1, p2, into, kids });
    }
    check_no_cycle(p, &fams)?;

    // ---- write
    let link = picture.map(|pic| MediaLink {
        media: pic.media.common.id,
        region: None,
        primary: false,
    });
    if let (Some(pic), Some(l)) = (picture, &link) {
        let mut m = pic.media.clone();
        m.common.changed = Some(p.now);
        let mut src = Source {
            common: new_common(p.now),
            title: Some(PICTURE_SOURCE_TITLE.into()),
            author: None,
            publication: None,
            abbreviation: None,
            text: m.original_name.clone().map(|n| format!("File: {n}")),
            repositories: Vec::new(),
        };
        src.common.media.push(l.clone());
        p.put(m);
        p.put(src);
    }

    let mut first_new: Option<Uuid> = None;
    for d in persons.iter().filter(|d| d.existing.is_none()) {
        let id = ids.get(d.key.trim()).copied().unwrap_or_default();
        let (given, surnames, patronymic) = name_of(d);
        let mut common = new_common(p.now);
        common.id = id;
        if let Some(l) = &link {
            common.media.push(l.clone());
        }
        if let Some(t) = text(d.notes.as_deref()) {
            let note = Note {
                common: new_common(p.now),
                kind: NoteKind::General,
                text: t,
                lang: None,
                shared: false,
            };
            common.notes.push(note.common.id);
            p.put(note);
        }
        p.put(Person {
            common,
            sex: sex_of(d.sex, None, None),
            names: vec![Name {
                id: new_id(),
                kind: NameKind::Birth,
                assertion: assertion(true),
                private: false,
                prefix: None,
                given,
                call: None,
                surname_prefix: None,
                surnames,
                patronymic,
                suffix: None,
                nickname: None,
                lang: None,
                date: None,
                raw: None,
                citations: Vec::new(),
                notes: Vec::new(),
                unknown: Vec::new(),
            }],
            living_override: None,
            sibling_marker: false,
        });
        for (kind, raw) in [
            (EventKind::Birth, d.birth_date_raw.as_deref()),
            (EventKind::Death, d.death_date_raw.as_deref()),
        ] {
            if let Some(raw) = raw.map(str::trim).filter(|r| !r.is_empty()) {
                let ev = event(p, kind, raw, id);
                p.put(ev);
            }
        }
        first_new.get_or_insert(id);
    }

    for f in &fams {
        // A family between two people who already share one in the tree is reused.
        let reuse = match (f.p1, f.p2) {
            _ if f.into.is_some() => f.into,
            (Some(a), Some(b)) => p
                .tree
                .families
                .values()
                .find(|x| {
                    (x.partner1 == Some(a) && x.partner2 == Some(b))
                        || (x.partner1 == Some(b) && x.partner2 == Some(a))
                })
                .map(|x| x.common.id),
            _ => None,
        };
        let mut fam: Family = match reuse {
            Some(id) => {
                let mut x: Family = p.get(id)?;
                x.common.changed = Some(p.now);
                x
            }
            None => Family {
                common: new_common(p.now),
                partner1: f.p1,
                partner2: f.p2,
                kind: UnionKind::Unknown,
                children: Vec::new(),
            },
        };
        let fid = fam.common.id;
        for k in &f.kids {
            if fam.children.iter().any(|c| c.person == *k) {
                continue;
            }
            // A child keeps one preferred parent set: the first one it gets.
            let has_other = p.tree.families_as_child(*k).next().is_some()
                || p.written_families()
                    .any(|x| x.common.id != fid && x.children.iter().any(|c| c.person == *k));
            fam.children.push(ChildLink {
                person: *k,
                rel1: ParentRel::Birth,
                rel2: ParentRel::Birth,
                preferred: !has_other,
                assertion: assertion(false),
                citations: Vec::new(),
                unknown: Vec::new(),
            });
        }
        p.put(fam);
    }
    Ok(first_new)
}

/// Rejects links that would make a person their own ancestor.
fn check_no_cycle(p: &Planner, fams: &[Fam]) -> Result<(), ApiError> {
    // child -> parents, from the tree and from the draft.
    let mut parents: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for f in p.tree.families.values() {
        for c in &f.children {
            parents
                .entry(c.person)
                .or_default()
                .extend(f.partner1.into_iter().chain(f.partner2));
        }
    }
    for f in fams {
        for k in &f.kids {
            parents
                .entry(*k)
                .or_default()
                .extend(f.p1.into_iter().chain(f.p2));
        }
    }
    for f in fams {
        for k in &f.kids {
            // Walk up from the parents; meeting the child again is a loop.
            let mut stack: Vec<Uuid> = f.p1.into_iter().chain(f.p2).collect();
            let mut seen: HashSet<Uuid> = HashSet::new();
            while let Some(n) = stack.pop() {
                if n == *k {
                    return Err(invalid(
                        "These links make someone their own ancestor. Check the parent and child links.",
                    ));
                }
                if seen.insert(n)
                    && let Some(ps) = parents.get(&n)
                {
                    stack.extend(ps.iter().copied());
                }
            }
        }
    }
    Ok(())
}
