<script lang="ts">
  import { onMount } from 'svelte';
  import PagedList from '../ui/PagedList.svelte';
  import PersonRowView from '../ui/PersonRow.svelte';
  import PeopleFilters from './PeopleFilters.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import Button from '../ui/Button.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import { getApi } from '../api/context';
  import type { Page, PersonQuery, PersonRow } from '../api/types';
  import { nav } from '../state/nav.svelte';
  import { project } from '../state/project.svelte';
  import { settings } from '../state/settings.svelte';
  import { ui } from '../state/ui.svelte';
  import { registerShortcuts } from '../state/shortcuts';

  const api = getApi();
  const PAGE = 200;

  const DEFAULT: PersonQuery = {
    sort: 'surname',
    desc: false,
    text: '',
    living: 'any',
    noSource: false,
    noBirthDate: false,
    privateOnly: false,
  };

  let query = $state<PersonQuery>({ ...DEFAULT });
  let applied = $state<PersonQuery>({ ...DEFAULT });
  let first = $state<Page<PersonRow> | null>(null);
  let generation = $state(0);
  let selected = $state(0);
  let list = $state<ReturnType<typeof PagedList>>();
  let confirmDelete = $state<PersonRow | null>(null);
  let failed = $state('');

  const homeId = $derived(project.info?.homePerson ?? settings.value.homePerson ?? null);
  const total = $derived(first?.total ?? 0);
  const filtered = $derived(
    applied.text.trim() !== '' || applied.living !== 'any' || applied.noSource || applied.noBirthDate || applied.privateOnly,
  );
  const canRail = $derived(applied.sort === 'surname' || applied.sort === 'given');

  // Debounce every filter change by 150 ms; the first load is immediate.
  let timer: ReturnType<typeof setTimeout> | undefined;
  let primed = false;
  $effect(() => {
    const snapshot = { ...query };
    if (!primed) {
      primed = true;
      applied = snapshot;
      return;
    }
    clearTimeout(timer);
    timer = setTimeout(() => (applied = snapshot), 150);
    return () => clearTimeout(timer);
  });

  let token = 0;
  $effect(() => {
    const q = { ...applied };
    void project.rev;
    const mine = ++token;
    api
      .listPersons(q, 0, PAGE)
      .then((p) => {
        if (mine !== token) return;
        failed = '';
        first = p;
        selected = Math.min(selected, Math.max(0, p.total - 1));
        generation++;
      })
      .catch((e: Error) => {
        if (mine === token) failed = e.message;
      });
  });

  const loadPage = (offset: number, limit: number) =>
    api.listPersons({ ...applied }, offset, limit).then((p) => p.items as unknown[]);

  function isRow(x: unknown): x is PersonRow {
    return !!x && typeof x === 'object' && 'surname' in x;
  }
  const rowAt = (i: number): PersonRow | null => {
    const r = list?.itemAt(i);
    return isRow(r) ? r : null;
  };

  function open(i: number) {
    const r = rowAt(i);
    if (!r) return;
    nav.focus(r.id);
    nav.go({ name: 'family' });
  }

  function jump(offset: number) {
    selected = offset;
    list?.scrollToOffset(offset);
  }

  async function doDelete() {
    const r = confirmDelete;
    if (!r) return;
    confirmDelete = null;
    try {
      await api.apply({ type: 'deletePerson', id: r.id });
    } catch (e) {
      failed = (e as Error).message;
    }
  }

  function clearFilters() {
    query = { ...DEFAULT, sort: query.sort, desc: query.desc };
  }

  const LETTERS = Array.from({ length: 26 }, (_, i) => String.fromCharCode(65 + i));
  const letterMap = $derived.by(() => {
    const m: Record<string, number> = {};
    let other: number | null = null;
    for (const l of first?.letters ?? []) {
      if (/^[A-Z]$/.test(l.letter)) m[l.letter] = l.offset;
      else if (other === null || l.offset < other) other = l.offset;
    }
    return { m, other };
  });

  onMount(() =>
    registerShortcuts([
      {
        id: 'people-edit',
        keys: 'Mod+E',
        label: 'Edit the selected person',
        run: () => {
          const r = rowAt(selected);
          if (r) {
            nav.focus(r.id);
            nav.go({ name: 'person', id: r.id });
          }
        },
        when: () => rowAt(selected) !== null,
      },
    ]),
  );
