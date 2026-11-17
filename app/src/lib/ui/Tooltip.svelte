<script lang="ts">
  import type { Snippet } from 'svelte';

  /**
   * A tip for whatever it wraps: it shows beside the pointer on hover, below the item on keyboard focus, and on a long
   * press with a finger. A tap still goes to the wrapped control, so opening it is never blocked. The wrapped control
   * keeps its own accessible name; a fact is never only in the tip.
   */
  let { children, tip, delay = 350 }: { children: Snippet; tip: Snippet; delay?: number } = $props();

  const uid = $props.id();
  let host = $state<HTMLElement>();
  let at = $state<{ x: number; y: number } | null>(null);
  let box = $state<HTMLElement>();
  let timer: ReturnType<typeof setTimeout> | undefined;
  let press: ReturnType<typeof setTimeout> | undefined;
  let longPressed = false;

  function show(x: number, y: number) {
    clearTimeout(timer);
    timer = setTimeout(() => (at = { x, y }), delay);
  }
  function hide() {
    clearTimeout(timer);
    clearTimeout(press);
    at = null;
  }

  function enter(e: PointerEvent) {
    if (e.pointerType === 'touch') return;
    show(e.clientX, e.clientY);
  }
  function move(e: PointerEvent) {
    if (e.pointerType === 'touch') return;
    if (at) at = { x: e.clientX, y: e.clientY };
  }
  function focus() {
    const r = (host?.firstElementChild ?? host)?.getBoundingClientRect();
    if (r) show(r.left, r.bottom - 18);
  }
  function down(e: PointerEvent) {
    if (e.pointerType !== 'touch') {
      hide();
      return;
    }
    longPressed = false;
    clearTimeout(press);
    const x = e.clientX;
    const y = e.clientY;
    press = setTimeout(() => {
      longPressed = true;
      at = { x, y };
    }, 500);
  }
  function cancelPress() {
    clearTimeout(press);
  }
  /** The click that ends a long press does not open the item: the finger was asking what it is. */
  function click(e: MouseEvent) {
    if (longPressed) {
      e.preventDefault();
      e.stopPropagation();
      longPressed = false;
    }
  }

  // The tip is placed at the pointer, then kept inside the window.
  const place = $derived.by(() => {
    if (!at) return '';
    const w = box?.offsetWidth ?? 0;
    const h = box?.offsetHeight ?? 0;
    const left = Math.max(8, Math.min(at.x + 14, window.innerWidth - w - 8));
    const top = at.y + 18 + h > window.innerHeight - 8 ? Math.max(8, at.y - h - 12) : at.y + 18;
    return `left: ${left}px; top: ${top}px`;
  });
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && hide()} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:this={host}
  class="host"
  aria-describedby={at ? `${uid}-tip` : undefined}
  onpointerenter={enter}
  onpointermove={move}
  onpointerleave={hide}
  onpointerdown={down}
  onpointerup={cancelPress}
  onpointercancel={cancelPress}
  onfocusin={focus}
  onfocusout={hide}
  onclickcapture={click}
>
  {@render children()}
</div>
{#if at}
  <div bind:this={box} id="{uid}-tip" class="tip" role="tooltip" style={place} dir="auto">{@render tip()}</div>
{/if}

<style>
  .host {
    display: contents;
  }
  .tip {
    position: fixed;
    z-index: 300;
    max-width: min(22rem, calc(100vw - 16px));
    padding: var(--s-2) var(--s-3);
    font-size: var(--t-sm);
    color: var(--ink-0);
    background: var(--paper-0);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    box-shadow: var(--shadow-pop);
    pointer-events: none;
    overflow-wrap: anywhere;
  }
</style>
