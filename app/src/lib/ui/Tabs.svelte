<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    label,
    tabs,
    selected = $bindable(),
    panel,
  }: {
    label: string;
    tabs: { id: string; label: string }[];
    selected?: string;
    panel: Snippet<[string]>;
  } = $props();

  const uid = $props.id();
  const current = $derived(selected ?? tabs[0]?.id);

  function onkeydown(e: KeyboardEvent) {
    const i = tabs.findIndex((t) => t.id === current);
    let n: number;
    if (e.key === 'ArrowRight') n = (i + 1) % tabs.length;
    else if (e.key === 'ArrowLeft') n = (i - 1 + tabs.length) % tabs.length;
    else if (e.key === 'Home') n = 0;
    else if (e.key === 'End') n = tabs.length - 1;
    else return;
    e.preventDefault();
    selected = tabs[n].id;
    document.getElementById(`${uid}-tab-${tabs[n].id}`)?.focus();
  }
</script>

<div class="tabs">
  <div class="list" role="tablist" aria-label={label} tabindex="-1" {onkeydown}>
    {#each tabs as t (t.id)}
      <button
        type="button"
        role="tab"
        id="{uid}-tab-{t.id}"
        aria-selected={t.id === current}
        aria-controls="{uid}-panel-{t.id}"
        tabindex={t.id === current ? 0 : -1}
        onclick={() => (selected = t.id)}
      >
        {t.label}
      </button>
    {/each}
  </div>
  {#each tabs as t (t.id)}
    {#if t.id === current}
      <div role="tabpanel" id="{uid}-panel-{t.id}" aria-labelledby="{uid}-tab-{t.id}" tabindex="0">
        {@render panel(t.id)}
      </div>
    {/if}
  {/each}
</div>

<style>
  .list {
    display: flex;
    gap: var(--s-5);
    border-bottom: 1px solid var(--rule);
  }
  [role='tab'] {
    margin-bottom: -1px;
    padding: var(--s-3) var(--s-1);
    border: 0;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--ink-1);
    font-weight: 500;
    cursor: pointer;
    transition: color var(--dur-fast) var(--ease);
  }
  [role='tab']:hover {
    color: var(--ink-0);
  }
  [role='tab'][aria-selected='true'] {
    color: var(--ink-0);
    border-bottom-color: var(--ribbon);
  }
  [role='tabpanel'] {
    padding-top: var(--s-5);
  }
</style>
