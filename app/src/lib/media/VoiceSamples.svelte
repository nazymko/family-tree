<script lang="ts" module>
  import type { MediaLinkDto } from '../api/api';
  import { isSound } from './sound';
  /** The whole recordings among a record's media (not the clips cut from one), in their order. */
  export const voiceSamples = (links: readonly MediaLinkDto[]): MediaLinkDto[] => links.filter((l) => !l.clip && isSound(l.media));
</script>

<script lang="ts">
  import VoiceSample from './VoiceSample.svelte';
  import ClipPlayer from './ClipPlayer.svelte';
  import { clipsOf } from './clips';
  import type { MediaDto } from '../api/api';
  import { t } from '../i18n';

  /** A record's recordings: its voice samples, then the clips of recordings linked to it, each with its player. */
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
  const clips = $derived(clipsOf(items));
</script>

{#if samples.length > 0}
  <ul class="samples" aria-label={t('voice.title')}>
    {#each samples as link (link.media.id)}
      <VoiceSample media={link.media} {onRemove} {onTitle} {busy} />
    {/each}
  </ul>
{/if}
{#if clips.length > 0}
  <ul class="samples" aria-label={t('rec.clips')}>
    {#each clips as link (link.clip.id)}
      <ClipPlayer {link} />
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
