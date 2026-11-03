<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    tone = 'neutral',
    dot = false,
    children,
  }: { tone?: 'neutral' | 'living' | 'warning' | 'ribbon'; dot?: boolean; children: Snippet } = $props();
</script>

<span class="badge {tone}">
  {#if dot}<span class="dot" aria-hidden="true"></span>{/if}
  {@render children()}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    padding: 0 var(--s-3);
    border: 1px solid var(--rule);
    border-radius: 999px;
    font-size: var(--t-xs);
    line-height: var(--t-xs-lh);
    font-weight: 500;
    color: var(--ink-1);
    background: var(--paper-1);
    white-space: nowrap;
  }
  .dot {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
    background: currentColor;
  }
  .living {
    color: var(--verdigris);
    border-color: var(--verdigris);
  }
  .warning {
    color: var(--foxing);
    background: var(--foxing-wash);
    border-color: var(--foxing);
  }
  .ribbon {
    color: var(--ribbon);
    border-color: var(--ribbon);
  }
</style>
