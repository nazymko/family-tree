<script lang="ts">
  import { untrack } from 'svelte';
  import { getApi, getPlatform } from '../api/context';
  import type { ChartEdge, ChartKind, ChartMeta, ChartNode, ChartRequest, ChartWindow, Id, Rect } from '../api/types';
  import { added } from '../state/added.svelte';
  import { project } from '../state/project.svelte';
  import { undoState as undo } from '../state/undo.svelte';
  import Button from '../ui/Button.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import FlipTreeButton from '../ui/FlipTreeButton.svelte';
  import IconButton from '../ui/IconButton.svelte';
  import Select from '../ui/Select.svelte';
  import { centerOn, fit, panBy, tween, viewRect, zoomAt, type Camera } from './camera';
  import { hitHandle, hitTest, neighbor, type Dir } from './hit';
  import { draw, themeFromCss, type Theme } from './renderer';
  import { WindowCache } from './window';
  import AddRelativeMenu from './AddRelativeMenu.svelte';
  import ChartPictureMenu from './ChartPictureMenu.svelte';
  import { isKeyboardMenu, mergeScenes, planPicture, sceneFor, tileRects, type PictureFormat, type PictureScope, type Scene } from './picture';
  import { captionText, later, savePicture, suggestedName, type BuiltPicture } from './pictureExport';
  import { t, plural } from '../i18n';
  import { nodeLabel, relationText } from '../i18n/relation';
  import { agentChanges } from '../state/agentChanges.svelte';

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
    reversed = false,
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
    /** The tree grows the other way: ancestors at the bottom (pedigree: on the left). */
    reversed?: boolean;
  } = $props();

  const api = getApi();
  const platform = getPlatform();
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
  let pictureMenu = $state<{ openAt(x: number, y: number, person?: Id | null): void }>();
  let savingPicture = $state(false);
  let rootName = '';

  let cache: WindowCache | null = null;
  let cam: Camera = { x: 0, y: 0, zoom: 1 };
  let size = { w: 800, h: 600 };
  // The camera is only framed once a real size is known and until the user moves it.
  let sized = false;
  let userMoved = false;
  let childrenRect: Rect | null = null;
  /** Relatives: the centred person with partners, parents and children, for the first view. */
  let frameRect: Rect | null = null;
  let parentsRect: Rect | null = null;
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
  const chartLabel = $derived(
    kind === 'pedigree' ? t('chartCanvas.pedigreeChart') : kind === 'relatives' ? t('chartCanvas.relativesChart') : t('chartCanvas.descendantChart'),
  );
  const toolsLabel = $derived(
    kind === 'pedigree' ? t('chartCanvas.pedigreeTools') : kind === 'relatives' ? t('chartCanvas.relativesTools') : t('chartCanvas.descendantTools'),
  );

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
    draw(ctx, cur.win, cam, { w: size.w, h: size.h, dpr }, { selected, focus: rootNodeId, theme, kind, reversed, badged: agentChanges.unreviewed });
  }

  $effect(() => {
    void agentChanges.unreviewed;
    invalidate();
  });

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
      const top = Math.min(rootRect.y, childrenRect.y);
      const u: Rect = {
        x: Math.min(rootRect.x, childrenRect.x),
        y: top,
        w: Math.max(rootRect.x + rootRect.w, childrenRect.x + childrenRect.w) - Math.min(rootRect.x, childrenRect.x),
        h: Math.max(rootRect.y + rootRect.h, childrenRect.y + childrenRect.h) - top,
      };
      const m = 24;
      const z = Math.min(1, (size.w - 2 * m) / u.w, (size.h - 2 * m) / u.h);
      if (z >= 0.55) cam = { zoom: z, x: u.x + u.w / 2 - size.w / z / 2, y: u.y + u.h / 2 - size.h / z / 2 };
      else {
        const y = reversed ? rootRect.y + rootRect.h + 40 / 0.55 - size.h / 0.55 : rootRect.y - 40 / 0.55;
        cam = { zoom: 0.55, x: rootRect.x + rootRect.w / 2 - size.w / 0.55 / 2, y };
        hint = plural('chartCanvas.wide', childCount);
      }
    } else if (kind === 'relatives') {
      // The centred person with parents and children, fully visible and centred at a readable zoom (about 1:1).
      // If that is too wide, keep the parents and at least the person; if even that is wider than the
      // window at a readable zoom, start at the readable zoom, centred on the person.
      const m = 32;
      const tryFrame = (u: Rect | null): Camera | null => {
        if (!u) return null;
        const z = Math.min(1, (size.w - 2 * m) / u.w, (size.h - 2 * m) / u.h);
        return z >= 0.55 ? { zoom: z, x: u.x + u.w / 2 - size.w / z / 2, y: u.y + u.h / 2 - size.h / z / 2 } : null;
      };
      const union = (a: Rect, b: Rect | null): Rect => {
        if (!b) return a;
        const x0 = Math.min(a.x, b.x);
        const y0 = Math.min(a.y, b.y);
        return { x: x0, y: y0, w: Math.max(a.x + a.w, b.x + b.w) - x0, h: Math.max(a.y + a.h, b.y + b.h) - y0 };
      };
      const c = tryFrame(frameRect) ?? tryFrame(union(rootRect, parentsRect)) ?? tryFrame(rootRect);
      if (c) cam = c;
      else {
        const z = 0.55;
        cam = { zoom: z, x: rootRect.x + rootRect.w / 2 - size.w / z / 2, y: rootRect.y + rootRect.h / 2 - size.h / z / 2 };
      }
    } else if (kind === 'pedigree' && (f.zoom >= 0.55 || generations >= ALL_GENERATIONS)) cam = f;
    else if (kind === 'descendants' && f.zoom >= 0.55) cam = f;
    else if (kind === 'pedigree') {
      const x = reversed ? rootRect.x + rootRect.w + 40 - size.w : rootRect.x - 40;
      cam = { zoom: 1, x, y: rootRect.y + rootRect.h / 2 - size.h / 2 };
    } else {
      const y = reversed ? rootRect.y + 2 * rootRect.h - size.h : rootRect.y - rootRect.h;
      cam = { zoom: 1, x: rootRect.x + rootRect.w / 2 - size.w / 2, y };
    }
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

  /** `keepView`: reopened after an edit. The camera stays where it was and the selected person stays selected. */
  async function open(keepView = false): Promise<void> {
    const seq = ++openSeq;
    const keptPerson = keepView ? (selNode?.person?.id ?? null) : null;
    const keepCam = keepView && sized && rootRect !== null;
    closeChart();
    errorText = '';
    rootRect = null;
    selected = null;
    // After an edit the selected person stays in the toolbar while the chart reloads (the "Add relative" button keeps focus).
    if (!keepCam) selNode = null;
    let m: ChartMeta;
    try {
      const req: ChartRequest = kind === 'relatives' ? { kind, root, generations, auntsUncles } : { kind, root, generations };
      m = await api.chartOpen(req);
    } catch (e) {
      if (seq === openSeq) errorText = e instanceof Error ? e.message : t('chartCanvas.openFailed');
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
          ? { x: reversed ? m.bounds.w - 177 : -1, y: 0, w: 178, h: Math.max(1, m.bounds.h) }
          : { x: 0, y: reversed ? m.bounds.h - 57 : -1, w: Math.max(1, m.bounds.w), h: 58 };
    let first: ChartWindow;
    try {
      first = await api.chartWindow(m.chartId, probe, 1);
    } catch (e) {
      if (isStale(e) && seq === openSeq) void open();
      return;
    }
    if (seq !== openSeq) return;
    const rn = first.nodes.find((n) => n.id === m.rootNode) ?? first.nodes[0] ?? null;
    const restoring = keepCam && keptPerson !== null && keptPerson !== rn?.person?.id;
    if (!restoring) {
      selected = rn?.id ?? null;
      selNode = rn;
      if (rn) onSelect?.(rn.id);
    }
    rootName = rn?.person?.name ?? '';
    // Descendants: the root couple is the root card plus its partners (same row, same depth).
    const couple = kind === 'descendants' && rn ? first.nodes.filter((n) => n.y === rn.y) : rn ? [rn] : [];
    if (couple.length) {
      const x0 = Math.min(...couple.map((n) => n.x));
      const x1 = Math.max(...couple.map((n) => n.x + n.w));
      rootRect = { x: x0, y: rn!.y, w: x1 - x0, h: rn!.h };
    } else rootRect = null;
    childrenRect = null;
    childCount = 0;
    frameRect = null;
    parentsRect = null;
    if (kind === 'relatives' && rn && rootRect) {
      const byId = new Map(first.nodes.map((n) => [n.id, n]));
      const box = (ns: ChartNode[], extra: Rect | null): Rect | null => {
        if (!ns.length) return extra;
        const x0 = Math.min(...ns.map((n) => n.x), extra?.x ?? Infinity);
        const y0 = Math.min(...ns.map((n) => n.y), extra?.y ?? Infinity);
        const x1 = Math.max(...ns.map((n) => n.x + n.w), extra ? extra.x + extra.w : -Infinity);
        const y1 = Math.max(...ns.map((n) => n.y + n.h), extra ? extra.y + extra.h : -Infinity);
        return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
      };
      const of = (pred: (e: ChartEdge) => boolean, pick: (e: ChartEdge) => number): ChartNode[] =>
        first.edges.filter(pred).map((e) => byId.get(pick(e))).filter((n): n is ChartNode => !!n);
      const partners = of((e) => e.kind === 'partner' && e.from === rn.id, (e) => e.to);
      const parents = of((e) => e.kind === 'parent' && e.to === rn.id, (e) => e.from);
      const kids = of((e) => e.kind === 'parent' && e.from === rn.id, (e) => e.to);
      rootRect = box(partners, rootRect) ?? rootRect;
      parentsRect = box(parents, null);
      frameRect = box(kids, box(parents, rootRect));
    }
    if (kind === 'descendants' && rn && rootRect) {
      try {
        // The children's row is the nearest one below the root (above it when the tree is reversed).
        const bandY = reversed ? rn.y - 1 - 400 : rn.y + rn.h + 1;
        const band = await api.chartWindow(m.chartId, { x: -1, y: bandY, w: Math.max(1, m.bounds.w + 2), h: 400 }, 1);
        if (seq !== openSeq) return;
        const ys = band.nodes.map((n) => n.y);
        const rowY = ys.length ? (reversed ? Math.max(...ys) : Math.min(...ys)) : null;
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
    if (keepCam) {
      userMoved = true;
      cache?.need(viewRect(cam, size), cam.zoom);
      invalidate();
    } else {
      userMoved = false;
      place();
    }
    pendingAnnounce = true;
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
    if (keptPerson && !pendingAdd && keptPerson !== rn?.person?.id && !(await selectPerson(keptPerson, seq)) && seq === openSeq) {
      // The person left the chart: fall back to the centre.
      selected = rn?.id ?? null;
      selNode = rn;
    }
    const keepAfterFlip = flipKeep;
    flipKeep = null;
    if (keepAfterFlip && keepAfterFlip !== rn?.person?.id && seq === openSeq && (await selectPerson(keepAfterFlip, seq)) && seq === openSeq) {
      const n = selNode;
      if (n) {
        cam = centerOn(cam, n, size);
        cache?.need(viewRect(cam, size), cam.zoom);
        invalidate();
      }
    }
    if (seq === openSeq) {
      await trySelectAdded();
      // A person the chart does not show is not looked for again after later edits.
      pendingAdd = null;
    }
  }

  /** Selects the box of a person, wherever it is in the chart. The camera does not move. False when the chart does not show them. */
  async function selectPerson(personId: Id, seq: number): Promise<boolean> {
    const m = meta;
    if (!m) return false;
    const shown = (w: ChartWindow) => w.nodes.find((x) => x.person?.id === personId && x.dupOf === null);
    let n = cache?.current() ? shown(cache.current()!.win) : undefined;
    if (!n) {
      try {
        n = shown(await api.chartWindow(m.chartId, { x: 0, y: 0, w: m.bounds.w, h: m.bounds.h }, 0.2));
      } catch {
        return false;
      }
    }
    if (!n || seq !== openSeq) return false;
    selected = n.id;
    selNode = n;
    onSelect?.(n.id);
    pendingAnnounce = true;
    invalidate();
    return true;
  }

  // A person added from a dialog: select them once the chart shows them. The edit and this note can arrive in either order.
  let pendingAdd: { id: Id; at: number } | null = null;
  let seenAdd = untrack(() => added.n);
  async function trySelectAdded(): Promise<void> {
    const p = pendingAdd;
    if (!p) return;
    if (Date.now() - p.at > 5000) {
      pendingAdd = null;
      return;
    }
    if (await selectPerson(p.id, openSeq)) {
      if (pendingAdd === p) pendingAdd = null;
    }
  }
  $effect(() => {
    const n = added.n;
    untrack(() => {
      if (n === seenAdd || !added.id) return;
      seenAdd = n;
      pendingAdd = { id: added.id, at: added.at };
      void trySelectAdded();
    });
  });

  // A flip re-lays the chart out: the selected person stays selected and is brought back into view.
  let lastReversed = untrack(() => reversed);
  let flipKeep: Id | null = null;
  $effect(() => {
    void kind;
    void root;
    void generations;
    void auntsUncles;
    const flipped = reversed !== lastReversed;
    lastReversed = reversed;
    untrack(() => {
      flipKeep = flipped ? (selNode?.person?.id ?? null) : null;
      void open();
    });
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
    return i < 0 ? '' : t('chartCanvas.position', { i: i + 1, n: same.length });
  }

  function announce(): void {
    announcement = selNode ? `${agentChanges.spoken(nodeLabel(selNode), selNode.person?.id)}${counts()}` : '';
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
      if (kind === 'pedigree') return { x: reversed ? sx : sx - ref.w, y: sy - ref.h / 2, w: ref.w, h: ref.h };
      return { x: sx - ref.w / 2, y: reversed ? sy : sy - ref.h, w: ref.w, h: ref.h };
    }
    const px = e.pts[e.pts.length - 2];
    const py = e.pts[e.pts.length - 1];
    if (e.kind === 'dup') return { x: reversed && kind === 'pedigree' ? px : px - ref.w, y: py - ref.h / 2, w: ref.w, h: ref.h };
    if (kind === 'pedigree') return { x: reversed ? px - ref.w : px, y: py - ref.h / 2, w: ref.w, h: ref.h };
    return { x: px - ref.w / 2, y: reversed ? py - ref.h : py, w: ref.w, h: ref.h };
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
    if (n.relation) onRecentre(n.person.id, relationText(n));
    else onRecentre(n.person.id);
  }

  // ------------------------------------------------------------ pointer

  // Non-reactive bookkeeping for gestures.
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  const pointers = new Map<number, { x: number; y: number }>();
  let drag: { x: number; y: number; moved: boolean } | null = null;
  let pinch = 0;

  function onpointerdown(e: PointerEvent): void {
    if (e.button === 2) return; // the context menu handles the right button
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
    const handle = hitHandle(cur.win, kind, w.x, w.y, cam.zoom, reversed);
    if (handle?.person) {
      if (handle.relation) onRecentre(handle.person.id, relationText(handle));
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

  // ------------------------------------------------------------ pictures

  /** Every node and line of the open chart, fetched in tiles at 100% so names and dates are full. */
  async function wholeScene(): Promise<Scene> {
    const m = meta;
    if (!m) return { nodes: [], edges: [] };
    const parts: Scene[] = [];
    for (const r of tileRects({ x: -1, y: -1, w: m.bounds.w + 2, h: m.bounds.h + 2 })) {
      parts.push(await api.chartWindow(m.chartId, r, 1));
      await later();
    }
    return mergeScenes(parts);
  }

  async function buildPicture(scope: PictureScope, format: PictureFormat): Promise<BuiltPicture | null> {
    if (!meta || !cache?.current()) return null;
    theme ??= themeFromCss();
    const th = theme;
    const sel = selected;
    const focus = rootNodeId;
    const shot = { cam: { ...cam }, size: { ...size }, dpr };
    // The screen picture asks for its own fresh window: the cached one can lag behind the camera (or be at another zoom level).
    const scene: Scene =
      scope === 'screen' ? await api.chartWindow(meta.chartId, viewRect(shot.cam, shot.size), shot.cam.zoom) : sceneFor(scope, kind, await wholeScene(), sel);
    if (scene.nodes.length === 0) return null;
    const plan = planPicture(scope, scene, scope === 'screen' ? shot : null, [], { dpr, vector: format === 'svg' });
    return {
      plan,
      theme: { paper: th.paper, ink2: th.ink2, font: th.font },
      caption: captionText(kind, rootName),
      paint: (ctx, p) =>
        draw(ctx, scene, p.cam, { w: p.view.w, h: p.view.h, dpr: p.scale }, { selected: scope === 'screen' ? sel : null, focus, theme: th, kind, reversed, handles: scope === 'screen' }),
    };
  }

  function savePictureAs(scope: PictureScope): void {
    if (savingPicture) return;
    const name = suggestedName(scope, kind, rootName, selNode?.person?.name ?? '');
    void savePicture({ api, platform }, { fileName: name, build: (format) => buildPicture(scope, format), setBusy: (b) => (savingPicture = b) });
  }

  /** Opens the picture menu over the selected box (or the middle of the chart when it is not in view). */
  function pictureMenuAtSelection(): void {
    const r = canvas?.getBoundingClientRect();
    if (!r) return;
    const n = selNode;
    const v = viewRect(cam, size);
    const inside = n && n.x < v.x + v.w && n.x + n.w > v.x && n.y < v.y + v.h && n.y + n.h > v.y;
    const x = inside ? r.left + (n.x + n.w / 2 - cam.x) * cam.zoom : r.left + r.width / 2;
    const y = inside ? r.top + (n.y + n.h / 2 - cam.y) * cam.zoom : r.top + r.height / 2;
    void pictureMenu?.openAt(x, y, selNode?.person?.id ?? null);
  }

  function oncontextmenu(e: MouseEvent): void {
    e.preventDefault();
    canvas?.focus();
    if (isKeyboardMenu(e as PointerEvent)) return pictureMenuAtSelection();
    const cur = cache?.current();
    let person: Id | null = null;
    if (cur) {
      // A right-click on a box selects that person first; a box shown elsewhere selects the original.
      const hit = hitTest(cur.win, toWorld(e).x, toWorld(e).y);
      const n = hit && hit.dupOf !== null ? (cur.win.nodes.find((x) => x.id === hit.dupOf) ?? hit) : hit;
      if (n) {
        select(n);
        person = n.person?.id ?? null;
      }
    }
    void pictureMenu?.openAt(e.clientX, e.clientY, person);
  }

  // ------------------------------------------------------------ keyboard

  function dirFor(key: string): Dir | null {
    // Relatives: up to a parent, down to a child, left and right along the row (siblings and partners).
    // A reversed tree swaps the two arrows along its growing axis, so an arrow still moves the way it points.
    const swap: Record<string, string> = { ArrowUp: 'ArrowDown', ArrowDown: 'ArrowUp', ArrowLeft: 'ArrowRight', ArrowRight: 'ArrowLeft' };
    const k = reversed ? (swap[key] ?? key) : key;
    if (kind === 'relatives') {
      return k === 'ArrowUp' ? 'parent' : k === 'ArrowDown' ? 'child' : key === 'ArrowLeft' ? 'up' : key === 'ArrowRight' ? 'down' : null;
    }
    if (kind === 'pedigree') {
      return k === 'ArrowRight' ? 'child' : k === 'ArrowLeft' ? 'parent' : key === 'ArrowUp' ? 'up' : key === 'ArrowDown' ? 'down' : null;
    }
    return k === 'ArrowDown' ? 'child' : k === 'ArrowUp' ? 'parent' : key === 'ArrowLeft' ? 'up' : key === 'ArrowRight' ? 'down' : null;
  }

  function onkeydown(e: KeyboardEvent): void {
    if (e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey)) {
      e.preventDefault();
      pictureMenuAtSelection();
      return;
    }
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

  /** "Show all" in the pedigree: the most the backend takes, far beyond any real family line. */
  const ALL_GENERATIONS = 255;
  const genRange = $derived(kind === 'relatives' ? { min: 1, max: 6 } : { min: 3, max: 8 });
  const genOptions = $derived(
    Array.from({ length: genRange.max - genRange.min + 1 }, (_, i) => genRange.min + i).map((n) => ({
      value: String(n),
      label: plural('chartCanvas.nGenerations', n),
    })).concat(kind === 'pedigree' ? [{ value: String(ALL_GENERATIONS), label: t('chartCanvas.showAll') }] : []),
  );
  let genValue = $derived(
    String(kind === 'pedigree' && generations >= ALL_GENERATIONS ? ALL_GENERATIONS : Math.min(genRange.max, Math.max(genRange.min, generations))),
  );
</script>

<div class="chart" {hidden}>
  <div class="toolbar" role="toolbar" aria-label={toolsLabel}>
    <Button onclick={() => zoomBy(1.25)}>{t('chartCanvas.zoomIn')}</Button>
    <Button onclick={() => zoomBy(0.8)}>{t('chartCanvas.zoomOut')}</Button>
    <Button onclick={fitAll}>{t('chartCanvas.fit')}</Button>
    <FlipTreeButton />
    {#if kind === 'relatives'}
      <Button disabled={!homeId} onclick={() => homeId && onRecentre(homeId)}>{t('chartCanvas.centreOnMe')}</Button>
    {:else}
      <IconButton label={t('chartCanvas.goHome')} icon="home" disabled={!homeId} onclick={() => homeId && onRecentre(homeId)} />
    {/if}
    {#if onAunts}
      <Checkbox label={t('chartCanvas.showAunts')} checked={auntsUncles} onchange={onAunts} />
    {/if}
    <ChartPictureMenu bind:this={pictureMenu} hasSelection={selected !== null} busy={savingPicture} onpick={savePictureAs} restoreFocus={() => canvas?.focus()} {onOpenPerson} />
    <AddRelativeMenu person={selNode?.person?.id ?? null} />
    {#if onGenerations}
      <div class="gens">
        <Select label={t('chartCanvas.generations')} options={genOptions} bind:value={genValue} onchange={(v) => onGenerations(Number(v))} />
      </div>
    {/if}
  </div>
  <div class="stage" bind:this={box}>
    <!-- svelte-ignore a11y_no_interactive_element_to_noninteractive_role -->
    <canvas
      bind:this={canvas}
      role="application"
      aria-label={chartLabel}
      aria-describedby="{uid}-help"
      aria-activedescendant={selected !== null ? `${uid}-sel` : undefined}
      tabindex="0"
      {onpointerdown}
      {onpointermove}
      {onpointerup}
      onpointercancel={onpointerup}
      {ondblclick}
      {oncontextmenu}
      {onkeydown}
    ></canvas>
    <div class="sr-only" role="listbox" aria-label={t('chartCanvas.selectedPerson')}>
      {#if selNode}
        <div id="{uid}-sel" role="option" aria-selected="true">{agentChanges.spoken(nodeLabel(selNode), selNode.person?.id)}</div>
      {/if}
    </div>
    {#if hint}
      <p class="hint">{hint}</p>
    {:else if meta?.truncated}
      <p class="hint" role="status">{t('chartCanvas.cutOff', { n: meta.nodeCount })}</p>
    {/if}
    {#if savingPicture}
      <p class="hint saving" role="status">{t('picture.saving')}</p>
    {/if}
    <p id="{uid}-help" class="sr-only">{t('chartCanvas.help')}</p>
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
  .saving {
    top: auto;
    bottom: var(--s-2);
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
