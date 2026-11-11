<script lang="ts" module>
  import type { TextRegionDto } from '../api/api';
  /** The piece being said at `ms`: the last one that has started and not yet ended. */
  export function pieceAt(pieces: readonly TextRegionDto[], ms: number): string | null {
    let hit: string | null = null;
    for (const p of pieces) if (p.startMs != null && p.endMs != null && p.startMs <= ms && ms < p.endMs) hit = p.id;
    return hit;
  }
</script>

<script lang="ts">
  import Button from '../ui/Button.svelte';
  import IconButton from '../ui/IconButton.svelte';
  import TextField from '../ui/TextField.svelte';
  import TextArea from '../ui/TextArea.svelte';
  import Banner from '../ui/Banner.svelte';
  import { getApi } from '../api/context';
  import type { MediaDto, TextRegionInput } from '../api/api';
  import { errMessage } from '../editors/errors';
  import { t } from '../i18n';
  import { clockMs, parseClock } from './clips';

  /** A recording's time-coded transcript: follows playback, seeks on a click, and is the owner's to edit. */
  let { media, now, onSeek, onSaved }: { media: MediaDto; now: number; onSeek: (ms: number) => void; onSaved: () => void } = $props();

  const api = getApi();
  const uid = $props.id();
  const pieces = $derived([...media.transcription].sort((a, b) => (a.startMs ?? Infinity) - (b.startMs ?? Infinity)));
  const current = $derived(pieceAt(pieces, now));
  let list = $state<HTMLElement>();

  // The piece being said stays in view while the recording plays.
  $effect(() => {
    const id = current;
    if (!id || !list) return;
    list.querySelector<HTMLElement>(`[data-piece="${CSS.escape(id)}"]`)?.scrollIntoView?.({ block: 'nearest' });
  });

  interface Row {
    id?: string;
    start: string;
    end: string;
    speaker: string;
    text: string;
  }
  let rows = $state<Row[] | null>(null);
  let error = $state('');
  let saving = $state(false);

  function edit() {
    error = '';
    rows = pieces.map((p) => ({
      id: p.id,
      start: p.startMs != null ? clockMs(p.startMs, true) : '',
      end: p.endMs != null ? clockMs(p.endMs, true) : '',
      speaker: p.speaker ?? '',
      text: p.text,
    }));
  }

  function addHere() {
    if (!rows) return;
    rows.push({ start: clockMs(now, true), end: clockMs(now + 5000, true), speaker: rows[rows.length - 1]?.speaker ?? '', text: '' });
  }

  async function save() {
    if (!rows) return;
    error = '';
    const regions: TextRegionInput[] = [];
    for (const r of rows) {
      if (!r.text.trim()) continue;
      const start = r.start.trim() ? parseClock(r.start) : undefined;
      const end = r.end.trim() ? parseClock(r.end) : undefined;
      if (start === null || end === null) {
        error = t('rec.badTime');
        return;
      }
      regions.push({
        ...(r.id ? { id: r.id } : {}),
        text: r.text,
        ...(start !== undefined ? { startMs: start } : {}),
        ...(end !== undefined ? { endMs: end } : {}),
        ...(r.speaker.trim() ? { speaker: r.speaker.trim() } : {}),
      });
    }
    saving = true;
    try {
      await api.apply({ type: 'setMediaTranscription', id: media.id, regions, known: pieces.map((p) => p.id) });
      rows = null;
      onSaved();
    } catch (e) {
      error = errMessage(e);
    } finally {
      saving = false;
    }
  }
</script>

<section class="transcript" aria-labelledby="{uid}-h">
  <div class="head">
    <h3 id="{uid}-h">{t('rec.transcript')}</h3>
    {#if !rows}<Button onclick={edit}>{t('rec.editTranscript')}</Button>{/if}
  </div>
  {#if rows}
    <div class="rows">
      {#each rows as r, i (i)}
        <div class="row">
          <div class="when" dir="ltr">
            <TextField label={t('rec.start')} bind:value={r.start} autocomplete="off" />
            <TextField label={t('rec.end')} bind:value={r.end} autocomplete="off" />
          </div>
          <TextField label={t('rec.speaker')} bind:value={r.speaker} dir="auto" />
          <TextArea label={t('rec.said')} bind:value={r.text} rows={2} dir="auto" />
          <IconButton icon="trash" label={t('rec.removePiece')} onclick={() => rows?.splice(i, 1)} />
        </div>
      {/each}
    </div>
    {#if error}<Banner kind="error">{error}</Banner>{/if}
    <div class="btns">
      <Button onclick={addHere}>{t('rec.addPiece', { time: clockMs(now) })}</Button>
      <span class="grow"></span>
      <Button onclick={() => (rows = null)}>{t('common.cancel')}</Button>
      <Button variant="primary" disabled={saving} onclick={save}>{t('rec.saveTranscript')}</Button>
    </div>
  {:else if pieces.length === 0}
    <p class="empty">{t('rec.transcriptNone')}</p>
  {:else}
    <ol class="pieces" bind:this={list}>
      {#each pieces as p (p.id)}
        <li data-piece={p.id} class:now={p.id === current} aria-current={p.id === current ? 'true' : undefined}>
          {#if p.startMs != null}
            <button type="button" class="at" aria-label={t('rec.playFrom', { time: clockMs(p.startMs) })} onclick={() => onSeek(p.startMs!)}>
              <span dir="ltr">{clockMs(p.startMs)}</span>
            </button>
          {/if}
          <p>
            {#if p.speaker}<b><bdi dir="auto">{p.speaker}</bdi>:</b>{/if}
            <bdi dir="auto">{p.text}</bdi>
          </p>
        </li>
      {/each}
    </ol>
  {/if}
</section>

<style>
  .transcript {
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
  .pieces {
    margin: 0;
    padding: 0;
    max-block-size: 22rem;
    overflow: auto;
    display: grid;
    gap: var(--s-1);
  }
  .pieces li {
    list-style: none;
    display: flex;
    align-items: baseline;
    gap: var(--s-3);
    padding: var(--s-2) var(--s-3);
    border-radius: var(--r-1);
    border-inline-start: 3px solid transparent;
  }
  .pieces li.now {
    background: var(--ribbon-wash);
    border-inline-start-color: var(--ribbon);
  }
  .pieces p {
    margin: 0;
    color: var(--ink-0);
  }
  .at {
    font: inherit;
    font-size: var(--t-sm);
    font-variant-numeric: tabular-nums;
    color: var(--ink-1);
    background: none;
    border: 0;
    padding: 0;
    cursor: pointer;
    text-decoration: underline;
  }
  .at:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .rows {
    display: grid;
    gap: var(--s-3);
  }
  .row {
    display: grid;
    grid-template-columns: 12rem 9rem 1fr auto;
    align-items: end;
    gap: var(--s-2);
  }
  .when {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--s-2);
  }
  @media (max-width: 760px) {
    .row {
      grid-template-columns: 1fr;
    }
  }
  .empty {
    margin: 0;
    color: var(--ink-1);
  }
  .btns {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
  .grow {
    flex: 1;
  }
</style>
