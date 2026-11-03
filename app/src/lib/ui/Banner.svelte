<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  let { kind = 'info', children }: { kind?: 'info' | 'warning' | 'error'; children: Snippet } = $props();
</script>

<div class="banner {kind}" role={kind === 'error' ? 'alert' : 'status'}>
  <Icon name={kind} />
  <div class="msg">{@render children()}</div>
</div>

<style>
  .banner {
    display: flex;
    gap: var(--s-4);
    align-items: flex-start;
    padding: var(--s-4) var(--s-5);
    border: 1px solid var(--rule);
    border-inline-start-width: 3px;
    border-radius: var(--r-1);
    background: var(--paper-1);
    color: var(--ink-0);
  }
  .banner :global(.icon) {
    margin-top: 0.2rem;
  }
  .msg {
    max-width: var(--measure);
  }
  .info {
    border-inline-start-color: var(--focus);
  }
  .warning {
    background: var(--foxing-wash);
    border-color: var(--foxing);
    color: var(--foxing);
  }
  .error {
    border-color: var(--danger);
    color: var(--danger);
  }
</style>
