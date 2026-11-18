<script lang="ts">
  import Dialog from '../ui/Dialog.svelte';
  import Banner from '../ui/Banner.svelte';
  import TextField from '../ui/TextField.svelte';
  import { getApi } from '../api/context';
  import { errMessage } from '../editors/errors';
  import { mediaLabel } from './MediaGrid.svelte';
  import { sizeText, versionsOf } from './versions';
  import type { Id, MediaDto } from '../api/api';
  import { plural, t } from '../i18n';

  let {
    exclude,
    onpick,
    onclose,
  }: {
    /** The files of the page already in the group: not offered again. */
    exclude: readonly Id[];
    onpick: (m: MediaDto) => void;
    onclose: () => void;
  } = $props();

  const api = getApi();
  let all = $state<MediaDto[] | null>(null);
  let error = $state('');
  let filter = $state('');

  $effect(() => {
    api.allPictures().then(
      (list) => (all = list),
      (e) => (error = errMessage(e, t('edit.wentWrong'))),
    );
  });

  const shown = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    return (all ?? [])
      .filter((m) => !exclude.includes(m.id) && !versionsOf(m).some((v) => exclude.includes(v.id)))
      .filter((m) => !q || [m.title, m.originalName, ...versionsOf(m).map((v) => v.originalName)].some((s) => s?.toLowerCase().includes(q)));
  });
</script>

<Dialog open title={t('media.pickVersionTitle')} {onclose} width="46rem">
  <div class="picker">
    <p class="help">{t('media.pickVersionHelp')}</p>
    {#if error}<Banner kind="error">{error}</Banner>{/if}
    <TextField label={t('media.pickVersionFilter')} bind:value={filter} />
    {#if all && shown.length === 0}
      <p class="help">{t('media.pickVersionNone')}</p>
    {:else}
      <ul class="list" data-testid="version-candidates">
        {#each shown as m (m.id)}
          {@const n = versionsOf(m).length}
          <li>
            <button type="button" class="pick" aria-label={t('media.pickVersionChoose', { label: mediaLabel(m) })} onclick={() => onpick(m)}>
              <img loading="lazy" alt="" src={api.mediaUrl(m.id, 'thumb')} style={m.rotation ? `transform: rotate(${m.rotation}deg)` : undefined} />
              <span class="name"><bdi dir="auto">{mediaLabel(m)}</bdi></span>
              <span class="size">{sizeText(m) ?? t('media.versionUnknownSize')}{n > 1 ? ` · ${plural('media.versions', n)}` : ''}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</Dialog>

<style>
  .picker {
    display: grid;
    gap: var(--s-3);
  }
  .help {
    margin: 0;
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: var(--s-3);
    max-height: 50vh;
    overflow: auto;
  }
  .pick {
    display: grid;
    gap: var(--s-1);
    width: 100%;
    padding: var(--s-2);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background: var(--paper-1);
    color: var(--ink-0);
    font-size: var(--t-sm);
    text-align: start;
    cursor: pointer;
  }
  .pick:hover,
  .pick:focus-visible {
    border-color: var(--ink-0);
  }
  img {
    width: 100%;
    aspect-ratio: 1;
    object-fit: cover;
    display: block;
    background: var(--paper-2);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .size {
    color: var(--ink-1);
  }
</style>
