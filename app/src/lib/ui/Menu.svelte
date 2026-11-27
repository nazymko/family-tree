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
    /**
     * A choice of one in a group (shown by the context menu): the item is a radio item, ticked when true. Choosing it
     * keeps the menu open, so the person sees the tick move and can go on to the action.
     */
    checked?: boolean;
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
    variant = 'quiet',
  }: {
    label: string;
    items: MenuItem[];
    icon?: IconName;
    /** Show this text on a regular button instead of the icon. */
    text?: string;
    /** Which edge of the button the popup lines up with. */
    align?: 'start' | 'end';
    /** Look of the text button. */
    variant?: 'secondary' | 'quiet';
  } = $props();

  const uid = $props.id();
  let open = $state(false);
  let wrap = $state<HTMLElement>();
  let menu = $state<HTMLElement>();

  function enabled(): HTMLElement[] {
    return menu ? Array.from(menu.querySelectorAll<HTMLElement>('[role="menuitem"]:not([aria-disabled="true"])')) : [];
  }

  /**
   * The popup is put in the browser's top layer and placed by the button's position on screen, so a card
   * with clipped corners or a scrolling list around the button cannot cut it off.
   */
  function place() {
    const btn = wrap?.querySelector<HTMLElement>('button');
    if (!btn || !menu) return;
    const b = btn.getBoundingClientRect();
    const w = menu.offsetWidth;
    const h = menu.offsetHeight;
    const vw = document.documentElement.clientWidth;
    const vh = document.documentElement.clientHeight;
    const rtl = getComputedStyle(btn).direction === 'rtl';
    const atLeft = (align === 'start') !== rtl;
    const left = Math.max(8, Math.min(atLeft ? b.left : b.right - w, vw - w - 8));
    const below = b.bottom + 4;
    const top = below + h > vh - 8 && b.top - 4 - h >= 8 ? b.top - 4 - h : below;
    menu.style.left = `${left}px`;
    menu.style.top = `${top}px`;
  }

  async function show() {
    open = true;
    await tick();
    try {
      menu?.showPopover?.();
    } catch {
      // No top layer here: the fixed position below still applies.
    }
    place();
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

<svelte:window
  onmousedown={(e) => open && !wrap?.contains(e.target as Node) && !menu?.contains(e.target as Node) && close(false)}
  onresize={() => open && place()}
  onscrollcapture={() => open && place()}
/>

<div class="menu-wrap" bind:this={wrap}>
  {#if text}
    <Button
      {variant}
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
    <div id="{uid}-menu" class="menu" popover="manual" role="menu" aria-label={label} tabindex="-1" bind:this={menu} {onkeydown}>
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
    position: fixed;
    z-index: 50;
    inset: auto;
    margin: 0;
    width: max-content;
    max-width: calc(100vw - 16px);
    max-height: calc(100vh - 16px);
    overflow-y: auto;
    color: inherit;
    min-width: 11rem;
    padding: var(--s-2);
    display: grid;
    align-content: start;
    grid-auto-rows: max-content;
    height: auto;
    background: var(--paper-1);
    border: 1px solid var(--rule);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-pop);
    animation: pop var(--dur-base) var(--ease);
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
