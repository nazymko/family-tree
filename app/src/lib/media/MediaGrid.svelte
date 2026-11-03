<script lang="ts" module>
  import type { MediaDto } from '../api/api';
  import { notices } from '../ui/notices.svelte';
  export const isPdf = (m: MediaDto) => m.mime === 'application/pdf' || /\.pdf$/i.test(m.originalName ?? '');
  /** Opens the original in the OS viewer; the backend refuses types that are not safe, and the reason is shown. */
  export async function openOriginal(api: { openMediaExternally(id: string): Promise<void> }, id: string): Promise<void> {
    try {
      await api.openMediaExternally(id);
    } catch (e) {
      notices.show({ message: e instanceof Error ? e.message : String(e) });
    }
  }
  export const mediaLabel = (m: MediaDto) => m.title || m.originalName || t('media.untitledFile');
</script>

<script lang="ts">
  import Icon from '../ui/Icon.svelte';
  import Ribbon from '../ui/Ribbon.svelte';
  import Button from '../ui/Button.svelte';
  import { getApi } from '../api/context';
  import type { MediaLinkDto, OwnerRef } from '../api/api';
  import { t } from '../i18n';

  let { items, onOpen }: { items: MediaLinkDto[]; /** Reserved for the owner of the items. */ target?: OwnerRef; onOpen: (i: number) => void } = $props();
  const api = getApi();
</script>

<ul class="grid" aria-label={t('media.photosAndDocs')}>
  {#each items as link, i (link.media.id)}
    {@const m = link.media}
    <li class="tile">
      {#if !m.resolved}
        <div class="face missing">
          <Icon name="file" size="2rem" />
          <span>{t('media.fileMissing')}</span>
          <span class="name">{mediaLabel(m)}</span>
        </div>
      {:else if isPdf(m)}
        <div class="face doc">
          <Icon name="file" size="2rem" />
          <span class="name">{mediaLabel(m)}</span>
          <Button aria-label={t('media.openNamed', { label: mediaLabel(m) })} onclick={() => openOriginal(api, m.id)}>{t('common.open')}</Button>
        </div>
      {:else}
        <button type="button" class="face photo" aria-label={t('media.viewNamed', { label: mediaLabel(m) })} onclick={() => onOpen(i)}>
          <img loading="lazy" alt={mediaLabel(m)} src={api.mediaUrl(m.id, 'thumb')} />
        </button>
      {/if}
      {#if link.primary}
        <span class="rib"><Ribbon title={t('media.primaryPhoto')} size="1.5rem" /></span>
      {/if}
    </li>
  {/each}
</ul>

<style>
  .grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, 140px);
    gap: var(--s-4);
  }
  .tile {
    position: relative;
    width: 140px;
    height: 140px;
  }
  .face {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--s-2);
    width: 100%;
    height: 100%;
    padding: var(--s-3);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background: var(--paper-2);
    color: var(--ink-1);
    font-size: var(--t-sm);
    text-align: center;
    overflow: hidden;
  }
  .photo {
    padding: 0;
    cursor: pointer;
    background: var(--paper-1);
  }
  .photo:hover {
    border-color: var(--ink-0);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .name {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .missing {
    border-style: dashed;
  }
  .rib {
    position: absolute;
    inset-block-start: -1px;
    inset-inline-end: var(--s-3);
    pointer-events: none;
  }
</style>
