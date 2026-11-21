<script lang="ts">
  import { errMessage } from '../editors/errors';
  import { untrack } from 'svelte';
  import { getApi, getPlatform } from '../api/context';
  import { nodeKeys, refTagAt, screenFrames, type NodeKey, type PrintLayout } from './sheets';
  import { planSheets, type PlannedPage, type PlannedPiece, type SheetPlan } from './sheetPlan';
  import { tagDir, type DirScene } from './pieceView';
  import { CAPTION_MAX_MM, CAPTION_MM, CAPTION_PT } from './pack';
  import type { AroundKin, BranchDepth, OpenBranch, ChartEdge, ChartKind, ChartMeta, ChartNode, ChartRequest, ChartWindow, Id, Rect } from '../api/types';
  import { added } from '../state/added.svelte';
  import { project } from '../state/project.svelte';
  import { undoState as undo } from '../state/undo.svelte';
  import Button from '../ui/Button.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import DirectionControl from '../ui/DirectionControl.svelte';
  import { DEFAULT_DIRECTIONS, DIRECTIONS, arrowStep, camAtSide, edgeStrip, isHorizontal, opposite, rootSide, type ChartDirection } from './direction';
  import IconButton from '../ui/IconButton.svelte';
  import Menu, { type MenuItem } from '../ui/Menu.svelte';
  import { compact } from '../ui/compact.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import Select from '../ui/Select.svelte';
  import { centerOn, fit, panBy, tween, viewRect, zoomAt, type Camera } from './camera';
  import { hitHandle, hitTest, neighbor, type Dir } from './hit';
  import { draw, lightPrintTheme, printCardsNeed, themeFromCss, type SheetFrame, type Theme } from './renderer';
  import { SvgContext } from './svgContext';
  import { WindowCache } from './window';
  import AddRelativeMenu from './AddRelativeMenu.svelte';
  import ChartPictureMenu from './ChartPictureMenu.svelte';
  import { createDoubleTap, createLongPress, isTouchPointer } from '../ui/longPress';
  import { isKeyboardMenu, mergeScenes, planPicture, sceneFor, tileRects, type PictureFormat, type PictureScope, type Scene } from './picture';
  import { captionText, chartTitle, later, printPicture, printPictures, savePicture, savePictures, suggestedName, type BuiltPicture } from './pictureExport';
  import { A3_MIN_PT, A3_SUB_PT, A3_TITLE_PT, CARD_SMALL_PX, PT_PX, ellipsize, layoutA3, mmToPx, nameFloorPx, printDirection, printStretch, stretchScene, targetScale, type A3Background, type A3Choice, type A3DirectionMode, type A3Layout, type DirectionFit } from './a3';
  import { a3Background, a3Direction, a3Title } from './a3Pref.svelte';
  import { namesAsRecorded, oldTypeface } from '../ui/namesPref.svelte';
  import { loadOldTypeface } from '../ui/fonts';
  import { sceneBounds, pictureFileName } from './picture';
  import { settings } from '../state/settings.svelte';
  import { t, plural, localeStore, currentTag } from '../i18n';
  import { nodeLabel, relationText } from '../i18n/relation';
  import { agentChanges } from '../state/agentChanges.svelte';

  let {
    kind,
    root,
    roots,
    generations,
    onRecentre,
    onOpenPerson,
    onGenerations,
    auntsUncles = true,
    onAunts,
    kin = 'line',
    onKin,
    printLayout = 'today',
    onPrintLayout,
    openings = [],
    onOpenings,
    onOpened,
    onSelect,
    onBuildAround,
    onGone,
    selectId = null,
    hidden = false,
    direction,
  }: {
    kind: ChartKind;
    root: Id;
    /** The chart built around several people: all of them, `root` first (absent or one person: the ordinary chart). */
    roots?: Id[];
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
    /** The chart built around a person: who is shown besides the line. */
    kin?: AroundKin;
    /** Shows the "Relatives" choice (line only, brothers and sisters, all relatives) in the toolbar when given. */
    onKin?: (kin: AroundKin) => void;
    /** Chart built around one person: how it is laid out for the page (as today, compact, or sheets by branch). */
    printLayout?: PrintLayout;
    /** Shows the "Layout" choice in the toolbar when given. */
    onPrintLayout?: (v: PrintLayout) => void;
    /** Chart built around one person: the side branches opened per person. */
    openings?: OpenBranch[];
    /** Shows the controls to open the brothers and sisters, or the children, of the selected person when given. */
    onOpenings?: (open: OpenBranch[]) => void;
    /** Called with the chart meta each time a chart is (re)opened. */
    onOpened?: (meta: ChartMeta) => void;
    /** Called whenever the selected node changes (also from the keyboard). */
    onSelect?: (nodeId: number) => void;
    /** Chart built around a person: shows the "Build around this person" and "Open person" buttons; builds around the selected person, or around everybody marked with Shift+click. */
    onBuildAround?: (ids: Id[]) => void;
    /** Chart built around a person: the person is not in the project any more. */
    onGone?: () => void;
    /** Selection driven from outside (the outline); the canvas scrolls to it. */
    selectId?: number | null;
    /** Keeps the chart open but not drawn (the outline view is showing). */
    hidden?: boolean;
    /** The side the ancestors are drawn on (absent: what this kind of chart always had: the pedigree's on the right, the others' on top). */
    direction?: ChartDirection;
  } = $props();
  const dirNow = $derived(direction ?? DEFAULT_DIRECTIONS[kind]);

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
  /** The person behind a node that has a page: the unnamed-siblings marker has none. */
  const realPerson = (n: ChartNode | null) => (n?.person && !n.person.siblingMarker ? n.person : null);
  let announcement = $state('');
  let homeId = $state<Id | null>(null);
  let pictureMenu = $state<{ openAt(x: number, y: number, person?: Id | null): void }>();
  let savingPicture = $state(false);
  let rootName = '';
  /** People marked with Shift+click, in the order marked: the next chart is built around them. */
  let markedIds = $state<Id[]>([]);
  /** The chart is built around several people: they are marked on it and only the line is shown. */
  const many = $derived((roots?.length ?? 0) > 1);
  /** The boxes of the other chosen people of a chart built around several. */
  let chosenMore = new Set<number>();

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
  /** Chart built around a person: the person's own box and the children's boxes. */
  let centreRect: Rect | null = null;
  let kidsRect: Rect | null = null;
  let childCount = 0;
  let hint = $state('');
  let rootRect: Rect | null = null;
  let dpr = 1;
  let dirty = false;
  let rafId = 0;
  let openSeq = 0;
  let activeTween: { cancel(): void } | null = null;
  let theme: Theme | null = null;
  /** The A3 sheets' frames on the screen chart; only with the "sheets" layout of an around chart. */
  let sheetRects: Omit<SheetFrame, 'label'>[] = [];
  let rootNodeId = 0;
  let pendingAnnounce = false;
  // Relatives chart: the person the previous chart was centred on, to glide from when re-centring.
  let lastRoot: Id | null = null;

  const reduced = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  /** The charts that name people by how they are related to the centred person, with ancestors above and descendants below. */
  const related = $derived(kind === 'relatives' || kind === 'around');
  const chartLabel = $derived(
    kind === 'pedigree'
      ? t('chartCanvas.pedigreeChart')
      : kind === 'relatives'
        ? t('chartCanvas.relativesChart')
        : kind === 'around'
          ? t('chartCanvas.aroundChart')
          : t('chartCanvas.descendantChart'),
  );
  const toolsLabel = $derived(
    kind === 'pedigree'
      ? t('chartCanvas.pedigreeTools')
      : kind === 'relatives'
        ? t('chartCanvas.relativesTools')
        : kind === 'around'
          ? t('chartCanvas.aroundTools')
          : t('chartCanvas.descendantTools'),
  );

  // Switching "names as recorded" redraws the cards (and the next print or picture uses the same names).
  $effect(() => {
    void namesAsRecorded.value;
    untrack(() => invalidate());
  });

  // The old typeface of names as recorded: fetched before the cards are drawn with it, and the cards redraw once it is there.
  $effect(() => {
    if (!oldTypeface.value) {
      untrack(() => invalidate());
      return;
    }
    let live = true;
    void loadOldTypeface().then(() => live && invalidate());
    untrack(() => invalidate());
    return () => {
      live = false;
    };
  });

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
    draw(ctx, cur.win, cam, { w: size.w, h: size.h, dpr }, { selected, focus: rootNodeId, focusMore: chosenMore, marked: new Set(markedIds), theme, kind, dir: dirNow, badged: agentChanges.unreviewed, sheetFrames: sheetRects.map((r) => ({ ...r, label: t('picture.sheetFrame', { n: r.no }) })) });
  }

  $effect(() => {
    void agentChanges.unreviewed;
    void localeStore.locale; // words drawn on the canvas (a nameless person's name, relations) follow the language
    void markedIds;
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
        cam = camAtSide(rootRect, dirNow, 0.55, size, 40 / 0.55);
        hint = plural('chartCanvas.wide', childCount);
      }
    } else if (kind === 'around' && centreRect) {
      // The person exactly in the middle of the view, at the largest zoom (up to 100%) that still shows their parents and
      // children and their partners; never below a readable zoom.
      const c = centreRect;
      const cx = c.x + c.w / 2;
      const cy = c.y + c.h / 2;
      let halfW = c.w / 2;
      let halfH = c.h / 2;
      for (const r of [parentsRect, kidsRect, rootRect]) {
        if (!r) continue;
        halfW = Math.max(halfW, Math.abs(r.x - cx), Math.abs(r.x + r.w - cx));
        halfH = Math.max(halfH, Math.abs(r.y - cy), Math.abs(r.y + r.h - cy));
      }
      const m = 32;
      const z = Math.max(0.55, Math.min(1, (size.w / 2 - m) / halfW, (size.h / 2 - m) / halfH));
      cam = { zoom: z, x: cx - size.w / z / 2, y: cy - size.h / z / 2 };
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
      cam = camAtSide(rootRect, rootSide(kind, dirNow), 1, size, 40);
    } else {
      cam = camAtSide(rootRect, rootSide(kind, dirNow), 1, size, rootRect.h);
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

  /** How long the chart on screen waits before it lays out the print chart for its sheet frames. */
  const FRAMES_DELAY_MS = 250;
  let framesTimer: ReturnType<typeof setTimeout> | null = null;

  /**
   * The frames of the A3 sheets on the chart on screen, numbered as the print numbers them: the print charts are laid out
   * and planned by the same helper the print uses ({@link planA3Sheets}, for "Print on A3", which picks the orientation
   * itself), and each piece is framed around the screen boxes of its people. Runs a little after the chart opened (a
   * quick change of the layout or the person lays out one print chart, not one per step); nothing is framed until it
   * arrives, and a result for a chart that is no longer open is dropped.
   */
  function scheduleFrames(m: ChartMeta, seq: number): void {
    if (framesTimer) clearTimeout(framesTimer);
    framesTimer = setTimeout(() => {
      framesTimer = null;
      void frameSheets(m, seq);
    }, FRAMES_DELAY_MS);
  }

  async function frameSheets(m: ChartMeta, seq: number): Promise<void> {
    try {
      if (seq !== openSeq || printLayout !== 'sheets') return;
      const planned = await planA3Sheets('auto');
      if (!planned || seq !== openSeq || printLayout !== 'sheets') return;
      const scene = await wholeScene(m);
      if (seq !== openSeq) return;
      const keys = new Map(planned.dirs.map((d) => [d.direction, d.keys]));
      sheetRects = screenFrames(planned.plan, (id, d) => keys.get(d)?.get(id), scene);
      invalidate();
    } catch {
      // The frames are a hint only: the chart stays usable without them.
    }
  }

  function closeChart(): void {
    if (framesTimer) clearTimeout(framesTimer);
    framesTimer = null;
    activeTween?.cancel();
    cache?.dispose();
    cache = null;
    if (meta) void api.chartClose(meta.chartId).catch(() => {});
    meta = null;
  }

  function isStale(e: unknown): boolean {
    return typeof e === 'object' && e !== null && (e as { code?: string }).code === 'stale';
  }

  function chartRequest(direction?: ChartDirection, print = false, layout: PrintLayout = printLayout): ChartRequest {
    const req: ChartRequest =
      kind === 'relatives' ? { kind, root, generations, auntsUncles } : kind === 'around' ? (many ? { kind, root, generations, kin, roots } : openings.length > 0 ? { kind, root, generations, kin, open: openings } : { kind, root, generations, kin }) : { kind, root, generations };
    const folded: ChartRequest = kind === 'around' && layout !== 'today' ? { ...req, fold: true } : req;
    return direction ? { ...folded, direction, ...(print ? { print } : {}) } : folded;
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
      m = await api.chartOpen(chartRequest());
    } catch (e) {
      if (seq !== openSeq) return;
      // The person was deleted while the screen was open: leave it quietly instead of showing an error.
      if (kind === 'around' && onGone && (e as { code?: string } | null)?.code === 'not_found') onGone();
      else errorText = errMessage(e, t('chartCanvas.openFailed'));
      return;
    }
    if (seq !== openSeq) {
      void api.chartClose(m.chartId).catch(() => {});
      return;
    }
    meta = m;
    rootNodeId = m.rootNode;
    chosenMore = new Set(m.chosenNodes ?? []);
    const c = new WindowCache(async (r, z) => {
      try {
        return await api.chartWindow(m.chartId, r, z);
      } catch (e) {
        if (isStale(e) && seq === openSeq) void open();
        throw e;
      }
    });
    cache = c;
    sheetRects = [];
    if (kind === 'around' && printLayout === 'sheets') scheduleFrames(m, seq);
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
      related
        ? { x: 0, y: 0, w: Math.max(1, m.bounds.w), h: Math.max(1, m.bounds.h) }
        : edgeStrip(m.bounds, rootSide(kind, dirNow));
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
    // (The same row on a chart that grows up or down; the same column on one that grows sideways.)
    const sideways = isHorizontal(dirNow);
    const couple = kind === 'descendants' && rn ? first.nodes.filter((n) => (sideways ? n.x === rn.x : n.y === rn.y)) : rn ? [rn] : [];
    if (couple.length) {
      const x0 = Math.min(...couple.map((n) => n.x));
      const x1 = Math.max(...couple.map((n) => n.x + n.w));
      const y0 = Math.min(...couple.map((n) => n.y));
      const y1 = Math.max(...couple.map((n) => n.y + n.h));
      rootRect = { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
    } else rootRect = null;
    childrenRect = null;
    childCount = 0;
    frameRect = null;
    parentsRect = null;
    centreRect = null;
    kidsRect = null;
    if (related && rn && rootRect) {
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
      centreRect = { x: rn.x, y: rn.y, w: rn.w, h: rn.h };
      rootRect = box(partners, rootRect) ?? rootRect;
      parentsRect = box(parents, null);
      kidsRect = box(kids, null);
      frameRect = box(kids, box(parents, rootRect));
      // Several people: the first view holds all of them, at a readable zoom.
      const more = first.nodes.filter((n) => chosenMore.has(n.id));
      if (more.length > 1) {
        // The chosen people and their parents.
        const parentIds = new Set(first.edges.filter((e) => e.kind === 'parent' && chosenMore.has(e.to)).map((e) => e.from));
        const all = box([...more, ...first.nodes.filter((n) => parentIds.has(n.id) && !chosenMore.has(n.id))], null);
        // Too wide for a readable zoom: keep the first person's own view (with its parents) instead of a middle point
        // where nobody is.
        const fits = !!all && 0.55 * Math.max(all.w + 64, 1) <= size.w && 0.55 * Math.max(all.h + 64, 1) <= size.h;
        if (all && fits) {
          centreRect = all;
          rootRect = all;
          frameRect = all;
          parentsRect = null;
          kidsRect = null;
        }
      }
    }
    if (kind === 'descendants' && rn && rootRect) {
      try {
        // The children's generation is the nearest one beyond the root, away from the side the ancestors are on.
        const d = dirNow;
        const strip: Rect =
          d === 'top'
            ? { x: -1, y: rn.y + rn.h + 1, w: Math.max(1, m.bounds.w + 2), h: 400 }
            : d === 'bottom'
              ? { x: -1, y: rn.y - 1 - 400, w: Math.max(1, m.bounds.w + 2), h: 400 }
              : d === 'left'
                ? { x: rn.x + rn.w + 1, y: -1, w: 400, h: Math.max(1, m.bounds.h + 2) }
                : { x: rn.x - 1 - 400, y: -1, w: 400, h: Math.max(1, m.bounds.h + 2) };
        const band = await api.chartWindow(m.chartId, strip, 1);
        if (seq !== openSeq) return;
        const along = (n: ChartNode) => (isHorizontal(d) ? n.x : n.y);
        const keys = band.nodes.map(along);
        const nearest = keys.length ? (d === 'top' || d === 'left' ? Math.min(...keys) : Math.max(...keys)) : null;
        const row = band.nodes.filter((n) => along(n) === nearest);
        if (row.length) {
          const x0 = Math.min(...row.map((n) => n.x));
          const y0 = Math.min(...row.map((n) => n.y));
          childrenRect = { x: x0, y: y0, w: Math.max(...row.map((n) => n.x + n.w)) - x0, h: Math.max(...row.map((n) => n.y + n.h)) - y0 };
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

  // Turning the chart re-lays it out: the selected person stays selected and is brought back into view.
  let lastDirection = untrack(() => dirNow);
  let flipKeep: Id | null = null;
  $effect(() => {
    void kind;
    void root;
    void generations;
    void auntsUncles;
    void kin;
    void printLayout;
    void roots?.join();
    const flipped = dirNow !== lastDirection;
    lastDirection = dirNow;
    untrack(() => {
      flipKeep = flipped ? (selNode?.person?.id ?? null) : null;
      if (!flipped) markedIds = [];
      void open();
    });
    return () => {
      openSeq++;
      closeChart();
    };
  });

  // The print's direction and title block change how the sheets are cut: the frames follow.
  $effect(() => {
    void a3Direction.value;
    void a3Title.value;
    untrack(() => {
      if (meta && kind === 'around' && printLayout === 'sheets') scheduleFrames(meta, openSeq);
    });
  });

  // Opening or closing a side branch re-lays the chart out; the view and the selected person stay where they are.
  let lastOpenings = untrack(() => JSON.stringify(openings));
  $effect(() => {
    const now = JSON.stringify(openings);
    if (now === lastOpenings) return;
    lastOpenings = now;
    untrack(() => {
      if (kind === 'around' && !many) void open(true);
    });
  });

  /** Set the depth of one opening of the selected person (null: closed); entries that open nothing are dropped. */
  function setOpening(person: Id, field: 'siblings' | 'children', depth: BranchDepth | null): void {
    const cur = openings.find((o) => o.person === person) ?? { person };
    const next: OpenBranch = { ...cur };
    if (depth) next[field] = depth;
    else delete next[field];
    const rest = openings.filter((o) => o.person !== person);
    onOpenings?.(next.siblings || next.children ? [...rest, next] : rest);
  }
  const depthOf = (person: Id, field: 'siblings' | 'children'): string => openings.find((o) => o.person === person)?.[field] ?? 'off';
  const depthOptions = (kids: boolean) => [
    { value: 'off', label: t('chartCanvas.openOff') },
    { value: 'children', label: kids ? t('chartCanvas.openKids') : t('chartCanvas.openWithChildren') },
    { value: 'all', label: t('chartCanvas.openAll') },
  ];

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
    announcement = selNode ? `${agentChanges.spoken(nodeLabel(selNode), realPerson(selNode)?.id)}${counts()}` : '';
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
      // An edge leaves a card by the side that faces the card it leads to.
      return rectAtEdge(out.pts[0], out.pts[1], kind === 'pedigree' ? dirNow : opposite(dirNow), ref);
    }
    const px = e.pts[e.pts.length - 2];
    const py = e.pts[e.pts.length - 1];
    // A link back to where the person was drawn first reaches that card on its far (right-hand) side.
    if (e.kind === 'dup') return rectAtEdge(px, py, kind === 'pedigree' ? dirNow : isHorizontal(dirNow) ? 'bottom' : 'right', ref);
    return rectAtEdge(px, py, kind === 'pedigree' ? opposite(dirNow) : dirNow, ref);
  }

  /** The card the size of `ref` whose side `side` has the middle point (px, py). */
  function rectAtEdge(px: number, py: number, side: ChartDirection, ref: ChartNode): Rect {
    const { w, h } = ref;
    if (side === 'left') return { x: px, y: py - h / 2, w, h };
    if (side === 'right') return { x: px - w, y: py - h / 2, w, h };
    if (side === 'top') return { x: px - w / 2, y: py, w, h };
    return { x: px - w / 2, y: py - h, w, h };
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
    // The "more brothers and sisters" entry is nobody to centre on.
    if (!n.person || n.person.siblingMarker) return;
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
  // A finger held on a box opens the same menu as a right-click; two quick taps recentre like a double-click.
  const press = createLongPress({ onLongPress: (p) => openMenuAt(p.x, p.y) });
  const taps = createDoubleTap();
  let lastPointerType = '';

  function onpointerdown(e: PointerEvent): void {
    if (e.button === 2) return; // the context menu handles the right button
    lastPointerType = e.pointerType;
    press.pointerdown(e);
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
    press.pointermove(e);
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
    const heldOpen = press.pointerup(e);
    const wasClick = !heldOpen && drag !== null && !drag.moved && pointers.size === 1;
    pointers.delete(e.pointerId);
    if (pointers.size < 2) pinch = 0;
    drag = null;
    if (!wasClick) return;
    const cur = cache?.current();
    if (!cur) return;
    const w = toWorld(e);
    if (isTouchPointer(e) && taps.tap(e.clientX, e.clientY)) {
      const n = hitTest(cur.win, w.x, w.y);
      if (n && n.dupOf === null) recentre(n);
      return;
    }
    // Shift+click marks people, to build the next chart around all of them.
    if (e.shiftKey && onBuildAround) {
      const hit = hitTest(cur.win, w.x, w.y);
      if (hit) toggleMark(hit);
      return;
    }
    const handle = hitHandle(cur.win, kind, w.x, w.y, cam.zoom, dirNow);
    if (handle?.person && !handle.person.siblingMarker) {
      if (handle.relation) onRecentre(handle.person.id, relationText(handle));
      else onRecentre(handle.person.id);
      return;
    }
    const n = hitTest(cur.win, w.x, w.y);
    if (!n) return;
    if (markedIds.length > 0) markedIds = [];
    if (n.dupOf !== null) jumpToOriginal(n);
    else select(n);
  }

  /** Shift+click: adds the person to the marked ones, or takes them out. The first mark also takes in the selected person. */
  function toggleMark(n: ChartNode): void {
    const person = realPerson(n);
    if (!person) return;
    const next = [...markedIds];
    if (next.length === 0) {
      const before = realPerson(selNode);
      if (before && before.id !== person.id) next.push(before.id);
    }
    const at = next.indexOf(person.id);
    if (at >= 0) next.splice(at, 1);
    else next.push(person.id);
    markedIds = next;
    select(n);
    announcement = markedIds.length === 0 ? t('whole.markedNone') : plural('whole.nMarked', markedIds.length);
  }

  function clearMarked(): void {
    if (markedIds.length === 0) return;
    markedIds = [];
    announcement = t('whole.markedNone');
  }

  /** The button: everybody marked with Shift+click when two or more are, otherwise the selected person. */
  function buildAround(): void {
    if (markedIds.length >= 2) onBuildAround?.([...markedIds]);
    else if (realPerson(selNode)) onBuildAround?.([realPerson(selNode)!.id]);
  }

  function ondblclick(e: MouseEvent): void {
    // A finger's double tap is handled in onpointerup; the browser's own double-click would recentre twice.
    if (lastPointerType === 'touch' || lastPointerType === 'pen') return;
    const cur = cache?.current();
    if (!cur) return;
    const w = toWorld(e);
    const n = hitTest(cur.win, w.x, w.y);
    if (n && n.dupOf === null) recentre(n);
  }

  // ------------------------------------------------------------ pictures

  /** Every node and line of the open chart, fetched in tiles at 100% so names and dates are full. */
  async function wholeScene(m: ChartMeta | null = meta): Promise<Scene> {
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
    // White and transparent pages use the light colours whatever the app's theme is.
    const bg = a3Background.value;
    const th = bg === 'paper' ? theme : lightPrintTheme(theme);
    const header = a3Title.value;
    const sel = selected;
    const focus = rootNodeId;
    const focusMore = chosenMore;
    const shot = { cam: { ...cam }, size: { ...size }, dpr };
    // The screen picture asks for its own fresh window: the cached one can lag behind the camera (or be at another zoom level).
    const scene: Scene =
      scope === 'screen' ? await api.chartWindow(meta.chartId, viewRect(shot.cam, shot.size), shot.cam.zoom) : sceneFor(scope, kind, await wholeScene(), sel);
    if (scene.nodes.length === 0) return null;
    const plan = planPicture(scope, scene, scope === 'screen' ? shot : null, [], { dpr, vector: format === 'svg', caption: header });
    return {
      plan,
      theme: { paper: th.paper, ink2: th.ink2, font: th.font },
      transparent: bg === 'transparent',
      caption: header ? captionText(kind, rootName) : '',
      paint: (ctx, p) =>
        draw(ctx, scene, p.cam, { w: p.view.w, h: p.view.h, dpr: p.scale }, { selected: scope === 'screen' ? sel : null, focus, focusMore, theme: th, kind, dir: dirNow, handles: scope === 'screen' }),
    };
  }

  /** Size (page px) of a "→ sheet N" tag on a print at `scale`: the size of a card's dates on paper, kept between 6 and 9 pt. */
  function refTagPx(scale: number): number {
    return Math.min(9 * PT_PX, Math.max(A3_MIN_PT * PT_PX, CARD_SMALL_PX * scale));
  }

  /** Colours and title choice of an A3 picture, read once when it starts. */
  interface A3Look {
    th: Theme;
    bg: A3Background;
    header: boolean;
  }

  /** The print chart of an A3 picture in its chosen direction, cards stretched for their text (see printChart). */
  interface A3Chart {
    scene: Scene;
    pick: DirectionFit;
    meta: ChartMeta;
    look: A3Look;
    mode: A3DirectionMode;
  }

  /**
   * Lays the chart out for an A3 print. A print keeps the direction on the screen; with "best fit" chosen it lays the chart
   * out in every direction and keeps the one that prints largest. The screen's direction is not changed either way.
   * The layouts are print charts: compact, outside the two-chart limit, so no chart the person has open is closed. Each is
   * closed as soon as its boxes are read, whatever happens. `layout` is the print layout chosen when the print started:
   * a change of the choice while it runs does not reach its requests.
   */
  async function printChart(choice: A3Choice, layout: PrintLayout): Promise<A3Chart | null> {
    if (!meta) return null;
    theme ??= themeFromCss();
    // White and transparent pages use the light colours whatever the app's theme is; the paper tint keeps the app's colours.
    const bg = a3Background.value;
    const th = bg === 'paper' ? theme : lightPrintTheme(theme);
    const header = a3Title.value;
    const mode = a3Direction.value;
    // Measures the cards' text as the picture draws it (with a real canvas when there is one).
    const meter = new SvgContext(1, 1) as unknown as CanvasRenderingContext2D;
    const found: { direction: ChartDirection; scene: Scene; meta: ChartMeta; bounds: Rect }[] = [];
    for (const d of mode === 'best' ? DIRECTIONS : [dirNow]) {
      const probe = await api.chartOpen(chartRequest(d, true, layout));
      try {
        const laid = await wholeScene(probe);
        // Cards whose text needs more room (a long name on three lines, the relationship under the dates) make every card
        // taller: the chart's height is stretched, into the page's empty room only, so the print is never smaller.
        const scale = layoutA3(sceneBounds(laid), choice, header).scale;
        const need = printCardsNeed(meter, laid.nodes, { theme: th, kind, focus: probe.rootNode, focusMore: new Set(probe.chosenNodes ?? []), nameMinPx: nameFloorPx(scale) });
        const sc = stretchScene(laid, printStretch(sceneBounds(laid), need, choice, header));
        found.push({ direction: d, scene: sc, meta: probe, bounds: sceneBounds(sc) });
      } finally {
        try {
          await api.chartClose(probe.chartId);
        } catch {
          // The engine keeps at most six print charts and drops the oldest, so a close that failed cannot pile up.
        }
      }
    }
    const pick = printDirection(found, dirNow, mode, choice, header);
    const chosen = found.find((f) => f.direction === pick.direction)!;
    if (chosen.scene.nodes.length === 0) return null;
    return { scene: chosen.scene, pick, meta: chosen.meta, look: { th, bg, header }, mode };
  }

  /** The tree name and today's date, for the right-hand part of an A3 title block. */
  function a3Sub(): string {
    const treeName = settings.value.treeName.trim();
    const date = new Date().toLocaleDateString(currentTag(), { year: 'numeric', month: 'long', day: 'numeric' });
    return treeName ? `${treeName} · ${date}` : date;
  }

  /** The "names are small" warning of a single A3 page. */
  function a3Warning(c: A3Chart): string | undefined {
    const lay = c.pick.layout;
    if (!lay.tooSmall) return undefined;
    return t('picture.a3Small', { pt: Math.round(lay.namePt * 10) / 10 }) + (c.mode === 'screen' ? ` ${t('picture.a3SmallBestHint')}` : '');
  }

  /**
   * One A3 page of a print chart: `scene` drawn with the page layout `lay`, the title block (`title` on the left, `sub` on
   * the right), and under every box of `refs` a small tag naming the sheet that shows the person in full.
   */
  function buildA3For(
    c: A3Chart,
    scene: Scene,
    lay: A3Layout,
    format: PictureFormat,
    text: { title: string; sub: string; warning?: string },
    refs: Map<number, number> = new Map(),
  ): BuiltPicture {
    const { th, bg, header } = c.look;
    // The centre person (and the others it is built around) are the ones of the print chart, not the screen's box numbers.
    const printFocus = c.meta.rootNode;
    const printMore = new Set(c.meta.chosenNodes ?? []);
    const dir = c.pick.direction;
    const pxScale = format === 'svg' ? 1 : lay.pngScale;
    const byId = new Map(scene.nodes.map((n) => [n.id, n]));
    const plan = {
      cam: lay.cam,
      view: { ...lay.px },
      scale: pxScale,
      width: format === 'svg' ? lay.px.w : lay.png.w,
      height: format === 'svg' ? lay.px.h : lay.png.h,
      reduced: false,
      percent: 100,
    };
    return {
      plan,
      theme: { paper: th.paper, ink2: th.ink2, font: th.font },
      transparent: bg === 'transparent',
      caption: '',
      pageMm: { ...lay.mm, orientation: lay.orientation },
      warning: text.warning,
      paint: (ctx, p) => {
        draw(ctx, scene, lay.cam, { w: lay.px.w, h: lay.px.h, dpr: p.scale }, { selected: null, focus: printFocus, focusMore: printMore, theme: th, kind, dir, handles: false, lod: 'full', compact: true, nameMinPx: nameFloorPx(lay.scale) });
        if (refs.size > 0) {
          // "→ sheet N" beside each reference box, on the side away from its ancestors (see refTagAt), the size of a
          // card's dates on paper (kept between 6 and 9 pt).
          ctx.setTransform(p.scale, 0, 0, p.scale, 0, 0);
          const px = refTagPx(lay.scale);
          ctx.font = `600 ${px}px ${th.font}`;
          ctx.fillStyle = th.ink2;
          const z = lay.cam.zoom;
          for (const [id, no] of refs) {
            const n = byId.get(id);
            if (!n) continue;
            const at = refTagAt({ x: (n.x - lay.cam.x) * z, y: (n.y - lay.cam.y) * z, w: n.w * z, h: n.h * z }, dir, 2 * lay.scale);
            ctx.textAlign = at.align;
            ctx.textBaseline = at.baseline;
            ctx.fillText(t('picture.sheetRef', { n: no }), at.x, at.y);
          }
        }
        if (!header) return;
        drawTitleBlock(ctx, p.scale, lay, th, text);
      },
    };
  }

  /** The title block of an A3 page: the chart name on the left, the tree name and date on the right, a rule under both. */
  function drawTitleBlock(ctx: CanvasRenderingContext2D, pxScale: number, lay: A3Layout, th: Theme, text: { title: string; sub: string }): void {
    const left = lay.area.x;
    const right = lay.px.w - lay.area.x;
    const top = lay.area.y;
    ctx.setTransform(pxScale, 0, 0, pxScale, 0, 0);
    ctx.textBaseline = 'alphabetic';
    // The tree name and date go to the right (at most 45% of the width); the chart name gets the rest. Long ones end in an ellipsis.
    ctx.font = `${A3_SUB_PT * PT_PX}px ${th.font}`;
    const subText = ellipsize((x) => ctx.measureText(x).width, text.sub, (right - left) * 0.45);
    const subW = ctx.measureText(subText).width;
    ctx.font = `600 ${A3_TITLE_PT * PT_PX}px ${th.serif}`;
    const titleText = ellipsize((x) => ctx.measureText(x).width, text.title, right - left - subW - mmToPx(8));
    ctx.textAlign = 'left';
    ctx.fillStyle = th.ink;
    ctx.fillText(titleText, left, top - mmToPx(6));
    ctx.textAlign = 'right';
    ctx.fillStyle = th.ink2;
    ctx.font = `${A3_SUB_PT * PT_PX}px ${th.font}`;
    ctx.fillText(subText, right, top - mmToPx(6));
    ctx.beginPath();
    ctx.moveTo(left, top - mmToPx(3));
    ctx.lineTo(right, top - mmToPx(3));
    ctx.strokeStyle = th.rule;
    ctx.lineWidth = 1;
    ctx.stroke();
  }

  /** The whole chart laid out on one A3 page: title block on top, the chart scaled to fill the rest. */
  async function buildA3(choice: A3Choice, format: PictureFormat, layout: PrintLayout): Promise<BuiltPicture | null> {
    const c = await printChart(choice, layout);
    if (!c) return null;
    return buildA3For(c, c.scene, c.pick.layout, format, { title: chartTitle(kind, rootName), sub: a3Sub(), warning: a3Warning(c) });
  }

  /** The surname a branch is named after: the top person's birth surname when one is shown, else the last word of the name. */
  function branchSurname(n: ChartNode | undefined): string {
    if (!n) return '';
    if (!n.person) return n.label;
    if (n.person.birthSurname) return n.person.birthSurname;
    const words = n.person.name.trim().split(/\s+/);
    return words[words.length - 1] ?? '';
  }

  /** The first word of the top person's name (their given name). */
  function givenName(n: ChartNode | undefined): string {
    const words = n?.person?.name.trim().split(/\s+/) ?? [];
    return words.length > 1 ? words[0] : '';
  }

  /**
   * A packed piece's caption: «Branch: <surname> (<given name>)», or «Branch: <surname>» without a given name; for a piece
   * of childless brothers and sisters, «Brothers and sisters of <name>».
   */
  function branchCaption(top: ChartNode, siblingsOf: ChartNode | null = null): string {
    if (siblingsOf?.person) return t('picture.siblingsTitle', { name: siblingsOf.person.name });
    const surname = branchSurname(top);
    const given = givenName(top);
    return t('picture.branchTitle', { name: given ? `${surname} (${given})` : surname });
  }

  /** A piece's boxes as printed: a box standing for children on other pieces reads «Children», with no dates. */
  function groupBoxes(pc: PlannedPiece): PlannedPiece['view']['scene'] {
    const sc = pc.view.scene;
    if (pc.refGroups.size === 0) return sc;
    const label = t('picture.childrenRef');
    return {
      ...sc,
      nodes: sc.nodes.map((n) =>
        pc.refGroups.has(n.id) && n.person
          ? { ...n, ahnen: null, relation: undefined, role: undefined, kinship: undefined, person: { ...n.person, name: label, lifespan: '', birthSurname: undefined, asRecorded: undefined } }
          : n,
      ),
    };
  }

  /** One direction's print chart for "sheets by branch", with the chart it came from. */
  interface PrintDirScene extends DirScene {
    meta: ChartMeta;
  }

  /**
   * The print chart of "sheets by branch" in all four directions, whatever the A3 direction setting says (spec amendment
   * 21.11.2026: every piece takes its own direction). Compact folded cards, each direction's chart stretched once, fully,
   * for its cards' text with the name floor of the target scale. Every print chart is closed as soon as its boxes are read.
   */
  async function printScenes(): Promise<{ dirs: PrintDirScene[]; look: A3Look } | null> {
    if (!meta) return null;
    theme ??= themeFromCss();
    const bg = a3Background.value;
    const th = bg === 'paper' ? theme : lightPrintTheme(theme);
    const header = a3Title.value;
    const meter = new SvgContext(1, 1) as unknown as CanvasRenderingContext2D;
    const nameMinPx = nameFloorPx(targetScale());
    const dirs: PrintDirScene[] = [];
    for (const d of DIRECTIONS) {
      const probe = await api.chartOpen(chartRequest(d, true, 'sheets'));
      try {
        const laid = await wholeScene(probe);
        const need = printCardsNeed(meter, laid.nodes, { theme: th, kind, focus: probe.rootNode, focusMore: new Set(probe.chosenNodes ?? []), nameMinPx });
        const scene = stretchScene(laid, Math.max(1, need));
        dirs.push({ direction: d, scene, keys: nodeKeys(scene), meta: probe });
      } finally {
        try {
          await api.chartClose(probe.chartId);
        } catch {
          // The engine keeps at most six print charts and drops the oldest, so a close that failed cannot pile up.
        }
      }
    }
    if (dirs.every((d) => d.scene.nodes.length === 0)) return null;
    return { dirs, look: { th, bg, header } };
  }

  /**
   * "Sheets by branch" planned (spec amendment 21.11.2026): the four print charts, cut once on the screen direction's,
   * each piece in its own direction, packed onto A3 pages. The print and the frames on screen both plan with this, so the
   * numbers on screen are the printed ones.
   */
  async function planA3Sheets(choice: A3Choice): Promise<{ dirs: PrintDirScene[]; look: A3Look; plan: SheetPlan } | null> {
    const ps = await printScenes();
    if (!ps) return null;
    const { dirs, look } = ps;
    const cut = dirs.find((d) => d.direction === dirNow) ?? dirs[0];
    const centres = [...new Set([cut.meta.rootNode, ...(cut.meta.chosenNodes ?? [])])]
      .map((id) => cut.keys.get(id))
      .filter((k): k is NodeKey => k !== undefined);
    const meter = new SvgContext(1, 1) as unknown as CanvasRenderingContext2D;
    const capMax = mmToPx(CAPTION_MAX_MM);
    const capW = (top: ChartNode, siblingsOf: ChartNode | null): number => {
      meter.font = `${CAPTION_PT * PT_PX}px ${look.th.font}`;
      return Math.min(capMax, meter.measureText(branchCaption(top, siblingsOf)).width);
    };
    // The tag of a reference box at the target scale, in world units (for the widest page number a set may have).
    const ts = targetScale();
    const tagPx = refTagPx(ts);
    meter.font = `600 ${tagPx}px ${look.th.font}`;
    const tagW = meter.measureText(t('picture.sheetRef', { n: 99 })).width / ts;
    const tagH = (tagPx * 1.2) / ts;
    const plan = planSheets(dirs, { choice, header: look.header, screenDir: cut.direction, centres, capW, tagW, tagH });
    return { dirs, look, plan };
  }

  /**
   * One packed A3 page: each piece drawn in its own direction at the page's scale in its slot, the "→ sheet N" tags of its
   * reference boxes, its caption above it, and the title block (the chart title, "sheet N of M" and the tree name).
   */
  function buildA3Page(pg: PlannedPage, dirs: PrintDirScene[], look: A3Look, format: PictureFormat, text: { title: string; sub: string; warning?: string }): BuiltPicture {
    const { th, bg, header } = look;
    const lay = layoutA3({ x: 0, y: 0, w: 1, h: 1 }, pg.orientation, header);
    const pxScale = format === 'svg' ? 1 : lay.pngScale;
    const s = pg.scale;
    const plan = {
      cam: { x: 0, y: 0, zoom: 1 },
      view: { ...lay.px },
      scale: pxScale,
      width: format === 'svg' ? lay.px.w : lay.png.w,
      height: format === 'svg' ? lay.px.h : lay.png.h,
      reduced: false,
      percent: 100,
    };
    return {
      plan,
      theme: { paper: th.paper, ink2: th.ink2, font: th.font },
      transparent: bg === 'transparent',
      caption: '',
      pageMm: { ...lay.mm, orientation: lay.orientation },
      warning: text.warning,
      paint: (ctx, p) => {
        pg.pieces.forEach((pc, i) => {
          const v = { ...pc.view, scene: groupBoxes(pc) };
          const m = dirs.find((d) => d.direction === v.direction)!.meta;
          const cam = { zoom: s, x: v.bounds.x - pc.x / s, y: v.bounds.y - pc.y / s };
          draw(ctx, v.scene, cam, { w: lay.px.w, h: lay.px.h, dpr: p.scale }, { selected: null, focus: m.rootNode, focusMore: new Set(m.chosenNodes ?? []), theme: th, kind, dir: v.direction, handles: false, lod: 'full', compact: true, nameMinPx: nameFloorPx(s), keep: i > 0 });
        });
        ctx.setTransform(p.scale, 0, 0, p.scale, 0, 0);
        // "→ sheet N" beside each reference box, on the side away from its ancestors, or towards them for a parent of
        // someone on the piece (see tagDir).
        ctx.font = `600 ${refTagPx(s)}px ${th.font}`;
        ctx.fillStyle = th.ink2;
        for (const pc of pg.pieces) {
          const byId = new Map(pc.view.scene.nodes.map((n) => [n.id, n]));
          const b = pc.view.bounds;
          for (const [id, no] of pc.refPages) {
            const n = byId.get(id);
            if (!n) continue;
            const at = refTagAt({ x: pc.x + (n.x - b.x) * s, y: pc.y + (n.y - b.y) * s, w: n.w * s, h: n.h * s }, tagDir(pc.view, id), 2 * s);
            ctx.textAlign = at.align;
            ctx.textBaseline = at.baseline;
            const many = pc.refGroups.get(id);
            ctx.fillText(many && many.length > 1 ? t('picture.sheetRefMany', { n: many.join(', ') }) : t('picture.sheetRef', { n: many?.[0] ?? no }), at.x, at.y);
          }
        }
        // Each packed piece's caption, in its band above it.
        ctx.font = `${CAPTION_PT * PT_PX}px ${th.font}`;
        ctx.fillStyle = th.ink2;
        ctx.textAlign = 'left';
        ctx.textBaseline = 'alphabetic';
        for (const pc of pg.pieces) {
          if (pc.capY === null || !pc.top) continue;
          const cap = ellipsize((x) => ctx.measureText(x).width, branchCaption(pc.top, pc.siblingsOf), mmToPx(CAPTION_MAX_MM));
          ctx.fillText(cap, pc.x, pc.capY + mmToPx(CAPTION_MM) - mmToPx(1.8));
        }
        if (header) drawTitleBlock(ctx, p.scale, lay, th, text);
      },
    };
  }

  /**
   * "Sheets by branch" (spec amendment 21.11.2026): the whole chart on one page as "compact" prints it when it reads
   * there at the target size; otherwise the packed pages, each numbered "sheet N of M", the names-are-small warning on
   * the first naming the pages below 6 pt.
   */
  async function buildA3Sheets(choice: A3Choice, format: PictureFormat): Promise<BuiltPicture[] | null> {
    const planned = await planA3Sheets(choice);
    if (!planned || planned.plan.pages.length === 0) return null;
    const { dirs, look, plan } = planned;
    const title = chartTitle(kind, rootName);
    const sub = a3Sub();
    if (plan.single) {
      const v = plan.pages[0].pieces[0].view;
      const layout = layoutA3(v.bounds, choice, look.header);
      const own = dirs.find((d) => d.direction === v.direction)!;
      const c: A3Chart = { scene: v.scene, pick: { direction: v.direction, layout }, meta: own.meta, look, mode: a3Direction.value };
      return [buildA3For(c, v.scene, layout, format, { title, sub, warning: a3Warning(c) })];
    }
    const total = plan.pages.length;
    const small = plan.pages.filter((pg) => pg.namePt < A3_MIN_PT);
    const smallest = Math.min(...small.map((pg) => pg.namePt));
    const warning = small.length
      ? `${t('picture.a3SmallSheets', { list: small.map((pg) => pg.no).join(', ') })} ${t('picture.a3Small', { pt: Math.round(smallest * 10) / 10 })}`
      : undefined;
    return plan.pages.map((pg, i) => buildA3Page(pg, dirs, look, format, { title, sub: `${t('picture.sheetOf', { n: pg.no, m: total })} · ${sub}`, warning: i === 0 ? warning : undefined }));
  }

  function pictureA3(choice: A3Choice, print: boolean): void {
    if (savingPicture) return;
    const setBusy = (b: boolean) => (savingPicture = b);
    // The layout chosen when the print started: a change of the choice while it runs does not reach its requests.
    const layout = printLayout;
    const name = pictureFileName(`${chartTitle(kind, rootName)} (A3)`);
    if (kind === 'around' && layout === 'sheets') {
      const build = (f: PictureFormat) => buildA3Sheets(choice, f);
      if (print) return void printPictures({ build, setBusy });
      return void savePictures({ api, platform }, { fileName: name, build, setBusy });
    }
    if (print) return void printPicture({ build: (f) => buildA3(choice, f, layout), setBusy });
    void savePicture({ api, platform }, { fileName: name, build: (f) => buildA3(choice, f, layout), setBusy });
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
    void pictureMenu?.openAt(x, y, realPerson(selNode)?.id ?? null);
  }

  function oncontextmenu(e: MouseEvent): void {
    e.preventDefault();
    canvas?.focus();
    // The browser's own menu event for a finger press: the long-press helper has opened (or opens) the menu.
    if (press.isNativeTouchMenu(e)) return;
    if (isKeyboardMenu(e as PointerEvent)) return pictureMenuAtSelection();
    openMenuAt(e.clientX, e.clientY);
  }

  /** The picture/chart menu at a screen position; a box there is selected first. */
  function openMenuAt(clientX: number, clientY: number): void {
    canvas?.focus();
    const cur = cache?.current();
    let person: Id | null = null;
    if (cur) {
      // A right-click on a box selects that person first; a box shown elsewhere selects the original.
      const hit = hitTest(cur.win, toWorld({ clientX, clientY }).x, toWorld({ clientX, clientY }).y);
      const n = hit && hit.dupOf !== null ? (cur.win.nodes.find((x) => x.id === hit.dupOf) ?? hit) : hit;
      if (n) {
        select(n);
        person = realPerson(n)?.id ?? null;
      }
    }
    void pictureMenu?.openAt(clientX, clientY, person);
  }

  // ------------------------------------------------------------ keyboard

  function dirFor(key: string): Dir | null {
    // An arrow still moves the way it points, however the chart is turned: towards the ancestors' side goes to a
    // parent (a pedigree's links run the other way: that is a "child" link), the opposite way to a child, and across
    // to the previous / next brother, sister or partner.
    const step = arrowStep(key, dirNow);
    if (step === null) return null;
    if (step === 'prev') return 'up';
    if (step === 'next') return 'down';
    if (kind === 'pedigree') return step === 'ancestors' ? 'child' : 'parent';
    return step === 'ancestors' ? 'parent' : 'child';
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
    } else if (e.key === 'Escape') {
      if (markedIds.length > 0) {
        e.preventDefault();
        clearMarked();
      }
    } else if (e.key === 'e' || e.key === 'E') {
      if (realPerson(selNode)) onOpenPerson(realPerson(selNode)!.id);
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
  const genRange = $derived(kind === 'relatives' ? { min: 1, max: 6 } : kind === 'around' ? { min: 1, max: 8 } : { min: 3, max: 8 });
  /** Pedigree and the chart built around a person can show every generation. */
  const canShowAll = $derived(kind === 'pedigree' || kind === 'around');
  const genOptions = $derived(
    Array.from({ length: genRange.max - genRange.min + 1 }, (_, i) => genRange.min + i).map((n) => ({
      value: String(n),
      label: plural('chartCanvas.nGenerations', n),
    })).concat(canShowAll ? [{ value: String(ALL_GENERATIONS), label: t('chartCanvas.showAll') }] : []),
  );
  let genValue = $derived(
    String(canShowAll && generations >= ALL_GENERATIONS ? ALL_GENERATIONS : Math.min(genRange.max, Math.max(genRange.min, generations))),
  );

  // A phone: icons and a "more" menu instead of the long row of text buttons.
  const overflow = $derived<MenuItem[]>([
    { label: t('chartCanvas.zoomIn'), onselect: () => zoomBy(1.25) },
    { label: t('chartCanvas.zoomOut'), onselect: () => zoomBy(0.8) },
    ...(onBuildAround
      ? [
          {
            label: markedIds.length >= 2 ? t('whole.buildAroundMany', { n: markedIds.length }) : t('whole.buildAround'),
            disabled: markedIds.length < 2 && !realPerson(selNode),
            onselect: buildAround,
          },
          { label: t('whole.openPerson'), disabled: !realPerson(selNode), onselect: () => realPerson(selNode) && onOpenPerson(realPerson(selNode)!.id) },
        ]
      : []),
    ...(kind === 'relatives' ? [{ label: t('chartCanvas.centreOnMe'), disabled: !homeId, onselect: () => homeId && onRecentre(homeId) }] : []),
    ...(markedIds.length > 0 ? [{ label: t('whole.clearMarked'), onselect: clearMarked }] : []),
  ]);
</script>

<div class="chart" {hidden}>
  <div class="toolbar" class:compact={compact.value} role="toolbar" aria-label={toolsLabel}>
    {#if compact.value}
      <IconButton label={t('chartCanvas.fit')} icon="whole" onclick={fitAll} />
      <IconButton label={t('chartCanvas.goHome')} icon="home" disabled={!homeId} onclick={() => homeId && onRecentre(homeId)} />
      <Menu label={t('chartCanvas.moreTools')} items={overflow} />
      <DirectionControl chart={kind} />
    {:else}
    <Button onclick={() => zoomBy(1.25)}>{t('chartCanvas.zoomIn')}</Button>
    <Button onclick={() => zoomBy(0.8)}>{t('chartCanvas.zoomOut')}</Button>
    <Button onclick={fitAll}>{t('chartCanvas.fit')}</Button>
    <DirectionControl chart={kind} />
    {#if onBuildAround}
      <Button disabled={markedIds.length < 2 && !realPerson(selNode)} onclick={buildAround}>{markedIds.length >= 2 ? t('whole.buildAroundMany', { n: markedIds.length }) : t('whole.buildAround')}</Button>
      <Button disabled={!realPerson(selNode)} onclick={() => realPerson(selNode) && onOpenPerson(realPerson(selNode)!.id)}>{t('whole.openPerson')}</Button>
    {/if}
    {#if kind === 'relatives'}
      <Button disabled={!homeId} onclick={() => homeId && onRecentre(homeId)}>{t('chartCanvas.centreOnMe')}</Button>
    {:else}
      <IconButton label={t('chartCanvas.goHome')} icon="home" disabled={!homeId} onclick={() => homeId && onRecentre(homeId)} />
    {/if}
    {/if}
    {#if markedIds.length > 0}
      <div class="tool"><span class="marked" data-testid="marked-count">{plural('whole.nMarked', markedIds.length)}</span></div>
      {#if !compact.value}<Button onclick={clearMarked}>{t('whole.clearMarked')}</Button>{/if}
    {/if}
    {#if onAunts}
      <div class="tool"><Checkbox label={t('chartCanvas.showAunts')} checked={auntsUncles} onchange={onAunts} /></div>
    {/if}
    {#if onKin}
      <div class="tool">
        <Segmented
          legend={t('chartCanvas.kin')}
          options={[
            { value: 'line', label: t('chartCanvas.kinLine') },
            { value: 'siblings', label: t('chartCanvas.kinSiblings') },
            { value: 'all', label: t('chartCanvas.kinAll') },
          ]}
          value={kin}
          onchange={(v) => onKin(v as AroundKin)}
        />
      </div>
    {/if}
    {#if onPrintLayout && kind === 'around'}
      <div class="tool">
        <Segmented
          legend={t('chartCanvas.layout')}
          options={[
            { value: 'today', label: t('chartCanvas.layoutToday') },
            { value: 'compact', label: t('chartCanvas.layoutCompact') },
            { value: 'sheets', label: t('chartCanvas.layoutSheets') },
          ]}
          value={printLayout}
          onchange={(v) => onPrintLayout(v as PrintLayout)}
        />
      </div>
    {/if}
    {#if onOpenings && !many}
      {@const sel = realPerson(selNode)}
      {#if sel && selNode?.canOpenSiblings}
        <div class="tool" data-testid="open-siblings">
          <Segmented legend={t('chartCanvas.openSiblings')} options={depthOptions(false)} value={depthOf(sel.id, 'siblings')} onchange={(v) => setOpening(sel.id, 'siblings', v === 'off' ? null : (v as BranchDepth))} />
        </div>
      {/if}
      {#if sel && selNode?.canOpenChildren}
        <div class="tool" data-testid="open-children">
          <Segmented legend={t('chartCanvas.openChildren')} options={depthOptions(true)} value={depthOf(sel.id, 'children')} onchange={(v) => setOpening(sel.id, 'children', v === 'off' ? null : (v as BranchDepth))} />
        </div>
      {/if}
      {#if openings.length > 0}
        <Button onclick={() => onOpenings([])}>{t('chartCanvas.clearOpenings')}</Button>
      {/if}
    {/if}
    <ChartPictureMenu bind:this={pictureMenu} personCount={openings.length > 0 ? meta?.personCount : undefined} ona3={pictureA3} hasSelection={selected !== null} busy={savingPicture} onpick={savePictureAs} restoreFocus={() => canvas?.focus()} {onOpenPerson} />
    <AddRelativeMenu person={realPerson(selNode)?.id ?? null} />
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
        <div id="{uid}-sel" role="option" aria-selected="true">{agentChanges.spoken(nodeLabel(selNode), realPerson(selNode)?.id)}</div>
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
  /* A phone: one row of tools to swipe sideways, so the chart keeps the screen. */
  @media (max-width: 940px) {
    .toolbar {
      flex-wrap: nowrap;
      /* Never squeezed away by the chart below it (a phone in landscape has little height): the row keeps its height and swipes sideways. */
      flex: none;
      overflow-x: auto;
      scrollbar-width: none;
    }
    .toolbar > :global(*) {
      flex: none;
    }
    .toolbar.compact {
      flex-wrap: wrap;
      overflow: visible;
      align-items: center;
    }
    /* The generations picker keeps its (screen-reader) label but not the line above it: that line made the toolbar one row taller. */
    .toolbar.compact .gens :global(label) {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
    }
  }
  /* The toolbar rests on the bottom line so the labelled Select lines up with the buttons; an item shorter than a
     button (a checkbox, the radio group) would then sit low. Give it a button's height and centre it in that. */
  .tool {
    display: flex;
    align-items: center;
    min-height: 2.25rem;
  }
  .gens {
    margin-inline-start: auto;
  }
  .marked,
  .hint {
    position: absolute;
    top: var(--s-2);
    inset-inline-start: var(--s-2);
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
    -webkit-touch-callout: none;
    user-select: none;
    -webkit-user-select: none;
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
