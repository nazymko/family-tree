<script lang="ts">
  import EmptyState from '../ui/EmptyState.svelte';
  import Dropzone from '../media/Dropzone.svelte';
  import MediaGrid from '../media/MediaGrid.svelte';
  import MediaViewer from '../media/MediaViewer.svelte';
  import { getApi } from '../api/context';
  import type { Id, MediaLinkDto, OwnerRef } from '../api/api';

  let {
    personId,
    onSuggestEvent,
  }: {
    /** The current focus person. */
    personId: Id;
    onSuggestEvent?: (date: string, target: OwnerRef) => void;
  } = $props();

  const api = getApi();
  let name = $state('');
  let items = $state<MediaLinkDto[]>([]);
  let loaded = $state(false);
  let open = $state<number | null>(null);
  const target = $derived<OwnerRef>({ kind: 'person', id: personId });

  async function load() {
    const p = await api.getPerson(personId);
    name = p.ref.name;
    items = p.media;
    loaded = true;
  }

  $effect(() => {
    void personId;
    void load();
  });
</script>

<section class="media">
  <h1>Media</h1>
  {#if loaded}
    <Dropzone {target} targetName={name} onAttached={() => load()} onUndone={() => load()} {onSuggestEvent} />
    {#if items.length === 0}
      <EmptyState title="Nothing attached">
        No photos or documents for <bdi dir="auto">{name}</bdi>. Drop files here to attach them.
      </EmptyState>
    {:else}
      <MediaGrid {items} {target} onOpen={(i) => (open = i)} />
    {/if}
    {#if open !== null}
      <MediaViewer {items} index={open} {target} onClose={() => (open = null)} onChanged={() => load()} />
    {/if}
  {/if}
</section>

<style>
  .media {
    display: grid;
    gap: var(--s-5);
    max-width: var(--content-max);
  }
</style>
