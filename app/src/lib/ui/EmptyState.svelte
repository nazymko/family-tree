<script lang="ts">
  import type { Snippet } from 'svelte';

  let { title, children, action }: { title: string; children?: Snippet; action?: Snippet } = $props();
</script>

<section class="empty">
  <h2>{title}</h2>
  {#if children}<p>{@render children()}</p>{/if}
  {#if action}<div class="action">{@render action()}</div>{/if}
</section>

<style>
  .empty {
    display: grid;
    justify-items: start;
    gap: var(--s-3);
    max-width: var(--measure);
    padding: var(--s-7) 0;
  }
  p {
    color: var(--ink-1);
  }
  .action {
    margin-top: var(--s-3);
  }
</style>
