<script lang="ts">
  import { untrack } from 'svelte';
  import { getApi } from '../api/context';
  import type { ChartNode, ChartWindow, Id, Rect, SearchHitDto, WholeMeta } from '../api/types';
  import { project } from '../state/project.svelte';
  import { undoState as undo } from '../state/undo.svelte';
  import Button from '../ui/Button.svelte';
  import Combobox from '../ui/Combobox.svelte';
  import IconButton from '../ui/IconButton.svelte';
  import { centerOn, fit, fitZoom, panBy, tween, viewRect, ZOOM_MIN, zoomAt, type Camera } from './camera';
  import { hitTest, neighbor, type Dir } from './hit';
  import Minimap from './Minimap.svelte';
  import { WindowCache } from './window';
  import { drawWhole, wholeThemeFromCss, type WholeTheme } from './wholeRenderer';
  import { nodeLabel } from '../i18n/relation';
  import { plural, t } from '../i18n';

  let {
    homeId = null,
    onOpenPerson,
    onOpenInPedigree,
    onOpened,
  }: {
    homeId?: Id | null;
    onOpenPerson: (id: Id) => void;
    onOpenInPedigree: (id: Id) => void;
    /** Called with the chart each time it is (re)opened. */
    onOpened?: (meta: WholeMeta) => void;
  } = $props();

  const api = getApi();
  const uid = $props.id();
  const TWEEN_MS = 320;

  let canvas = $state<HTMLCanvasElement | null>(null);
  let box = $state<HTMLDivElement | null>(null);
  let meta = $state<WholeMeta | null>(null);
  let errorText = $state('');
  let selected = $state<number | null>(null);
  let selNode = $state<ChartNode | null>(null);
  let announcement = $state('');
  let hint = $state('');
  let lineageOn = $state(true);
  let lineage = $state<ReadonlySet<number> | null>(null);
  let homeNode = $state<number | null>(null);
  let miniWin = $state<ChartWindow | null>(null);
  let seen = $state<Rect>({ x: 0, y: 0, w: 1, h: 1 });
  let searchText = $state('');
  let hits = $state<SearchHitDto[]>([]);
  let zoomPct = $state(100);

  let cache: WindowCache | null = null;
  let cam: Camera = { x: 0, y: 0, zoom: 1 };
  let size = { w: 800, h: 600 };
  let sized = false;
  let userMoved = false;
  let dpr = 1;
  let dirty = false;
  let rafId = 0;
  let openSeq = 0;
  let lineSeq = 0;
  let findSeq = 0;
  let findTimer: ReturnType<typeof setTimeout> | undefined;
  let activeTween: { cancel(): void } | null = null;
  let theme: WholeTheme | null = null;
  let pendingAnnounce = false;

  const reduced = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

  /** A huge tree needs to zoom out further than a small one. */
  function minZoom(): number {
    if (!meta) return ZOOM_MIN;
    return Math.max(0.004, Math.min(ZOOM_MIN, fitZoom(meta.meta.bounds, size) * 0.9));
  }

  function invalidate(): void {
    dirty = true;
    if (!rafId) rafId = requestAnimationFrame(frame);
  }

  function frame(): void {
    rafId = 0;
    if (!dirty || !canvas || !meta) return;
    dirty = false;
    const cur = cache?.current();
    const ctx = canvas.getContext('2d');
    if (!cur || !ctx) return;
    theme ??= wholeThemeFromCss();
    drawWhole(ctx, cur.win, meta.blocks, cam, { w: size.w, h: size.h, dpr }, { selected, home: homeNode, lineage, theme });
  }

  function setCam(c: Camera, byUser = true): void {
    if (byUser) userMoved = true;
    hint = '';
    cam = c;
    seen = viewRect(cam, size);
    zoomPct = Math.round(cam.zoom * 100);
    cache?.need(seen, cam.zoom);
    adoptSelection();
    invalidate();
  }

  /** A card picked by a jump has no details until a window holds it; take them from the cached window when it already does. */
  function adoptSelection(): void {
    if (selected === null || selNode?.id === selected) return;
    const n = cache?.current()?.win.nodes.find((x) => x.id === selected);
    if (!n) return;
    selNode = n;
    if (n.label === '') void refine(n);
    else announceOnce();
  }

  function animateTo(target: Camera): void {
    activeTween?.cancel();
    activeTween = tween(cam, target, TWEEN_MS, (c) => setCam(c), reduced());
  }

  /** Where the camera starts: everyone when that is readable, otherwise the home person or the largest family. */
  function place(homeRect: Rect | null): void {
    if (!sized || !meta) return;
    const m = meta;
    const f = fit(m.meta.bounds, size, 24, minZoom());
    hint = '';
    if (f.zoom >= 0.25) cam = f;
    else if (homeRect) cam = centerOn({ x: 0, y: 0, zoom: 0.7 }, homeRect, size);
    else {
      const first = m.blocks[0]?.rect;
      cam = { zoom: 0.7, x: (first?.x ?? 0) - 24 / 0.7, y: (first?.y ?? 0) - 24 / 0.7 };
    }
    if (f.zoom < 0.25) hint = plural('whole.zoomHint', m.meta.nodeCount);
    seen = viewRect(cam, size);
    zoomPct = Math.round(cam.zoom * 100);
    cache?.need(seen, cam.zoom);
    invalidate();
  }

  function applySize(): void {
    if (!canvas) return;
    dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(size.w * dpr);
    canvas.height = Math.round(size.h * dpr);
  }

  let homeRect: Rect | null = null;

  function resize(w: number, h: number): void {
    if (w <= 0 || h <= 0 || !canvas) return;
    size = { w, h };
    sized = true;
    applySize();
    if (!userMoved && meta) place(homeRect);
    else {
      seen = viewRect(cam, size);
      cache?.need(seen, cam.zoom);
      invalidate();
    }
  }

  function closeChart(): void {
    activeTween?.cancel();
    cache?.dispose();
    cache = null;
    meta = null;
    miniWin = null;
  }

  function isStale(e: unknown): boolean {
    return typeof e === 'object' && e !== null && (e as { code?: string }).code === 'stale';
  }

  async function open(keepView = false): Promise<void> {
    const seq = ++openSeq;
    // After an edit the view stays where it is and the selected person stays selected when still there.
    const keptPerson = keepView ? (selNode?.person?.id ?? null) : null;
    closeChart();
    errorText = '';
    selected = null;
    selNode = null;
    lineage = null;
    homeNode = null;
    homeRect = null;
    let m: WholeMeta;
    try {
      m = await api.chartOpenWhole();
    } catch (e) {
      if (seq === openSeq) errorText = e instanceof Error ? e.message : t('whole.openFailed');
      return;
    }
    if (seq !== openSeq) return;
    const id = m.meta.chartId;
    const c = new WindowCache(async (r, z) => {
      try {
        return await api.chartWindow(id, r, z);
      } catch (e) {
        if (isStale(e) && seq === openSeq) void open();
        throw e;
      }
    });
    cache = c;
    c.onChange(() => {
      if (selected !== null) {
        const n = c.current()?.win.nodes.find((x) => x.id === selected);
        if (n && (selNode?.id !== n.id || selNode.label === '')) {
          selNode = n;
          if (pendingAnnounce || n.label !== '') announceOnce();
        }
      }
      invalidate();
    });
    meta = m;
    if (homeId) {
      try {
        const loc = await api.chartLocate(id, homeId);
        if (seq !== openSeq) return;
        homeNode = loc?.nodeId ?? null;
        homeRect = loc?.rect ?? null;
      } catch (e) {
        if (isStale(e) && seq === openSeq) void open();
        return;
      }
    }
    if (keepView && userMoved) {
      cache?.need(seen, cam.zoom);
      invalidate();
      if (keptPerson) {
        try {
          const loc = await api.chartLocate(id, keptPerson);
          if (seq === openSeq && loc) {
            selected = loc.nodeId;
            adoptSelection();
          }
        } catch {
          // The selection is simply cleared.
        }
      }
    } else {
      userMoved = false;
      place(homeRect);
    }
    onOpened?.(m);
    // The minimap shows everyone at the lowest detail, fetched once per opened chart.
    try {
      const all = await api.chartWindow(id, { x: 0, y: 0, w: m.meta.bounds.w + 1, h: m.meta.bounds.h + 1 }, 0.01);
      if (seq === openSeq) miniWin = all;
    } catch {
      // The minimap stays empty; the chart still works.
    }
  }

  $effect(() => {
    void homeId;
    untrack(() => void open());
    return () => {
      openSeq++;
      closeChart();
    };
  });

  // Edits, undo and redo bump project.rev (and change the undo state); reopen so the chart shows the new tree.
  const undoState = () => `${project.rev}|${undo.canUndo}|${undo.undoLabel}|${undo.canRedo}|${undo.redoLabel}`;
  let undoKey = untrack(() => undoState());
  $effect(() => {
    const k = undoState();
    if (k !== undoKey) {
      undoKey = k;
      untrack(() => {
        if (meta) void open(true);
      });
    }
  });

  // The highlighted line follows the selection.
  $effect(() => {
    const on = lineageOn;
    const sel = selected;
    const m = meta;
    const seq = ++lineSeq;
    if (!on || sel === null || !m) {
      lineage = null;
      invalidate();
      return;
    }
    void api
      .chartLineage(m.meta.chartId, sel)
      .then((ids) => {
        if (seq !== lineSeq) return;
        lineage = new Set(ids);
        invalidate();
      })
      .catch((e) => {
        if (isStale(e) && seq === lineSeq) untrack(() => void open());
      });
  });

  $effect(() => {
    theme = null;
    invalidate();
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
      setCam(zoomAt(cam, factor, { x: e.clientX - r.left, y: e.clientY - r.top }, minZoom()));
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
  });

  $effect(() => () => {
    if (rafId) cancelAnimationFrame(rafId);
    rafId = 0;
    clearTimeout(findTimer);
  });

  // ------------------------------------------------------------ selection

  function announceOnce(): void {
    pendingAnnounce = false;
    announcement = selNode && selNode.label ? nodeLabel(selNode) : '';
  }

  /** Far out the window carries only the surname; ask for the real name and dates of the one selected card. */
  async function refine(n: ChartNode): Promise<void> {
    const m = meta;
    const seq = openSeq;
    if (!m || n.label !== '') return;
    try {
      const w = await api.chartWindow(m.meta.chartId, { x: n.x + 1, y: n.y + 1, w: 1, h: 1 }, 1);
      const full = w.nodes.find((x) => x.id === n.id);
      if (full && seq === openSeq && selected === n.id) {
        selNode = full;
        announceOnce();
      }
    } catch {
      // The selection keeps the short form.
    }
  }

  function select(n: ChartNode): void {
    selected = n.id;
    selNode = n;
    if (n.label === '') void refine(n);
    else announceOnce();
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

  /** Moves the camera so `rect` is in the middle at `zoom`, and selects node `id`. */
  function flyTo(id: number, rect: Rect, zoom: number): void {
    selected = id;
    selNode = null;
    pendingAnnounce = true;
    animateTo(centerOn({ ...cam, zoom }, rect, size));
    invalidate();
  }

  async function jumpToPerson(person: Id): Promise<void> {
    const m = meta;
    if (!m) return;
    try {
      const loc = await api.chartLocate(m.meta.chartId, person);
      if (!loc) {
        announcement = t('whole.notInTree');
        return;
      }
      flyTo(loc.nodeId, loc.rect, Math.max(cam.zoom, 0.8));
    } catch (e) {
      if (isStale(e)) void open();
    }
  }

  function goHome(): void {
    if (homeId) void jumpToPerson(homeId);
  }

  function actualSize(): void {
    const n = selNode ?? cache?.current()?.win.nodes.find((x) => x.id === selected);
    if (n) animateTo(centerOn({ ...cam, zoom: 1 }, n, size));
  }

  // ------------------------------------------------------------ search

  function onFind(q: string): void {
    searchText = q;
    clearTimeout(findTimer);
    const text = q.trim();
    if (!text) {
      findSeq++;
      hits = [];
      return;
    }
    findTimer = setTimeout(async () => {
      const mine = ++findSeq;
      try {
        const found = await api.search(text, 12);
        if (mine === findSeq) hits = found.filter((h) => h.kind === 'person' && h.person);
      } catch {
        if (mine === findSeq) hits = [];
      }
    }, 150);
  }

  function pickHit(h: SearchHitDto): void {
    searchText = h.label;
    if (h.person) void jumpToPerson(h.person.id);
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
        setCam(zoomAt(cam, d / pinch, { x: (a.x + b.x) / 2 - r.left, y: (a.y + b.y) / 2 - r.top }, minZoom()));
      }
      pinch = d;
      return;
    }
    if (!drag) return;
    if (!drag.moved && Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 4) return;
    drag.moved = true;
    activeTween?.cancel();
    setCam(panBy(cam, dx, dy, meta?.meta.bounds ?? { w: 1, h: 1 }, size));
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
    const n = hitTest(cur.win, w.x, w.y);
    if (n) select(n);
  }

  function ondblclick(e: MouseEvent): void {
    const cur = cache?.current();
    if (!cur) return;
    const w = toWorld(e);
    const n = hitTest(cur.win, w.x, w.y);
    if (n?.person) onOpenPerson(n.person.id);
  }

  // ------------------------------------------------------------ keyboard

  function dirFor(key: string): Dir | null {
    return key === 'ArrowDown' ? 'child' : key === 'ArrowUp' ? 'parent' : key === 'ArrowLeft' ? 'up' : key === 'ArrowRight' ? 'down' : null;
  }

  const PAN_STEP = 80;

  function panKey(key: string): void {
    const dx = key === 'ArrowLeft' ? PAN_STEP : key === 'ArrowRight' ? -PAN_STEP : 0;
    const dy = key === 'ArrowUp' ? PAN_STEP : key === 'ArrowDown' ? -PAN_STEP : 0;
    activeTween?.cancel();
    setCam(panBy(cam, dx, dy, meta?.meta.bounds ?? { w: 1, h: 1 }, size));
  }

  /** The next card to the left or right in the same row; reaches people (a married-in partner) that no sibling link leads to. */
  function rowStep(win: ChartWindow, from: number, dir: 1 | -1): ChartNode | null {
    const n = win.nodes.find((x) => x.id === from);
    if (!n) return null;
    let best: ChartNode | null = null;
    for (const o of win.nodes) {
      if (o.y !== n.y || o.id === n.id || (o.x - n.x) * dir <= 0) continue;
      if (!best || Math.abs(o.x - n.x) < Math.abs(best.x - n.x)) best = o;
    }
    return best;
  }

  function onkeydown(e: KeyboardEvent): void {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    const cur = cache?.current();
    const dir = dirFor(e.key);
    if (dir) {
      e.preventDefault();
      // Arrows pan the view; with a person selected (and no Shift) they move between relatives instead.
      if (e.shiftKey || selected === null || !cur) {
        panKey(e.key);
        return;
      }
      const id = neighbor(cur.win, selected, dir);
      let n = id === null ? null : cur.win.nodes.find((x) => x.id === id);
      if (!n && (dir === 'up' || dir === 'down')) n = rowStep(cur.win, selected, dir === 'down' ? 1 : -1);
      if (n) {
        select(n);
        ensureVisible(n);
      } else if (id !== null) {
        // A relative far outside the loaded window: the minimap's window knows where every card is.
        const far = miniWin?.nodes.find((x) => x.id === id);
        if (far) flyTo(id, far, cam.zoom);
      }
    } else if (e.key === 'Escape') {
      if (selected !== null) {
        e.preventDefault();
        selected = null;
        selNode = null;
        announcement = t('whole.selectionCleared');
        invalidate();
      }
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (selNode?.person) onOpenPerson(selNode.person.id);
    } else if (e.key === 'p' || e.key === 'P') {
      if (selNode?.person) onOpenInPedigree(selNode.person.id);
    } else if (e.key === 'l' || e.key === 'L') {
      lineageOn = !lineageOn;
    } else if (e.key === 'Home') {
      e.preventDefault();
      goHome();
    } else if (e.key === '1') actualSize();
    else if (e.key === '+' || e.key === '=') zoomBy(1.25);
    else if (e.key === '-') zoomBy(0.8);
    else if (e.key === '0' || e.key === 'f' || e.key === 'F') fitAll();
  }

  function zoomBy(f: number): void {
    activeTween?.cancel();
    setCam(zoomAt(cam, f, { x: size.w / 2, y: size.h / 2 }, minZoom()));
  }

  function fitAll(): void {
    if (meta) animateTo(fit(meta.meta.bounds, size, 24, minZoom()));
  }

  function fromMinimap(x: number, y: number): void {
    activeTween?.cancel();
    setCam({ zoom: cam.zoom, x: x - size.w / cam.zoom / 2, y: y - size.h / cam.zoom / 2 });
  }
