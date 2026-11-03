<script lang="ts">
  import type { Snippet } from 'svelte';
  import Ribbon from './Ribbon.svelte';

  let {
    children,
    body,
    ribbon = false,
    ribbonTitle = 'Home person',
    as = 'div',
  }: {
    /** The title block: plain paper, no ruling. */
    children: Snippet;
    /** Optional ruled area below the ribbon-coloured header line; each text line sits on a rule. */
    body?: Snippet;
    ribbon?: boolean;
    ribbonTitle?: string;
    as?: 'div' | 'section' | 'article';
  } = $props();
</script>

<svelte:element this={as} class="index-card">
  {#if ribbon}
    <span class="tab"><Ribbon title={ribbonTitle} size="1.75rem" /></span>
  {/if}
  <div class="title" class:lined={!!body}>{@render children()}</div>
  {#if body}<div class="body">{@render body()}</div>{/if}
</svelte:element>

<style>
  .index-card {
    position: relative;
    padding: var(--s-3) var(--s-6) 0;
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background-color: var(--paper-1);
    overflow: hidden;
  }
  .title {
    padding-bottom: var(--s-3);
  }
  /* The card's header line, full width. */
  .title.lined {
    margin-inline: calc(-1 * var(--s-6));
    padding-inline: var(--s-6);
    border-bottom: 1px solid var(--ribbon);
  }
  /* Faint rules only below the header line, one per text line. */
  .body {
    margin-inline: calc(-1 * var(--s-6));
    padding-inline: var(--s-6);
    min-height: calc(2 * var(--rule-step));
    margin-bottom: -1px; /* the last rule is hidden under the card border */
    line-height: var(--rule-step);
    background-image: repeating-linear-gradient(to bottom, transparent 0 27px, var(--rule) 27px 28px);
  }
  .tab {
    position: absolute;
    inset-block-start: -1px;
    inset-inline-end: var(--s-6);
    display: block;
    line-height: 0;
  }
</style>
