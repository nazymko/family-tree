//! FAM records, and the FAMC/FAMS side of INDI records.

use super::indi::read_event_child;
use super::{
    Ctx, Rec, assertion_child, child, common_child, link_child, new_assertion, new_common,
    push_unknown, resolve, val,
};
use crate::maps::parent_rel_from_text;
use crate::node::GedNode;
use gt_core::model::*;
use gt_core::raw::RawNode;
use uuid::Uuid;

use super::Repair;

/// What an INDI's `FAMC` said about the link to a family.
pub(crate) struct FamcInfo {
    pedi: Option<ParentRel>,
    frel: Option<ParentRel>,
    mrel: Option<ParentRel>,
    preferred: bool,
    assertion: Assertion,
    citations: Vec<Uuid>,
    unknown: Vec<RawNode>,
}

pub(crate) fn note_famc(ctx: &mut Ctx, indi: Uuid, c: &GedNode) {
    let Some(fam) = resolve(ctx, c, Rec::Fam) else {
        return;
    };
    if ctx.famc.contains_key(&(indi, fam)) {
        return;
    }
    let order = *ctx
        .famc_count
        .entry(indi)
        .and_modify(|n| *n += 1)
        .or_insert(0);
    let mut info = FamcInfo {
        pedi: None,
        frel: None,
        mrel: None,
        preferred: false,
        assertion: new_assertion(),
        citations: vec![],
        unknown: vec![],
    };
    for (pos, k) in c.children.iter().enumerate() {
        let v = val(k).map(str::trim);
        match (k.tag.as_str(), v) {
            ("PEDI", Some(v)) => {
                let t = if v.eq_ignore_ascii_case("OTHER") {
                    child(k, "PHRASE").and_then(val).unwrap_or(v)
                } else {
                    v
                };
                info.pedi = Some(parent_rel_from_text(t));
            }
            ("_FREL", Some(v)) => info.frel = Some(parent_rel_from_text(v)),
            ("_MREL", Some(v)) => info.mrel = Some(parent_rel_from_text(v)),
            ("_PREF", Some(v)) if v.eq_ignore_ascii_case("Y") => info.preferred = true,
            _ => {
                if assertion_child(&mut info.assertion, k) {
                    continue;
                }
                let mut notes = Vec::new();
                if link_child(
                    ctx,
                    crate::idgen::pair_id(indi, fam),
                    k,
                    &mut info.citations,
                    &mut notes,
                    None,
                ) {
                    if !notes.is_empty() {
                        super::count_dropped(ctx, "FAMC.NOTE");
                    }
                } else {
                    push_unknown(ctx, &mut info.unknown, k, pos, c.children.len());
                }
            }
        }
    }
    ctx.famc_order.insert((indi, fam), order);
    ctx.famc.insert((indi, fam), info);
    ctx.famc_by_fam.entry(fam).or_default().push(indi);
}

pub(crate) fn note_fams(ctx: &mut Ctx, indi: Uuid, c: &GedNode) {
    if let Some(fam) = resolve(ctx, c, Rec::Fam)
        && ctx.fams.insert((indi, fam))
    {
        ctx.fams_by_fam.entry(fam).or_default().push(indi);
    }
}

fn one_way(ctx: &mut Ctx, from: Uuid, to: Uuid) {
    let f = ctx.id_xref.get(&from).cloned().unwrap_or_default();
    let t = ctx.id_xref.get(&to).cloned().unwrap_or_default();
    ctx.report.repairs.push(Repair::OneWayLink {
        xref_from: f,
        xref_to: t,
    });
}

