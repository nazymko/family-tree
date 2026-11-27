//! Merging a duplicate person into another, by an agent: everything of the merged person moves to
//! the kept one in one change set under the agent's batch, so the owner undoes it with the batch.
//!
//! This is the one place an agent removes a person. It is allowed because nothing is lost: names
//! stay as alternative names, and links, events, notes, citations, pictures and numbers move to
//! the kept person. Whether the two really are the same person is the agent's call, and its
//! reason is kept in the log and in a research note on the kept person.

use super::{AgentCtx, CORRECTION_REASON_MAX, people_reached, refused};
use crate::dto::NameOrder;
use crate::error::{ApiError, ErrorCode};
use crate::names::display_name;
use crate::ops::places::full_text;
use crate::ops::{Planner, new_common};
use crate::session::Session;
use gt_core::model::{
    Event, EventKind, ExternalId, Family, GodparentRole, LivingOverride, MediaLink, Name, Note,
    NoteKind, ParticipantRef, Person, Role, Sex, UnionKind,
};
use gt_core::tree::Tree;
use gt_store::{Actor, ChangeSet};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// What moved to the kept person, in counts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeopleMergeCounts {
    /// Names of the merged person now kept as alternative names.
    pub names: usize,
    /// Events the merged person took part in that now name the kept person.
    pub events: usize,
    /// Events that said the same thing as one the kept person had: folded into it.
    pub events_folded: usize,
    /// Families the merged person was a partner in.
    pub families_as_partner: usize,
    /// Families the merged person was a child in.
    pub families_as_child: usize,
    /// Families that were the same couple as another and were folded into it.
    pub families_folded: usize,
    /// Godparent entries (the person as godparent on other people's events) that now name the
    /// kept person.
    pub godparent_links: usize,
    pub notes: usize,
    pub citations: usize,
    pub pictures: usize,
    pub numbers: usize,
}

/// What a merge did, and what the agent should look at.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeopleMergeOutcome {
    pub kept: Option<Uuid>,
    pub removed: Option<Uuid>,
    pub counts: PeopleMergeCounts,
    pub conflicts: Vec<String>,
}

fn research_note(text: String, now: i64) -> Note {
    Note {
        common: new_common(now),
        kind: NoteKind::Research,
        text,
        lang: None,
        shared: false,
        story: None,
    }
}

fn key(s: &str) -> String {
    gt_store::fold_search(s.trim())
}

fn same_name(a: &Name, b: &Name) -> bool {
    a.kind == b.kind
        && key(a.given.as_deref().unwrap_or("")) == key(b.given.as_deref().unwrap_or(""))
        && key(a.patronymic.as_deref().unwrap_or("")) == key(b.patronymic.as_deref().unwrap_or(""))
        && a.surnames.len() == b.surnames.len()
        && a.surnames
            .iter()
            .zip(&b.surnames)
            .all(|(x, y)| key(x) == key(y))
        && key(a.raw.as_deref().unwrap_or("")) == key(b.raw.as_deref().unwrap_or(""))
}

fn union<T: PartialEq + Clone>(into: &mut Vec<T>, from: &[T]) -> usize {
    let mut added = 0;
    for x in from {
        if !into.contains(x) {
            into.push(x.clone());
            added += 1;
        }
    }
    added
}

fn union_media(into: &mut Vec<MediaLink>, from: &[MediaLink]) -> usize {
    let has_primary = into.iter().any(|l| l.primary);
    let mut added = 0;
    for l in from {
        if into
            .iter()
            .any(|k| k.media == l.media && k.region == l.region && k.clip == l.clip)
        {
            continue;
        }
        let mut l = l.clone();
        if has_primary {
            l.primary = false;
        }
        into.push(l);
        added += 1;
    }
    added
}

/// Every ancestor of `of`: parents (the partners of the families it is a child in), their
/// parents, and so on. A cycle already stored ends the walk.
fn ancestors(tree: &Tree, of: Uuid) -> HashSet<Uuid> {
    let mut seen: HashSet<Uuid> = HashSet::new();
    let mut queue = vec![of];
    while let Some(p) = queue.pop() {
        for f in tree.families.values() {
            if f.children.iter().any(|c| c.person == p) {
                for parent in [f.partner1, f.partner2].into_iter().flatten() {
                    if seen.insert(parent) {
                        queue.push(parent);
                    }
                }
            }
        }
    }
    seen
}

