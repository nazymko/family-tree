<script lang="ts">
  import TextField from '../ui/TextField.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Select from '../ui/Select.svelte';
  import Button from '../ui/Button.svelte';
  import type { PersonQuery } from '../api/types';

  let { query = $bindable() }: { query: PersonQuery } = $props();

  const LIVING: { value: PersonQuery['living']; label: string }[] = [
    { value: 'any', label: 'Any' },
    { value: 'living', label: 'Living' },
    { value: 'deceased', label: 'Deceased' },
  ];
</script>

<div class="bar">
  <div class="text">
    <TextField
      label="Filter by name"
      placeholder="Type to filter"
      autocomplete="off"
      data-filter-input
      bind:value={query.text}
    />
  </div>

  <div class="grp" role="group" aria-label="Living status">
    <span class="gl" aria-hidden="true">Status</span>
    <div class="seg">
      {#each LIVING as o (o.value)}
        <button type="button" aria-pressed={query.living === o.value} onclick={() => (query.living = o.value)}>
          {o.label}
        </button>
      {/each}
    </div>
  </div>

  <div class="checks">
    <Checkbox label="No source" bind:checked={query.noSource} />
    <Checkbox label="No birth date" bind:checked={query.noBirthDate} />
    <Checkbox label="Private only" bind:checked={query.privateOnly} />
  </div>

  <div class="sort">
    <Select
      label="Sort by"
      bind:value={query.sort}
      options={[
        { value: 'surname', label: 'Surname' },
        { value: 'given', label: 'Given name' },
        { value: 'birth', label: 'Birth' },
        { value: 'changed', label: 'Recently changed' },
      ]}
    />
    <Button
      aria-label="Sort direction: {query.desc ? 'descending' : 'ascending'}"
      aria-pressed={query.desc}
      onclick={() => (query.desc = !query.desc)}
    >
      {query.desc ? 'Z to A' : 'A to Z'}
    </Button>
  </div>
</div>

<style>
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--s-4) var(--s-6);
  }
  .text {
    flex: 1 1 14rem;
    max-width: 22rem;
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
  .sort {
    display: flex;
    align-items: flex-end;
    gap: var(--s-3);
  }
</style>
