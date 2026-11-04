<script lang="ts">
  import type { ChartWindow, Rect } from '../api/types';

  let {
    bounds,
    win,
    view,
    onJump,
  }: {
    bounds: { w: number; h: number };
    /** Every person of the chart at the lowest detail; null until it has loaded. */
    win: ChartWindow | null;
    /** The part of the chart now on screen, in chart units. */
    view: Rect;
    /** The chart point the person clicked or dragged to; the main view centres there. */
    onJump: (x: number, y: number) => void;
  } = $props();

  const W = 220;
  const H = 120;
  let canvas = $state<HTMLCanvasElement | null>(null);
  let dragging = false;

  // The tree is scaled to fit and centred (letterboxed), so a wide strip keeps its shape and every click maps back exactly.
  const scale = $derived(Math.min((W - 8) / Math.max(1, bounds.w), (H - 8) / Math.max(1, bounds.h)));
  const ox = $derived((W - bounds.w * scale) / 2);
  const oy = $derived((H - bounds.h * scale) / 2);

  $effect(() => {
    const el = canvas;
    const ctx = el?.getContext('2d');
    if (!el || !ctx) return;
    const s = scale;
    const cs = getComputedStyle(document.documentElement);
    const ink = cs.getPropertyValue('--ink-2').trim() || '#5A6272';
    const accent = cs.getPropertyValue('--ribbon').trim() || '#9B2C3F';
    const rule = cs.getPropertyValue('--rule').trim() || '#C6CABF';
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, W, H);
    // The extent of the tree.
    ctx.strokeStyle = rule;
    ctx.lineWidth = 1;
    ctx.strokeRect(ox - 0.5, oy - 0.5, bounds.w * s + 1, bounds.h * s + 1);
    if (win) {
      ctx.beginPath();
      for (const n of win.nodes) ctx.rect(ox + n.x * s, oy + n.y * s, Math.max(1, n.w * s), Math.max(1, n.h * s));
      ctx.fillStyle = ink;
      ctx.fill();
    }
    // The view: never smaller than 10 px each way, so it can always be seen and grabbed.
    const vw = Math.min(W - 2, Math.max(10, view.w * s));
    const vh = Math.min(H - 2, Math.max(10, view.h * s));
    const cx = ox + (view.x + view.w / 2) * s;
    const cy = oy + (view.y + view.h / 2) * s;
    const x = Math.min(W - vw - 1, Math.max(1, cx - vw / 2));
    const y = Math.min(H - vh - 1, Math.max(1, cy - vh / 2));
    ctx.globalAlpha = 0.15;
    ctx.fillStyle = accent;
    ctx.fillRect(x, y, vw, vh);
    ctx.globalAlpha = 1;
    ctx.lineWidth = 2;
    ctx.strokeStyle = accent;
    ctx.strokeRect(x, y, vw, vh);
  });

  function at(e: { clientX: number; clientY: number }): void {
    const r = canvas!.getBoundingClientRect();
    const px = ((e.clientX - r.left) / Math.max(1, r.width)) * W;
    const py = ((e.clientY - r.top) / Math.max(1, r.height)) * H;
    onJump(Math.min(bounds.w, Math.max(0, (px - ox) / scale)), Math.min(bounds.h, Math.max(0, (py - oy) / scale)));
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
    width: 220px;
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