fn pair(f: &Family) -> Option<(Uuid, Uuid)> {
    match (f.partner1, f.partner2) {
        (Some(a), Some(b)) => Some(if a <= b { (a, b) } else { (b, a) }),
        _ => None,
    }
}

fn describe_event(tree: &Tree, e: &Event) -> String {
    let date = e
        .date
        .as_ref()
        .map_or_else(|| "no date".to_string(), |d| d.display());
    let place = e
        .place
        .and_then(|p| full_text(tree, p))
        .map(|t| format!(", {t}"))
        .unwrap_or_default();
    format!("{:?} ({date}{place}) [{}]", e.kind, e.common.id)
}

/// The facts a person has once; two of them after a merge are for the agent to look at.
fn once_in_a_life(k: &EventKind) -> bool {
    matches!(
        k,
        EventKind::Birth
            | EventKind::Christening
            | EventKind::Baptism
            | EventKind::Death
            | EventKind::Burial
    )
}

/// The checks that need only the tree.
fn check_pair(tree: &Tree, keep: Uuid, gone: Uuid) -> Result<(), ApiError> {
    if keep == gone {
        return Err(refused(
            "keep_id and merge_id are the same person: a person cannot be merged into themselves.",
        ));
    }
    let Some(k) = tree.persons.get(&keep) else {
        return Err(ApiError::not_found("person", keep));
    };
    let Some(m) = tree.persons.get(&gone) else {
        return Err(ApiError::not_found("person", gone));
    };
    if k.sibling_marker || m.sibling_marker {
        return Err(refused(
            "The entry for unnamed brothers and sisters is not a person and cannot be merged.",
        ));
    }
    if k.sex != Sex::Unknown && m.sex != Sex::Unknown && k.sex != m.sex {
        return Err(refused(
            "The two have different recorded sexes, so they are not the same person as far as \
             the tree knows. Leave them and list the pair in your report, or ask the owner to \
             correct a wrong sex first.",
        ));
    }
    if ancestors(tree, keep).contains(&gone) || ancestors(tree, gone).contains(&keep) {
        return Err(refused(
            "One of them is an ancestor of the other, so merging would make a person their own \
             ancestor and descendant. They are not the same person; leave them and list the pair \
             in your report.",
        ));
    }
    if tree.families.values().any(|f| {
        (f.partner1 == Some(keep) && f.partner2 == Some(gone))
            || (f.partner1 == Some(gone) && f.partner2 == Some(keep))
    }) {
        return Err(refused(
            "The two are partners of the same family, so they are not the same person. Leave \
             them and list the pair in your report.",
        ));
    }
    Ok(())
}

type Plan = (ChangeSet, PeopleMergeCounts, Vec<String>);

