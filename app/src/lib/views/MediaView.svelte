<script lang="ts">
  import EmptyState from '../ui/EmptyState.svelte';
  import Dropzone from '../media/Dropzone.svelte';
  import MediaGrid from '../media/MediaGrid.svelte';
  import MediaViewer from '../media/MediaViewer.svelte';
  import VoiceSamples, { voiceSamples } from '../media/VoiceSamples.svelte';
  import { isSound } from '../media/sound';
  import { getApi } from '../api/context';
  import { project } from '../state/project.svelte';
  import Button from '../ui/Button.svelte';
  import type { Id, MediaDto, MediaLinkDto, OwnerRef } from '../api/api';
  import { t } from '../i18n';

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
  let all = $state<MediaLinkDto[]>([]);
  /** Pictures and documents, for the grid and the viewer. */
  const items = $derived(all.filter((l) => !isSound(l.media)));
  /** Recordings attached to the person: a row and a player each, never a thumbnail. */
  const sounds = $derived(voiceSamples(all));
  let loaded = $state(false);
  let open = $state<number | null>(null);
  const target = $derived<OwnerRef>({ kind: 'person', id: personId });

  async function load() {
    const p = await api.getPerson(personId);
    name = p.ref.name;
    all = p.media;
    loaded = true;
  }

  /** Files a deleted note or person left behind; deleting the owner can be undone, so the file is kept until removed here. */
  let orphans = $state<MediaDto[]>([]);
  async function loadOrphans() {
    orphans = await api.unlinkedMedia();
  }
  async function deleteOrphan(m: MediaDto) {
    await api.apply({ type: 'deleteMedia', id: m.id });
    await loadOrphans();
  }

  async function unlink(media: Id) {
    await api.apply({ type: 'detachMedia', target, media });
    await load();
  }

  $effect(() => {
    void personId;
    void project.rev; // undo/redo and edits elsewhere
    void load();
    void loadOrphans();
  });
</script>

<section class="media">
  <h1>{t('nav.media')}</h1>
  {#if loaded}
    <Dropzone {target} targetName={name} onAttached={() => load()} onUndone={() => load()} {onSuggestEvent} />
    {#if all.length === 0}
      <EmptyState title={t('mediaView.nothingAttached')}>
        {t('mediaView.noneBefore')}<bdi dir="auto">{name}</bdi>{t('mediaView.noneAfter')}
      </EmptyState>
    {:else}
      {#if items.length > 0}<MediaGrid {items} {target} onOpen={(i) => (open = i)} />{/if}
      {#if sounds.length > 0}
        <div class="sounds">
          <h2>{t('voice.audioName')}</h2>
          <VoiceSamples items={sounds} onRemove={(m) => void unlink(m.id)} />
        </div>
      {/if}
    {/if}
    {#if open !== null}
      <MediaViewer {items} index={open} {target} onClose={() => (open = null)} onChanged={() => load()} />
    {/if}
  {/if}
  {#if orphans.length > 0}
    <div class="orphans" data-testid="unlinked-media">
      <h2>{t('mediaView.unlinkedTitle')}</h2>
      <p class="hint">{t('mediaView.unlinkedHint')}</p>
      <ul>
        {#each orphans as m (m.id)}
          {@const label = m.title || m.originalName || t('media.untitledFile')}
          <li>
            <span><bdi dir="auto">{label}</bdi></span>
            <Button variant="danger" aria-label={t('mediaView.unlinkedDelete', { label })} onclick={() => deleteOrphan(m)}>{t('mediaView.unlinkedDeleteButton')}</Button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</section>

<style>
  .orphans h2 {
    margin: 0 0 var(--s-2);
    font-size: var(--t-md);
  }
  .hint {
    margin: 0 0 var(--s-3);
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  .orphans ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--s-2);
  }
  .orphans li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-3);
  }
  .sounds h2 {
    margin: 0 0 var(--s-3);
    font-size: var(--t-md);
  }
  .media {
    display: grid;
    gap: var(--s-5);
    max-width: var(--content-max);
  }
</style>
