<script lang="ts">
  import { onMount } from 'svelte';
  import type { MenuItem } from './Menu.svelte';

  /**
   * A menu shown at a point (a right-click, or a key press over a box). The parent renders it only while open.
   * Arrow keys move between the items (a disabled one too, so its hint can be read), Escape closes it.
   * `onclose(true)` asks the parent to give focus back to where the menu came from.
   */
  let { label, items, x, y, onclose }: { label: string; items: MenuItem[]; x: number; y: number; onclose: (restoreFocus: boolean) => void } = $props();

  const uid = $props.id();
  let menu = $state<HTMLElement>();
  let left = $state<number | null>(null);
  let top = $state<number | null>(null);

  const buttons = (): HTMLElement[] => (menu ? Array.from(menu.querySelectorAll<HTMLElement>('[role="menuitem"]')) : []);

  onMount(() => {
    // Keep the whole menu inside the window.
    const w = menu?.offsetWidth ?? 0;
    const h = menu?.offsetHeight ?? 0;
    left = Math.max(4, Math.min(x, window.innerWidth - w - 4));
    top = Math.max(4, Math.min(y, window.innerHeight - h - 4));
    const first = buttons().find((b) => b.getAttribute('aria-disabled') !== 'true') ?? buttons()[0];
    first?.focus();
  });

  function onkeydown(e: KeyboardEvent): void {
    const list = buttons();
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
      onclose(true);
    } else if (e.key === 'Tab') {
      e.preventDefault();
      onclose(true);
    }
  }

  function choose(item: MenuItem): void {
    if (item.disabled) return;
    onclose(true);
    item.onselect();
  }
</script>

<svelte:window onmousedown={(e) => menu && !menu.contains(e.target as Node) && onclose(false)} onblur={() => onclose(false)} onresize={() => onclose(false)} />

<div
  bind:this={menu}
  id="{uid}-menu"
  class="menu"
  role="menu"
  aria-label={label}
  tabindex="-1"
  style:left={left === null ? `${x}px` : `${left}px`}
  style:top={top === null ? `${y}px` : `${top}px`}
  {onkeydown}
  oncontextmenu={(e) => e.preventDefault()}
>
  {#each items as item (item.label)}
    <button
      type="button"
      role="menuitem"
      tabindex="-1"
      class:danger={item.danger}
      aria-disabled={item.disabled ? 'true' : undefined}
      aria-describedby={item.disabled && item.title ? `${uid}-hint` : undefined}
      onclick={() => choose(item)}
    >
      <span>{item.label}</span>
      {#if item.disabled && item.title}
        <small id="{uid}-hint">{item.title}</small>
      {/if}
    </button>
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 60;
    min-width: 14rem;
    max-width: min(24rem, calc(100vw - 8px));
    padding: var(--s-2);
    display: grid;
    background: var(--paper-1);
    border: 1px solid var(--rule);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-pop);
    animation: pop var(--dur-base) var(--ease);
  }
  button[role='menuitem'] {
    display: grid;
    gap: 2px;
    padding: var(--s-3) var(--s-4);
    border: 0;
    border-radius: var(--r-1);
    background: transparent;
    color: var(--ink-0);
    text-align: start;
    cursor: pointer;
  }
  button[role='menuitem']:hover,
  button[role='menuitem']:focus-visible {
    background: var(--paper-2);
  }
  button[role='menuitem']:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: -2px;
  }
  button[role='menuitem'][aria-disabled='true'] {
    color: var(--ink-2);
    cursor: not-allowed;
  }
  button[role='menuitem'][aria-disabled='true'] span {
    opacity: 0.6;
  }
  small {
    color: var(--ink-2);
    font-size: var(--t-sm);
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