</script>

<section class="people">
  <h1>People</h1>

  {#if failed}
    <p class="err" role="alert">{failed}</p>
  {/if}

  <PeopleFilters bind:query />

  <p class="status" role="status" aria-live="polite">
    {#if first}<span class="tnum">{total}</span> {total === 1 ? 'person' : 'people'}{/if}
  </p>

  {#if first && total === 0 && !filtered}
    <EmptyState title="This tree is empty. Add the first person.">
      {#snippet action()}
        <Button variant="primary" onclick={() => ui.requestAdd()}>Add person</Button>
      {/snippet}
    </EmptyState>
  {:else if first && total === 0}
    <p class="none">Nobody matches these filters. <Button variant="quiet" onclick={clearFilters}>Clear filters</Button></p>
  {:else if first}
    <div class="body">
      <div class="listbox">
        {#key generation}
          <PagedList
            bind:this={list}
            {total}
            rowHeight={40}
            pageSize={PAGE}
            {loadPage}
            initialPage={{ offset: first.offset, items: first.items }}
            selectedIndex={selected}
            onSelect={(i) => (selected = i)}
            onActivate={open}
            onDelete={(i) => (confirmDelete = rowAt(i))}
            ariaLabel="People"
            keyPrefix="person-row"
          >
            <!-- eslint-disable-next-line @typescript-eslint/no-unused-vars -->
            {#snippet row(item, _index, sel)}
              <PersonRowView row={isRow(item) ? item : null} home={isRow(item) && item.id === homeId} selected={sel} />
            {/snippet}
          </PagedList>
        {/key}
      </div>
      {#if canRail}
        <nav class="rail" aria-label="Jump to letter">
          {#each LETTERS as l (l)}
            <button type="button" disabled={letterMap.m[l] === undefined} onclick={() => jump(letterMap.m[l])}>
              {l}
            </button>
          {/each}
          <button
            type="button"
            class="other"
            disabled={letterMap.other === null}
            onclick={() => letterMap.other !== null && jump(letterMap.other)}
          >
            Other scripts
          </button>
        </nav>
      {/if}
    </div>
  {/if}
</section>

<Dialog open={confirmDelete !== null} title="Delete person" onclose={() => (confirmDelete = null)}>
  <p>
    Delete <bdi dir="auto">{confirmDelete?.name}</bdi>? Their links to parents, partners and children are removed too. You
    can undo this.
  </p>
  {#snippet footer()}
    <Button onclick={() => (confirmDelete = null)}>Cancel</Button>
    <Button variant="danger" onclick={doDelete}>Delete person</Button>
  {/snippet}
</Dialog>

<style>
  .people {
    display: grid;
    gap: var(--s-4);
  }
  .status {
    color: var(--ink-1);
    font-size: var(--t-sm);
    min-height: var(--t-sm-lh);
  }
  .body {
    display: flex;
    gap: var(--s-3);
    align-items: stretch;
    height: max(22rem, calc(100vh - 16rem));
  }
  .listbox {
    flex: 1;
    min-width: 0;
  }
  .rail {
    display: flex;
    flex-direction: column;
    width: 2rem;
    overflow-y: auto;
    align-items: center;
  }
  .rail button {
    flex: none;
    width: 100%;
    min-height: 1.2rem;
    border: 0;
    background: transparent;
    font-size: var(--t-xs);
    font-weight: 700;
    color: var(--ink-0);
    cursor: pointer;
    padding: 0;
  }
  .rail button:hover:not(:disabled) {
    background: var(--ribbon-wash);
  }
  .rail button:disabled {
    color: var(--ink-2);
    opacity: 0.55;
    cursor: default;
  }
  .rail .other {
    writing-mode: vertical-rl;
    min-height: 5.5rem;
    margin-top: var(--s-2);
  }
  .none {
    color: var(--ink-1);
  }
  .err {
    color: var(--danger);
  }
</style>
