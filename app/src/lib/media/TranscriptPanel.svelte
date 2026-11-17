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
  import { tick } from 'svelte';
  import Button from '../ui/Button.svelte';
  import IconButton from '../ui/IconButton.svelte';
  import TextField from '../ui/TextField.svelte';
  import TextArea from '../ui/TextArea.svelte';
  import Select from '../ui/Select.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import Banner from '../ui/Banner.svelte';
  import { getApi } from '../api/context';
  import type { MediaDto, TextRegionInput } from '../api/api';
  import { errMessage } from '../editors/errors';
  import { t, plural } from '../i18n';
  import { clockMs, parseClock } from './clips';
  import { NO_SPEAKER, foldQuery, highlightParts, pieceShown, speakerCounts, withSpeakers } from './transcriptTools';

  /** A recording's time-coded transcript: follows playback, seeks on a click, and is the owner's to edit. */
  let { media, now, onSeek, onSaved }: { media: MediaDto; now: number; onSeek: (ms: number) => void; onSaved: () => void } = $props();

  const api = getApi();
  const uid = $props.id();
  const pieces = $derived([...media.transcription].sort((a, b) => (a.startMs ?? Infinity) - (b.startMs ?? Infinity)));
  const current = $derived(pieceAt(pieces, now));
  let list = $state<HTMLElement>();

  // Search and speaker filter.
  let query = $state('');
  let picked = $state<string[]>([]);
  const folded = $derived(foldQuery(query));
  const pickedSet = $derived(new Set(picked));
  const speakers = $derived(speakerCounts(pieces));
  const shown = $derived(folded || picked.length ? pieces.filter((p) => pieceShown(p, pickedSet, folded)) : pieces);
  const filtering = $derived(Boolean(folded) || picked.length > 0);
  // A chosen speaker that is gone (renamed away) is no longer a choice.
  $effect(() => {
    const have = new Set(speakers.map((s) => s.speaker));
    if (picked.some((k) => !have.has(k))) picked = picked.filter((k) => have.has(k));
  });

  const speakerName = (s: string) => (s === NO_SPEAKER ? t('rec.noSpeaker') : s);

  function toggle(key: string) {
    picked = picked.includes(key) ? picked.filter((k) => k !== key) : [...picked, key];
  }

  let searchBox = $state<HTMLElement>();
  function clearSearch() {
    query = '';
    searchBox?.querySelector<HTMLElement>('input')?.focus();
  }

  function searchKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && query) {
      e.preventDefault();
      e.stopPropagation();
      query = '';
    }
  }

  // The piece being said stays in view while the recording plays. A piece the search or the speaker choice
  // hides has no row: then the list stays where the reader put it.
  $effect(() => {
    const id = current;
    if (!id || !list) return;
    list.querySelector<HTMLElement>(`[data-piece="${CSS.escape(id)}"]`)?.scrollIntoView?.({ block: 'nearest' });
  });

  let error = $state('');
  let saving = $state(false);

  /** Saves the whole transcript with `speaker(piece)` as each piece's speaker: ONE command, so one history entry. */
  async function saveSpeakers(speaker: (p: TextRegionDto) => string | undefined): Promise<boolean> {
    error = '';
    saving = true;
    try {
      const { regions, known } = withSpeakers(media.transcription, speaker);
      await api.apply({ type: 'setMediaTranscription', id: media.id, regions, known });
      onSaved();
      return true;
    } catch (e) {
      error = errMessage(e);
      return false;
    } finally {
      saving = false;
    }
  }

  // The speaker of one piece.
  let editing = $state<{ id: string; value: string } | null>(null);
  let refocus = $state<string | null>(null);
  const labelsId = `${uid}-labels`;

  function focusMe(node: HTMLElement) {
    node.focus();
    if (node instanceof HTMLInputElement) node.select();
  }

  // Back to the speaker button of the piece when its editor closes (the button was replaced by the editor).
  $effect(() => {
    const id = refocus;
    if (!id) return;
    void tick().then(() => {
      list?.querySelector<HTMLElement>(`[data-piece="${CSS.escape(id)}"] .who`)?.focus();
      refocus = null;
    });
  });

  function startSpeaker(p: TextRegionDto) {
    error = '';
    editing = { id: p.id, value: p.speaker ?? '' };
  }

  function cancelSpeaker() {
    if (!editing) return;
    refocus = editing.id;
    editing = null;
  }

  async function saveSpeaker() {
    if (!editing || saving) return;
    const { id, value } = editing;
    const next = value.trim();
    const was = media.transcription.find((p) => p.id === id)?.speaker?.trim() ?? '';
    if (next === was) return cancelSpeaker();
    if (await saveSpeakers((p) => (p.id === id ? next : p.speaker ?? undefined))) {
      refocus = id;
      editing = null;
    }
  }

  function speakerKey(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      void saveSpeaker();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      cancelSpeaker();
    }
  }

  // Rename a speaker everywhere.
  let renaming = $state(false);
  let from = $state(NO_SPEAKER);
  let to = $state('');
  let onlyShown = $state(false);
  const fromOptions = $derived(speakers.map((s) => ({ value: s.speaker, label: `${speakerName(s.speaker)} (${s.count})` })));
  const targets = $derived((onlyShown && filtering ? shown : pieces).filter((p) => (p.speaker?.trim() ?? '') === from));
  const toTrim = $derived(to.trim());
  const willChange = $derived(targets.filter((p) => (p.speaker?.trim() ?? '') !== toTrim).length);
  const removing = $derived(toTrim === '' && from !== NO_SPEAKER);
  const otherNames = $derived(speakers.filter((s) => s.speaker !== NO_SPEAKER && s.speaker !== from).map((s) => s.speaker));

  function openRename() {
    error = '';
    from = picked.length === 1 ? picked[0] : (speakers.find((s) => s.speaker !== NO_SPEAKER) ?? speakers[0])?.speaker ?? NO_SPEAKER;
    to = '';
    onlyShown = false;
    renaming = true;
  }

  async function applyRename() {
    if (saving || willChange === 0) return;
    const ids = new Set(targets.map((p) => p.id));
    const name = toTrim;
    if (await saveSpeakers((p) => (ids.has(p.id) ? name : p.speaker ?? undefined))) {
      if (picked.includes(from) && name) picked = [...new Set(picked.map((k) => (k === from ? name : k)))];
      renaming = false;
    }
  }

  function renameKey(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      void applyRename();
    }
  }

  // Edit mode.
  interface Row {
    id?: string;
    start: string;
    end: string;
    speaker: string;
    text: string;
  }
  /** Edit rows drawn at a time: a transcript of a thousand pieces would otherwise draw four thousand fields. */
  const ROWS_PAGE = 50;
  let rows = $state<Row[] | null>(null);
  let rowLimit = $state(ROWS_PAGE);

  function edit() {
    error = '';
    rowLimit = ROWS_PAGE;
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
    rowLimit = rows.length;
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

{#snippet marked(s: string)}
  {#each highlightParts(s, folded) as part, i (i)}{#if part.hit}<mark>{part.text}</mark>{:else}{part.text}{/if}{/each}
{/snippet}

<section class="transcript" aria-labelledby="{uid}-h">
  <div class="head">
    <h3 id="{uid}-h">{t('rec.transcript')}</h3>
    {#if !rows}<Button onclick={edit}>{t('rec.editTranscript')}</Button>{/if}
  </div>
  {#if rows}
    <div class="rows">
      {#each rows.slice(0, rowLimit) as r, i (i)}
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
      {#if rows.length > rowLimit}
        <div><Button onclick={() => (rowLimit += ROWS_PAGE)}>{plural('rec.showMore', rows.length - rowLimit)}</Button></div>
      {/if}
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
    <div class="tools">
      <div class="search" bind:this={searchBox}>
        <TextField label={t('rec.searchTranscript')} bind:value={query} autocomplete="off" dir="auto" onkeydown={searchKey} />
        {#if query}<IconButton icon="x" label={t('rec.searchClear')} onclick={clearSearch} />{/if}
        <span class="count" role="status" aria-live="polite">{t('rec.foundOf', { shown: shown.length, total: pieces.length })}</span>
      </div>
      <div class="chips" role="group" aria-label={t('rec.onlySpeakers')}>
        {#each speakers as s (s.speaker)}
          <Button variant="secondary" aria-pressed={picked.includes(s.speaker)} onclick={() => toggle(s.speaker)}>
            <bdi dir="auto">{speakerName(s.speaker)}</bdi> ({s.count})
          </Button>
        {/each}
        <Button onclick={openRename}>{t('rec.renameSpeaker')}</Button>
      </div>
    </div>
    {#if error && !renaming && !editing}<Banner kind="error">{error}</Banner>{/if}
    <datalist id={labelsId}>
      {#each speakers.filter((s) => s.speaker !== NO_SPEAKER) as s (s.speaker)}<option value={s.speaker}></option>{/each}
    </datalist>
    {#if shown.length === 0}<p class="empty">{t('rec.noneFound')}</p>{/if}
    <ol class="pieces" bind:this={list}>
      {#each shown as p (p.id)}
        <li data-piece={p.id} class:now={p.id === current} aria-current={p.id === current ? 'true' : undefined}>
          {#if p.startMs != null}
            <button type="button" class="at" aria-label={t('rec.playFrom', { time: clockMs(p.startMs) })} onclick={() => onSeek(p.startMs!)}>
              <span dir="ltr">{clockMs(p.startMs)}</span>
            </button>
          {/if}
          <p>
            {#if editing?.id === p.id}
              <span class="inline">
                <input
                  class="control who-input"
                  aria-label={t('rec.speaker')}
                  list={labelsId}
                  dir="auto"
                  autocomplete="off"
                  bind:value={editing.value}
                  onkeydown={speakerKey}
                  use:focusMe
                />
                <IconButton icon="check" label={t('common.save')} disabled={saving} onclick={saveSpeaker} />
                <IconButton icon="x" label={t('common.cancel')} onclick={cancelSpeaker} />
              </span>
              {#if error}<span class="err" role="alert">{error}</span>{/if}
            {:else if p.speaker}
              <button type="button" class="who" aria-label={t('rec.changeSpeaker', { name: p.speaker })} onclick={() => startSpeaker(p)}>
                <b><bdi dir="auto">{@render marked(p.speaker)}</bdi>:</b>
              </button>
            {:else}
              <button type="button" class="who add" onclick={() => startSpeaker(p)}>{t('rec.addSpeaker')}</button>
            {/if}
            <bdi dir="auto">{@render marked(p.text)}</bdi>
          </p>
        </li>
      {/each}
    </ol>
  {/if}
</section>

<Dialog open={renaming} title={t('rec.renameTitle')} onclose={() => (renaming = false)} width="30rem">
  <div class="rename">
    <Select label={t('rec.renameFrom')} options={fromOptions} bind:value={from} />
    <TextField label={t('rec.renameTo')} hint={t('rec.renameToHint')} bind:value={to} list={`${uid}-names`} dir="auto" autocomplete="off" onkeydown={renameKey} />
    <datalist id="{uid}-names">
      {#each otherNames as n (n)}<option value={n}></option>{/each}
    </datalist>
    {#if filtering}<Checkbox label={t('rec.renameOnlyShown')} bind:checked={onlyShown} />{/if}
    <p class="will" role="status">{plural('rec.renameWillChange', willChange)}</p>
    {#if removing && willChange > 0}<Banner kind="warning">{plural('rec.renameRemoves', willChange)}</Banner>{/if}
    {#if error}<Banner kind="error">{error}</Banner>{/if}
  </div>
  {#snippet footer()}
    <Button onclick={() => (renaming = false)}>{t('common.cancel')}</Button>
    <Button variant={removing ? 'danger' : 'primary'} disabled={saving || willChange === 0} onclick={applyRename}>
      {removing ? t('rec.renameRemove') : t('rec.renameApply')}
    </Button>
  {/snippet}
</Dialog>

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
  .tools {
    display: grid;
    gap: var(--s-2);
  }
  .search {
    display: flex;
    align-items: end;
    gap: var(--s-2);
  }
  .search :global(.field) {
    flex: 1;
  }
  .count {
    color: var(--ink-1);
    font-size: var(--t-sm);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    padding-block-end: var(--s-3);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
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
  mark {
    background: var(--foxing-wash);
    color: inherit;
    border-radius: 2px;
  }
  .at,
  .who {
    font: inherit;
    background: none;
    border: 0;
    padding: 0;
    cursor: pointer;
  }
  .at {
    font-size: var(--t-sm);
    font-variant-numeric: tabular-nums;
    color: var(--ink-1);
    text-decoration: underline;
  }
  .who {
    color: inherit;
    margin-inline-end: var(--s-1);
  }
  .who:hover {
    text-decoration: underline dotted;
  }
  .who.add {
    font-size: var(--t-sm);
    color: var(--ink-1);
    text-decoration: underline dotted;
  }
  .at:focus-visible,
  .who:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .inline {
    display: inline-flex;
    align-items: center;
    gap: var(--s-1);
  }
  .who-input {
    inline-size: 14rem;
    max-inline-size: 100%;
  }
  .err {
    color: var(--danger);
    font-size: var(--t-sm);
    margin-inline-end: var(--s-2);
  }
  .rename {
    display: grid;
    gap: var(--s-3);
  }
  .will {
    margin: 0;
    font-weight: 500;
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
  @media (max-width: 400px) {
    .when {
      grid-template-columns: minmax(0, 1fr);
    }
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
