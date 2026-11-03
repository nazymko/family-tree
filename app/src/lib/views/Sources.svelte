<script lang="ts">
  import TextField from '../ui/TextField.svelte';
  import Button from '../ui/Button.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import Banner from '../ui/Banner.svelte';
  import SourceForm, { type SourceValues } from './SourceForm.svelte';
  import { getApi } from '../api/context';
  import { notices } from '../ui/notices.svelte';
  import type { Id, SourceDto } from '../api/api';

  let { selectedId = null }: { selectedId?: Id | null } = $props();
  const api = getApi();
  let filter = $state('');
  let list = $state<SourceDto[]>([]);
  let selected = $state<Id | null>(null);
  let creating = $state(false);
  let error = $state('');
  let loaded = $state(false);
  let seq = 0;

  const current = $derived(list.find((s) => s.id === selected) ?? null);
  const count = (n: number) => `${n} ${n === 1 ? 'citation' : 'citations'}`;

  async function reload(q: string) {
    const mine = ++seq;
    const r = await api.suggestSources(q, 200);
    if (mine !== seq) return;
    list = r;
    loaded = true;
  }

  $effect(() => {
    void reload(filter);
  });
  $effect(() => {
    if (selectedId) selected = selectedId;
  });

  const clean = (v: SourceValues) => ({
    title: v.title.trim(),
    author: v.author,
    publication: v.publication,
    abbreviation: v.abbreviation,
    text: v.text,
  });

  async function save(v: SourceValues) {
    if (!selected) return;
    error = '';
    try {
      await api.apply({ type: 'saveSource', source: { id: selected, ...clean(v) } });
      notices.show({ message: 'Source saved' });
      await reload(filter);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function create(v: SourceValues) {
    error = '';
    try {
      const r = await api.apply({ type: 'saveSource', source: clean(v) });
      creating = false;
      notices.show({ message: 'Source added' });
      selected = r.touched[0] ?? null;
      filter = '';
      await reload('');
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }
</script>

<section class="sources">
  <header>
    <h1>Sources</h1>
    <Button
      variant="primary"
      onclick={() => {
        creating = true;
        error = '';
      }}>New source</Button
    >
  </header>
  <div class="cols">
    <div class="left">
      <TextField label="Filter sources" type="search" bind:value={filter} autocomplete="off" />
      {#if loaded && list.length === 0}
        <EmptyState title={filter ? 'No matching sources' : 'No sources yet'}>
          {filter ? 'Try a different word from the title.' : 'Sources you cite will be listed here.'}
        </EmptyState>
      {:else}
        <ul aria-label="Sources">
          {#each list as s (s.id)}
            <li>
              <button
                type="button"
                class:cur={s.id === selected}
                aria-current={s.id === selected ? 'true' : undefined}
                onclick={() => {
                  selected = s.id;
                  error = '';
                }}
              >
                <span class="title">{s.title ?? '(untitled source)'}</span>
                <span class="n">{count(s.citationCount)}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
    <div class="right">
      {#if current}
        {#key current.id}
          <h2>{current.title ?? '(untitled source)'}</h2>
          <p class="n">{count(current.citationCount)}</p>
          {#if error}<Banner kind="error">{error}</Banner>{/if}
          <SourceForm source={current} submitLabel="Save source" onSubmit={save} />
        {/key}
      {:else}
        <p class="n">Choose a source to see and edit its details.</p>
      {/if}
    </div>
  </div>
</section>

<Dialog open={creating} title="New source" onclose={() => (creating = false)}>
  {#if error}<Banner kind="error">{error}</Banner>{/if}
  <SourceForm submitLabel="Add source" onSubmit={create} />
</Dialog>

<style>
  .sources {
    display: grid;
    gap: var(--s-5);
    max-width: var(--content-max);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(16rem, 22rem) 1fr;
    gap: var(--s-7);
    align-items: start;
  }
  .left {
    display: grid;
    gap: var(--s-4);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    border-top: 1px solid var(--rule);
    max-height: 60vh;
    overflow: auto;
  }
  li button {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    width: 100%;
    min-height: var(--row-h);
    padding: var(--s-3) var(--s-4);
    border: 0;
    border-bottom: 1px solid var(--rule);
    border-inline-start: 3px solid transparent;
    background: transparent;
    color: var(--ink-0);
    text-align: start;
    font: inherit;
    cursor: pointer;
  }
  li button:hover {
    background: var(--paper-2);
  }
  li button.cur {
    background: var(--ribbon-wash);
    border-inline-start-color: var(--ribbon);
  }
  .n {
    font-size: var(--t-sm);
    color: var(--ink-1);
  }
  .right {
    display: grid;
    gap: var(--s-4);
  }
</style>