pub(crate) fn read_fam(ctx: &mut Ctx, node: &GedNode, id: Uuid) {
    ctx.fam_seq += 1;
    let seq = ctx.fam_seq;
    let mut common = new_common(id);
    if let Some(x) = &node.xref {
        common.ext_ids.push(ExternalId {
            kind: ExtIdKind::Xref,
            value: x.clone(),
        });
    }
    let mut husb: Vec<Uuid> = vec![];
    let mut wife: Vec<Uuid> = vec![];
    let mut chil: Vec<(Uuid, Option<ParentRel>, Option<ParentRel>)> = vec![];
    let mut married = false;
    for (pos, c) in node.children.iter().enumerate() {
        match c.tag.as_str() {
            "HUSB" => husb.extend(resolve(ctx, c, Rec::Indi)),
            "WIFE" => wife.extend(resolve(ctx, c, Rec::Indi)),
            "CHIL" => {
                if let Some(p) = resolve(ctx, c, Rec::Indi)
                    && !chil.iter().any(|x| x.0 == p)
                {
                    let f = child(c, "_FREL").and_then(val).map(parent_rel_from_text);
                    let m = child(c, "_MREL").and_then(val).map(parent_rel_from_text);
                    chil.push((p, f, m));
                }
            }
            _ => {
                if read_event_child(ctx, ParticipantRef::Family(id), c) {
                    married |= c.tag == "MARR";
                } else if !common_child(ctx, c, &mut common) {
                    push_unknown(ctx, &mut common.unknown, c, pos, node.children.len());
                }
            }
        }
    }

    // Partners: HUSB -> partner1, WIFE -> partner2; same-sex pairs keep both.
    let (mut p1, mut p2) = if husb.is_empty() && wife.len() >= 2 {
        (wife.first().copied(), wife.get(1).copied())
    } else {
        let mut p2 = wife.first().copied();
        let mut p1 = husb.first().copied();
        if p2.is_none() {
            p2 = husb.get(1).copied();
        }
        if p1.is_none() {
            p1 = wife.get(1).copied();
        }
        (p1, p2)
    };
    for p in p1.iter().chain(p2.iter()).copied().collect::<Vec<_>>() {
        if !ctx.fams.contains(&(p, id)) {
            one_way(ctx, id, p);
        }
    }
    let listed: Vec<Uuid> = ctx.fams_by_fam.get(&id).cloned().unwrap_or_default();
    for p in listed {
        if p1 == Some(p) || p2 == Some(p) {
            continue;
        }
        one_way(ctx, p, id);
        if p1.is_none() {
            p1 = Some(p);
        } else if p2.is_none() {
            p2 = Some(p);
        }
    }
    // Canonical order: a female first partner with a male second partner are swapped.
    let sex = |ctx: &Ctx, p: Option<Uuid>| {
        p.and_then(|p| ctx.tree.persons.get(&p))
            .map(|p| p.sex.clone())
    };
    if sex(ctx, p1) == Some(Sex::Female) && sex(ctx, p2) == Some(Sex::Male) {
        std::mem::swap(&mut p1, &mut p2);
    }

    // Children: union of FAM.CHIL and INDI.FAMC.
    let mut children: Vec<ChildLink> = Vec::new();
    let mut from_famc: Vec<Uuid> = ctx.famc_by_fam.get(&id).cloned().unwrap_or_default();
    from_famc.retain(|i| !chil.iter().any(|c| c.0 == *i));
    for (p, f, m) in chil {
        let info = ctx.famc.remove(&(p, id));
        if info.is_none() {
            one_way(ctx, id, p);
        }
        children.push(link_of(p, f, m, info));
    }
    for p in from_famc {
        one_way(ctx, p, id);
        let info = ctx.famc.remove(&(p, id));
        children.push(link_of(p, None, None, info));
    }
    for (idx, l) in children.iter().enumerate() {
        let key = match ctx.famc_order.get(&(l.person, id)) {
            Some(o) => (0u8, *o),
            None => (1u8, seq),
        };
        ctx.pref_cands
            .entry(l.person)
            .or_default()
            .push((key, id, idx));
    }
    ctx.tree.families.insert(
        id,
        Family {
            common,
            partner1: p1,
            partner2: p2,
            kind: if married {
                UnionKind::Married
            } else {
                UnionKind::Unknown
            },
            children,
        },
    );
}

fn link_of(
    person: Uuid,
    f: Option<ParentRel>,
    m: Option<ParentRel>,
    info: Option<FamcInfo>,
) -> ChildLink {
    let (pedi, frel, mrel, preferred, assertion, citations, unknown) = match info {
        Some(i) => (
            i.pedi,
            i.frel,
            i.mrel,
            i.preferred,
            i.assertion,
            i.citations,
            i.unknown,
        ),
        None => (None, None, None, false, new_assertion(), vec![], vec![]),
    };
    let base = pedi.unwrap_or(ParentRel::Birth);
    ChildLink {
        person,
        rel1: f.or(frel).unwrap_or_else(|| base.clone()),
        rel2: m.or(mrel).unwrap_or(base),
        preferred,
        assertion,
        citations,
        unknown,
    }
}
