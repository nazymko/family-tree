<script lang="ts">
  // Places: find one, then read what happened there (the «Events» tab) and bring in records from church books.
  import { untrack } from 'svelte';
  import Tabs from '../ui/Tabs.svelte';
  import Button from '../ui/Button.svelte';
  import TextField from '../ui/TextField.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import PlaceEvents from './PlaceEvents.svelte';
  import PlaceImportDialog from './PlaceImportDialog.svelte';
  import { getApi } from '../api/context';
  import type { Id, PlaceOption } from '../api/types';
  import { t } from '../i18n';

  let { onOpenPerson }: { onOpenPerson: (id: Id) => void } = $props();
  const api = getApi();

  let filter = $state('');
  let list = $state<PlaceOption[]>([]);
  let selected = $state<PlaceOption | null>(null);
  let tab = $state('events');
  let importing = $state(false);
  let refresh = $state(0);
  let seq = 0;

  $effect(() => {
    const q = filter.trim();
    const mine = ++seq;
    untrack(() => {
      void api
        .suggestPlaces(q, 100)
        .then((r) => {
          if (mine === seq) list = r;
        })
        .catch(() => {
          if (mine === seq) list = [];
        });
    });
  });
</script>

<div class="places">
  <aside class="side">
    <h1>{t('places.title')}</h1>
    <TextField label={t('places.find')} hint={t('places.findHint')} bind:value={filter} data-filter-input />
    {#if list.length === 0}
      <p class="none">{t('places.none')}</p>
    {:else}
      <ul aria-label={t('places.title')}>
        {#each list as p (p.id)}
          <li>
            <button type="button" class:on={selected?.id === p.id} aria-current={selected?.id === p.id ? 'true' : undefined} onclick={() => (selected = p)}>
              <span class="n">{p.name}</span>
              {#if p.full !== p.name}<span class="f">{p.full}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </aside>

  <div class="main">
    {#if selected}
      <div class="bar">
        <Button variant="secondary" onclick={() => (importing = true)}>{t('places.importButton')}</Button>
      </div>
      {#key selected.id}
        <Tabs label={t('places.tabs')} tabs={[{ id: 'events', label: t('places.tabEvents') }]} bind:selected={tab}>
          {#snippet panel()}
            <PlaceEvents place={selected!} {onOpenPerson} {refresh} />
          {/snippet}
        </Tabs>
      {/key}
    {:else}
      <EmptyState title={t('places.chooseFirst')}>
        {#snippet action()}
          <Button variant="secondary" onclick={() => (importing = true)}>{t('places.importButton')}</Button>
        {/snippet}
      </EmptyState>
    {/if}
  </div>
</div>

<PlaceImportDialog open={importing} onclose={() => (importing = false)} defaultPlace={selected} ondone={() => refresh++} />

<style>
  .places {
    display: grid;
    grid-template-columns: minmax(14rem, 20rem) 1fr;
    gap: var(--s-6);
    align-items: start;
  }
  @media (max-width: 48rem) {
    .places {
      grid-template-columns: 1fr;
    }
  }
  h1 {
    margin: 0 0 var(--s-3);
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: var(--s-3);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 60vh;
    overflow: auto;
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
  }
  li button {
    display: flex;
    flex-direction: column;
    width: 100%;
    text-align: start;
    border: 0;
    border-bottom: 1px solid var(--rule);
    background: var(--paper-1);
    padding: var(--s-3) var(--s-4);
    cursor: pointer;
    color: inherit;
  }
  li button.on {
    background: var(--paper-2);
    border-inline-start: 3px solid var(--ribbon);
  }
  .f {
    color: var(--ink-3);
    font-size: var(--t-xs);
  }
  .none {
    color: var(--ink-3);
  }
  .bar {
    display: flex;
    justify-content: flex-end;
    margin-bottom: var(--s-3);
  }
  .main {
    min-width: 0;
  }
</style>
