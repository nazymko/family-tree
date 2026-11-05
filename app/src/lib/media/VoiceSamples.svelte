<script lang="ts" module>
  import type { MediaLinkDto } from '../api/api';
  /** The recordings among a record's media, in their order. */
  export const voiceSamples = (links: readonly MediaLinkDto[]): MediaLinkDto[] => links.filter((l) => l.media.mime?.startsWith('audio/'));
</script>

<script lang="ts">
  import VoiceSample from './VoiceSample.svelte';
  import type { MediaDto } from '../api/api';
  import { t } from '../i18n';

  let {
    items,
    onRemove,
    onTitle,
    busy = false,
  }: {
    items: readonly MediaLinkDto[];
    onRemove?: (m: MediaDto) => void;
    onTitle?: (m: MediaDto, title: string) => void;
    busy?: boolean;
  } = $props();

  const samples = $derived(voiceSamples(items));
</script>

{#if samples.length > 0}
  <ul class="samples" aria-label={t('voice.title')}>
    {#each samples as link (link.media.id)}
      <VoiceSample media={link.media} {onRemove} {onTitle} {busy} />
    {/each}
  </ul>
{/if}

<style>
  .samples {
    display: grid;
    gap: var(--s-3);
    margin: var(--s-3) 0 0;
    padding: 0;
  }
</style>