/// Moves `gone` into `keep` as one change set. See the module comment for what moves.
fn plan_merge_people(
    tree: &Tree,
    keep: Uuid,
    gone: Uuid,
    reason: &str,
    now: i64,
    order: gt_core::date::DayOrder,
) -> Result<Plan, ApiError> {
    let mut counts = PeopleMergeCounts::default();
    let mut conflicts: Vec<String> = Vec::new();
    let mut p = Planner::new(tree, now, order);
    let mut k: Person = p.get(keep)?;
    let m: Person = p.get(gone)?;
    let gone_name = display_name(&m, NameOrder::GivenFirst);

    // The person's own record.
    if k.sex == Sex::Unknown {
        k.sex = m.sex.clone();
    }
    for n in &m.names {
        if let Some(existing) = k.names.iter_mut().find(|e| same_name(e, n)) {
            union(&mut existing.citations, &n.citations);
            union(&mut existing.notes, &n.notes);
        } else {
            let mut n = n.clone();
            // A name row is keyed by its id, and the merged person's rows go in this change set.
            n.id = gt_core::ids::new_id();
            n.assertion.preferred = false;
            k.names.push(n);
            counts.names += 1;
        }
    }
    counts.notes += union(&mut k.common.notes, &m.common.notes);
    counts.citations += union(&mut k.common.citations, &m.common.citations);
    counts.pictures += union_media(&mut k.common.media, &m.common.media);
    counts.numbers += union::<ExternalId>(&mut k.common.ext_ids, &m.common.ext_ids);
    k.common.private |= m.common.private;
    if m.living_override == Some(LivingOverride::Deceased) {
        k.living_override = Some(LivingOverride::Deceased);
    }

    let (fams, folded_into) = move_families(tree, keep, gone, now, &mut counts, &mut conflicts);
    for f in fams.values() {
        p.put(f.clone());
    }
    for fid in folded_into.keys() {
        p.remove::<Family>(*fid);
    }
    move_events(
        &mut p,
        tree,
        keep,
        gone,
        &folded_into,
        now,
        &mut counts,
        &mut conflicts,
    );
    // Two birth families for one child.
    let birth_families: Vec<Uuid> = tree
        .families
        .values()
        .map(|f| fams.get(&f.common.id).unwrap_or(f))
        .filter(|f| !folded_into.contains_key(&f.common.id))
        .filter(|f| f.children.iter().any(|c| c.person == keep))
        .map(|f| f.common.id)
        .collect();
    if birth_families.len() > 1 {
        conflicts.push(format!(
            "The kept person is a child in {} families ({}): they had different parents. Check \
             which is right.",
            birth_families.len(),
            birth_families
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let kept_name = display_name(&k, NameOrder::GivenFirst);
    let note = research_note(
        format!("Merged {gone_name} [{gone}] into {kept_name} by an agent. Reason: {reason}"),
        now,
    );
    k.common.notes.push(note.common.id);
    k.common.changed = Some(now);
    p.put(note);
    p.put(k);
    p.remove::<Person>(gone);
    Ok((
        p.finish(format!("Merge people: {reason}")),
        counts,
        conflicts,
    ))
}

/// The families of `gone` with `keep` in their place, and the families that were the same couple
/// as another and so folded into it (folded id to surviving id).
fn move_families(
    tree: &Tree,
    keep: Uuid,
    gone: Uuid,
    now: i64,
    counts: &mut PeopleMergeCounts,
    conflicts: &mut Vec<String>,
) -> (HashMap<Uuid, Family>, HashMap<Uuid, Uuid>) {
    let mut fams: HashMap<Uuid, Family> = HashMap::new();
    let mut touched: Vec<Uuid> = Vec::new();
    for f in tree.families.values() {
        let is_partner = f.partner1 == Some(gone) || f.partner2 == Some(gone);
        let is_child = f.children.iter().any(|c| c.person == gone);
        if !is_partner && !is_child {
            continue;
        }
        let mut f = f.clone();
        if is_partner {
            counts.families_as_partner += 1;
            if f.partner1 == Some(gone) {
                f.partner1 = Some(keep);
            }
            if f.partner2 == Some(gone) {
                f.partner2 = Some(keep);
            }
        }
        if is_child {
            counts.families_as_child += 1;
            let gi = f.children.iter().position(|c| c.person == gone);
            let already = f.children.iter().any(|c| c.person == keep);
            if let (Some(gi), true) = (gi, already) {
                // Already a child here: one link, with both links' evidence.
                let g = f.children.remove(gi);
                if let Some(kl) = f.children.iter_mut().find(|c| c.person == keep) {
                    if kl.rel1 != g.rel1 || kl.rel2 != g.rel2 {
                        conflicts.push(format!(
                            "Family [{}] had both people as children with different parent \
                             relations; the kept person's link stays.",
                            f.common.id
                        ));
                    }
                    union(&mut kl.citations, &g.citations);
                }
            } else {
                for c in &mut f.children {
                    if c.person == gone {
                        c.person = keep;
                    }
                }
            }
        }
        f.common.changed = Some(now);
        touched.push(f.common.id);
        fams.insert(f.common.id, f);
    }
    // The same couple in two families becomes one: the merged person's family folds into the
    // family the kept person already had.
    let mut folded_into: HashMap<Uuid, Uuid> = HashMap::new();
    for fid in touched {
        let Some(f) = fams.get(&fid).cloned() else {
            continue;
        };
        let Some(couple) = pair(&f) else { continue };
        let target = tree
            .families
            .values()
            .map(|g| fams.get(&g.common.id).unwrap_or(g))
            .find(|g| {
                g.common.id != fid
                    && !folded_into.contains_key(&g.common.id)
                    && pair(g) == Some(couple)
            })
            .cloned();
        let Some(mut g) = target else { continue };
        for c in &f.children {
            match g.children.iter_mut().find(|x| x.person == c.person) {
                Some(x) => {
                    union(&mut x.citations, &c.citations);
                }
                None => g.children.push(c.clone()),
            }
        }
        union(&mut g.common.citations, &f.common.citations);
        union(&mut g.common.notes, &f.common.notes);
        union_media(&mut g.common.media, &f.common.media);
        union(&mut g.common.ext_ids, &f.common.ext_ids);
        if g.kind != f.kind {
            if g.kind == UnionKind::Unknown {
                g.kind = f.kind.clone();
            } else if f.kind != UnionKind::Unknown {
                conflicts.push(format!(
                    "The same couple was recorded with two kinds of union ({:?} and {:?}); the \
                     kept family keeps {:?}.",
                    g.kind, f.kind, g.kind
                ));
            }
        }
        g.common.changed = Some(now);
        counts.families_folded += 1;
        folded_into.insert(fid, g.common.id);
        fams.remove(&fid);
        fams.insert(g.common.id, g);
    }
    (fams, folded_into)
}

/// Events: participants and godparents that named `gone` (or a folded family) name the kept ones;
/// an event that then says exactly what another event of the kept person says is folded into it;
/// a fact a person has once, now there twice, is reported.
#[allow(clippy::too_many_arguments)]
fn move_events(
    p: &mut Planner,
    tree: &Tree,
    keep: Uuid,
    gone: Uuid,
    folded_into: &HashMap<Uuid, Uuid>,
    now: i64,
    counts: &mut PeopleMergeCounts,
    conflicts: &mut Vec<String>,
) {
    let mut moved: Vec<Uuid> = Vec::new();
    for e in tree.events.values() {
        let hit = e.participants.iter().any(|x| match &x.target {
            ParticipantRef::Person(id) => *id == gone,
            ParticipantRef::Family(id) => folded_into.contains_key(id),
        }) || e.godparents.iter().any(|g| g.person == Some(gone));
        if !hit {
            continue;
        }
        let mut e = e.clone();
        let mut as_participant = false;
        for x in &mut e.participants {
            match x.target.clone() {
                ParticipantRef::Person(id) if id == gone => {
                    x.target = ParticipantRef::Person(keep);
                    as_participant = true;
                }
                ParticipantRef::Family(id) => {
                    if let Some(to) = folded_into.get(&id) {
                        x.target = ParticipantRef::Family(*to);
                    }
                }
                ParticipantRef::Person(_) => {}
            }
        }
        let mut seen: Vec<(ParticipantRef, Role)> = Vec::new();
        e.participants.retain(|x| {
            let id = (x.target.clone(), x.role.clone());
            if seen.contains(&id) {
                false
            } else {
                seen.push(id);
                true
            }
        });
        for g in &mut e.godparents {
            if g.person == Some(gone) {
                g.person = Some(keep);
                counts.godparent_links += 1;
            }
        }
        let mut seen_gp: Vec<(Uuid, GodparentRole)> = Vec::new();
        e.godparents.retain(|g| {
            let Some(who) = g.person else { return true };
            if seen_gp.contains(&(who, g.role)) {
                false
            } else {
                seen_gp.push((who, g.role));
                true
            }
        });
        if as_participant {
            counts.events += 1;
        }
        e.common.changed = Some(now);
        moved.push(e.common.id);
        p.put(e);
    }
    let mut folded: HashSet<Uuid> = HashSet::new();
    for id in &moved {
        let Some(mine) = p.find::<Event>(*id) else {
            continue;
        };
        let twin = tree
            .events
            .keys()
            .filter(|o| *o != id && !folded.contains(o))
            .filter_map(|o| p.find::<Event>(*o))
            .find(|o| {
                o.kind == mine.kind
                    && o.date == mine.date
                    && o.place == mine.place
                    && o.description == mine.description
                    && o.negative == mine.negative
                    && !o.participants.is_empty()
                    && o.participants.len() == mine.participants.len()
                    && o.participants.iter().all(|x| mine.participants.contains(x))
                    && o.participants
                        .iter()
                        .any(|x| x.target == ParticipantRef::Person(keep))
            });
        let Some(mut twin) = twin else { continue };
        union(&mut twin.common.citations, &mine.common.citations);
        union(&mut twin.common.notes, &mine.common.notes);
        union_media(&mut twin.common.media, &mine.common.media);
        for g in &mine.godparents {
            if !twin
                .godparents
                .iter()
                .any(|x| x.person == g.person && x.name == g.name && x.role == g.role)
            {
                twin.godparents.push(g.clone());
            }
        }
        twin.common.changed = Some(now);
        p.put(twin);
        p.remove::<Event>(*id);
        folded.insert(*id);
        counts.events_folded += 1;
        counts.events = counts.events.saturating_sub(1);
    }
    let mut once: HashMap<String, Vec<Event>> = HashMap::new();
    for id in tree.events.keys() {
        let Some(e) = p.find::<Event>(*id) else {
            continue;
        };
        if e.negative || !once_in_a_life(&e.kind) {
            continue;
        }
        let principal = e
            .participants
            .iter()
            .any(|x| x.role == Role::Principal && x.target == ParticipantRef::Person(keep));
        if principal {
            once.entry(format!("{:?}", e.kind)).or_default().push(e);
        }
    }
    let mut kinds: Vec<_> = once.into_iter().collect();
    kinds.sort_by(|a, b| a.0.cmp(&b.0));
    for (kind, list) in kinds {
        if list.len() > 1 {
            conflicts.push(format!(
                "Two {kind} events, kept as two events: {}. Check which is right.",
                list.iter()
                    .map(|e| describe_event(tree, e))
                    .collect::<Vec<_>>()
                    .join(" and ")
            ));
        }
    }
}

impl Session {
    /// Merges the person `merge` into `keep`, in one change set (one undo step). Refused when the
    /// two are the same id or one is missing, when the merge would make a person their own
    /// ancestor or descendant, when their recorded sexes differ (unless one is unknown), when
    /// the merged person is the owner's home person, and when anyone who counts as dead would
    /// count as alive afterwards. When either counted as dead, the kept person counts as dead.
    pub fn merge_people_as_agent(
        &mut self,
        keep: Uuid,
        merge: Uuid,
        reason: &str,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(PeopleMergeOutcome, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(refused(
                "Give the reason: the evidence that the two are the same person (the same \
                 parents or spouse, matching dates).",
            ));
        }
        if reason.chars().count() > CORRECTION_REASON_MAX {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "The reason holds up to 1000 characters: put the rest in a note.",
            ));
        }
        check_pair(&self.tree, keep, merge)?;
        if self.settings.home_person == Some(merge) {
            return Err(refused(
                "merge_id is the owner's home person. Swap keep_id and merge_id so that the home \
                 person stays.",
            ));
        }
        let (_, now) = (self.clock.0)();
        let (cs, counts, conflicts) =
            plan_merge_people(&self.tree, keep, merge, reason, now, self.day_order())?;
        self.check_merge_living(&cs, keep, merge)?;
        let changed: Vec<Uuid> = cs.changes.iter().map(|c| c.id).collect();
        let mut reached = people_reached(&self.tree, &changed);
        let actor = Actor::Agent {
            id: ctx.id,
            name: ctx.name.clone(),
        };
        self.commit(&cs, actor, Some(batch))?;
        for id in people_reached(&self.tree, &changed) {
            if !reached.contains(&id) {
                reached.push(id);
            }
        }
        reached.retain(|id| *id != merge);
        if !reached.contains(&keep) {
            reached.push(keep);
        }
        Ok((
            PeopleMergeOutcome {
                kept: Some(keep),
                removed: Some(merge),
                counts,
                conflicts,
            },
            reached,
        ))
    }

    /// Nobody may count as alive who counted as dead before. The two being merged count as one
    /// person: if either was dead, the kept person must be dead after. Tried on the tree and taken
    /// off again.
    fn check_merge_living(
        &mut self,
        cs: &ChangeSet,
        keep: Uuid,
        gone: Uuid,
    ) -> Result<(), ApiError> {
        let (before, _) = self.living_maps();
        let mut removed_at = HashMap::new();
        crate::ops::apply_to_tree_tracked(&mut self.tree, cs, &mut removed_at);
        let (after, _) = self.living_maps();
        crate::ops::apply_to_tree_tracked(&mut self.tree, &cs.inverse(), &mut removed_at);
        let dead_before = before.get(&keep) == Some(&false) || before.get(&gone) == Some(&false);
        if dead_before && after.get(&keep) != Some(&false) {
            return Err(refused(
                "One of the two counts as dead, and after the merge the kept person would count \
                 as alive. An agent never makes anyone count as alive, so nothing was merged. \
                 List the pair in your report for the owner.",
            ));
        }
        for (id, was) in &before {
            if *id == keep || *id == gone {
                continue;
            }
            if after.get(id) != Some(was) {
                return Err(refused(format!(
                    "The merge would change whether {} counts as alive or dead. Only the owner \
                     can do that. List the pair in your report.",
                    self.person_name(*id)
                )));
            }
        }
        Ok(())
    }
}
