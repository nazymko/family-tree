<script lang="ts" module>
  /** 75 s -> "1:15"; an hour or more -> "1:02:03". */
  export function clock(seconds: number): string {
    const s = Math.round(seconds);
    const h = Math.floor(s / 3600);
    const mm = Math.floor((s % 3600) / 60);
    const ss = String(s % 60).padStart(2, '0');
    return h > 0 ? `${h}:${String(mm).padStart(2, '0')}:${ss}` : `${mm}:${ss}`;
  }
</script>

<script lang="ts">
  import Icon from '../ui/Icon.svelte';
  import Button from '../ui/Button.svelte';
  import TextField from '../ui/TextField.svelte';
  import { getApi } from '../api/context';
  import type { MediaDto } from '../api/api';
  import { t } from '../i18n';

  let {
    media,
    onRemove,
    onTitle,
    busy = false,
  }: {
    media: MediaDto;
    /** When given, the row has a remove button. */
    onRemove?: (m: MediaDto) => void;
    /** When given, the title can be edited; called with the new text when the field is left. */
    onTitle?: (m: MediaDto, title: string) => void;
    busy?: boolean;
  } = $props();

  const api = getApi();
  // The browser reports the length once it has read the start of the file; until then it is NaN.
  let duration = $state(Number.NaN);
  let failed = $state(false);
  const label = $derived(media.title || media.originalName || t('voice.untitled'));
  const known = $derived(Number.isFinite(duration) && duration > 0);
</script>

<li class="sample" data-testid="voice-sample">
  <div class="head">
    <Icon name="audio" />
    <span class="label"><bdi dir="auto">{label}</bdi></span>
    {#if media.resolved && !failed}
      <span class="len" data-testid="voice-length">
        {#if known}{t('voice.length', { time: clock(duration) })}{:else}{t('voice.loading')}{/if}
      </span>
    {/if}
    {#if onRemove}
      <Button variant="quiet" disabled={busy} aria-label={t('voice.removeNamed', { label })} onclick={() => onRemove(media)}>{t('voice.remove')}</Button>
    {/if}
  </div>
  {#if !media.resolved}
    <p class="problem" role="alert" data-testid="voice-missing">{t('voice.fileMissing')}</p>
  {:else if failed}
    <p class="problem" role="alert" data-testid="voice-error">{t('voice.cannotPlay')}</p>
  {:else}
    <!-- The browser's own player: play, pause, seek and volume, keyboard and screen-reader ready. -->
    <audio
      controls
      preload="metadata"
      src={api.mediaUrl(media.id, 'original')}
      aria-label={t('voice.playNamed', { label })}
      bind:duration
      onerror={() => (failed = true)}
    ></audio>
  {/if}
  {#if onTitle}
    <TextField
      label={t('voice.titleLabel')}
      value={media.title ?? ''}
      disabled={busy}
      onchange={(e) => onTitle(media, e.currentTarget.value)}
    />
  {/if}
</li>

<style>
  .sample {
    display: grid;
    gap: var(--s-3);
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background: var(--paper-2);
    list-style: none;
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    min-width: 0;
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-0);
  }
  .len {
    font-size: var(--t-sm);
    color: var(--ink-1);
    white-space: nowrap;
  }
  audio {
    width: 100%;
    max-width: 28rem;
  }
  .problem {
    margin: 0;
    color: var(--ink-0);
    font-size: var(--t-sm);
    border-inline-start: 3px solid var(--ribbon);
    padding-inline-start: var(--s-3);
  }
</style>
