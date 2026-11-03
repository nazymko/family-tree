//! Post-pass repairs.

use super::Ctx;

/// A person's preferred parent link: explicit `_PREF Y`, else the first link (FAMC order, then FAM order).
pub(crate) fn finish_preferred_links(ctx: &mut Ctx) {
    let cands = std::mem::take(&mut ctx.pref_cands);
    for (_, mut list) in cands {
        list.sort();
        let any = list.iter().any(|(_, fam, idx)| {
            ctx.tree
                .families
                .get(fam)
                .and_then(|f| f.children.get(*idx))
                .is_some_and(|l| l.preferred)
        });
        if !any
            && let Some((_, fam, idx)) = list.first()
            && let Some(l) = ctx
                .tree
                .families
                .get_mut(fam)
                .and_then(|f| f.children.get_mut(*idx))
        {
            l.preferred = true;
        }
    }
}
