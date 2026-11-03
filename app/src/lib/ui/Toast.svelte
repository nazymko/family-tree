<script lang="ts">
  import Button from './Button.svelte';
  import IconButton from './IconButton.svelte';
  import Icon from './Icon.svelte';

  let {
    message,
    actionLabel,
    onaction,
    ondismiss,
  }: { message: string; actionLabel?: string; onaction?: () => void; ondismiss?: () => void } = $props();
</script>

<div class="toast" role="status">
  <Icon name="check-circle" />
  <span class="msg">{message}</span>
  {#if actionLabel}
    <Button variant="quiet" onclick={() => onaction?.()}>{actionLabel}</Button>
  {/if}
  {#if ondismiss}
    <IconButton label="Dismiss" icon="x" onclick={ondismiss} />
  {/if}
</div>

<style>
  .toast {
    display: flex;
    align-items: center;
    gap: var(--s-4);
    padding: var(--s-3) var(--s-3) var(--s-3) var(--s-5);
    background: var(--ink-0);
    color: var(--paper-1);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-pop);
    animation: in var(--dur-base) var(--ease);
  }
  .toast :global(.icon) {
    color: var(--verdigris);
    filter: brightness(1.5);
  }
  .msg {
    flex: 1;
  }
  .toast :global(.btn),
  .toast :global(.ib) {
    color: var(--paper-1);
  }
  .toast :global(.btn:hover:not(:disabled)),
  .toast :global(.ib:hover:not(:disabled)) {
    background: rgb(255 255 255 / 0.14);
    color: var(--paper-1);
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
</style>
