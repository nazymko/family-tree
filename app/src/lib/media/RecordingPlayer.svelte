<script lang="ts" module>
  /** The speeds the player offers. */
  export const SPEEDS = [0.75, 1, 1.25, 1.5, 2];

  /** Keys the player answers wherever focus is inside it, except while typing. */
  export function playerKey(e: KeyboardEvent): 'toggle' | 'back' | 'forward' | 'prevClip' | 'nextClip' | null {
    const el = e.target as HTMLElement | null;
    const tag = el?.tagName ?? '';
    const range = tag === 'INPUT' && (el as HTMLInputElement).type === 'range';
    const typing = (!range && (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT')) || !!el?.isContentEditable;
    if (typing || e.ctrlKey || e.metaKey || e.altKey) return null;
    switch (e.key) {
      case ' ':
        // A button answers its own Space.
        return tag === 'BUTTON' ? null : 'toggle';
      case 'ArrowLeft':
        return range ? null : 'back';
      case 'ArrowRight':
        return range ? null : 'forward';
      case 'PageUp':
        return 'prevClip';
      case 'PageDown':
        return 'nextClip';
      default:
        return null;
    }
  }
</script>

<script lang="ts">
  import Dialog from '../ui/Dialog.svelte';
  import Button from '../ui/Button.svelte';
  import IconButton from '../ui/IconButton.svelte';
  import Banner from '../ui/Banner.svelte';
  import '../ui/controls.css';
  import ClipEditor from './ClipEditor.svelte';
  import PlacePicker from '../editors/PlacePicker.svelte';
  import TranscriptPanel from './TranscriptPanel.svelte';
  import { getApi } from '../api/context';
  import type { Id, PersonRef, RecordingClipDto, RecordingDto } from '../api/api';
  import { errMessage } from '../editors/errors';
  import { eventKindLabel } from '../i18n/backend';
  import { t } from '../i18n';
  import { nav } from '../state/nav.svelte';
  import { project } from '../state/project.svelte';
  import { clipAt, clockMs, stepClip } from './clips';
  import { onDestroy, untrack } from 'svelte';
  import { playOnly, watchEnd } from './playback';

  let { media, clip: startClip, onclose }: { media: Id; clip?: Id; onclose: () => void } = $props();

  const api = getApi();
  const uid = $props.id();
  let rec = $state<RecordingDto | null>(null);
  let error = $state('');
  let audio = $state<HTMLAudioElement>();
  let now = $state(0);
  let playing = $state(false);
  let speed = $state(1);
  let failed = $state(false);
  let fileDuration = $state(Number.NaN);
  /** Where to go once the file can seek (the clip the player was opened at). */
  let pending: number | null = null;
  /**
   * "Clip mode": a clip played from its row stops at its end. Leaving its range by seeking ends the mode and the
   * recording plays on from the new place.
   */
  let limit = $state<{ startMs: number; endMs: number; label: string } | null>(null);
  /** Said once the player has left clip mode, until the next clip is played. */
  let notice = $state('');
  let unwatch: (() => void) | null = null;
  let editor = $state<{ editing?: RecordingClipDto } | null>(null);
  /** Counts loads; only the latest one may show its result (an older one can resolve after a newer one). */
  let loadSeq = 0;
  /** The recording and clip the state below belongs to. */
  let shown = '';

  function unwatchEnd() {
    unwatch?.();
    unwatch = null;
  }
  onDestroy(() => {
    loadSeq++;
    unwatchEnd();
  });

  /** Back to a blank player: another recording (or clip) was asked for. */
  function reset() {
    loadSeq++;
    unwatchEnd();
    limit = null;
    notice = '';
    rec = null;
    error = '';
    failed = false;
    fileDuration = Number.NaN;
    now = 0;
    playing = false;
    pending = null;
    editor = null;
  }

  async function load(id: Id, startAt: Id | undefined, first: boolean) {
    const mine = ++loadSeq;
    try {
      const r = await api.getRecording(id);
      if (mine !== loadSeq) return;
      if (first && startAt) {
        const c = r.clips.find((x) => x.clip.id === startAt);
        if (c) {
          now = c.clip.startMs;
          pending = c.clip.startMs;
        }
      }
      rec = r;
      error = '';
    } catch (e) {
      if (mine === loadSeq) error = errMessage(e);
    }
  }

  $effect(() => {
    void project.rev; // a clip added, changed or undone anywhere
    const id = media;
    const startAt = startClip;
    const key = `${id}|${startAt ?? ''}`;
    const first = key !== shown;
    shown = key;
    if (first) untrack(reset);
    void load(id, startAt, first);
  });

  const durationMs = $derived(rec?.media.durationMs ?? (Number.isFinite(fileDuration) && fileDuration > 0 ? Math.round(fileDuration * 1000) : null));
  const title = $derived(rec ? rec.media.title || rec.media.originalName || t('voice.untitled') : t('rec.title'));
  const clips = $derived(rec?.clips ?? []);
  const heard = $derived(clipAt(clips.map((c) => ({ ...c.clip })), now));
  /** The person the player was opened about: the clip's, else the first clip's. */
  const aboutPerson = $derived<PersonRef | null>(
    clips.find((c) => c.clip.id === startClip)?.person ?? clips[0]?.person ?? null,
  );
  const span = $derived(Math.max(durationMs ?? 0, ...clips.map((c) => c.clip.endMs), 1));

  /** Stops the clip at its end: pause and leave clip mode. */
  function clipEnded() {
    const a = audio;
    unwatchEnd();
    if (limit && a) now = limit.endMs;
    limit = null;
    a?.pause();
  }

  /** Leaves clip mode; the recording goes on from where it is. */
  function leaveClip() {
    unwatchEnd();
    limit = null;
    notice = t('rec.clipLeft');
  }

  function setLimit(next: { startMs: number; endMs: number; label: string } | null) {
    unwatchEnd();
    limit = next;
    if (next && audio) unwatch = watchEnd(audio, next.endMs, clipEnded);
  }

  function seek(ms: number) {
    const to = Math.max(0, durationMs !== null ? Math.min(ms, durationMs) : ms);
    // Past the clip's end (or before its start): no longer a clip. Playing goes on, it does not stop.
    if (limit && (to < limit.startMs || to >= limit.endMs)) leaveClip();
    now = to;
    if (audio && audio.readyState > 0) audio.currentTime = to / 1000;
    else pending = to;
  }

  function toggle() {
    const a = audio;
    if (!a) return;
    if (a.paused) {
      if (limit && (now < limit.startMs || now >= limit.endMs)) setLimit(null);
      void a.play().catch(() => {});
    } else a.pause();
  }

  function playSpan(startMs: number, endMs: number, label = '') {
    setLimit(null);
    notice = '';
    seek(startMs);
    setLimit({ startMs, endMs, label });
    void audio?.play().catch(() => {});
  }

  function step(dir: 1 | -1) {
    const to = stepClip(
      clips.map((c) => c.clip.startMs),
      now,
      dir,
    );
    if (to !== null) seek(to);
  }

  function onkeydown(e: KeyboardEvent) {
    const k = playerKey(e);
    if (!k) return;
    e.preventDefault();
    if (k === 'toggle') toggle();
    else if (k === 'back') seek(now - 5000);
    else if (k === 'forward') seek(now + 5000);
    else step(k === 'prevClip' ? -1 : 1);
  }

  function ontimeupdate() {
    const a = audio;
    if (!a) return;
    now = Math.round(a.currentTime * 1000);
    // The timer from `watchEnd` stops a clip exactly; this is the fallback if it did not run.
    if (limit && now >= limit.endMs) clipEnded();
  }

  function onloadedmetadata() {
    const a = audio;
    if (!a) return;
    fileDuration = a.duration;
    a.playbackRate = speed;
    if (pending !== null) {
      a.currentTime = pending / 1000;
      pending = null;
    }
  }

  $effect(() => {
    if (audio) audio.playbackRate = speed;
  });

  /** Where a clip is linked, in words: "Birth of Ivan Koval", "Note: He told this one…". */
  function whereText(c: RecordingClipDto): string {
    const kind = c.target.kind as 'note' | 'citation' | 'event' | 'person' | 'family';
    const what =
      kind === 'event' && c.eventKind
        ? eventKindLabel(c.eventKind)
        : kind === 'person'
          ? (c.person?.name ?? t('rec.target.person'))
          : c.title
            ? `${t(`rec.target.${kind}`)}: ${c.title}`
            : t(`rec.target.${kind}`);
    if (kind === 'event' && c.person) return t('rec.targetOf', { what, person: c.person.name });
    return kind !== 'person' && kind !== 'family' && c.person ? t('rec.targetOn', { what, person: c.person.name }) : what;
  }

  function openTarget(c: RecordingClipDto) {
    if (c.target.kind === 'family') nav.go({ name: 'familyPage', id: c.target.id });
    else if (c.target.kind === 'person') nav.go({ name: 'person', id: c.target.id });
    else if (c.person) nav.go({ name: 'person', id: c.person.id });
    else return;
    onclose();
  }

  async function remove(c: RecordingClipDto) {
    error = '';
    try {
      await api.apply({ type: 'deleteAudioClip', target: c.target, clip: c.clip.id });
    } catch (e) {
      error = errMessage(e);
    }
  }

  // Where it was recorded: picked or typed, saved with the button so typing alone makes no places.
  let pendingPlace = $state<{ placeId?: Id; placeText?: string } | null | undefined>(undefined);
  async function savePlace() {
    if (pendingPlace === undefined) return;
    const v = pendingPlace;
    try {
      await api.apply({ type: 'updateMedia', id: media, ...(v?.placeId ? { placeId: v.placeId } : { placeText: v?.placeText ?? '' }) });
      pendingPlace = undefined;
      error = '';
      await load(media, undefined, false);
    } catch (e) {
      error = errMessage(e);
    }
  }

  const clipName = (c: RecordingClipDto) => c.clip.label || t('rec.untitledClip');
  const pct = (ms: number) => `${(Math.min(ms, span) / span) * 100}%`;
</script>

<Dialog open {title} {onclose} width="64rem" dismissOnScrim={false}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="player" {onkeydown} data-testid="recording-player">
    {#if error}<Banner kind="error">{error}</Banner>{/if}
    {#if !rec}
      {#if !error}<p class="muted">{t('rec.loading')}</p>{/if}
    {:else}
      {#if rec.media.date?.display || rec.media.place}
        <p class="muted">
          {#if rec.media.date?.display}<span data-testid="recording-date">{rec.media.date.display}</span>{/if}
          {#if rec.media.date?.display && rec.media.place}<span aria-hidden="true"> · </span>{/if}
          {#if rec.media.place}<span data-testid="recording-place">{rec.media.place.full}</span>{/if}
        </p>
      {/if}
      {#key rec.media.id}
        <div class="place">
          <PlacePicker label={t('rec.placeLabel')} value={rec.media.place ?? null} text="" onChange={(v) => (pendingPlace = v)} />
          {#if pendingPlace !== undefined}<Button onclick={savePlace}>{t('rec.savePlace')}</Button>{/if}
        </div>
      {/key}
      <p class="sources">
        {#if rec.sources.length}
          {t('rec.onSources')}
          {#each rec.sources as s, i (s.id)}
            {#if i > 0},
            {/if}
            <button
              type="button"
              class="link"
              aria-label={t('rec.openSource', { title: s.title ?? t('rec.untitledSource') })}
              onclick={() => {
                nav.go({ name: 'sources', id: s.id });
                onclose();
              }}><bdi dir="auto">{s.title ?? t('rec.untitledSource')}</bdi></button
            >
          {/each}
        {:else}
          {t('rec.noSource')}
        {/if}
      </p>
      {#if !rec.media.resolved}
        <Banner kind="warning">{t('voice.fileMissing')}</Banner>
      {:else if failed}
        <Banner kind="warning">{t('voice.cannotPlay')}</Banner>
      {/if}
      {#if rec.media.resolved}
        <audio
          bind:this={audio}
          preload="metadata"
          src={api.mediaUrl(rec.media.id, 'original')}
          {ontimeupdate}
          {onloadedmetadata}
          onplay={(e) => {
            playOnly(e.currentTarget);
            playing = true;
          }}
          onpause={() => (playing = false)}
          onended={() => {
            playing = false;
            unwatchEnd();
            limit = null;
          }}
          onerror={() => (failed = true)}
        ></audio>
      {/if}

      <!-- Time runs left to right on the bar in every language, like the tape it stands for. -->
      <div class="seek" dir="ltr">
        <div class="marks" aria-hidden="true">
          {#each clips as c (c.clip.id)}
            <span
              class="mark"
              class:on={heard?.id === c.clip.id}
              style:left={pct(c.clip.startMs)}
              style:width="calc({pct(c.clip.endMs - c.clip.startMs)} + 2px)"
              title={t('rec.clipMark', { label: clipName(c), start: clockMs(c.clip.startMs), end: clockMs(c.clip.endMs) })}
            ></span>
          {/each}
        </div>
        <input
          type="range"
          min="0"
          max={span}
          step="100"
          value={now}
          disabled={!rec.media.resolved || failed}
          aria-label={t('rec.seek')}
          aria-valuetext={t('rec.time', { now: clockMs(now), total: durationMs !== null ? clockMs(durationMs) : t('rec.lengthUnknown') })}
          oninput={(e) => seek(Number(e.currentTarget.value))}
        />
      </div>

      <div class="controls">
        <div class="btns" dir="ltr">
          <IconButton icon="skip-back" label={t('rec.prevClip')} onclick={() => step(-1)} disabled={clips.length === 0} />
          <Button onclick={() => seek(now - 10_000)} aria-label={t('rec.back10')}><span aria-hidden="true">−10</span></Button>
          <Button variant="primary" onclick={toggle} disabled={!rec.media.resolved || failed} aria-label={playing ? t('rec.pause') : t('rec.play')}>
            {playing ? t('rec.pause') : t('rec.play')}
          </Button>
          <Button onclick={() => seek(now + 10_000)} aria-label={t('rec.forward10')}><span aria-hidden="true">+10</span></Button>
          <IconButton icon="skip-forward" label={t('rec.nextClip')} onclick={() => step(1)} disabled={clips.length === 0} />
        </div>
        <span class="time" dir="ltr" aria-live="off">{clockMs(now)} / {durationMs !== null ? clockMs(durationMs) : '–:––'}</span>
        <label class="speed">
          <span>{t('rec.speed')}</span>
          <select bind:value={speed} class="control">
            {#each SPEEDS as s (s)}<option value={s}>{s}×</option>{/each}
          </select>
        </label>
      </div>
      {#if limit}
        <p class="limit" data-testid="clip-limit">
          <span>{t('rec.clipOnly', { label: limit.label || t('rec.untitledClip'), start: clockMs(limit.startMs), end: clockMs(limit.endMs) })}</span>
          <Button variant="quiet" onclick={leaveClip}>{t('rec.playWhole')}</Button>
        </p>
      {:else if notice}
        <p class="hint" role="status" data-testid="clip-left">{notice}</p>
      {/if}
      <p class="hint">{t('rec.keys')}</p>

      <div class="cols">
        <section class="clips" aria-labelledby="{uid}-clips">
          <div class="head">
            <h3 id="{uid}-clips">{t('rec.clips')}</h3>
            {#if !editor}<Button onclick={() => (editor = {})}>{t('rec.newClip')}</Button>{/if}
          </div>
          {#if editor}
            {#key editor}
              <ClipEditor
                media={rec.media}
                {durationMs}
                now={() => now}
                editing={editor.editing}
                person={aboutPerson}
                onListen={playSpan}
                onDone={() => (editor = null)}
                onCancel={() => (editor = null)}
              />
            {/key}
          {/if}
          {#if clips.length === 0}
            <p class="muted">{t('rec.clipsNone')}</p>
          {:else}
            <ol class="list">
              {#each clips as c (c.clip.id)}
                <li class:on={heard?.id === c.clip.id} class:start={c.clip.id === startClip}>
                  <button
                    type="button"
                    class="at"
                    aria-label={t('rec.playClipNamed', { label: clipName(c) })}
                    onclick={() => playSpan(c.clip.startMs, c.clip.endMs, clipName(c))}
                  >
                    <span dir="ltr">{clockMs(c.clip.startMs)}–{clockMs(c.clip.endMs)}</span>
                  </button>
                  <div class="what">
                    <span class="label"><bdi dir="auto">{clipName(c)}</bdi></span>
                    <button type="button" class="link where" onclick={() => openTarget(c)} disabled={c.target.kind !== 'family' && c.target.kind !== 'person' && !c.person}>
                      <bdi dir="auto">{whereText(c)}</bdi>
                    </button>
                  </div>
                  <Button variant="quiet" aria-label={t('rec.editNamed', { label: clipName(c) })} onclick={() => (editor = { editing: c })}>{t('rec.edit')}</Button>
                  <Button variant="quiet" aria-label={t('rec.deleteNamed', { label: clipName(c) })} onclick={() => remove(c)}>{t('rec.delete')}</Button>
                </li>
              {/each}
            </ol>
          {/if}
        </section>
        <TranscriptPanel media={rec.media} {now} onSeek={seek} onSaved={() => void load(media, startClip, false)} />
      </div>
    {/if}
  </div>
</Dialog>

<style>
  .place {
    display: grid;
    gap: var(--s-2);
    justify-items: start;
  }
  .player {
    display: grid;
    gap: var(--s-4);
    min-width: 0;
  }
  .limit {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-3);
    color: var(--ink-0);
    font-size: var(--t-sm);
    border-inline-start: 3px solid var(--verdigris);
    padding-inline-start: var(--s-3);
  }
  .muted,
  .hint {
    margin: 0;
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  .sources {
    margin: 0;
    color: var(--ink-1);
  }
  .link {
    font: inherit;
    color: var(--ink-0);
    background: none;
    border: 0;
    padding: 0;
    text-decoration: underline;
    cursor: pointer;
    text-align: start;
  }
  .link:disabled {
    text-decoration: none;
    cursor: default;
  }
  .link:focus-visible,
  .at:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .seek {
    position: relative;
    padding-block-start: 10px;
  }
  .marks {
    position: absolute;
    inset: 0 0 auto 0;
    block-size: 8px;
  }
  .mark {
    position: absolute;
    top: 0;
    block-size: 8px;
    min-width: 3px;
    border-radius: 2px;
    background: var(--verdigris);
    opacity: 0.55;
  }
  .mark.on {
    opacity: 1;
  }
  input[type='range'] {
    inline-size: 100%;
    margin: 0;
    accent-color: var(--ribbon);
  }
  input[type='range']:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .controls {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-4);
  }
  .btns {
    display: flex;
    align-items: center;
    gap: var(--s-2);
  }
  .time {
    font-variant-numeric: tabular-nums;
    color: var(--ink-0);
  }
  .speed {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    margin-inline-start: auto;
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  .speed select {
    inline-size: auto;
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr);
    gap: var(--s-6);
    align-items: start;
  }
  @media (max-width: 900px) {
    .cols {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  .clips {
    display: grid;
    gap: var(--s-3);
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-3);
  }
  h3 {
    margin: 0;
    font-size: var(--t-md);
  }
  .list {
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--s-1);
  }
  .list li {
    list-style: none;
    display: flex;
    align-items: center;
    gap: var(--s-3);
    padding: var(--s-2) var(--s-3);
    border-radius: var(--r-1);
    border-inline-start: 3px solid transparent;
  }
  .list li.on {
    background: var(--ribbon-wash);
    border-inline-start-color: var(--verdigris);
  }
  .at {
    font: inherit;
    font-size: var(--t-sm);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    color: var(--ink-0);
    background: var(--paper-2);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    padding: var(--s-1) var(--s-2);
    cursor: pointer;
  }
  .what {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .label {
    color: var(--ink-0);
  }
  .where {
    font-size: var(--t-sm);
    color: var(--ink-1);
  }
</style>
