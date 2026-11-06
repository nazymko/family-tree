<script lang="ts">
  import type { DateParts } from './dateParts';
  import { daysInMonth, isoFromParts, partsFromIso } from './dateParts';
  import Select from '../ui/Select.svelte';
  import TextField from '../ui/TextField.svelte';
  import { currentLocale, t } from '../i18n';

  let { legend, parts, onChange }: { legend: string; parts: DateParts; onChange: (p: DateParts) => void } = $props();

  // svelte-ignore state_referenced_locally
  let year = $state(parts.year);
  // svelte-ignore state_referenced_locally
  let month = $state(parts.month);
  // svelte-ignore state_referenced_locally
  let day = $state(parts.day);

  // A change from outside (the text was edited while the picker is open).
  $effect(() => {
    year = parts.year;
    month = parts.month;
    day = parts.day;
  });

  const NOT_SET = { value: '', label: '—' };
  const months = $derived.by(() => {
    const names = new Intl.DateTimeFormat(currentLocale(), { month: 'long', timeZone: 'UTC' });
    return [NOT_SET, ...Array.from({ length: 12 }, (_, i) => ({ value: String(i + 1), label: names.format(Date.UTC(2001, i, 1)) }))];
  });
  const lastDay = $derived(month ? daysInMonth(Number(year), Number(month)) : 31);
  const days = $derived([NOT_SET, ...Array.from({ length: lastDay }, (_, i) => ({ value: String(i + 1), label: String(i + 1) }))]);
  const hasYear = $derived(Number(year) >= 1);
  const iso = $derived(isoFromParts({ year, month, day }));

  function changed() {
    year = year.replace(/\D/g, '');
    if (Number(year) < 1) month = '';
    if (!month) day = '';
    else if (Number(day) > daysInMonth(Number(year), Number(month))) day = '';
    onChange({ year, month, day });
  }

  function calendar(e: Event & { currentTarget: HTMLInputElement }) {
    const p = partsFromIso(e.currentTarget.value);
    if (!p) return;
    year = p.year;
    month = p.month;
    day = p.day;
    changed();
  }
</script>

<fieldset>
  <legend>{legend}</legend>
  <div class="parts">
    <TextField label={t('fields.dateYear')} bind:value={year} oninput={changed} inputmode="numeric" maxlength={4} autocomplete="off" />
    <Select label={t('fields.dateMonth')} options={months} bind:value={month} onchange={changed} disabled={!hasYear} />
    <Select label={t('fields.dateDay')} options={days} bind:value={day} onchange={changed} disabled={!month} />
  </div>
  <label class="cal">
    <span>{t('fields.dateCalendar')}</span>
    <input class="control" type="date" min="0001-01-01" max="9999-12-31" value={iso} onchange={calendar} />
  </label>
</fieldset>

<style>
  fieldset {
    border: 0;
    margin: 0;
    padding: 0;
    min-width: 0;
    display: grid;
    gap: var(--s-3);
  }
  legend {
    padding: 0;
    margin-bottom: var(--s-2);
    font-weight: 500;
  }
  .parts {
    display: grid;
    grid-template-columns: minmax(4.5rem, 1fr) minmax(7rem, 2fr) minmax(4.5rem, 1fr);
    gap: var(--s-3);
    align-items: start;
  }
  .cal {
    display: grid;
    gap: var(--s-2);
    font-size: var(--t-sm);
    line-height: var(--t-sm-lh);
    color: var(--ink-1);
    max-width: 14rem;
  }
  .control {
    width: 100%;
    min-height: 2.25rem;
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--ink-2);
    border-radius: var(--r-1);
    background: var(--paper-1);
    color: var(--ink-0);
    color-scheme: light dark;
  }
</style>
