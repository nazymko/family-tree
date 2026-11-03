<script lang="ts">
  import type { Snippet } from 'svelte';
  let { children }: { children: Snippet } = $props();
</script>

<kbd>{@render children()}</kbd>

<style>
  kbd {
    display: inline-block;
    min-width: 1.5rem;
    padding: 0 var(--s-3);
    border: 1px solid var(--rule);
    border-bottom-width: 2px;
    border-radius: var(--r-1);
    background: var(--paper-1);
    color: var(--ink-1);
    font-family: var(--font-sans);
    font-size: var(--t-xs);
    line-height: 1.25rem;
    text-align: center;
  }
</style>
