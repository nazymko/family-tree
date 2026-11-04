<script lang="ts">
  import type { ChartWindow, Rect } from '../api/types';

  let {
    bounds,
    win,
    view,
    onJump,
    ink = '#5A6272',
    accent = '#9B2C3F',
  }: {
    bounds: { w: number; h: number };
    /** Every person of the chart at the lowest detail; null until it has loaded. */
    win: ChartWindow | null;
    /** The part of the chart now on screen, in chart units. */
    view: Rect;
    /** The chart point the person clicked or dragged to; the main view centres there. */
    onJump: (x: number, y: number) => void;
    ink?: string;
    accent?: string;
  } = $props();

  const W = 180;
  const H = 120;
  let canvas = $state<HTMLCanvasElement | null>(null);
  let dragging = false;

  const scale = $derived(Math.min(W / Math.max(1, bounds.w), H / Math.max(1, bounds.h)));

  $effect(() => {
    const el = canvas;
    const ctx = el?.getContext('2d');
    if (!el || !ctx) return;
    const s = scale;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, W, H);
    if (win) {
      ctx.beginPath();
      for (const n of win.nodes) ctx.rect(n.x * s, n.y * s, Math.max(1, n.w * s), Math.max(1, n.h * s));
      ctx.fillStyle = ink;
      ctx.fill();
    }
    ctx.beginPath();
    const x = Math.max(0, view.x * s);
    const y = Math.max(0, view.y * s);
    const w = Math.max(3, Math.min(W - x, view.w * s));
    const h = Math.max(3, Math.min(H - y, view.h * s));
    ctx.rect(x, y, w, h);
    ctx.lineWidth = 2;
    ctx.strokeStyle = accent;
    ctx.stroke();
  });

  function at(e: { clientX: number; clientY: number }): void {
    const r = canvas!.getBoundingClientRect();
    const px = ((e.clientX - r.left) / Math.max(1, r.width)) * W;
    const py = ((e.clientY - r.top) / Math.max(1, r.height)) * H;
    onJump(Math.min(bounds.w, Math.max(0, px / scale)), Math.min(bounds.h, Math.max(0, py / scale)));
  }

  function onpointerdown(e: PointerEvent): void {
    dragging = true;
    try {
      canvas?.setPointerCapture?.(e.pointerId);
    } catch {
      // Not every environment supports capture.
    }
    at(e);
  }

  function onpointermove(e: PointerEvent): void {
    if (dragging) at(e);
  }

  function onpointerup(): void {
    dragging = false;
  }

  function onkeydown(e: KeyboardEvent): void {
    const step = e.shiftKey ? 3 : 1;
    const dx = e.key === 'ArrowLeft' ? -1 : e.key === 'ArrowRight' ? 1 : 0;
    const dy = e.key === 'ArrowUp' ? -1 : e.key === 'ArrowDown' ? 1 : 0;
    if (!dx && !dy) return;
    e.preventDefault();
    onJump(
      Math.min(bounds.w, Math.max(0, view.x + view.w / 2 + (dx * step * view.w) / 4)),
      Math.min(bounds.h, Math.max(0, view.y + view.h / 2 + (dy * step * view.h) / 4)),
    );
  }
</script>

<!-- svelte-ignore a11y_no_interactive_element_to_noninteractive_role -->
<canvas
  bind:this={canvas}
  class="mini"
  width={W}
  height={H}
  role="application"
  aria-label="Minimap of the whole tree. Click or drag to move the view, or use the arrow keys."
  tabindex="0"
  {onpointerdown}
  {onpointermove}
  {onpointerup}
  onpointercancel={onpointerup}
  {onkeydown}
></canvas>

<style>
  .mini {
    display: block;
    width: 180px;
    height: 120px;
    background: var(--paper-1);
    border: 1px solid var(--rule);
    box-shadow: var(--shadow-pop);
    cursor: pointer;
    touch-action: none;
  }
  .mini:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
</style>
