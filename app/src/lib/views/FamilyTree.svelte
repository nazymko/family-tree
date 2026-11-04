<script lang="ts">
  import { tick } from 'svelte';
  import Ribbon from '../ui/Ribbon.svelte';
  import Lock from '../ui/Lock.svelte';
  import FlipTreeButton from '../ui/FlipTreeButton.svelte';
  import { settings } from '../state/settings.svelte';
  import { getApi } from '../api/context';
  import type { FamilyViewDto, PartnershipDto, PersonRef } from '../api/types';
  import { project } from '../state/project.svelte';
  import type { AddRelation } from '../state/ui.svelte';
  import { t, type MessageKey } from '../i18n';

  let {
    view,
    homeId = null,
    onopen,
    onadd,
  }: {
    view: FamilyViewDto;
    homeId?: string | null;
    onopen: (id: string) => void;
    onadd: (relation: AddRelation, anchor: string, newParentSet?: boolean) => void;
  } = $props();

  const api = getApi();
  /** Ancestors at the bottom and the descendants on top: the rows are shown in reverse (the DOM order stays). */
  const reversed = $derived(settings.value.treeDirection === 'reversed');
  const SIBLING_LIMIT = 4;
  const CHILD_LIMIT = 6;
  /** Children per partner, in order; "Add child" goes at the end of the last group. */
  const kidGroups = $derived(view.partnerships.filter((u, i) => i === 0 || u.children.length > 0));

  type Pair = [PersonRef | null, PersonRef | null];

  /** Father on the left, mother on the right; a parent of unknown sex takes the free place. */
  function place(pair: Pair): Pair {
    const f = pair.find((p) => p?.sex === 'male') ?? null;
    const m = pair.find((p) => p?.sex === 'female') ?? null;
    const rest = pair.filter((p): p is PersonRef => !!p && p !== f && p !== m);
    return [f ?? rest.shift() ?? null, m ?? rest.shift() ?? null];
  }

  let pickedSet = $state<number | null>(null);
  const setIndex = $derived.by(() => {
    if (pickedSet !== null && pickedSet < view.parentSets.length) return pickedSet;
    const i = view.parentSets.findIndex((s) => s.preferred);
    return i >= 0 ? i : 0;
  });
  const primary = $derived(view.parentSets[setIndex] ?? null);
  const parents = $derived<Pair>(primary ? place([primary.parents[0] ?? null, primary.parents[1] ?? null]) : [null, null]);
  const hasParent = $derived(!!parents[0] || !!parents[1]);
  const bothParents = $derived(!!parents[0] && !!parents[1]);
  const hasAnyParent = $derived(view.parentSets.some((s) => s.parents[0] || s.parents[1]));

  // The family view only lists the focus person's own relatives, so the grandparents come from one more
  // lookup per parent. A failed lookup just leaves the slots empty.
  // A parent with no entry yet (still loading) or a null entry (lookup failed) shows no grandparent boxes or slots.
  let grand = $state<Record<string, Pair | null>>({});
  let grandToken = 0;
  $effect(() => {
    const ids = parents.filter((p): p is PersonRef => !!p).map((p) => p.id);
    void project.rev;
    const mine = ++grandToken;
    if (ids.length === 0) {
      grand = {};
      return;
    }
    Promise.all(
      ids.map(async (id): Promise<[string, Pair | null]> => {
        try {
          const v = await api.getFamilyView(id);
          const set = v.parentSets.find((s) => s.preferred) ?? v.parentSets[0];
          return [id, place([set?.parents[0] ?? null, set?.parents[1] ?? null])];
        } catch {
          // Unknown: show nothing, so no "add" slot can duplicate a grandparent who does exist.
          return [id, null];
        }
      }),
    ).then((rows) => {
      if (mine === grandToken) grand = Object.fromEntries(rows);
    });
  });

  // With several parent sets the sibling list mixes full and half siblings: the shown set's family tells them apart.
  let fullIds = $state<Set<string> | null>(null);
  let fullToken = 0;
  $effect(() => {
    const famId = primary?.familyId;
    const several = view.parentSets.length > 1;
    void project.rev;
    const mine = ++fullToken;
    if (!several || !famId) {
      fullIds = null;
      return;
    }
    api
      .getFamily(famId)
      .then((f) => {
        if (mine === fullToken) fullIds = new Set(f.children.map((c) => c.person.id));
      })
      .catch(() => {
        if (mine === fullToken) fullIds = null;
      });
  });
  const isHalf = (s: PersonRef) => !!fullIds && !fullIds.has(s.id);
  const orderedSibs = $derived([...view.siblings.filter((s) => !isHalf(s)), ...view.siblings.filter(isHalf)]);

  let sibsOpen = $state(false);
  let kidsOpen = $state<Record<string, boolean>>({});
  $effect(() => {
    void view.focus.id;
    sibsOpen = false;
    kidsOpen = {};
    pickedSet = null;
  });

  const sibs = $derived(sibsOpen ? orderedSibs : orderedSibs.slice(0, SIBLING_LIMIT));
  const sibsHidden = $derived(orderedSibs.length - sibs.length);

  const focus = $derived(view.focus);

  type Role =
    | 'father' | 'mother' | 'son' | 'daughter' | 'child' | 'brother' | 'sister' | 'sibling' | 'partner'
    | 'grandfather' | 'grandmother' | 'halfBrother' | 'halfSister' | 'halfSibling' | 'husband' | 'wife'
    | `${(typeof LINKED)[number]}.${'father' | 'mother'}` | null;
  /** How a parent can be linked other than by birth; the box then says so (stepfather, adoptive mother, ...). */
  const LINKED = ['adopted', 'step', 'foster', 'guardian'] as const;
  const HALF = { halfBrother: 'family.treeHalfBrother', halfSister: 'family.treeHalfSister', halfSibling: 'family.treeHalfSibling' } as const;
  const roleKey = (r: NonNullable<Role>): MessageKey =>
    r === 'husband'
      ? 'family.treeHusband'
      : r === 'wife'
        ? 'family.treeWife'
        : r === 'grandfather'
      ? 'family.treeGrandfather'
      : r === 'grandmother'
        ? 'family.treeGrandmother'
        : r in HALF
          ? HALF[r as keyof typeof HALF]
          : (`kin.${r}` as MessageKey);
  function parentRole(p: PersonRef, slot: number, up = false): Role {
    const father = p.sex === 'male' || (p.sex !== 'female' && slot === 0);
    if (up) return father ? 'grandfather' : 'grandmother';
    // The person's own link to this parent: a stepfather reads "Stepfather", not "Father".
    const i = primary?.parents.findIndex((q) => q?.id === p.id) ?? -1;
    const rel = i >= 0 ? primary!.rel[i] : 'birth';
    const linked = (LINKED as readonly string[]).includes(rel) ? (rel as (typeof LINKED)[number]) : null;
    return linked ? `${linked}.${father ? 'father' : 'mother'}` : father ? 'father' : 'mother';
  }
  /**
   * A partner reads as husband or wife when the sex is known, unless the couple is recorded as not married (imported
   * families often carry no marriage record at all); anyone else is a partner.
   */
  const spouse = (u: PartnershipDto) => u.kind !== 'unmarried' && u.kind !== 'other';
  const partnerRole = (u: PartnershipDto): Role =>
    spouse(u) && u.partner?.sex === 'male' ? 'husband' : spouse(u) && u.partner?.sex === 'female' ? 'wife' : 'partner';
  /** With more than one partner, each group of children is captioned with its other parent. */
  const captioned = $derived(view.partnerships.length > 1);
  function otherParent(u: PartnershipDto): string {
    const p = u.partner;
    if (!p) return t('family.treeKidsNoParent');
    const key = p.sex === 'female' ? 'family.treeKidsMother' : p.sex === 'male' ? 'family.treeKidsFather' : 'family.treeKidsParent';
    return t(key, { who: p.name });
  }
  const kidRole = (p: PersonRef): Role => (p.sex === 'male' ? 'son' : p.sex === 'female' ? 'daughter' : 'child');
  const sibRole = (p: PersonRef): Role => {
    const k = p.sex === 'male' ? 'Brother' : p.sex === 'female' ? 'Sister' : 'Sibling';
    return (isHalf(p) ? `half${k}` : k.toLowerCase()) as Role;
  };

  const SLOT_LABEL: Record<AddRelation, MessageKey> = {
    father: 'family.addFatherOf',
    mother: 'family.addMotherOf',
    partner: 'family.addPartnerOf',
    child: 'family.addChildOf',
    sibling: 'family.addSiblingOf',
  };
  const SLOT_TEXT: Record<AddRelation, MessageKey> = {
    father: 'edit.addFather',
    mother: 'edit.addMother',
    partner: 'edit.addPartner',
    child: 'edit.addChild',
    sibling: 'edit.addSibling',
  };

  // Keyboard: Left/Right move along a row, Up/Down jump to the nearest box in the next row.
  let treeEl = $state<HTMLElement>();
  let leftEl = $state<HTMLElement>();
  let rightEl = $state<HTMLElement>();
  /**
   * How far the person sits off the middle of their own row: each side is only as wide as its boxes, so the other rows
   * move by the same amount to stay centred on the person (instead of padding the shorter side to the longer one).
   */
  let dx = $state(0);
  $effect(() => {
    if (!leftEl || !rightEl) return;
    const [l, r] = [leftEl, rightEl];
    const update = () => (dx = (l.offsetWidth - r.offsetWidth) / 2);
    update();
    const ro = new ResizeObserver(update);
    ro.observe(l);
    ro.observe(r);
    return () => ro.disconnect();
  });
  let active = $state<string | null>(null);
  $effect(() => {
    void view.focus.id;
    active = null;
  });
  const activeKey = $derived(active ?? `p:${focus.id}`);
  // If the remembered box left the page (a slot was filled, "+N more" was used), fall back to the person's own box.
  $effect(() => {
    void view;
    void grand;
    void sibsOpen;
    void kidsOpen;
    void active;
    void tick().then(() => {
      if (treeEl && !treeEl.querySelector('[data-nav][tabindex="0"]')) active = null;
    });
  });

  // A tree wider than the page opens scrolled to the person, not to its left edge.
  $effect(() => {
    void view.focus.id;
    void reversed;
    void tick().then(() => {
      const me = treeEl?.querySelector<HTMLElement>('.kid.self');
      if (!treeEl || !me || treeEl.scrollWidth <= treeEl.clientWidth) return;
      const tr = treeEl.getBoundingClientRect();
      const mr = me.getBoundingClientRect();
      treeEl.scrollLeft += mr.left + mr.width / 2 - (tr.left + tr.width / 2);
    });
  });

  function keys(e: KeyboardEvent) {
    if (!treeEl) return;
    const from = (e.target as HTMLElement).closest<HTMLElement>('[data-nav]');
    if (!from) return;
    const all = [...treeEl.querySelectorAll<HTMLElement>('[data-nav]')];
    const row = Number(from.dataset.row);
    const inRow = (n: number) => all.filter((el) => Number(el.dataset.row) === n);
    let to: HTMLElement | undefined;
    if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
      const same = inRow(row);
      to = same[same.indexOf(from) + (e.key === 'ArrowRight' ? 1 : -1)];
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      const rows = [...new Set(all.map((el) => Number(el.dataset.row)))].sort((a, b) => a - b);
      // Reversed, the rows run the other way up the page: Up goes to the next row number.
      const next = rows[rows.indexOf(row) + ((e.key === 'ArrowDown') !== reversed ? 1 : -1)];
      if (next === undefined) return;
      const here = from.getBoundingClientRect();
      const mid = here.left + here.width / 2;
      let best = Infinity;
      for (const el of inRow(next)) {
        const r = el.getBoundingClientRect();
        const d = Math.abs(r.left + r.width / 2 - mid);
        if (d < best) {
          best = d;
          to = el;
        }
      }
    } else if (e.key === 'Home' || e.key === 'End') {
      const same = inRow(row);
      to = e.key === 'Home' ? same[0] : same[same.length - 1];
    } else return;
    e.preventDefault();
    if (!to) return;
    active = to.dataset.nav ?? null;
    const target = to;
    void tick().then(() => target.focus());
  }
