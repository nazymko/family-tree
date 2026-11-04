<script lang="ts">
  import { getApi } from '../api/context';
  import type { Id, WholeMeta } from '../api/types';
  import WholeCanvas from '../chart/WholeCanvas.svelte';
  import { nav } from '../state/nav.svelte';
  import EmptyState from '../ui/EmptyState.svelte';

  let { onOpenPerson }: { onOpenPerson?: (id: Id) => void } = $props();

  const api = getApi();
  let home = $state<Id | null>(null);
  let ready = $state(false);
  let meta = $state<WholeMeta | null>(null);

  $effect(() => {
    void api
      .getSettings()
      .then((s) => (home = s.homePerson))
      .catch(() => {})
      .finally(() => (ready = true));
  });

  const openPerson = (id: Id) => (onOpenPerson ? onOpenPerson(id) : nav.go({ name: 'person', id }));
  function inPedigree(id: Id): void {
    nav.focus(id);
    nav.go({ name: 'pedigree' });
  }
  const empty = $derived(meta !== null && meta.meta.nodeCount === 0);
</script>

<section class="view" aria-labelledby="whole-title">
  <header>
    <h1 id="whole-title">Whole tree</h1>
    {#if meta && !empty}
      <p class="count">{meta.meta.nodeCount} {meta.meta.nodeCount === 1 ? 'person' : 'people'} in {meta.blocks.length} {meta.blocks.length === 1 ? 'group' : 'groups'}</p>
    {/if}
  </header>
  {#if empty}
    <EmptyState title="No people yet">Add a person in People and they will appear here.</EmptyState>
  {/if}
  {#if ready}
    <div class="body" hidden={empty}>
      <WholeCanvas homeId={home} onOpenPerson={openPerson} onOpenInPedigree={inPedigree} onOpened={(m) => (meta = m)} />
    </div>
  {/if}
</section>

<style>
  .view {
    display: flex;
    flex-direction: column;
    gap: var(--s-4);
    height: 100%;
    min-height: 0;
  }
  header {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--s-6);
  }
  h1 {
    margin: 0;
  }
  .count {
    margin: 0;
    color: var(--ink-2);
  }
  .body {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
  .body[hidden] {
    display: none;
  }
</style>
