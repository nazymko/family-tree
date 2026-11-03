<script lang="ts">
  import { tick, untrack } from 'svelte';
  import PersonCard from '../ui/PersonCard.svelte';
  import IndexCard from '../ui/IndexCard.svelte';
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import Menu, { type MenuItem } from '../ui/Menu.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import Lock from '../ui/Lock.svelte';
  import { getApi } from '../api/context';
  import type { ChildDto, FamilyViewDto, ParentRelId, PartnershipDto, PersonRef, UnionKindId } from '../api/types';
  import { nav } from '../state/nav.svelte';
  import { project } from '../state/project.svelte';
  import { settings } from '../state/settings.svelte';
  import { ui, type AddRelation } from '../state/ui.svelte';
  import FamilyEditor from '../editors/FamilyEditor.svelte';
  import { flipMove, snapshotRects } from './flip';

  let { onQuickAdd }: { onQuickAdd?: (relation: AddRelation, opts?: { newParentSet?: boolean }) => void } = $props();

  const api = getApi();

  let view = $state<FamilyViewDto | null>(null);
  let empty = $state(false);
  let error = $state('');
  let root = $state<HTMLElement>();
  let selectedUnion = $state<string | null>(null);
  /** The family editor dialog: a family id to edit, 'new' to start one with the focus person. */
  let familyDialog = $state<string | null>(null);
  let token = 0;

  const homeId = $derived(project.info?.homePerson ?? settings.value.homePerson ?? null);

  const CHIP: Partial<Record<ParentRelId, string>> = {
    adopted: 'Adopted',
    step: 'Step',
    foster: 'Foster',
    guardian: 'Guardian',
    sealed: 'Sealed',
  };
  const UNION: Record<UnionKindId, string> = {
    married: 'Married',
    civil_union: 'Civil union',
    unmarried: 'Not married',
    unknown: '',
    other: '',
  };

  const reduced = () =>
    typeof window !== 'undefined' && !!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;

  // Load (and re-centre) whenever the focus person changes or an edit lands.
  $effect(() => {
    const id = nav.focusPerson;
    void project.rev;
    if (!id) {
      untrack(findFocus);
      return;
    }
    const mine = ++token;
    const before = untrack(() => (root && view ? snapshotRects(root) : new Map<string, DOMRect>()));
    api
      .getFamilyView(id)
      .then(async (v) => {
        if (mine !== token) return;
        view = v;
        error = '';
        empty = false;
        await tick();
        if (mine === token && before.size > 0 && root) flipMove(root, before, 260, reduced());
      })
      .catch((e: Error) => {
        if (mine === token) {
          view = null;
          error = e.message;
        }
      });
  });

  async function findFocus() {
    if (homeId) {
      nav.focus(homeId);
      return;
    }
    const page = await api.listPersons(
      { sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false },
      0,
      1,
    );
    if (page.items[0]) nav.focus(page.items[0].id);
    else empty = true;
  }

  function refocus(id: string) {
    nav.visit(id);
  }

  function add(relation: AddRelation, opts?: { newParentSet?: boolean }) {
    if (onQuickAdd) onQuickAdd(relation, opts);
    else ui.requestAdd(relation, view?.focus.id, opts?.newParentSet);
  }

  const primary = $derived(view?.parentSets.find((s) => s.preferred) ?? view?.parentSets[0] ?? null);
  const otherSets = $derived((view?.parentSets ?? []).filter((s) => s !== primary));
  const bothParents = $derived(!!primary && !!primary.parents[0] && !!primary.parents[1]);
  const hasAnyParent = $derived((view?.parentSets ?? []).some((s) => s.parents[0] || s.parents[1]));

  const union = $derived<PartnershipDto | null>(
    view ? (view.partnerships.find((p) => p.familyId === selectedUnion) ?? view.partnerships[0] ?? null) : null,
  );

  const addItems = $derived.by<MenuItem[]>(() => {
    const items: MenuItem[] = [];
    if (bothParents) {
      items.push({ label: 'Add another set of parents', onselect: () => add('father', { newParentSet: true }) });
    } else {
      items.push({ label: 'Add father', onselect: () => add('father'), disabled: !!primary?.parents[0], title: primary?.parents[0] ? 'A father is already recorded' : undefined });
      items.push({ label: 'Add mother', onselect: () => add('mother'), disabled: !!primary?.parents[1], title: primary?.parents[1] ? 'A mother is already recorded' : undefined });
    }
    items.push({ label: 'Add partner', onselect: () => add('partner') });
    items.push({ label: 'Add family…', onselect: () => (familyDialog = 'new'), title: 'Record a partnership with an existing person' });
    items.push({ label: 'Add child', onselect: () => add('child') });
    items.push({
      label: 'Add sibling',
      onselect: () => add('sibling'),
      disabled: !hasAnyParent,
      title: hasAnyParent ? undefined : 'Add a parent first, so the sibling has someone to belong to',
    });
    return items;
  });

  function parentWord(p: PersonRef, slot: number): string {
    if (p.sex === 'male') return 'Father';
    if (p.sex === 'female') return 'Mother';
    return slot === 0 ? 'Father' : 'Mother';
  }
  function childWord(p: PersonRef): string {
    return p.sex === 'male' ? 'Son' : p.sex === 'female' ? 'Daughter' : 'Child';
  }
  function siblingWord(p: PersonRef): string {
    return p.sex === 'male' ? 'Brother' : p.sex === 'female' ? 'Sister' : 'Sibling';
  }
  function withChip(chip: string | undefined, word: string): string {
    return chip ? `${chip} ${word.toLowerCase()}` : word;
  }
  function childChip(c: ChildDto): string | undefined {
    return CHIP[c.rel[0]] ?? CHIP[c.rel[1]];
  }
  function partnerSummary(p: PartnershipDto): string {
    return [UNION[p.kind], p.summary].filter(Boolean).join(', ');
  }

  function optionKeys(e: KeyboardEvent) {
    if (!view) return;
    const list = view.partnerships;
    const at = Math.max(0, list.findIndex((p) => p.familyId === union?.familyId));
    let next = at;
    if (e.key === 'ArrowDown' || e.key === 'ArrowRight') next = Math.min(list.length - 1, at + 1);
    else if (e.key === 'ArrowUp' || e.key === 'ArrowLeft') next = Math.max(0, at - 1);
    else if (e.key === 'Home') next = 0;
    else if (e.key === 'End') next = list.length - 1;
    else return;
    e.preventDefault();
    selectedUnion = list[next].familyId;
    void tick().then(() => document.getElementById(`union-${list[next].familyId}`)?.focus());
  }
