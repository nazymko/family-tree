<script lang="ts">
  import { untrack } from 'svelte';
  import type { DatePreview, DayOrder } from '../api/types';
  import { getApi } from '../api/context';
  import TextField from '../ui/TextField.svelte';
  import Icon from '../ui/Icon.svelte';
  import DateBuilder from './DateBuilder.svelte';
  import { t } from '../i18n';

  let {
    label,
    value,
    onChange,
    onPreview,
    order,
    picker = true,
  }: {
    label: string;
    value: string;
    onChange: (raw: string) => void;
    onPreview?: (p: DatePreview) => void;
    order: DayOrder;
    /** A date picker (one date, about, before, after, between) that fills the text; off where the parent has its own. */
    picker?: boolean;
  } = $props();
  let picking = $state(false);

  const api = getApi();
  // svelte-ignore state_referenced_locally
  let text = $state(value);
  let preview = $state<DatePreview | null>(null);
  let seq = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;

  function schedule(raw: string, delay: number) {
    clearTimeout(timer);
    const mine = ++seq;
    if (!raw.trim()) {
      preview = null;
      return;
    }
    timer = setTimeout(async () => {
      try {
        const p = await api.parseDate(raw, order);
        if (mine !== seq) return; // an older response arriving late
        preview = p;
        onPreview?.(p);
      } catch {
        if (mine === seq) preview = null;
      }
    }, delay);
  }

  function oninput(e: Event & { currentTarget: HTMLInputElement }) {
    const raw = e.currentTarget.value;
    onChange(raw);
    schedule(raw, 80);
  }

  // A different day order re-reads the same text; an outside change of `value` shows in the input.
  $effect(() => {
    void order;
    schedule(untrack(() => text), 0);
    return () => clearTimeout(timer);
  });
  $effect(() => {
    const v = value;
    if (v !== untrack(() => text)) {
      text = v;
      schedule(v, 0);
    }
  });
</script>

<div class="date">
  <TextField {label} bind:value={text} {oninput} autocomplete="off" />
  <div class="status" role="status" aria-live="polite">
    {#if preview?.status === 'ok'}
      <span class="ok">{t('fields.understoodAs', { text: preview.understoodAs })}{preview.range ? ` (${preview.range})` : ''}</span>
    {:else if preview?.status === 'phrase'}
      <span class="ok">Saved as a phrase: "{preview.raw}"</span>
    {:else if preview?.status === 'unparsed'}
      <span class="warn"><Icon name="warning" /> {t('fields.notUnderstood')}</span>
    {/if}
    {#if preview}
      {#each preview.warnings as w (w)}<span class="note">{w}</span>{/each}
    {/if}
  </div>
  {#if picker}
    <button type="button" class="switch" aria-expanded={picking} onclick={() => (picking = !picking)}>
      {picking ? t('fields.datePickerHide') : t('fields.datePickerShow')}
    </button>
    {#if picking}<DateBuilder {value} {onChange} />{/if}
  {/if}
</div>

<style>
  .switch {
    justify-self: start;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    font-size: var(--t-sm);
    line-height: var(--t-sm-lh);
    text-decoration: underline;
    cursor: pointer;
  }
  .date {
    display: grid;
    gap: var(--s-2);
    max-width: var(--measure);
  }
  .status {
    display: grid;
    gap: var(--s-1);
    min-height: 1.125rem;
    font-size: var(--t-sm);
    line-height: var(--t-sm-lh);
    font-variant-numeric: tabular-nums;
  }
  .ok {
    color: var(--ink-1);
  }
  .warn {
    display: inline-flex;
    align-items: flex-start;
    gap: var(--s-3);
    color: var(--foxing);
    font-weight: 500;
  }
  .note {
    color: var(--foxing);
  }
</style>
