<script lang="ts">
  import TextField from '../ui/TextField.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Select from '../ui/Select.svelte';
  import Button from '../ui/Button.svelte';
  import type { PersonQuery } from '../api/types';
  import { t, type MessageKey } from '../i18n';

  let { query = $bindable() }: { query: PersonQuery } = $props();

  const LIVING: { value: PersonQuery['living']; label: MessageKey }[] = [
    { value: 'any', label: 'people.any' },
    { value: 'living', label: 'people.living' },
    { value: 'deceased', label: 'people.deceased' },
  ];
</script>

<div class="bar">
  <div class="text">
    <TextField
      label={t('people.filterByName')}
      placeholder={t('people.typeToFilter')}
      autocomplete="off"
      data-filter-input
      bind:value={query.text}
    />
  </div>

  <div class="grp" role="group" aria-label={t('people.livingStatus')}>
    <span class="gl" aria-hidden="true">{t('people.status')}</span>
    <div class="seg">
      {#each LIVING as o (o.value)}
        <button type="button" aria-pressed={query.living === o.value} onclick={() => (query.living = o.value)}>
          {t(o.label)}
        </button>
      {/each}
    </div>
  </div>

  <div class="checks">
    <Checkbox label={t('people.noSource')} bind:checked={query.noSource} />
    <Checkbox label={t('people.noBirthDate')} bind:checked={query.noBirthDate} />
    <Checkbox label={t('people.privateOnly')} bind:checked={query.privateOnly} />
  </div>

  <div class="sort">
    <Select
      label={t('people.sortBy')}
      bind:value={query.sort}
      options={[
        { value: 'surname', label: t('people.surname') },
        { value: 'given', label: t('people.givenName') },
        { value: 'birth', label: t('people.birth') },
        { value: 'changed', label: t('people.recentlyChanged') },
      ]}
    />
    <Button
      aria-label={t('people.sortDirection', { direction: query.desc ? t('people.descending') : t('people.ascending') })}
      aria-pressed={query.desc}
      onclick={() => (query.desc = !query.desc)}
    >
      {query.desc ? t('people.zToA') : t('people.aToZ')}
    </Button>
  </div>
</div>

<style>
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--s-4) var(--s-5);
  }
  .text {
    flex: 1 1 11rem;
    max-width: 15rem;
  }
  .grp {
    display: grid;
    gap: var(--s-2);
  }
  .gl {
    font-size: var(--t-sm);
    font-weight: 500;
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--ink-2);
    border-radius: var(--r-1);
    overflow: hidden;
  }
  .seg button {
    min-height: 2.25rem;
    padding: 0 var(--s-4);
    border: 0;
    background: var(--paper-1);
    cursor: pointer;
  }
  .seg button + button {
    border-inline-start: 1px solid var(--ink-2);
  }
  .seg button[aria-pressed='true'] {
    background: var(--ink-0);
    color: var(--paper-1);
    font-weight: 700;
  }
  .checks {
    display: flex;
    gap: var(--s-5);
    flex-wrap: wrap;
    padding-bottom: var(--s-3);
  }
  .sort :global(.btn.quiet) {
    border-color: var(--ink-2);
  }
  .sort :global(select) {
    min-width: 10rem;
  }
  .sort {
    display: flex;
    align-items: flex-end;
    gap: var(--s-3);
  }
</style>