</script>

<div class="family" bind:this={root}>
  {#if error}
    <Banner kind="error">{error}</Banner>
    <div class="gap"><Button onclick={() => nav.go({ name: 'people' })}>Go to People</Button></div>
  {:else if empty}
    <EmptyState title="No one to show. Add the first person.">
      {#snippet action()}
        <Button variant="primary" onclick={() => ui.requestAdd()}>Add person</Button>
      {/snippet}
    </EmptyState>
  {:else if view}
    {@const focus = view.focus}
    <section class="band" aria-label="Parents">
      <h2>Parents</h2>
      <div class="cards">
        {#each [0, 1] as slot (slot)}
          {@const p = primary?.parents[slot] ?? null}
          {#if p && primary}
            <PersonCard
              name={p.name}
              lifespan={p.lifespan}
              sex={p.sex}
              living={p.living}
              isPrivate={p.private}
              home={p.id === homeId}
              flipKey={p.id}
              chips={CHIP[primary.rel[slot]] ? [CHIP[primary.rel[slot]]!] : []}
              relationship={withChip(CHIP[primary.rel[slot]], parentWord(p, slot))}
              onclick={() => refocus(p.id)}
            />
          {:else}
            <div class="missing">
              <span>{slot === 0 ? 'Father' : 'Mother'} not recorded</span>
              <Button variant="quiet" onclick={() => add(slot === 0 ? 'father' : 'mother')}>
                Add {slot === 0 ? 'father' : 'mother'}
              </Button>
            </div>
          {/if}
        {/each}
      </div>
      {#if otherSets.length > 0}
        <details class="more">
          <summary>Other parents ({otherSets.length})</summary>
          {#each otherSets as set (set.familyId)}
            <div class="cards set">
              {#each [0, 1] as slot (slot)}
                {@const p = set.parents[slot]}
                {#if p}
                  <PersonCard
                    name={p.name}
                    lifespan={p.lifespan}
                    sex={p.sex}
                    living={p.living}
                    isPrivate={p.private}
                    flipKey={p.id}
                    chips={CHIP[set.rel[slot]] ? [CHIP[set.rel[slot]]!] : []}
                    relationship={withChip(CHIP[set.rel[slot]], parentWord(p, slot))}
                    onclick={() => refocus(p.id)}
                  />
                {/if}
              {/each}
            </div>
          {/each}
        </details>
      {/if}
    </section>

    <div class="mid">
      <IndexCard as="section" ribbon={focus.id === homeId} flipKey={focus.id}>
        <h1 class="fname"><bdi dir="auto">{focus.name}</bdi></h1>
        <p class="flife tnum">
          {#if focus.living}<span class="living"><span class="dot" aria-hidden="true"></span>Living</span>{/if}
          {#if focus.lifespan}<span>{focus.lifespan}</span>{/if}
          {#if focus.private}<Lock />{/if}
        </p>
        {#if view.focusSummary}<p class="fsum">{view.focusSummary}</p>{/if}
        <div class="acts">
          <Button onclick={() => nav.go({ name: 'person', id: focus.id })}>Edit person</Button>
          <Menu label="Add" icon="plus" items={addItems} />
        </div>
      </IndexCard>

      <section class="band partners" aria-label="Partners">
        <h2>Partners</h2>
        {#if view.partnerships.length === 0}
          <p class="none">No partners recorded.</p>
        {:else}
          {#if view.partnerships.length > 1}
            <div class="union-list" role="listbox" aria-label="Choose a partner" tabindex="-1">
              {#each view.partnerships as p (p.familyId)}
                {@const on = p.familyId === union?.familyId}
                <!-- Roving focus: the selected option is the tab stop. -->
                <div
                  id="union-{p.familyId}"
                  class="opt"
                  class:on
                  role="option"
                  tabindex={on ? 0 : -1}
                  aria-selected={on}
                  onclick={() => (selectedUnion = p.familyId)}
                  onkeydown={optionKeys}
                >
                  <bdi dir="auto" class="oname">{p.partner?.name ?? 'Partner not recorded'}</bdi>
                  <span class="osum">{partnerSummary(p)}</span>
                </div>
              {/each}
            </div>
          {/if}
          {#if union}
            <div class="partner">
              {#if union.partner}
                <PersonCard
                  name={union.partner.name}
                  lifespan={union.partner.lifespan}
                  sex={union.partner.sex}
                  living={union.partner.living}
                  isPrivate={union.partner.private}
                  home={union.partner.id === homeId}
                  flipKey={union.partner.id}
                  relationship="Partner"
                  onclick={() => refocus(union!.partner!.id)}
                />
              {:else}
                <div class="missing"><span>Partner not recorded</span></div>
              {/if}
              {#if partnerSummary(union)}<p class="usum">{partnerSummary(union)}</p>{/if}
              <div class="uacts"><Button variant="quiet" onclick={() => (familyDialog = union!.familyId)}>Edit family</Button></div>
            </div>
          {/if}
        {/if}
      </section>
    </div>

    <section class="band" aria-label="Children">
      <h2>Children</h2>
      {#if union && union.children.length > 0}
        <ul class="cards kids" role="list">
          {#each union.children as c (c.person.id)}
            <li>
              <PersonCard
                name={c.person.name}
                lifespan={c.person.lifespan}
                sex={c.person.sex}
                living={c.person.living}
                isPrivate={c.person.private}
                flipKey={c.person.id}
                detail={c.birth ? `b. ${c.birth}` : undefined}
                chips={childChip(c) ? [childChip(c)!] : []}
                relationship={withChip(childChip(c), childWord(c.person))}
                onclick={() => refocus(c.person.id)}
              />
            </li>
          {/each}
        </ul>
      {:else}
        <p class="none">No children recorded.</p>
      {/if}
    </section>

    {#if view.siblings.length > 0}
      <details class="band sibs">
        <summary>Siblings ({view.siblings.length})</summary>
        <ul class="cards" role="list">
          {#each view.siblings as s (s.id)}
            <li>
              <PersonCard
                name={s.name}
                lifespan={s.lifespan}
                sex={s.sex}
                living={s.living}
                isPrivate={s.private}
                flipKey={s.id}
                relationship={siblingWord(s)}
                onclick={() => refocus(s.id)}
              />
            </li>
          {/each}
        </ul>
      </details>
    {/if}
  {/if}
</div>

{#if familyDialog && view}
  <FamilyEditor
    family={familyDialog === 'new' ? undefined : familyDialog}
    anchor={view.focus.id}
    seedPartner={familyDialog === 'new' ? view.focus.id : undefined}
    onDone={() => (familyDialog = null)}
    onCancel={() => (familyDialog = null)}
  />
{/if}

<style>
  .family {
    display: grid;
    gap: var(--s-6);
    min-width: 0;
  }
  .band {
    display: grid;
    gap: var(--s-3);
    min-width: 0;
  }
  .band h2 {
    font-size: var(--t-md);
    line-height: var(--t-md-lh);
    color: var(--ink-1);
    font-weight: 600;
  }
  .cards {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-3) var(--s-4);
    list-style: none;
    margin: 0;
    padding: 0;
    min-width: 0;
  }
  .cards > li {
    display: contents;
  }
  .set {
    margin-top: var(--s-3);
  }
  .missing {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-4);
    min-width: 11rem;
    min-height: 3.5rem;
    padding: var(--s-3) var(--s-4);
    border: 1px dashed var(--ink-2);
    border-radius: var(--r-1);
    color: var(--ink-1);
  }
  .more summary,
  .sibs summary {
    cursor: pointer;
    color: var(--ink-1);
    font-weight: 500;
  }
  .more[open] summary,
  .sibs[open] summary {
    margin-bottom: var(--s-3);
  }
  .mid {
    display: grid;
    grid-template-columns: minmax(18rem, 30rem) minmax(14rem, 1fr);
    gap: var(--s-7);
    align-items: start;
  }
  .fname {
    font-size: var(--t-2xl);
    line-height: 56px;
    padding-top: var(--s-2);
    padding-inline-end: var(--s-8);
    overflow-wrap: anywhere;
  }
  .flife,
  .fsum {
    line-height: var(--rule-step);
    color: var(--ink-1);
  }
  .flife {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-4);
    align-items: center;
    color: var(--ink-0);
    font-weight: 500;
  }
  .living {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    color: var(--verdigris);
  }
  .dot {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
    background: currentColor;
  }
  .acts {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    min-height: 56px;
    padding-top: var(--s-3);
  }
  .acts :global(.btn.quiet),
  .acts :global(.ib) {
    border: 1px solid var(--ink-2);
  }
  .acts :global(.ib) {
    min-height: 2.25rem;
    min-width: 2.25rem;
  }
  .union-list {
    display: grid;
    max-height: 13rem;
    overflow: auto;
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background: var(--paper-1);
  }
  .opt {
    display: flex;
    justify-content: space-between;
    gap: var(--s-4);
    padding: var(--s-2) var(--s-4);
    border-inline-start: 3px solid transparent;
    border-block-end: 1px solid var(--rule);
    cursor: pointer;
  }
  .opt.on {
    background: var(--ribbon-wash);
    border-inline-start-color: var(--ribbon);
  }
  .oname {
    font-family: var(--font-serif);
    font-weight: 600;
  }
  .osum,
  .uacts {
    margin-inline-start: calc(-1 * var(--s-3));
  }
  .usum {
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  .partner {
    display: grid;
    gap: var(--s-2);
    justify-items: start;
  }
  .none {
    color: var(--ink-1);
  }
  .gap {
    margin-top: var(--s-4);
  }
  @media (max-width: 1100px) {
    .mid {
      grid-template-columns: 1fr;
    }
  }
</style>
