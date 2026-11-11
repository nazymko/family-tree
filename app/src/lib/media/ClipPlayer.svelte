<script lang="ts">
  import IconButton from '../ui/IconButton.svelte';
  import Button from '../ui/Button.svelte';
  import { getApi } from '../api/context';
  import { t } from '../i18n';
  import { clockMs, type ClipLink } from './clips';
  import { playOnly } from './playback';
  import { recordingView } from '../state/recording.svelte';

  /** A stretch of a recording where it is linked: plays only that stretch, and opens the whole recording. */
  let {
    link,
    where,
  }: {
    link: ClipLink;
    /** Where the clip is linked, shown instead of the recording's name where the recording is already named. */
    where?: string;
  } = $props();

  const api = getApi();
  let audio = $state<HTMLAudioElement>();
  let playing = $state(false);
  let now = $state(0);
  let failed = $state(false);

  const clip = $derived(link.clip);
  const recording = $derived(link.media.title || link.media.originalName || t('voice.untitled'));
  const label = $derived(clip.label || t('rec.untitledClip'));
  const span = $derived(t('rec.span', { start: clockMs(clip.startMs), end: clockMs(clip.endMs) }));
  const done = $derived(Math.min(1, Math.max(0, (now * 1000 - clip.startMs) / Math.max(1, clip.endMs - clip.startMs))));

  function toggle() {
    const a = audio;
    if (!a) return;
    if (!a.paused) {
      a.pause();
      return;
    }
    const at = a.currentTime * 1000;
    if (at < clip.startMs || at >= clip.endMs - 50) a.currentTime = clip.startMs / 1000;
    void a.play().catch(() => (failed = true));
  }

  function ontimeupdate() {
    const a = audio;
    if (!a) return;
    now = a.currentTime;
    // The clip ends where it ends, not where the recording does.
    if (a.currentTime * 1000 >= clip.endMs) {
      a.pause();
      a.currentTime = clip.startMs / 1000;
      now = a.currentTime;
    }
  }
</script>

<li class="clip" data-testid="clip-player">
  <div class="row">
    {#if link.media.resolved && !failed}
      <IconButton
        icon={playing ? 'pause' : 'play'}
        label={playing ? t('rec.pauseClipNamed', { label }) : t('rec.playClipNamed', { label })}
        onclick={toggle}
      />
    {/if}
    <div class="text">
      <span class="label"><bdi dir="auto">{label}</bdi></span>
      <span class="meta"><span dir="ltr">{span}</span> · <bdi dir="auto">{where ?? t('rec.clipOf', { recording })}</bdi></span>
    </div>
    <Button variant="quiet" aria-label={t('rec.wholeNamed', { label: recording })} onclick={() => recordingView.show(link.media.id, clip.id)}>{t('rec.whole')}</Button>
  </div>
  {#if !link.media.resolved}
    <p class="problem" role="alert" data-testid="clip-missing">{t('rec.clipMissing')}</p>
  {:else if failed}
    <p class="problem" role="alert">{t('voice.cannotPlay')}</p>
  {:else}
    <div class="bar" aria-hidden="true"><span style:inline-size="{done * 100}%"></span></div>
    <audio
      bind:this={audio}
      preload="none"
      src={api.mediaUrl(link.media.id, 'original')}
      onplay={(e) => {
        playOnly(e.currentTarget);
        playing = true;
      }}
      onpause={() => (playing = false)}
      onended={() => (playing = false)}
      {ontimeupdate}
      onerror={() => (failed = true)}
    ></audio>
  {/if}
</li>

<style>
  .clip {
    display: grid;
    gap: var(--s-2);
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--rule);
    border-inline-start: 3px solid var(--verdigris);
    border-radius: var(--r-1);
    background: var(--paper-2);
    list-style: none;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    min-width: 0;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .label {
    color: var(--ink-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    font-size: var(--t-sm);
    color: var(--ink-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .bar {
    block-size: 3px;
    background: var(--rule);
    border-radius: 2px;
    overflow: hidden;
  }
  .bar span {
    display: block;
    block-size: 100%;
    background: var(--ink-1);
  }
  .problem {
    margin: 0;
    color: var(--ink-0);
    font-size: var(--t-sm);
    border-inline-start: 3px solid var(--ribbon);
    padding-inline-start: var(--s-3);
  }
</style>
