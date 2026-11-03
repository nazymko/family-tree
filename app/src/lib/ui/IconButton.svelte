<script lang="ts">
  import Icon from './Icon.svelte';
  import type { IconName } from './icons';
  import type { HTMLButtonAttributes } from 'svelte/elements';

  interface Props extends Omit<HTMLButtonAttributes, 'children' | 'onclick'> {
    /** Accessible name; also shown as the tooltip. */
    label: string;
    icon: IconName;
    onclick?: (e: MouseEvent) => void;
    disabled?: boolean;
    pressed?: boolean;
  }
  let { label, icon, onclick, disabled = false, pressed, ...rest }: Props = $props();
</script>

<button
  type="button"
  class="ib"
  aria-label={label}
  title={label}
  aria-pressed={pressed}
  {disabled}
  {onclick}
  {...rest}
>
  <Icon name={icon} />
</button>

<style>
  .ib {
    display: inline-grid;
    place-items: center;
    width: 2.25rem;
    height: 2.25rem;
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--r-1);
    background: transparent;
    color: var(--ink-1);
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .ib:hover:not(:disabled) {
    background: var(--paper-2);
    color: var(--ink-0);
  }
  .ib[aria-pressed='true'] {
    background: var(--ribbon-wash);
    color: var(--ribbon);
  }
  .ib:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
