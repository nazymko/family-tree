<script lang="ts">
  import { tick } from 'svelte';
  import type { DayOrder } from '../api/types';
  import Select from '../ui/Select.svelte';
  import TextField from '../ui/TextField.svelte';
  import DateField from './DateField.svelte';
  import { composeDate, daysInMonth, parseDateParts } from './dateParts';
  import { currentLocale, t } from '../i18n';

  let {
    label,
    value,
    onChange,
    order,
  }: {
    label: string;
    value: string;
    onChange: (raw: string) => void;
    order: DayOrder;
  } = $props();

  // svelte-ignore state_referenced_locally
  const start = parseDateParts(value);
  // A date the parts cannot show exactly opens as text, so that nothing of it is lost.
  let typing = $state(start === null);
  let year = $state(start?.year ?? '');
  let month = $state(start?.month ?? '');
  let day = $state(start?.day ?? '');
  // svelte-ignore state_referenced_locally
  let sent = value;
  let root = $state<HTMLElement>();

  const NOT_SET = { value: '', label: '—' };
  const months = $derived.by(() => {
    const names = new Intl.DateTimeFormat(currentLocale(), { month: 'long', timeZone: 'UTC' });
    return [NOT_SET, ...Array.from({ length: 12 }, (_, i) => ({ value: String(i + 1), label: names.format(Date.UTC(2001, i, 1)) }))];
  });
  const lastDay = $derived(month ? daysInMonth(Number(year), Number(month)) : 31);
  const days = $derived([NOT_SET, ...Array.from({ length: lastDay }, (_, i) => ({ value: String(i + 1), label: String(i + 1) }))]);
  const hasYear = $derived(Number(year) >= 1);
  const canPick = $derived(parseDateParts(value) !== null);

  function send(raw: string) {
    sent = raw;
    onChange(raw);
  }

  /** What is on screen is what is saved: a part that lost what it depends on is cleared, not kept out of sight. */
  function changed() {
    year = year.replace(/\D/g, '');
    if (Number(year) < 1) month = '';
    if (!month) day = '';
    else if (Number(day) > daysInMonth(Number(year), Number(month))) day = '';
    send(composeDate({ year, month, day }));
  }

  function show(parts: { year: string; month: string; day: string }) {
    year = parts.year;
    month = parts.month;
    day = parts.day;
  }

  async function switchTo(toTyping: boolean) {
    if (!toTyping) show(parseDateParts(value) ?? { year: '', month: '', day: '' });
    typing = toTyping;
    // The button that had the focus has just gone.
    await tick();
    root?.querySelector<HTMLElement>('input')?.focus();
  }

  // A change from outside, such as a dialog starting over.
  $effect(() => {
    const v = value;
    if (v === sent) return;
    sent = v;
    const parts = parseDateParts(v);
    typing = parts === null;
    if (parts) show(parts);
  });
</script>

<div class="wrap" bind:this={root}>
  {#if typing}
    <DateField {label} {value} onChange={send} {order} />
    {#if canPick}
      <button type="button" class="switch" onclick={() => switchTo(false)}>{t('fields.dateChoose')}</button>
    {/if}
  {:else}
    <fieldset>
      <legend>{label}</legend>
      <p class="hint">{t('fields.dateOptional')}</p>
      <div class="parts">
        <TextField label={t('fields.dateYear')} bind:value={year} oninput={changed} inputmode="numeric" maxlength={4} autocomplete="off" />
        <Select label={t('fields.dateMonth')} options={months} bind:value={month} onchange={changed} disabled={!hasYear} />
        <Select label={t('fields.dateDay')} options={days} bind:value={day} onchange={changed} disabled={!month} />
      </div>
    </fieldset>
    <button type="button" class="switch" onclick={() => switchTo(true)}>{t('fields.dateType')}</button>
  {/if}
</div>

<style>
  .wrap {
    display: grid;
    gap: var(--s-2);
    justify-items: start;
    max-width: var(--measure);
  }
  .wrap > :global(*) {
    width: 100%;
  }
  fieldset {
    border: 0;
    margin: 0;
    padding: 0;
    min-width: 0;
  }
  legend {
    padding: 0;
    font-weight: 500;
  }
  .hint {
    margin: var(--s-1) 0 var(--s-2);
    color: var(--ink-1);
    font-size: var(--t-sm);
    line-height: var(--t-sm-lh);
  }
  .parts {
    display: grid;
    grid-template-columns: minmax(4.5rem, 1fr) minmax(7rem, 2fr) minmax(4.5rem, 1fr);
    gap: var(--s-3);
    align-items: start;
  }
  .switch {
    width: auto;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    font-size: var(--t-sm);
    line-height: var(--t-sm-lh);
    text-align: left;
    text-decoration: underline;
    cursor: pointer;
  }
</style>