</script>

{#snippet box(p: PersonRef, role: Role, row: number, current = false)}
  <button
    type="button"
    class="box"
    class:current
    class:male={p.sex === 'male'}
    class:female={p.sex === 'female'}
    data-nav="p:{p.id}"
    data-row={row}
    tabindex={activeKey === `p:${p.id}` ? 0 : -1}
    aria-current={current ? 'true' : undefined}
    aria-label={[p.name, p.lifespan, role ? t(roleKey(role)) : ''].filter(Boolean).join(', ')}
    onfocus={() => (active = `p:${p.id}`)}
    onclick={() => {
      if (!current) onopen(p.id);
    }}
  >
    {#if role}<span class="kin" aria-hidden="true">{t(roleKey(role))}</span>{/if}
    <bdi dir="auto" class="nm">{p.name}</bdi>
    <span class="meta tnum">
      {#if p.living}<span class="living">{t('common.living')}</span>{/if}
      {#if p.lifespan}<span>{p.lifespan}</span>{/if}
      {#if p.private}<Lock />{/if}
    </span>
    {#if p.id === homeId}<span class="rib"><Ribbon size="0.9rem" /></span>{/if}
  </button>
{/snippet}

{#snippet slot(relation: AddRelation, anchor: PersonRef, row: number)}
  <button
    type="button"
    class="box slot"
    data-nav="s:{relation}:{anchor.id}"
    data-row={row}
    tabindex={activeKey === `s:${relation}:${anchor.id}` ? 0 : -1}
    aria-label={t(SLOT_LABEL[relation], { who: anchor.name })}
    onfocus={() => (active = `s:${relation}:${anchor.id}`)}
    onclick={() => onadd(relation, anchor.id)}
  >
    <span aria-hidden="true" class="plus">+</span>
    <span class="nm">{t(SLOT_TEXT[relation])}</span>
  </button>
{/snippet}

<div class="treebar"><FlipTreeButton /></div>
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="tree" class:reversed data-direction={reversed ? 'reversed' : 'normal'} bind:this={treeEl} role="group" aria-label={t('family.treeLabel', { who: focus.name })} onkeydown={keys}>
  <!-- Every row keeps to one line (wrapped rows would lose their lines); the tree scrolls sideways when wider. -->
  <div class="inner" style:--dx="{dx}px">
  {#if view.parentSets.length > 1 || bothParents}
    <div class="sets" role="group" aria-label={t('family.treeParentSets')}>
      {#each view.parentSets.length > 1 ? view.parentSets : [] as ps, i (ps.familyId)}
        <button type="button" class="setbtn" aria-pressed={i === setIndex} onclick={() => (pickedSet = i)}
          >{t('family.treeParentSet', { who: ps.parents.filter(Boolean).map((x) => x!.name).join(' + ') })}</button
        >
      {/each}
      {#if bothParents}
        <button
          type="button"
          class="box slot small"
          data-nav="s:newset:{focus.id}"
          data-row="-1"
          tabindex={activeKey === `s:newset:${focus.id}` ? 0 : -1}
          aria-label={t('family.addAnotherParents')}
          title={t('family.addAnotherParents')}
          onfocus={() => (active = `s:newset:${focus.id}`)}
          onclick={() => onadd('father', focus.id, true)}
        >
          <span aria-hidden="true" class="plus">+</span>
          <span class="nm">{t('family.treeOtherParents')}</span>
        </button>
      {/if}
    </div>
  {/if}

  <div class="anc">
    {#if hasParent}
      {#each [0, 1] as i (i)}
        {@const p = parents[i]}
        {@const gp = p ? grand[p.id] : null}
        <div class="gcell" class:has={!!gp}>
          {#if p && gp}
            <span class="cap">{t(i === 0 ? 'family.treeFatherParents' : 'family.treeMotherParents')}</span>
            <div class="couple" role="group" aria-label={t(i === 0 ? 'family.treeFatherParents' : 'family.treeMotherParents')}>
              {#each [0, 1] as j (j)}
                {#if j === 1}<span class="join" aria-hidden="true"></span>{/if}
                {#if gp[j]}
                  {@render box(gp[j]!, parentRole(gp[j]!, j, true), 0)}
                {:else}
                  {@render slot(j === 0 ? 'father' : 'mother', p, 0)}
                {/if}
              {/each}
            </div>
          {/if}
        </div>
      {/each}
    {/if}
    {#each [0, 1] as i (i)}
      <div class="pcell" class:left={i === 0} class:right={i === 1}>
        {#if parents[i]}
          {@render box(parents[i]!, parentRole(parents[i]!, i), 1)}
        {:else}
          {@render slot(i === 0 ? 'father' : 'mother', focus, 1)}
        {/if}
      </div>
    {/each}
  </div>
  <div class="drop" aria-hidden="true"></div>

  <div class="me">
    <div class="side left" bind:this={leftEl}>
      {#if hasAnyParent}
        <div class="kid">{@render slot('sibling', focus, 2)}</div>
      {/if}
      {#each sibs as s (s.id)}
        <div class="kid">{@render box(s, sibRole(s), 2)}</div>
      {/each}
      {#if sibsHidden > 0}
        <div class="kid">
          <button
            type="button"
            class="more"
            data-nav="more:sibs"
            data-row="2"
            tabindex={activeKey === 'more:sibs' ? 0 : -1}
            aria-label={t('family.treeShowSiblings', { n: sibsHidden })}
            onclick={() => (sibsOpen = true)}>{t('family.treeMore', { n: sibsHidden })}</button
          >
        </div>
      {/if}
    </div>
    <div class="kid self" class:alone={!hasAnyParent && view.siblings.length === 0}>
      {@render box(focus, null, 2, true)}<span class="stem" aria-hidden="true"></span>
    </div>
    <div class="side right" bind:this={rightEl}>
      {#each view.partnerships as u (u.familyId)}
        {#if u.partner}
          <div class="mate"><span class="join" aria-hidden="true"></span>{@render box(u.partner, partnerRole(u), 2)}</div>
        {/if}
      {/each}
      <div class="mate"><span class="join" aria-hidden="true"></span>{@render slot('partner', focus, 2)}</div>
    </div>
  </div>

  <div class="drop" aria-hidden="true"></div>
  <div class="row kidsrow">
    {#each kidGroups as u, gi (u.familyId)}
      {@const shown = kidsOpen[u.familyId] ? u.children : u.children.slice(0, CHILD_LIMIT)}
      {@const hidden = u.children.length - shown.length}
      <div class="kids" role="group" aria-label={u.partner ? t('family.treeChildrenWith', { who: u.partner.name }) : t('family.children')}>
        {#each shown as c (c.person.id)}
          <div class="kid">{@render box(c.person, kidRole(c.person), 3)}</div>
        {/each}
        {#if hidden > 0}
          <div class="kid">
            <button
              type="button"
              class="more"
              data-nav="more:{u.familyId}"
              data-row="3"
              tabindex={activeKey === `more:${u.familyId}` ? 0 : -1}
              aria-label={t('family.treeShowChildren', { n: hidden })}
              onclick={() => (kidsOpen = { ...kidsOpen, [u.familyId]: true })}>{t('family.treeMore', { n: hidden })}</button
            >
          </div>
        {/if}
        {#if gi === kidGroups.length - 1}
          <div class="kid">{@render slot('child', focus, 3)}</div>
        {/if}
        {#if captioned && u.children.length > 0}
          <span class="kcap" style:grid-column="1 / span {shown.length + (hidden > 0 ? 1 : 0)}" title={otherParent(u)}>{otherParent(u)}</span>
        {/if}
      </div>
    {:else}
      <div class="kids"><div class="kid">{@render slot('child', focus, 3)}</div></div>
    {/each}
  </div>
  </div>
</div>

<style>
  .treebar {
    display: flex;
    justify-content: flex-end;
  }
  .tree {
    display: flex;
    min-width: 0;
    max-width: 100%;
    overflow-x: auto;
    padding-block: var(--s-3);
  }
  /*
   * As wide as the widest row, so every row centres on the same axis; centred in the page by its auto margins, which
   * drop to zero when it is wider (so its start is never cut off).
   */
  .inner {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: max-content;
    margin-inline: auto;
  }
  /*
   * Reversed: the rows run bottom to top (kids, the person, parents, grandparents) and every connector sits on the
   * side that faces the row it joins. The DOM order, and with it the tab order, is unchanged.
   */
  .tree.reversed > .inner {
    flex-direction: column-reverse;
  }
  /* Every row but the person's own moves by --dx, so all of them centre on the person. */
  .inner > :not(.me) {
    margin-inline-start: max(0px, calc(2 * var(--dx, 0px)));
    margin-inline-end: max(0px, calc(-2 * var(--dx, 0px)));
  }
  .reversed .anc .pcell {
    order: -1;
  }
  .reversed .gcell {
    align-content: start;
    padding-bottom: 0;
    padding-top: var(--s-5);
  }
  /* The caption goes under the couple, so it does not sit across the line from the parent. */
  .reversed .gcell .cap {
    order: 1;
  }
  .reversed .gcell.has::after {
    bottom: auto;
    top: 0;
  }
  .reversed .pcell {
    padding: var(--s-5) var(--s-3) 0;
  }
  .reversed .pcell::before,
  .reversed .pcell::after {
    bottom: auto;
    top: 0;
  }
  .reversed .side.right {
    padding-top: 0;
    padding-bottom: var(--s-5);
    align-items: flex-end;
    align-content: flex-end;
  }
  /* Boxes of the person's row sit on the bar side, so the bar meets every box at the same height. */
  .reversed .me > .side > .kid {
    align-items: flex-end;
  }
  .reversed .kid.self {
    flex-direction: column-reverse;
  }
  .reversed .kid {
    padding: 0 calc(var(--s-3) / 2) var(--s-5);
  }
  .reversed .kid::before,
  .reversed .kid::after {
    top: auto;
    bottom: 0;
  }
  .reversed .more {
    align-self: flex-end;
  }
  .reversed .sets {
    margin-bottom: 0;
    margin-top: var(--s-3);
  }
  .row {
    display: flex;
    flex-wrap: nowrap;
    justify-content: center;
    align-items: stretch;
    gap: var(--s-3) var(--s-5);
  }
  .sets {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--s-2);
    margin-bottom: var(--s-3);
  }
  .setbtn {
    padding: var(--s-1) var(--s-4);
    border: 1px solid var(--ink-2);
    border-radius: 999px;
    background: transparent;
    color: var(--ink-1);
    font: inherit;
    cursor: pointer;
  }
  .setbtn[aria-pressed='true'] {
    background: var(--ribbon-wash);
    color: var(--ink-0);
    font-weight: 600;
  }
  .setbtn:focus-visible {
    outline: 2px solid var(--ribbon);
    outline-offset: 2px;
  }
  /* Grandparents over parents: two equal columns, so the middle is the axis the lines meet on. */
  .anc {
    display: grid;
    grid-template-columns: 1fr 1fr;
    width: max-content;
    column-gap: 0;
  }
  .gcell {
    display: grid;
    justify-items: center;
    align-content: end;
    gap: var(--s-1);
    padding-inline: var(--s-3);
    padding-bottom: var(--s-5);
    position: relative;
    min-width: 0;
  }
  .gcell.has::after {
    content: '';
    position: absolute;
    bottom: 0;
    inset-inline-start: 50%;
    /* Centred on the middle (like .drop and .stem), so the lines above and below a box meet without a step. */
    margin-inline-start: -1px;
    height: var(--s-5);
    border-inline-start: 2px solid var(--rule);
  }
  .cap {
    font-size: var(--t-xs);
    color: var(--ink-1);
    text-transform: none;
  }
  .couple {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: var(--s-2);
  }
  .join {
    width: var(--s-4);
    border-top: 2px solid var(--rule);
    flex: none;
  }
  .pcell {
    position: relative;
    display: flex;
    justify-content: center;
    /* No space on the grandparents' side: their line runs straight into the box. */
    padding: 0 var(--s-3) var(--s-5);
    min-width: 0;
  }
  .pcell::before {
    content: '';
    position: absolute;
    bottom: 0;
    height: 0;
    border-top: 2px solid var(--rule);
  }
  .pcell.left::before {
    inset-inline: 50% 0;
  }
  .pcell.right::before {
    inset-inline: 0 50%;
  }
  .pcell::after {
    content: '';
    position: absolute;
    bottom: 0;
    inset-inline-start: 50%;
    /* Centred on the middle (like .drop and .stem), so the lines above and below a box meet without a step. */
    margin-inline-start: -1px;
    height: var(--s-5);
    border-inline-start: 2px solid var(--rule);
  }
  .drop {
    width: 2px;
    height: var(--s-4);
    background: var(--rule);
  }
  /* The person's row: brothers and sisters, the person, then partners. The person sits on the axis. */
  .me {
    display: grid;
    /* Each side only as wide as its boxes; the other rows follow the person (see --dx). */
    grid-template-columns: max-content auto max-content;
    width: max-content;
    /* Every column takes the row's full height: the bars run along one edge of the row, the stem the other. */
    align-items: stretch;
  }
  .me > .side > .kid {
    align-items: flex-start;
  }
  /* The person's own box, then a line on to the edge of the row that faces the children. */
  .kid.self {
    flex-direction: column;
    align-items: center;
  }
  .stem {
    flex: 1 0 0;
    width: 0;
    border-inline-start: 2px solid var(--rule);
  }
  .side {
    display: flex;
    flex-wrap: nowrap;
  }
  .side.left {
    justify-content: flex-end;
  }
  .side.right {
    align-items: flex-start;
    padding-top: var(--s-5);
  }
  .mate {
    display: flex;
    align-items: center;
    gap: 0;
  }
  .mate .join {
    width: var(--s-4);
  }
  .kidsrow {
    align-items: flex-start;
    gap: 0;
  }
  /* One column per child; a caption naming the other parent may run under a group's children (above when reversed). */
  .kids {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: max-content;
    justify-content: center;
    gap: 0;
  }
  .kids > .kid {
    grid-row: 1;
  }
  .kcap {
    grid-row: 2;
    min-width: 0;
    margin-inline: calc(var(--s-3) / 2);
    padding: var(--s-1) var(--s-2);
    border-radius: var(--r-1);
    background: var(--paper-2);
    color: var(--ink-1);
    font-size: var(--t-xs);
    text-align: center;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .reversed .kids > .kid {
    grid-row: 2;
  }
  .reversed .kcap {
    grid-row: 1;
  }
  .kid {
    position: relative;
    padding: var(--s-5) calc(var(--s-3) / 2) 0;
    display: flex;
  }
  .kid::before {
    content: '';
    position: absolute;
    top: 0;
    inset-inline: 0;
    border-top: 2px solid var(--rule);
  }
  .kids > .kid:first-child::before {
    inset-inline-start: 50%;
  }
  .kids > .kid:last-of-type::before {
    inset-inline-end: 50%;
  }
  .kids > .kid:only-of-type::before {
    display: none;
  }
  /* Children of several partners: one bar runs on through every group, so each group joins the person's line. */
  .kidsrow > .kids:not(:first-child) > .kid:first-child::before {
    inset-inline-start: 0;
  }
  .kidsrow > .kids:not(:last-child) > .kid:last-of-type::before {
    inset-inline-end: 0;
  }
  .kidsrow > .kids:not(:only-child) > .kid:only-of-type::before {
    display: block;
  }
  .side.left > .kid:first-child::before {
    inset-inline-start: 50%;
  }
  .kid::after {
    content: '';
    position: absolute;
    top: 0;
    inset-inline-start: 50%;
    /* Centred on the middle (like .drop and .stem), so the lines above and below a box meet without a step. */
    margin-inline-start: -1px;
    height: var(--s-5);
    border-inline-start: 2px solid var(--rule);
  }
  /* The person's own tick: the bar comes in from the brothers and sisters on the left only. */
  .kid.self::before {
    inset-inline-end: 50%;
  }
  .kid.self.alone::before {
    display: none;
  }
  .box {
    position: relative;
    display: grid;
    align-content: center;
    min-width: 9.5rem;
    max-width: 13rem;
    /* One height for every box and "Add" slot (room for the relation, name and dates), so rows line up. */
    min-height: 4.25rem;
    padding: var(--s-2) var(--s-4);
    border: 1px solid var(--rule);
    border-inline-start: 4px solid var(--ink-2);
    border-radius: var(--r-1);
    background: var(--paper-1);
    color: var(--ink-0);
    text-align: start;
    cursor: pointer;
    font: inherit;
  }
  .box.male {
    border-inline-start-color: var(--verdigris);
  }
  .box.female {
    border-inline-start-color: var(--ribbon);
  }
  .box:hover {
    border-color: var(--ink-2);
  }
  .box.current {
    background: var(--ribbon-wash);
    border-color: var(--ribbon);
    cursor: default;
  }
  .box:focus-visible,
  .more:focus-visible {
    outline: 2px solid var(--ribbon);
    outline-offset: 2px;
  }
  .nm {
    font-family: var(--font-serif);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* The relation to the person in the middle, e.g. "Father" or "Half-sister". */
  .kin {
    font-size: var(--t-xs);
    line-height: 1.2;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--ink-1);
  }
  .meta {
    display: flex;
    gap: var(--s-3);
    font-size: var(--t-sm);
    line-height: var(--t-sm-lh);
    color: var(--ink-1);
  }
  .living {
    color: var(--verdigris);
    font-weight: 500;
  }
  .rib {
    position: absolute;
    top: 0;
    inset-inline-end: var(--s-2);
  }
  .slot {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    border: 1px dashed var(--ink-2);
    background: transparent;
    color: var(--ink-1);
  }
  .slot.small {
    min-height: 2.25rem;
    min-width: 0;
  }
  .plus {
    font-size: var(--t-md);
    font-weight: 600;
  }
  /* Against the line's side of the slot, so the tick reaches it. */
  .more {
    align-self: flex-start;
    min-height: 2.25rem;
    padding: 0 var(--s-4);
    border: 1px dashed var(--ink-2);
    border-radius: 999px;
    background: transparent;
    color: var(--ink-1);
    cursor: pointer;
    font: inherit;
  }
</style>
