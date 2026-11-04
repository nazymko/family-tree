<script lang="ts">
  import { untrack } from 'svelte';
  import { getApi } from '../api/context';
  import type { ChartKind, ChartMeta, ChartNode, ChartRequest, ChartWindow, Id, Rect } from '../api/types';
  import { project } from '../state/project.svelte';
  import { undoState as undo } from '../state/undo.svelte';
  import Button from '../ui/Button.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
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
    auntsUncles = true,
    onAunts,
    onOpened,
    onSelect,
    selectId = null,
    hidden = false,
  }: {
    kind: ChartKind;
    root: Id;
    generations: number;
    /** Called with the person to centre on and, on the relatives chart, how they are related to the current centre. */
    onRecentre: (id: Id, relation?: string) => void;
    onOpenPerson: (id: Id) => void;
    /** Shows the generations Select in the toolbar when given. */
    onGenerations?: (n: number) => void;
    /** Relatives chart: whether the parents' brothers and sisters are shown. */
    auntsUncles?: boolean;
    /** Shows the "Show aunts and uncles" checkbox in the toolbar when given. */
    onAunts?: (show: boolean) => void;
    /** Called with the chart meta each time a chart is (re)opened. */
    onOpened?: (meta: ChartMeta) => void;
    /** Called whenever the selected node changes (also from the keyboard). */
    onSelect?: (nodeId: number) => void;
    /** Selection driven from outside (the outline); the canvas scrolls to it. */
    selectId?: number | null;
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
  // The camera is only framed once a real size is known and until the user moves it.
  let sized = false;
  let userMoved = false;
  let childrenRect: Rect | null = null;
  let childCount = 0;
  let hint = $state('');
  let rootRect: Rect | null = null;
  let dpr = 1;
  let dirty = false;
  let rafId = 0;
  let openSeq = 0;
  let activeTween: { cancel(): void } | null = null;
  let theme: Theme | null = null;
  let rootNodeId = 0;
  let pendingAnnounce = false;
  // Relatives chart: the person the previous chart was centred on, to glide from when re-centring.
  let lastRoot: Id | null = null;

  const reduced = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const kindLabel = $derived(kind === 'pedigree' ? 'Pedigree' : kind === 'relatives' ? 'Relatives' : 'Descendant');

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
    userMoved = true;
    hint = '';
    cam = c;
    cache?.need(viewRect(cam, size), cam.zoom);
    invalidate();
  }

  function animateTo(target: Camera): void {
    activeTween?.cancel();
    activeTween = tween(cam, target, TWEEN_MS, setCam, reduced());
  }

  /** Initial framing: pedigree root at the left, descendants root couple centred one row below the top. */
  function place(): void {
    if (!sized || !meta || !rootRect) return;
    const f = fit(meta.bounds, size);
    hint = '';
    if (kind === 'descendants' && childrenRect) {
      // Couple plus its children row, never below the card LOD.
      const u: Rect = {
        x: Math.min(rootRect.x, childrenRect.x),
        y: rootRect.y,
        w: Math.max(rootRect.x + rootRect.w, childrenRect.x + childrenRect.w) - Math.min(rootRect.x, childrenRect.x),
        h: childrenRect.y + childrenRect.h - rootRect.y,
      };
      const m = 24;
      const z = Math.min(1, (size.w - 2 * m) / u.w, (size.h - 2 * m) / u.h);
      if (z >= 0.55) cam = { zoom: z, x: u.x + u.w / 2 - size.w / z / 2, y: u.y + u.h / 2 - size.h / z / 2 };
      else {
        cam = { zoom: 0.55, x: rootRect.x + rootRect.w / 2 - size.w / 0.55 / 2, y: rootRect.y - 40 / 0.55 };
        hint = `${childCount} ${childCount === 1 ? 'child' : 'children'}, wider than the window: drag or press F to fit all`;
      }
    } else if (kind === 'relatives') {
      // Whole hourglass when it fits at a readable size, otherwise centred on the starting person.
      if (f.zoom >= 0.55) cam = f;
      else {
        const z = 0.75;
        cam = { zoom: z, x: rootRect.x + rootRect.w / 2 - size.w / z / 2, y: rootRect.y + rootRect.h / 2 - size.h / z / 2 };
      }
    } else if (f.zoom >= 0.55 && kind === 'pedigree') cam = f;
    else if (kind === 'descendants' && f.zoom >= 0.55) cam = f;
    else if (kind === 'pedigree') cam = { zoom: 1, x: rootRect.x - 40, y: rootRect.y + rootRect.h / 2 - size.h / 2 };
    else cam = { zoom: 1, x: rootRect.x + rootRect.w / 2 - size.w / 2, y: rootRect.y - rootRect.h };
    cache?.need(viewRect(cam, size), cam.zoom);
    invalidate();
  }

  function applySize(): void {
    if (!canvas) return;
    dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(size.w * dpr);
    canvas.height = Math.round(size.h * dpr);
  }

  function resize(w: number, h: number): void {
    if (w <= 0 || h <= 0 || !canvas) return;
    size = { w, h };
    sized = true;
    applySize();
    if (!userMoved && rootRect) place();
    else {
      cache?.need(viewRect(cam, size), cam.zoom);
      invalidate();
    }
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
    rootRect = null;
    selected = null;
    selNode = null;
    let m: ChartMeta;
    try {
      const req: ChartRequest = kind === 'relatives' ? { kind, root, generations, auntsUncles } : { kind, root, generations };
      m = await api.chartOpen(req);
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
    // Relatives: the starting person can be anywhere, so ask for the whole chart once.
    const probe: Rect =
      kind === 'relatives'
        ? { x: 0, y: 0, w: Math.max(1, m.bounds.w), h: Math.max(1, m.bounds.h) }
        : kind === 'pedigree'
          ? { x: -1, y: 0, w: 178, h: Math.max(1, m.bounds.h) }
          : { x: 0, y: -1, w: Math.max(1, m.bounds.w), h: 58 };
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
    if (rn) onSelect?.(rn.id);
    // Descendants: the root couple is the root card plus its partners (same row, same depth).
    const couple = kind === 'descendants' && rn ? first.nodes.filter((n) => n.y === rn.y) : rn ? [rn] : [];
    if (couple.length) {
      const x0 = Math.min(...couple.map((n) => n.x));
      const x1 = Math.max(...couple.map((n) => n.x + n.w));
      rootRect = { x: x0, y: rn!.y, w: x1 - x0, h: rn!.h };
    } else rootRect = null;
    childrenRect = null;
    childCount = 0;
    if (kind === 'descendants' && rn && rootRect) {
      try {
        const band = await api.chartWindow(m.chartId, { x: -1, y: rn.y + rn.h + 1, w: Math.max(1, m.bounds.w + 2), h: 400 }, 1);
        if (seq !== openSeq) return;
        const ys = band.nodes.map((n) => n.y);
        const rowY = ys.length ? Math.min(...ys) : null;
        const row = band.nodes.filter((n) => n.y === rowY);
        if (row.length) {
          const x0 = Math.min(...row.map((n) => n.x));
          childrenRect = { x: x0, y: row[0].y, w: Math.max(...row.map((n) => n.x + n.w)) - x0, h: row[0].h };
          childCount = row.length;
        }
      } catch (e) {
        if (isStale(e) && seq === openSeq) void open();
        return;
      }
    }
    userMoved = false;
    pendingAnnounce = true;
    place();
    // Re-centred on another person: start from where the previous centre is in the new chart and glide to the new one.
    const before = lastRoot;
    lastRoot = root;
    if (kind === 'relatives' && before && before !== root && sized) {
      const old = first.nodes.find((n) => n.person?.id === before);
      if (old) {
        const target = cam;
        cam = centerOn(target, old, size);
        cache?.need(viewRect(cam, size), cam.zoom);
        animateTo(target);
      }
    }
    onOpened?.(m);
  }

  $effect(() => {
    void kind;
    void root;
    void generations;
    void auntsUncles;
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
        if (meta) void open();
      });
    }
  });

  // Selection made in the outline.
  $effect(() => {
    const id = selectId;
    untrack(() => {
      if (id === null || id === selected || !cache?.current()) return;
      const cur = cache.current()!;
      const n = cur.win.nodes.find((x) => x.id === id);
      if (n) {
        select(n);
        ensureVisible(n);
      } else if (selNode) goTo(id, selNode);
      else void revealFar(id);
    });
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
    // Moving the window to a screen with another pixel ratio changes the backing-store size.
    let dprMq: MediaQueryList | null = null;
    const onDpr = () => {
      applySize();
      invalidate();
      watchDpr();
    };
    function watchDpr() {
      dprMq?.removeEventListener?.('change', onDpr);
      dprMq = typeof matchMedia === 'function' ? matchMedia(`(resolution: ${window.devicePixelRatio || 1}dppx)`) : null;
      dprMq?.addEventListener?.('change', onDpr);
    }
    watchDpr();
    return () => {
      dprMq?.removeEventListener?.('change', onDpr);
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
    onSelect?.(n.id);
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
    if (!rect) {
      void revealFar(id);
      return;
    }
    selected = id;
    onSelect?.(id);
    pendingAnnounce = true;
    animateTo(centerOn(cam, rect, size));
    invalidate();
  }

  /** The node is neither cached nor linked by a cached edge (selected from the outline): ask for the whole chart once. */
  async function revealFar(id: number): Promise<void> {
    const m = meta;
    const seq = openSeq;
    if (!m) return;
    try {
      const all = await api.chartWindow(m.chartId, { x: 0, y: 0, w: m.bounds.w, h: m.bounds.h }, 0.2);
      const n = all.nodes.find((x) => x.id === id);
      if (!n || seq !== openSeq) return;
      selected = id;
      selNode = n;
      onSelect?.(id);
      pendingAnnounce = true;
      animateTo(centerOn(cam, n, size));
    } catch {
      // The selection stays where it was.
    }
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
      onSelect?.(n.dupOf);
      pendingAnnounce = true;
      animateTo(centerOn(cam, rect, size));
      invalidate();
    }
  }

  function recentre(n: ChartNode): void {
    if (!n.person) return;
    animateTo(centerOn(cam, n, size));
    if (n.relation) onRecentre(n.person.id, n.relation);
    else onRecentre(n.person.id);
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
    const handle = hitHandle(cur.win, kind, w.x, w.y, cam.zoom);
    if (handle?.person) {
      if (handle.relation) onRecentre(handle.person.id, handle.relation);
      else onRecentre(handle.person.id);
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
    // Relatives: up to a parent, down to a child, left and right along the row (siblings and partners).
    if (kind === 'relatives') {
      return key === 'ArrowUp' ? 'parent' : key === 'ArrowDown' ? 'child' : key === 'ArrowLeft' ? 'up' : key === 'ArrowRight' ? 'down' : null;
    }
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
    else if (e.key === '0' || e.key === 'f' || e.key === 'F') fitAll();
  }

  function zoomBy(f: number): void {
    activeTween?.cancel();
    setCam(zoomAt(cam, f, { x: size.w / 2, y: size.h / 2 }));
  }

  function fitAll(): void {
    if (meta) animateTo(fit(meta.bounds, size));
  }

  const genRange = $derived(kind === 'relatives' ? { min: 1, max: 6 } : { min: 3, max: 8 });
  const genOptions = $derived(
    Array.from({ length: genRange.max - genRange.min + 1 }, (_, i) => genRange.min + i).map((n) => ({
      value: String(n),
      label: `${n} ${n === 1 ? 'generation' : 'generations'}`,
    })),
  );
  let genValue = $derived(String(Math.min(genRange.max, Math.max(genRange.min, generations))));
</script>

<div class="chart" {hidden}>
  <div class="toolbar" role="toolbar" aria-label="{kindLabel} chart tools">
    <Button onclick={() => zoomBy(1.25)}>Zoom in</Button>
    <Button onclick={() => zoomBy(0.8)}>Zoom out</Button>
    <Button onclick={fitAll}>Fit</Button>
    {#if kind === 'relatives'}
      <Button disabled={!homeId} onclick={() => homeId && onRecentre(homeId)}>Centre on me</Button>
    {:else}
      <IconButton label="Go to home person" icon="home" disabled={!homeId} onclick={() => homeId && onRecentre(homeId)} />
    {/if}
    {#if onAunts}
      <Checkbox label="Show aunts and uncles" checked={auntsUncles} onchange={onAunts} />
    {/if}
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
      aria-label="{kindLabel} chart"
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
    <div class="sr-only" role="listbox" aria-label="Selected person">
      {#if selNode}
        <div id="{uid}-sel" role="option" aria-selected="true">{selNode.label}</div>
      {/if}
    </div>
    {#if hint}
      <p class="hint">{hint}</p>
    {/if}
    <p id="{uid}-help" class="sr-only">Use arrow keys to move between people, Enter to centre, E to edit.</p>
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
  .hint {
    position: absolute;
    top: var(--s-2);
    left: var(--s-2);
    margin: 0;
    padding: var(--s-1) var(--s-3);
    background: var(--paper-1, var(--paper-0));
    border: 1px solid var(--rule);
    border-radius: 999px;
    font-size: 0.875rem;
    color: var(--ink-2);
    pointer-events: none;
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
