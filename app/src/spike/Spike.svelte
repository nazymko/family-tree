<script lang="ts">
  import { onMount } from 'svelte';
  import { viewRect, type Camera } from '../lib/chart/camera';
  import { draw, themeFromCss } from '../lib/chart/renderer';
  import { synthChart } from '../lib/chart/synth';
  import { WindowCache } from '../lib/chart/window';

  const N = 10000;
  const DURATION_MS = 10000;

  let canvas: HTMLCanvasElement;
  let result = $state<string>('Running the 10 second camera path.');

  function percentile(sorted: number[], p: number): number {
    return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * p))] ?? 0;
  }

  onMount(() => {
    const chart = synthChart(N);
    const cache = new WindowCache(async (r) => chart.query(r));
    const dpr = window.devicePixelRatio || 1;
    const w = window.innerWidth;
    const h = window.innerHeight;
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    const ctx = canvas.getContext('2d')!;
    const theme = themeFromCss();
    const view = { w, h, dpr };
    const deltas: number[] = [];
    let last = 0;
    let t0 = 0;
    let raf = 0;
    let stopped = false;

    cache.need(viewRect({ x: 0, y: 0, zoom: 1 }, view), 1);

    const frame = (now: number) => {
      if (stopped) return;
      if (t0 === 0) t0 = now;
      if (last !== 0) deltas.push(now - last);
      last = now;
      const t = now - t0;
      if (t >= DURATION_MS) {
        finish();
        return;
      }
      // Zoom oscillates 0.15..1.5 (geometrically) with a 6 s period; pan is a Lissajous over the chart.
      const s = Math.sin((2 * Math.PI * t) / 6000);
      const zoom = Math.exp(Math.log(0.15) + ((Math.log(1.5) - Math.log(0.15)) * (s + 1)) / 2);
      const px = chart.meta.bounds.w * (0.5 + 0.45 * Math.sin((2 * Math.PI * t) / 9000));
      const py = chart.meta.bounds.h * (0.5 + 0.45 * Math.sin((2 * Math.PI * t) / 7000 + 1));
      const cam: Camera = { zoom, x: px - w / zoom / 2, y: py - h / zoom / 2 };
      const vr = viewRect(cam, view);
      cache.need(vr, zoom);
      const cur = cache.current();
      if (cur) draw(ctx, cur.win, cam, view, { selected: 0, focus: 0, theme });
      raf = requestAnimationFrame(frame);
    };

    function finish() {
      stopped = true;
      const sorted = [...deltas].sort((a, b) => a - b);
      const out = {
        n: N,
        frames: deltas.length,
        medianMs: +percentile(sorted, 0.5).toFixed(2),
        p95Ms: +percentile(sorted, 0.95).toFixed(2),
        p99Ms: +percentile(sorted, 0.99).toFixed(2),
        over25ms: deltas.filter((d) => d > 25).length,
        dpr,
        ua: navigator.userAgent,
      };
      const json = JSON.stringify(out);
      result = json;
      console.log('spike result', json);
      (window as unknown as { __spike?: unknown }).__spike = out;
      const tauri = (window as unknown as { __TAURI_INTERNALS__?: { invoke?: (c: string, a: unknown) => Promise<unknown> } })
        .__TAURI_INTERNALS__;
      if (tauri?.invoke) void tauri.invoke('spike_report', { json }).catch(() => {});
    }

    raf = requestAnimationFrame(frame);
    return () => {
      stopped = true;
      cancelAnimationFrame(raf);
      cache.dispose();
    };
  });
</script>

<canvas bind:this={canvas} aria-label="Chart frame-rate spike"></canvas>
<pre class="out" role="status">{result}</pre>

<style>
  canvas {
    position: fixed;
    inset: 0;
    width: 100vw;
    height: 100vh;
    background: var(--paper-0);
  }
  .out {
    position: fixed;
    left: 1rem;
    bottom: 1rem;
    margin: 0;
    padding: 0.5rem 0.75rem;
    max-width: calc(100vw - 2rem);
    white-space: pre-wrap;
    background: var(--paper-1);
    border: 1px solid var(--rule);
    color: var(--ink-0);
    font-size: 0.8125rem;
  }
</style>
