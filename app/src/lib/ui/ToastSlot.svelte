<script lang="ts">
  import Toast from './Toast.svelte';
  import { notices } from './notices.svelte';
</script>

{#if notices.current}
  {@const n = notices.current}
  <div class="slot">
    <Toast
      message={n.message}
      actionLabel={n.actionValid && !n.actionValid() ? undefined : n.actionLabel}
      onaction={() => {
        n.onaction?.();
        notices.dismiss();
      }}
      ondismiss={() => notices.dismiss()}
    />
  </div>
{/if}

<style>
  .slot {
    position: fixed;
    inset-block-end: var(--s-5);
    inset-inline-start: var(--s-5);
    z-index: 200;
    max-width: min(28rem, calc(100vw - 2rem));
  }
</style>
