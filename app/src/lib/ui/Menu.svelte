<script lang="ts" module>
  export interface MenuItem {
    label: string;
    onselect: () => void;
    danger?: boolean;
    disabled?: boolean;
    /** Tooltip; use it to say why an item is disabled. */
    title?: string;
    /** A thin line above this item, to start a new group (shown by the context menu). */
    separatorBefore?: boolean;
  }
</script>

<script lang="ts">
  import IconButton from './IconButton.svelte';
  import Button from './Button.svelte';
  import type { IconName } from './icons';
  import { tick } from 'svelte';

  let {
    label,
    items,
    icon = 'more',
    text,
    align = 'end',
  }: {
    label: string;
    items: MenuItem[];
    icon?: IconName;
    /** Show this text on a regular button instead of the icon. */
    text?: string;
    /** Which edge of the button the popup lines up with. */
    align?: 'start' | 'end';
  } = $props();

  const uid = $props.id();
  let open = $state(false);
  let wrap = $state<HTMLElement>();
  let menu = $state<HTMLElement>();

  function enabled(): HTMLElement[] {
    return menu ? Array.from(menu.querySelectorAll<HTMLElement>('[role="menuitem"]:not([aria-disabled="true"])')) : [];
  }

  async function show() {
    open = true;
    await tick();
    enabled()[0]?.focus();
  }

  function close(restore = true) {
    open = false;
    if (restore) wrap?.querySelector<HTMLElement>('button')?.focus();
  }

  function onkeydown(e: KeyboardEvent) {
    const list = enabled();
    const at = list.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      list[(at + 1) % list.length]?.focus();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      list[(at - 1 + list.length) % list.length]?.focus();
    } else if (e.key === 'Home') {
      e.preventDefault();
      list[0]?.focus();
    } else if (e.key === 'End') {
      e.preventDefault();
      list[list.length - 1]?.focus();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close();
    } else if (e.key === 'Tab') {
      close(false);
    }
  }

  function choose(item: MenuItem) {
    if (item.disabled) return;
    close();
    item.onselect();
  }
</script>

<svelte:window onmousedown={(e) => open && !wrap?.contains(e.target as Node) && close(false)} />

<div class="menu-wrap" bind:this={wrap}>
  {#if text}
    <Button
      aria-haspopup="menu"
      aria-expanded={open}
      aria-controls={open ? `${uid}-menu` : undefined}
      onclick={() => (open ? close() : show())}>{text}</Button
    >
  {:else}
    <IconButton
      {label}
      {icon}
      aria-haspopup="menu"
      aria-expanded={open}
      aria-controls={open ? `${uid}-menu` : undefined}
      onclick={() => (open ? close() : show())}
    />
  {/if}
  {#if open}
    <div id="{uid}-menu" class="menu" class:start={align === 'start'} role="menu" aria-label={label} tabindex="-1" bind:this={menu} {onkeydown}>
      {#each items as item (item.label)}
        <button
          type="button"
          role="menuitem"
          tabindex="-1"
          class:danger={item.danger}
          aria-disabled={item.disabled ? 'true' : undefined}
          title={item.title}
          onclick={() => choose(item)}
        >
          {item.label}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .menu-wrap {
    position: relative;
    display: inline-block;
  }
  .menu {
    position: absolute;
    z-index: 50;
    inset-inline-end: 0;
    top: calc(100% + 4px);
    min-width: 11rem;
    padding: var(--s-2);
    display: grid;
    background: var(--paper-1);
    border: 1px solid var(--rule);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-pop);
    animation: pop var(--dur-base) var(--ease);
  }
  .menu.start {
    inset-inline-end: auto;
    inset-inline-start: 0;
  }
  button[role='menuitem'] {
    padding: var(--s-3) var(--s-4);
    border: 0;
    border-radius: var(--r-1);
    background: transparent;
    text-align: start;
    cursor: pointer;
  }
  button[role='menuitem']:hover,
  button[role='menuitem']:focus-visible {
    background: var(--paper-2);
  }
  button[role='menuitem'][aria-disabled='true'] {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .danger {
    color: var(--danger);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
</style>
