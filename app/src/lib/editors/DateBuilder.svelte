<script lang="ts">
  import type { DateKind, DateChoice } from './dateParts';
  import { composeChoice, emptyChoice, parseChoice } from './dateParts';
  import DatePick from './DatePick.svelte';
  import Select from '../ui/Select.svelte';
  import { t } from '../i18n';

  let { value, onChange }: { value: string; onChange: (raw: string) => void } = $props();

  // svelte-ignore state_referenced_locally
  const start = parseChoice(value);
  // svelte-ignore state_referenced_locally
  let choice = $state<DateChoice>(start ?? emptyChoice());
  // svelte-ignore state_referenced_locally
  let sent = value;
  // svelte-ignore state_referenced_locally
  let kind = $state<DateKind>(choice.kind);
  // svelte-ignore state_referenced_locally
  let foreign = $state(start === null);

  const kinds = $derived([
    { value: 'exact', label: t('fields.kindExact') },
    { value: 'about', label: t('fields.kindAbout') },
    { value: 'before', label: t('fields.kindBefore') },
    { value: 'after', label: t('fields.kindAfter') },
    { value: 'between', label: t('fields.kindBetween') },
  ]);
  const incomplete = $derived(composeChoice(choice) === null);

  /** Nothing is written until the person changes something; a half-filled range writes nothing. */
  function push() {
    foreign = false;
    const raw = composeChoice(choice);
    if (raw === null) return;
    sent = raw;
    onChange(raw);
  }

  // The text was typed into while the picker is open: show it, when the picker can.
  $effect(() => {
    const v = value;
    if (v === sent) return;
    sent = v;
    const c = parseChoice(v);
    foreign = c === null;
    if (c) {
      choice = c;
      kind = c.kind;
    }
  });
</script>

<div class="builder">
  <Select
    label={t('fields.kindLabel')}
    options={kinds}
    bind:value={kind}
    onchange={(k) => {
      choice.kind = k as DateKind;
      push();
    }}
  />
  <DatePick
    legend={kind === 'between' ? t('fields.dateFirst') : t('fields.dateOne')}
    parts={choice.a}
    onChange={(p) => {
      choice.a = p;
      push();
    }}
  />
  {#if kind === 'between'}
    <DatePick
      legend={t('fields.dateSecond')}
      parts={choice.b}
      onChange={(p) => {
        choice.b = p;
        push();
      }}
    />
  {/if}
  {#if foreign}
    <p class="note">{t('fields.dateForeign')}</p>
  {:else if incomplete}
    <p class="note">{t('fields.dateIncomplete')}</p>
  {/if}
</div>

<style>
  .builder {
    display: grid;
    gap: var(--s-4);
    padding: var(--s-4);
    border: 1px solid var(--ink-2);
    border-radius: var(--r-1);
  }
  .note {
    margin: 0;
    font-size: var(--t-sm);
    line-height: var(--t-sm-lh);
    color: var(--ink-1);
  }
</style>