</script>

<div class="chart">
  <div class="toolbar" role="toolbar" aria-label={t('whole.tools')}>
    <div class="find">
      <Combobox
        label={t('whole.find')}
        items={hits}
        value={null}
        text={searchText}
        onText={onFind}
        onPick={pickHit}
        render={(h) => h.label}
        detail={(h) => h.snippet ?? undefined}
      />
    </div>
    <Button onclick={() => zoomBy(1.25)}>{t('chartCanvas.zoomIn')}</Button>
    <Button onclick={() => zoomBy(0.8)}>{t('chartCanvas.zoomOut')}</Button>
    <Button onclick={fitAll}>{t('whole.fitAll')}</Button>
    <Button disabled={selected === null} onclick={actualSize}>{t('whole.actualSize')}</Button>
    <IconButton label={t('chartCanvas.goHome')} icon="home" disabled={homeNode === null} onclick={goHome} />
    <Button aria-pressed={lineageOn} onclick={() => (lineageOn = !lineageOn)}>{t('whole.lineage')}</Button>
    <Button disabled={!selNode?.person} onclick={() => selNode?.person && onOpenPerson(selNode.person.id)}>{t('whole.openPerson')}</Button>
    <Button disabled={!selNode?.person} onclick={() => selNode?.person && onOpenInPedigree(selNode.person.id)}>{t('whole.showInPedigree')}</Button>
    <span class="zoom" aria-live="off">{zoomPct}%</span>
  </div>
  <div class="stage" bind:this={box}>
    <!-- svelte-ignore a11y_no_interactive_element_to_noninteractive_role -->
    <canvas
      bind:this={canvas}
      role="application"
      aria-label={t('whole.chart')}
      aria-describedby="{uid}-help"
      aria-activedescendant={selected !== null ? `${uid}-sel` : undefined}
      tabindex="0"
      {onpointerdown}
      {onpointermove}
      {onpointerup}
      onpointercancel={onpointerup}
      {ondblclick}
      {onkeydown}
    ></canvas>
    <div class="sr-only" role="listbox" aria-label={t('chartCanvas.selectedPerson')}>
      {#if selNode}
        <div id="{uid}-sel" role="option" aria-selected="true">{selNode.label ? nodeLabel(selNode) : selNode.person?.name}</div>
      {/if}
    </div>
    {#if hint}
      <p class="hint">{hint}</p>
    {/if}
    {#if meta}
      <div class="mini">
        <Minimap bounds={meta.meta.bounds} win={miniWin} view={seen} onJump={fromMinimap} />
      </div>
    {/if}
    <p id="{uid}-help" class="sr-only">
      {t('whole.help')}
    </p>
    <div class="sr-only" role="status" aria-live="polite">{announcement}</div>
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
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--s-3);
    padding-bottom: var(--s-3);
  }
  .find {
    min-width: 16rem;
  }
  .zoom {
    margin-inline-start: auto;
    color: var(--ink-2);
    font-size: var(--t-sm);
    align-self: center;
  }
  :global(.toolbar .btn[aria-pressed='true']) {
    background: var(--ribbon-wash);
    border-color: var(--ribbon);
  }
  .hint {
    position: absolute;
    top: var(--s-2);
    left: var(--s-2);
    margin: 0;
    padding: var(--s-1) var(--s-3);
    background: var(--paper-1);
    border: 1px solid var(--rule);
    border-radius: 999px;
    font-size: 0.875rem;
    color: var(--ink-2);
    pointer-events: none;
  }
  .mini {
    position: absolute;
    right: var(--s-3);
    bottom: var(--s-3);
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
