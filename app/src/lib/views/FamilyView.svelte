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
  import FamilyTree from './FamilyTree.svelte';
  import { flipMove, snapshotRects } from './flip';
  import { agentChanges } from '../state/agentChanges.svelte';
  import { t, type MessageKey } from '../i18n';

  let { onQuickAdd }: { onQuickAdd?: (relation: AddRelation, opts?: { newParentSet?: boolean; parentRel?: ParentRelId }) => void } = $props();

  const api = getApi();

  let view = $state<FamilyViewDto | null>(null);
  let empty = $state(false);
  let error = $state('');
  let root = $state<HTMLElement>();
  let selectedUnion = $state<string | null>(null);
  /** The family editor dialog: a family id to edit, 'new' to start one with the focus person. */
  let familyDialog = $state<string | null>(null);
  let token = 0;

  const MODE_KEY = 'gt.familyMode';
  function initialMode(): 'tree' | 'cards' {
    try {
      return localStorage.getItem(MODE_KEY) === 'cards' ? 'cards' : 'tree';
    } catch {
      return 'tree';
    }
  }
  let mode = $state<'tree' | 'cards'>(initialMode());
  function setMode(m: 'tree' | 'cards') {
    mode = m;
    try {
      localStorage.setItem(MODE_KEY, m);
    } catch {
      // Not remembered; the page still works.
    }
  }

  const homeId = $derived(project.info?.homePerson ?? settings.value.homePerson ?? null);

  type Chip = 'adopted' | 'step' | 'foster' | 'guardian' | 'sealed';
  const CHIPS: readonly string[] = ['adopted', 'step', 'foster', 'guardian', 'sealed'];
  type Role = 'father' | 'mother' | 'son' | 'daughter' | 'child' | 'brother' | 'sister' | 'sibling';
  const chipOf = (rel: ParentRelId | undefined): Chip | undefined => (rel && CHIPS.includes(rel) ? (rel as Chip) : undefined);
  const chipLabel = (c: Chip) => t(`kin.chip${c.charAt(0).toUpperCase()}${c.slice(1)}` as MessageKey);
  const UNION: Record<UnionKindId, MessageKey | null> = {
    married: 'family.married',
    civil_union: 'family.civilUnion',
    unmarried: 'family.unmarried',
    unknown: null,
    other: null,
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

  function add(relation: AddRelation, opts?: { newParentSet?: boolean; parentRel?: ParentRelId }) {
    if (onQuickAdd) onQuickAdd(relation, opts);
    else ui.requestAdd(relation, view?.focus.id, opts?.newParentSet, opts?.parentRel);
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
      items.push({ label: t('family.addAnotherParents'), onselect: () => add('father', { newParentSet: true }) });
    } else {
      items.push({ label: t('edit.addFather'), onselect: () => add('father'), disabled: !!primary?.parents[0], title: primary?.parents[0] ? t('family.fatherRecorded') : undefined });
      items.push({ label: t('edit.addMother'), onselect: () => add('mother'), disabled: !!primary?.parents[1], title: primary?.parents[1] ? t('family.motherRecorded') : undefined });
    }
    if (hasAnyParent) {
      items.push({ label: t('family.addStepFather'), onselect: () => add('father', { parentRel: 'step' }) });
      items.push({ label: t('family.addStepMother'), onselect: () => add('mother', { parentRel: 'step' }) });
    }
    items.push({ label: t('edit.addPartner'), onselect: () => add('partner') });
    items.push({ label: t('family.addFamily'), onselect: () => (familyDialog = 'new'), title: t('family.addFamilyHint') });
    items.push({ label: t('edit.addChild'), onselect: () => add('child') });
    items.push({
      label: t('edit.addSibling'),
      onselect: () => add('sibling'),
      disabled: !hasAnyParent,
      title: hasAnyParent ? undefined : t('family.addParentFirst'),
    });
    return items;
  });

  function parentRole(p: PersonRef, slot: number): Role {
    if (p.sex === 'male') return 'father';
    if (p.sex === 'female') return 'mother';
    return slot === 0 ? 'father' : 'mother';
  }
  function childRole(p: PersonRef): Role {
    return p.sex === 'male' ? 'son' : p.sex === 'female' ? 'daughter' : 'child';
  }
  function siblingRole(p: PersonRef): Role {
    return p.sex === 'male' ? 'brother' : p.sex === 'female' ? 'sister' : 'sibling';
  }
  /** "Adopted father" / "Father": the whole phrase is looked up, so other languages can agree in gender. */
  function withChip(chip: Chip | undefined, role: Role): string {
    return chip ? t(`kin.${chip}.${role}` as MessageKey) : t(`kin.${role}` as MessageKey);
  }
  /** The child's link to the person shown: to their mother a child can be her own and her husband's stepchild. */
  function childChip(c: ChildDto, u: PartnershipDto | null): Chip | undefined {
    return chipOf(c.rel[u?.focusSecond ? 1 : 0]);
  }
  function partnerSummary(p: PartnershipDto): string {
    const kind = UNION[p.kind];
    return [kind ? t(kind) : '', p.summary].filter(Boolean).join(', ');
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
    <div class="gap"><Button onclick={() => nav.go({ name: 'people' })}>{t('family.goToPeople')}</Button></div>
  {:else if empty}
    <EmptyState title={t('family.emptyTitle')}>
      {#snippet action()}
        <Button variant="primary" onclick={() => ui.requestAdd()}>{t('family.addPerson')}</Button>
      {/snippet}
    </EmptyState>
  {:else if view}
    {@const focus = view.focus}
    <div class="modes" role="group" aria-label={t('family.viewMode')}>
      <button type="button" class="mode" aria-pressed={mode === 'tree'} onclick={() => setMode('tree')}>{t('family.modeTree')}</button>
      <button type="button" class="mode" aria-pressed={mode === 'cards'} onclick={() => setMode('cards')}>{t('family.modeCards')}</button>
    </div>
    {#if mode === 'cards'}
    <section class="band" aria-label={t('family.parents')}>
      <h2>{t('family.parents')}</h2>
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
              agentChanged={agentChanges.unreviewed.has(p.id)}
              home={p.id === homeId}
              flipKey={p.id}
              chips={chipOf(primary.rel[slot]) ? [chipLabel(chipOf(primary.rel[slot])!)] : []}
              relationship={withChip(chipOf(primary.rel[slot]), parentRole(p, slot))}
              onclick={() => refocus(p.id)}
            />
          {:else}
            <div class="missing">
              <span>{slot === 0 ? t('family.parentMissing') : t('family.motherMissing')}</span>
              <Button variant="quiet" onclick={() => add(slot === 0 ? 'father' : 'mother')}>
                {slot === 0 ? t('edit.addFather') : t('edit.addMother')}
              </Button>
            </div>
          {/if}
        {/each}
      </div>
      {#if otherSets.length > 0}
        <details class="more">
          <summary>{t('family.otherParents', { n: otherSets.length })}</summary>
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
                    agentChanged={agentChanges.unreviewed.has(p.id)}
                    flipKey={p.id}
                    chips={chipOf(set.rel[slot]) ? [chipLabel(chipOf(set.rel[slot])!)] : []}
                    relationship={withChip(chipOf(set.rel[slot]), parentRole(p, slot))}
                    onclick={() => refocus(p.id)}
                  />
                {/if}
              {/each}
            </div>
          {/each}
        </details>
      {/if}
    </section>
    {/if}

    <div class="mid" class:solo={mode === 'tree'}>
      <IndexCard as="section" ribbon={focus.id === homeId} flipKey={focus.id}>
        <h1 class="fname"><bdi dir="auto">{focus.name}</bdi></h1>
        <p class="flife tnum">
          {#if focus.living}<span class="living"><span class="dot" aria-hidden="true"></span>{t('common.living')}</span>{/if}
          {#if focus.lifespan}<span>{focus.lifespan}</span>{/if}
          {#if focus.private}<Lock />{/if}
        </p>
        {#if view.focusSummary}<p class="fsum">{view.focusSummary}</p>{/if}
        <div class="acts">
          <Button onclick={() => nav.go({ name: 'person', id: focus.id })}>{t('family.editPerson')}</Button>
          <Menu label={t('family.add')} icon="plus" items={addItems} />
        </div>
      </IndexCard>

      {#if mode === 'cards'}
      <section class="band partners" aria-label={t('family.partners')}>
        <h2>{t('family.partners')}</h2>
        {#if view.partnerships.length === 0}
          <p class="none">{t('family.noPartners')}</p>
        {:else}
          {#if view.partnerships.length > 1}
            <div class="union-list" role="listbox" aria-label={t('family.choosePartner')} tabindex="-1">
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
                  <bdi dir="auto" class="oname">{p.partner?.name ?? t('family.partnerMissing')}</bdi>
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
                  agentChanged={agentChanges.unreviewed.has(union.partner.id)}
                  home={union.partner.id === homeId}
                  flipKey={union.partner.id}
                  relationship={t('kin.partner')}
                  onclick={() => refocus(union!.partner!.id)}
                />
              {:else}
                <div class="missing"><span>{t('family.partnerMissing')}</span></div>
              {/if}
              {#if partnerSummary(union)}<p class="usum">{partnerSummary(union)}</p>{/if}
              <div class="uacts"><Button variant="quiet" onclick={() => (familyDialog = union!.familyId)}>{t('family.editFamily')}</Button></div>
            </div>
          {/if}
        {/if}
      </section>
      {/if}
    </div>

    {#if mode === 'tree'}
      <FamilyTree {view} {homeId} onopen={refocus} onadd={(...args) => ui.requestAdd(...args)} />
    {:else}
    <section class="band" aria-label={t('family.children')}>
      <h2>{t('family.children')}</h2>
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
                agentChanged={agentChanges.unreviewed.has(c.person.id)}
                flipKey={c.person.id}
                detail={c.birth ? t('family.bornShort', { birth: c.birth }) : undefined}
                chips={childChip(c, union) ? [chipLabel(childChip(c, union)!)] : []}
                relationship={withChip(childChip(c, union), childRole(c.person))}
                onclick={() => refocus(c.person.id)}
              />
            </li>
          {/each}
        </ul>
      {:else}
        <p class="none">{t('family.noChildren')}</p>
      {/if}
    </section>

    {#if view.siblings.length > 0}
      <details class="band sibs">
        <summary>{t('family.siblings', { n: view.siblings.length })}</summary>
        <ul class="cards" role="list">
          {#each view.siblings as s (s.id)}
            <li>
              <PersonCard
                name={s.name}
                lifespan={s.lifespan}
                sex={s.sex}
                living={s.living}
                isPrivate={s.private}
                agentChanged={agentChanges.unreviewed.has(s.id)}
                flipKey={s.id}
                relationship={withChip(undefined, siblingRole(s))}
                onclick={() => refocus(s.id)}
              />
            </li>
          {/each}
        </ul>
      </details>
    {/if}
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
  .mid.solo {
    grid-template-columns: minmax(min(18rem, 100%), 30rem);
  }
  .modes {
    display: inline-flex;
    justify-self: start;
    border: 1px solid var(--ink-2);
    border-radius: var(--r-1);
    overflow: hidden;
  }
  .mode {
    padding: var(--s-2) var(--s-5);
    border: 0;
    background: transparent;
    color: var(--ink-1);
    font: inherit;
    cursor: pointer;
  }
  .mode[aria-pressed='true'] {
    background: var(--ribbon-wash);
    color: var(--ink-0);
    font-weight: 600;
  }
  .mode:focus-visible {
    outline: 2px solid var(--ribbon);
    outline-offset: -2px;
  }
  @media (max-width: 1100px) {
    .mid {
      grid-template-columns: 1fr;
    }
  }
</style>
