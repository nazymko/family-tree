<script lang="ts">
  import { tick } from 'svelte';
  import Button from '../ui/Button.svelte';
  import '../ui/controls.css';
  import type { Id, TextRegionDto } from '../api/api';
  import { t } from '../i18n';
  import { moved, newPieceId, nextRotation } from './transcription';

  let {
    regions,
    editing = false,
    selected = null,
    onselect,
    onchange,
    onleave,
  }: {
    regions: TextRegionDto[];
    editing?: boolean;
    selected?: Id | null;
    onselect: (id: Id | null) => void;
    onchange: (regions: TextRegionDto[]) => void;
    /** A focused control went away and nothing near it is left: focus goes back to the picture. */
    onleave?: () => void;
  } = $props();

  let panel = $state<HTMLElement>();
  const shown = $derived(editing ? regions : regions.filter((r) => r.text.trim()));

  // A piece picked on the picture comes into view here.
  $effect(() => {
    if (!selected || !panel) return;
    panel.querySelector<HTMLElement>(`[data-para="${selected}"]`)?.scrollIntoView?.({ block: 'nearest' });
  });

  /** Puts the cursor in a piece's text (after drawing it on the picture, or adding it here). */
  export async function focusText(id: Id) {
    await tick();
    panel?.querySelector<HTMLTextAreaElement>(`[data-para="${id}"] textarea`)?.focus();
  }

  /** Text typed and not yet saved (a field saves when it loses focus), by piece. Nothing draws from it, so it is
   *  deliberately not reactive. */
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  const drafts = new Map<Id, string>();

  /** Saves what is typed but not yet saved: the viewer calls it before it closes or changes picture. */
  export function flush() {
    if (drafts.size === 0) return;
    const next = regions.map((x) => (drafts.has(x.id) ? { ...x, text: drafts.get(x.id) ?? x.text } : x));
    drafts.clear();
    if (next.some((x, i) => x.text !== regions[i].text)) onchange(next);
  }

  function setText(id: Id, text: string) {
    drafts.delete(id);
    const r = regions.find((x) => x.id === id);
    if (!r || r.text === text) return;
    onchange(regions.map((x) => (x.id === id ? { ...x, text } : x)));
  }

  /** Moves a piece in reading order; the button keeps focus, or its partner takes it at either end. */
  async function shift(id: Id, by: -1 | 1, e: MouseEvent) {
    const at = regions.findIndex((r) => r.id === id);
    onchange(moved(regions, at, at + by));
    const own = e.currentTarget as HTMLElement;
    const cls = own.dataset.act;
    await tick();
    const row = panel?.querySelector<HTMLElement>(`[data-para="${id}"]`);
    const same = row?.querySelector<HTMLButtonElement>(`[data-act="${cls}"]:not(:disabled)`);
    const other = row?.querySelector<HTMLButtonElement>(`[data-act="${cls === 'up' ? 'down' : 'up'}"]:not(:disabled)`);
    (same ?? other)?.focus();
  }

  async function remove(id: Id) {
    const at = regions.findIndex((r) => r.id === id);
    const next = regions[at + 1]?.id ?? regions[at - 1]?.id ?? null;
    onchange(regions.filter((r) => r.id !== id));
    onselect(next);
    await tick();
    const el = next ? panel?.querySelector<HTMLElement>(`[data-para="${next}"] textarea`) : null;
    if (el) el.focus();
    else onleave?.();
  }

  async function addTextOnly() {
    const id = newPieceId();
    onchange([...regions, { id, area: null, rotation: 0, text: '' }]);
    onselect(id);
    await focusText(id);
  }
</script>

<section bind:this={panel} class="full" aria-labelledby="full-text-title">
  <h3 id="full-text-title">{t('media.fullText')}</h3>
  {#if shown.length === 0}
    <p class="empty">{t('media.noText')}</p>
  {/if}
  <ol>
    {#each shown as r (r.id)}
      {@const i = regions.indexOf(r)}
      <li data-para={r.id} class:selected={selected === r.id}>
        {#if editing}
          <label class="lbl" for="para-{r.id}">
            {t('media.pieceText', { i: i + 1 })}
            {#if !r.area}<span class="hint">— {t('media.noPlace')}</span>{/if}
            {#if r.rotation}<span class="hint">— {t('media.turned', { deg: r.rotation })}</span>{/if}
          </label>
          <textarea
            id="para-{r.id}"
            class="control"
            dir="auto"
            rows="2"
            value={r.text}
            onfocus={() => onselect(r.id)}
            oninput={(e) => drafts.set(r.id, e.currentTarget.value)}
            onchange={(e) => setText(r.id, e.currentTarget.value)}
          ></textarea>
          {#if selected === r.id}
            <div class="acts">
              <Button data-act="up" disabled={i === 0} onclick={(e: MouseEvent) => shift(r.id, -1, e)}>{t('media.moveUp')}</Button>
              <Button data-act="down" disabled={i === regions.length - 1} onclick={(e: MouseEvent) => shift(r.id, 1, e)}>{t('media.moveDown')}</Button>
              {#if r.area}
                <Button onclick={() => onchange(regions.map((x) => (x.id === r.id ? { ...x, rotation: nextRotation(x.rotation) } : x)))}>{t('media.turn')}</Button>
              {/if}
              <Button variant="danger" onclick={() => remove(r.id)}>{t('media.deletePiece')}</Button>
            </div>
          {/if}
        {:else}
          <button type="button" class="para" dir="auto" aria-pressed={selected === r.id} onclick={() => onselect(selected === r.id ? null : r.id)}>{r.text.trim()}</button>
        {/if}
      </li>
    {/each}
  </ol>
  {#if editing}
    <div><Button onclick={addTextOnly}>{t('media.addTextOnly')}</Button></div>
  {/if}
</section>

<style>
  .full {
    display: grid;
    gap: var(--s-3);
  }
  h3 {
    margin: 0;
    font-size: var(--t-md);
  }
  .empty {
    margin: 0;
    color: var(--ink-1);
  }
  ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--s-3);
    max-height: 18rem;
    overflow: auto;
  }
  li {
    display: grid;
    gap: var(--s-2);
    padding-inline-start: var(--s-3);
    border-inline-start: 3px solid transparent;
  }
  li.selected {
    border-inline-start-color: var(--danger);
  }
  .lbl {
    font-size: var(--t-sm);
    color: var(--ink-1);
  }
  .hint {
    font-style: italic;
  }
  textarea {
    resize: vertical;
    min-height: 3rem;
  }
  .para {
    all: unset;
    display: block;
    white-space: pre-wrap;
    cursor: pointer;
    border-radius: var(--r-1);
    padding: var(--s-1) var(--s-2);
  }
  .para:focus-visible {
    outline: 2px solid var(--focus);
  }
  .para[aria-pressed='true'] {
    background: var(--paper-2);
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
</style>
