<script lang="ts">
  import { untrack } from 'svelte';
  import { getApi } from '../api/context';
  import type { ChartMeta, ChartNode, ChartWindow, Id, Rect } from '../api/types';
  import { undoState as undo } from '../state/undo.svelte';
  import Button from '../ui/Button.svelte';
  import IconButton from '../ui/IconButton.svelte';
  import Select from '../ui/Select.svelte';
  import { centerOn, fit, panBy, tween, viewRect, zoomAt, type Camera } from './camera';
  import { hitHandle, hitTest, neighbor, type Dir } from './hit';
  import { draw, themeFromCss, type Theme } from './renderer';
  import { WindowCache } from './window';

  let {
    kind,
    root,
    generations,
    onRecentre,
    onOpenPerson,
    onGenerations,
    onOpened,
    hidden = false,
  }: {
    kind: 'pedigree' | 'descendants';
    root: Id;
    generations: number;
    onRecentre: (id: Id) => void;
    onOpenPerson: (id: Id) => void;
    /** Shows the generations Select in the toolbar when given. */
    onGenerations?: (n: number) => void;
    /** Called with the chart meta each time a chart is (re)opened. */
    onOpened?: (meta: ChartMeta) => void;
    /** Keeps the chart open but not drawn (the outline view is showing). */
    hidden?: boolean;
  } = $props();

  const api = getApi();
  const uid = $props.id();
  const TWEEN_MS = 280;

  let canvas = $state<HTMLCanvasElement | null>(null);
  let box = $state<HTMLDivElement | null>(null);
  let meta = $state<ChartMeta | null>(null);
  let errorText = $state('');
  let selected = $state<number | null>(null);
  let selNode = $state<ChartNode | null>(null);
  let announcement = $state('');
  let homeId = $state<Id | null>(null);

  let cache: WindowCache | null = null;
  let cam: Camera = { x: 0, y: 0, zoom: 1 };
  let size = { w: 800, h: 600 };
  let dpr = 1;
  let dirty = false;
  let rafId = 0;
  let openSeq = 0;
  let activeTween: { cancel(): void } | null = null;
  let theme: Theme | null = null;
  let rootNodeId = 0;
  let pendingAnnounce = false;

  const reduced = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const personName = $derived(selNode?.person?.name ?? '');
  const kindLabel = $derived(kind === 'pedigree' ? 'Pedigree' : 'Descendant');

  function invalidate(): void {
    dirty = true;
    if (!rafId && !hidden) rafId = requestAnimationFrame(frame);
  }

  function frame(): void {
    rafId = 0;
    if (!dirty || hidden || !canvas) return;
    dirty = false;
    const cur = cache?.current();
    const ctx = canvas.getContext('2d');
    if (!cur || !ctx) return;
    theme ??= themeFromCss();
    draw(ctx, cur.win, cam, { w: size.w, h: size.h, dpr }, { selected, focus: rootNodeId, theme, kind });
  }

  function setCam(c: Camera): void {
    cam = c;
    cache?.need(viewRect(cam, size), cam.zoom);
    invalidate();
  }

  function animateTo(target: Camera): void {
    activeTween?.cancel();
    activeTween = tween(cam, target, TWEEN_MS, setCam, reduced());
  }

  function resize(w: number, h: number): void {
    if (w <= 0 || h <= 0 || !canvas) return;
    size = { w, h };
    dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    cache?.need(viewRect(cam, size), cam.zoom);
    invalidate();
  }

  function closeChart(): void {
    activeTween?.cancel();
    cache?.dispose();
    cache = null;
    if (meta) void api.chartClose(meta.chartId).catch(() => {});
    meta = null;
  }

  function isStale(e: unknown): boolean {
    return typeof e === 'object' && e !== null && (e as { code?: string }).code === 'stale';
  }

  async function open(): Promise<void> {
    const seq = ++openSeq;
    closeChart();
    errorText = '';
    selected = null;
    selNode = null;
    let m: ChartMeta;
    try {
      m = await api.chartOpen({ kind, root, generations });
    } catch (e) {
      if (seq === openSeq) errorText = e instanceof Error ? e.message : 'The chart could not be opened.';
      return;
    }
    if (seq !== openSeq) {
      void api.chartClose(m.chartId).catch(() => {});
      return;
    }
    meta = m;
    rootNodeId = m.rootNode;
    const c = new WindowCache(async (r, z) => {
      try {
        return await api.chartWindow(m.chartId, r, z);
      } catch (e) {
        if (isStale(e) && seq === openSeq) void open();
        throw e;
      }
    });
    cache = c;
    c.onChange(() => {
      if (selected !== null) {
        const n = c.current()?.win.nodes.find((x) => x.id === selected);
        if (n) {
          selNode = n;
          if (pendingAnnounce) {
            pendingAnnounce = false;
            announce();
          }
        }
      }
      invalidate();
    });
    // Find the root node with a thin query, then place the camera.
    const probe: Rect = kind === 'pedigree' ? { x: -1, y: 0, w: 178, h: Math.max(1, m.bounds.h) } : { x: 0, y: -1, w: Math.max(1, m.bounds.w), h: 58 };
    let first: ChartWindow;
    try {
      first = await api.chartWindow(m.chartId, probe, 1);
    } catch (e) {
      if (isStale(e) && seq === openSeq) void open();
      return;
    }
    if (seq !== openSeq) return;
    const rn = first.nodes.find((n) => n.id === m.rootNode) ?? first.nodes[0] ?? null;
    selected = rn?.id ?? null;
    selNode = rn;
    const f = fit(m.bounds, size);
    if (f.zoom >= 0.55 || !rn) cam = f;
    else if (kind === 'pedigree') cam = { zoom: 1, x: rn.x - 40, y: rn.y + rn.h / 2 - size.h / 2 };
    else cam = { zoom: 1, x: rn.x + rn.w / 2 - size.w / 2, y: rn.y - 40 };
    pendingAnnounce = true;
    c.need(viewRect(cam, size), cam.zoom);
    onOpened?.(m);
  }

  $effect(() => {
    void kind;
    void root;
    void generations;
    untrack(() => void open());
    return () => {
      openSeq++;
      closeChart();
    };
  });

  // Edits elsewhere change the undo state; reopen so the chart shows the new tree.
  const undoState = () => `${undo.canUndo}|${undo.undoLabel}|${undo.canRedo}|${undo.redoLabel}`;
  let undoKey = untrack(() => undoState());
  $effect(() => {
    const k = undoState();
    if (k !== undoKey) {
      undoKey = k;
      untrack(() => {
        if (meta) void open();
      });
    }
  });

  $effect(() => {
    void api
      .getSettings()
      .then((s) => (homeId = s.homePerson))
      .catch(() => {});
  });

  $effect(() => {
    if (!hidden) {
      theme = null;
      invalidate();
    }
  });

  // Resize, theme change and wheel handling need the real element.
  $effect(() => {
    const el = box;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      const r = entries[0]?.contentRect;
      if (r) resize(r.width, r.height);
    });
    ro.observe(el);
    const mq = typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: dark)') : null;
    const onScheme = () => {
      theme = null;
      invalidate();
    };
    mq?.addEventListener?.('change', onScheme);
    const mo = new MutationObserver(onScheme);
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
    return () => {
      ro.disconnect();
      mq?.removeEventListener?.('change', onScheme);
      mo.disconnect();
    };
  });

  $effect(() => {
    const el = canvas;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      activeTween?.cancel();
      const r = el.getBoundingClientRect();
      const factor = Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.0015));
      setCam(zoomAt(cam, factor, { x: e.clientX - r.left, y: e.clientY - r.top }));
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
  });

  $effect(() => () => {
    if (rafId) cancelAnimationFrame(rafId);
    rafId = 0;
  });

  // ------------------------------------------------------------ selection

  function counts(): string {
    const cur = cache?.current();
    if (!cur || !meta || !selNode || cur.win.nodes.length !== meta.nodeCount) return '';
    const same = cur.win.nodes.filter((n) => n.gen === selNode!.gen).sort((a, b) => a.y - b.y || a.x - b.x);
    const i = same.findIndex((n) => n.id === selNode!.id);
    return i < 0 ? '' : `, ${i + 1} of ${same.length} in this generation`;
  }

  function announce(): void {
    announcement = selNode ? `${selNode.label}${counts()}` : '';
  }

  function select(n: ChartNode): void {
    selected = n.id;
    selNode = n;
    announce();
    invalidate();
  }

  function ensureVisible(n: ChartNode): void {
    const v = viewRect(cam, size);
    const inside = n.x >= v.x && n.y >= v.y && n.x + n.w <= v.x + v.w && n.y + n.h <= v.y + v.h;
    if (!inside) animateTo(centerOn(cam, n, size));
  }

  function toWorld(e: { clientX: number; clientY: number }): { x: number; y: number } {
    const r = canvas!.getBoundingClientRect();
    return { x: cam.x + (e.clientX - r.left) / cam.zoom, y: cam.y + (e.clientY - r.top) / cam.zoom };
  }

  /** Where node `id` is, from an edge in the window that ends at it (the node itself may be outside the cache). */
  function rectFromEdge(win: ChartWindow, id: number, ref: ChartNode): Rect | null {
    const e = win.edges.find((x) => x.to === id && x.pts.length >= 4);
    if (!e) {
      // Towards the root: use the start of an edge that leaves the node.
      const out = win.edges.find((x) => x.kind === 'parent' && x.from === id && x.pts.length >= 4);
      if (!out) return null;
      const sx = out.pts[0];
      const sy = out.pts[1];
      return kind === 'pedigree'
        ? { x: sx - ref.w, y: sy - ref.h / 2, w: ref.w, h: ref.h }
        : { x: sx - ref.w / 2, y: sy - ref.h, w: ref.w, h: ref.h };
    }
    const px = e.pts[e.pts.length - 2];
    const py = e.pts[e.pts.length - 1];
    if (e.kind === 'dup') return { x: px - ref.w, y: py - ref.h / 2, w: ref.w, h: ref.h };
    return kind === 'pedigree'
      ? { x: px, y: py - ref.h / 2, w: ref.w, h: ref.h }
      : { x: px - ref.w / 2, y: py, w: ref.w, h: ref.h };
  }

  /** Select node `id`, scrolling to it even when it is not in the cached window yet. */
  function goTo(id: number, from: ChartNode): void {
    const cur = cache?.current();
    if (!cur) return;
    const n = cur.win.nodes.find((x) => x.id === id);
    if (n) {
      select(n);
      ensureVisible(n);
      return;
    }
    const rect = rectFromEdge(cur.win, id, from);
    if (!rect) return;
    selected = id;
    pendingAnnounce = true;
    animateTo(centerOn(cam, rect, size));
    invalidate();
  }

  function jumpToOriginal(n: ChartNode): void {
    if (n.dupOf === null) return;
    const cur = cache?.current();
    const orig = cur?.win.nodes.find((x) => x.id === n.dupOf);
    if (orig) {
      select(orig);
      animateTo(centerOn(cam, orig, size));
    } else if (cur) {
      const rect = rectFromEdge(cur.win, n.dupOf, n);
      if (!rect) return;
      selected = n.dupOf;
      pendingAnnounce = true;
      animateTo(centerOn(cam, rect, size));
      invalidate();
    }
  }

  function recentre(n: ChartNode): void {
    if (!n.person) return;
    animateTo(centerOn(cam, n, size));
    onRecentre(n.person.id);
  }

  // ------------------------------------------------------------ pointer

  // Non-reactive bookkeeping for gestures.
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  const pointers = new Map<number, { x: number; y: number }>();
  let drag: { x: number; y: number; moved: boolean } | null = null;
  let pinch = 0;

  function onpointerdown(e: PointerEvent): void {
    canvas?.focus();
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    try {
      canvas?.setPointerCapture?.(e.pointerId);
    } catch {
      // Not every environment supports capture.
    }
    if (pointers.size === 1) drag = { x: e.clientX, y: e.clientY, moved: false };
    else {
      drag = null;
      const [a, b] = [...pointers.values()];
      pinch = Math.hypot(a.x - b.x, a.y - b.y);
    }
  }

  function onpointermove(e: PointerEvent): void {
    const p = pointers.get(e.pointerId);
    if (!p) return;
    const dx = e.clientX - p.x;
    const dy = e.clientY - p.y;
    p.x = e.clientX;
    p.y = e.clientY;
    if (pointers.size >= 2) {
      const [a, b] = [...pointers.values()];
      const d = Math.hypot(a.x - b.x, a.y - b.y);
      if (pinch > 0 && d > 0) {
        const r = canvas!.getBoundingClientRect();
        setCam(zoomAt(cam, d / pinch, { x: (a.x + b.x) / 2 - r.left, y: (a.y + b.y) / 2 - r.top }));
      }
      pinch = d;
      return;
    }
    if (!drag) return;
    if (!drag.moved && Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 4) return;
    drag.moved = true;
    activeTween?.cancel();
    setCam(panBy(cam, dx, dy, meta?.bounds ?? { w: 1, h: 1 }, size));
  }

  function onpointerup(e: PointerEvent): void {
    const wasClick = drag !== null && !drag.moved && pointers.size === 1;
    pointers.delete(e.pointerId);
    if (pointers.size < 2) pinch = 0;
    drag = null;
    if (!wasClick) return;
    const cur = cache?.current();
    if (!cur) return;
    const w = toWorld(e);
    const handle = hitHandle(cur.win, kind, w.x, w.y);
    if (handle?.person) {
      onRecentre(handle.person.id);
      return;
    }
    const n = hitTest(cur.win, w.x, w.y);
    if (!n) return;
    if (n.dupOf !== null) jumpToOriginal(n);
    else select(n);
  }

  function ondblclick(e: MouseEvent): void {
    const cur = cache?.current();
    if (!cur) return;
    const w = toWorld(e);
    const n = hitTest(cur.win, w.x, w.y);
    if (n && n.dupOf === null) recentre(n);
  }

  // ------------------------------------------------------------ keyboard

  function dirFor(key: string): Dir | null {
    if (kind === 'pedigree') {
      return key === 'ArrowRight' ? 'child' : key === 'ArrowLeft' ? 'parent' : key === 'ArrowUp' ? 'up' : key === 'ArrowDown' ? 'down' : null;
    }
    return key === 'ArrowDown' ? 'child' : key === 'ArrowUp' ? 'parent' : key === 'ArrowLeft' ? 'up' : key === 'ArrowRight' ? 'down' : null;
  }

  function onkeydown(e: KeyboardEvent): void {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    const cur = cache?.current();
    const dir = dirFor(e.key);
    if (dir) {
      e.preventDefault();
      if (!cur || selected === null) return;
      const id = neighbor(cur.win, selected, dir);
      if (id !== null && selNode && selNode.id === selected) goTo(id, selNode);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (selNode) (selNode.dupOf !== null ? jumpToOriginal : recentre)(selNode);
    } else if (e.key === 'e' || e.key === 'E') {
      if (selNode?.person) onOpenPerson(selNode.person.id);
    } else if (e.key === 'Home') {
      e.preventDefault();
      const n = cur?.win.nodes.find((x) => x.id === rootNodeId);
      if (n) {
        select(n);
        animateTo(centerOn(cam, n, size));
      }
    } else if (e.key === '+' || e.key === '=') zoomBy(1.25);
    else if (e.key === '-') zoomBy(0.8);
    else if (e.key === '0') fitAll();
  }

  function zoomBy(f: number): void {
    activeTween?.cancel();
    setCam(zoomAt(cam, f, { x: size.w / 2, y: size.h / 2 }));
  }

  function fitAll(): void {
    if (meta) animateTo(fit(meta.bounds, size));
  }

  const genOptions = [3, 4, 5, 6, 7, 8].map((n) => ({ value: String(n), label: `${n} generations` }));
  let genValue = $derived(String(Math.min(8, Math.max(3, generations))));
