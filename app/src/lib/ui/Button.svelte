<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';

  interface Props extends Omit<HTMLButtonAttributes, 'children' | 'onclick'> {
    variant?: 'primary' | 'quiet' | 'danger';
    type?: 'button' | 'submit' | 'reset';
    disabled?: boolean;
    onclick?: (e: MouseEvent) => void;
    children: Snippet;
  }
  let { variant = 'quiet', type = 'button', disabled = false, onclick, children, ...rest }: Props = $props();
</script>

<button class="btn {variant}" {type} {disabled} {onclick} {...rest}>
  {@render children()}
</button>

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--s-3);
    min-height: 2.25rem;
    padding: 0 var(--s-5);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background: var(--paper-1);
    color: var(--ink-0);
    font-weight: 500;
    line-height: var(--t-base-lh);
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease),
      border-color var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .btn:hover:not(:disabled) {
    background: var(--paper-2);
    border-color: var(--ink-2);
  }
  .primary {
    background: var(--ribbon);
    border-color: var(--ribbon);
    color: var(--on-ribbon);
  }
  .primary:hover:not(:disabled) {
    background: var(--ribbon);
    border-color: var(--ink-0);
    filter: brightness(0.92);
  }
  .quiet {
    background: transparent;
    border-color: transparent;
  }
  .quiet:hover:not(:disabled) {
    background: var(--paper-2);
    border-color: transparent;
  }
  .danger {
    color: var(--danger);
    border-color: var(--danger);
    background: transparent;
  }
  .danger:hover:not(:disabled) {
    background: var(--danger);
    border-color: var(--danger);
    color: var(--paper-1);
  }
  .btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
