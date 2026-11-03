<script lang="ts">
  import type { Snippet } from 'svelte';
  import Ribbon from './Ribbon.svelte';

  let {
    children,
    ribbon = false,
    ribbonTitle = 'Home person',
    as = 'div',
    flipKey,
  }: { children: Snippet; ribbon?: boolean; ribbonTitle?: string; as?: 'div' | 'section' | 'article'; flipKey?: string } = $props();
</script>

<svelte:element this={as} class="index-card" data-flip-key={flipKey}>
  {#if ribbon}
    <span class="tab"><Ribbon title={ribbonTitle} size="1.75rem" /></span>
  {/if}
  {@render children()}
</svelte:element>

<style>
  .index-card {
    position: relative;
    padding: 0 var(--s-6) var(--s-2);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background-color: var(--paper-1);
    background-image:
      linear-gradient(to right, transparent 3rem, var(--ribbon-wash) 3rem 3.0625rem, transparent 3.0625rem),
      repeating-linear-gradient(to bottom, transparent 0 27px, var(--rule) 27px 28px);
    background-position: 0 0, 0 var(--s-3);
    line-height: var(--rule-step);
  }
  .tab {
    position: absolute;
    inset-block-start: -1px;
    inset-inline-end: var(--s-6);
    display: block;
    line-height: 0;
  }
</style>
