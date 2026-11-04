<script lang="ts">
  import { tick } from 'svelte';
  import Ribbon from '../ui/Ribbon.svelte';
  import Lock from '../ui/Lock.svelte';
  import { getApi } from '../api/context';
  import type { FamilyViewDto, PersonRef } from '../api/types';
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
    onadd: (relation: AddRelation, anchor: string) => void;
  } = $props();

  const api = getApi();
  const SIBLING_LIMIT = 4;
  const CHILD_LIMIT = 6;

  type Pair = [PersonRef | null, PersonRef | null];

  const primary = $derived(view.parentSets.find((s) => s.preferred) ?? view.parentSets[0] ?? null);
  const parents = $derived<Pair>(primary ? [primary.parents[0] ?? null, primary.parents[1] ?? null] : [null, null]);
  const hasParent = $derived(!!parents[0] || !!parents[1]);
  const hasAnyParent = $derived(view.parentSets.some((s) => s.parents[0] || s.parents[1]));

  // The family view only lists the focus person's own relatives, so the grandparents come from one more
  // lookup per parent. A failed lookup just leaves the slots empty.
  let grand = $state<Record<string, Pair>>({});
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
      ids.map(async (id): Promise<[string, Pair]> => {
        try {
          const v = await api.getFamilyView(id);
          const set = v.parentSets.find((s) => s.preferred) ?? v.parentSets[0];
          return [id, [set?.parents[0] ?? null, set?.parents[1] ?? null]];
        } catch {
          return [id, [null, null]];
        }
      }),
    ).then((rows) => {
      if (mine === grandToken) grand = Object.fromEntries(rows);
    });
  });

  let sibsOpen = $state(false);
  let kidsOpen = $state<Record<string, boolean>>({});
  $effect(() => {
    void view.focus.id;
    sibsOpen = false;
    kidsOpen = {};
  });

  const sibs = $derived(sibsOpen ? view.siblings : view.siblings.slice(0, SIBLING_LIMIT));
  const sibsHidden = $derived(view.siblings.length - sibs.length);

  const focus = $derived(view.focus);

  type Role = 'father' | 'mother' | 'son' | 'daughter' | 'child' | 'brother' | 'sister' | 'sibling' | 'partner' | 'grandfather' | 'grandmother';
  const roleKey = (r: Role): MessageKey =>
    r === 'grandfather' ? 'family.treeGrandfather' : r === 'grandmother' ? 'family.treeGrandmother' : (`kin.${r}` as MessageKey);
  function parentRole(p: PersonRef, slot: number, up = false): Role {
    const father = p.sex === 'male' || (p.sex !== 'female' && slot === 0);
    return up ? (father ? 'grandfather' : 'grandmother') : father ? 'father' : 'mother';
  }
  const kidRole = (p: PersonRef): Role => (p.sex === 'male' ? 'son' : p.sex === 'female' ? 'daughter' : 'child');
  const sibRole = (p: PersonRef): Role => (p.sex === 'male' ? 'brother' : p.sex === 'female' ? 'sister' : 'sibling');

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
  let active = $state<string | null>(null);
  $effect(() => {
    void view.focus.id;
    active = null;
  });
  const activeKey = $derived(active ?? `p:${focus.id}`);

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
      const next = rows[rows.indexOf(row) + (e.key === 'ArrowDown' ? 1 : -1)];
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
    aria-label={[p.name, p.lifespan, t(roleKey(role))].filter(Boolean).join(', ')}
    onclick={() => {
      if (!current) onopen(p.id);
    }}
  >
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
    onclick={() => onadd(relation, anchor.id)}
  >
    <span aria-hidden="true" class="plus">+</span>
    <span class="nm">{t(SLOT_TEXT[relation])}</span>
  </button>
{/snippet}

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="tree" bind:this={treeEl} role="group" aria-label={t('family.treeLabel', { who: focus.name })} onkeydown={keys}>
  {#if hasParent}
    <div class="row">
      {#each [0, 1] as i (i)}
        {@const p = parents[i]}
        {#if p}
          {@const gp = grand[p.id] ?? [null, null]}
          <div class="couple">
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
      {/each}
    </div>
    <div class="drop" aria-hidden="true"></div>
  {/if}

  <div class="row">
    <div class="couple">
      {#each [0, 1] as i (i)}
        {#if i === 1}<span class="join" aria-hidden="true"></span>{/if}
        {#if parents[i]}
          {@render box(parents[i]!, parentRole(parents[i]!, i), 1)}
        {:else}
          {@render slot(i === 0 ? 'father' : 'mother', focus, 1)}
        {/if}
      {/each}
    </div>
  </div>
  <div class="drop" aria-hidden="true"></div>

  <div class="row">
    {#each sibs as s (s.id)}
      {@render box(s, sibRole(s), 2)}
    {/each}
    {#if sibsHidden > 0}
      <button
        type="button"
        class="more"
        data-nav="more:sibs"
        data-row="2"
        tabindex={activeKey === 'more:sibs' ? 0 : -1}
        aria-label={t('family.treeShowSiblings', { n: sibsHidden })}
        onclick={() => (sibsOpen = true)}>{t('family.treeMore', { n: sibsHidden })}</button
      >
    {/if}
    {#if hasAnyParent}
      {@render slot('sibling', focus, 2)}
    {/if}
    {@render box(focus, 'child', 2, true)}
    {#each view.partnerships as u (u.familyId)}
      {#if u.partner}
        {@render box(u.partner, 'partner', 2)}
      {/if}
    {/each}
    {@render slot('partner', focus, 2)}
  </div>

  <div class="drop" aria-hidden="true"></div>
  <div class="row kidsrow">
    {#each view.partnerships.filter((u, i) => i === 0 || u.children.length > 0) as u, gi (u.familyId)}
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
        {#if gi === 0}
          <div class="kid">{@render slot('child', focus, 3)}</div>
        {/if}
      </div>
    {:else}
      <div class="kids"><div class="kid">{@render slot('child', focus, 3)}</div></div>
    {/each}
  </div>
</div>

<style>
  .tree {
    display: grid;
    justify-items: center;
    min-width: 0;
    max-width: 100%;
    overflow-x: auto;
    padding-block: var(--s-3);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    align-items: stretch;
    gap: var(--s-3) var(--s-5);
    max-width: 100%;
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
  }
  .drop {
    width: 2px;
    height: var(--s-5);
    background: var(--rule);
  }
  .kidsrow {
    align-items: flex-start;
    gap: var(--s-6);
  }
  .kids {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 0 var(--s-3);
  }
  .kid {
    position: relative;
    padding-top: var(--s-5);
    display: flex;
    margin-inline: calc(var(--s-3) / -2);
    padding-inline: calc(var(--s-3) / 2);
  }
  .kid::before {
    content: '';
    position: absolute;
    top: 0;
    inset-inline: 0;
    border-top: 2px solid var(--rule);
  }
  .kid:first-child::before {
    inset-inline-start: 50%;
  }
  .kid:last-child::before {
    inset-inline-end: 50%;
  }
  .kid:only-child::before {
    display: none;
  }
  .kid::after {
    content: '';
    position: absolute;
    top: 0;
    inset-inline-start: 50%;
    height: var(--s-5);
    border-inline-start: 2px solid var(--rule);
  }
  .box {
    position: relative;
    display: grid;
    align-content: center;
    min-width: 9.5rem;
    max-width: 13rem;
    min-height: 3.25rem;
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
  .plus {
    font-size: var(--t-md);
    font-weight: 600;
  }
  .more {
    align-self: center;
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