</script>

<div class="chart" {hidden}>
  <div class="toolbar" role="toolbar" aria-label="{kindLabel} chart tools">
    <Button onclick={() => zoomBy(1.25)}>Zoom in</Button>
    <Button onclick={() => zoomBy(0.8)}>Zoom out</Button>
    <Button onclick={fitAll}>Fit</Button>
    <IconButton label="Go to home person" icon="home" disabled={!homeId} onclick={() => homeId && onRecentre(homeId)} />
    {#if onGenerations}
      <div class="gens">
        <Select label="Generations" options={genOptions} bind:value={genValue} onchange={(v) => onGenerations(Number(v))} />
      </div>
    {/if}
  </div>
  <div class="stage" bind:this={box}>
    <!-- svelte-ignore a11y_no_interactive_element_to_noninteractive_role -->
    <canvas
      bind:this={canvas}
      role="application"
      aria-label="{kindLabel} chart of {personName}. Use arrow keys to move between people, Enter to centre, E to edit."
      aria-activedescendant={selected !== null ? `${uid}-sel` : undefined}
      tabindex="0"
      {onpointerdown}
      {onpointermove}
      {onpointerup}
      onpointercancel={onpointerup}
      {ondblclick}
      {onkeydown}
    ></canvas>
    <div class="sr-only" role="listbox" aria-label="Selected person">
      {#if selNode}
        <div id="{uid}-sel" role="option" aria-selected="true">{selNode.label}</div>
      {/if}
    </div>
    <div class="sr-only" role="status">{announcement}</div>
    {#if errorText}
      <p class="err" role="alert">{errorText}</p>
    {/if}
  </div>
</div>

<style>
  .chart {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }
  .chart[hidden] {
    display: none;
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--s-3);
    padding-bottom: var(--s-3);
  }
  .gens {
    margin-inline-start: auto;
  }
  .stage {
    position: relative;
    flex: 1;
    min-height: 16rem;
    border: 1px solid var(--rule);
    background: var(--paper-0);
    overflow: hidden;
  }
  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    touch-action: none;
    cursor: grab;
  }
  canvas:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: -2px;
  }
  .err {
    position: absolute;
    inset: auto var(--s-4) var(--s-4) var(--s-4);
    margin: 0;
    padding: var(--s-3) var(--s-4);
    background: var(--paper-1);
    border: 1px solid var(--danger);
    color: var(--danger);
  }
</style>
