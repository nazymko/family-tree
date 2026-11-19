<script lang="ts">
  // The Map: where the people of the tree lived. One canvas, three modes (places, heat, movement), shared filters.
  import { onMount, untrack } from 'svelte';
  import Button from '../ui/Button.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import { getApi, getPlatform } from '../api/context';
  import { nav } from '../state/nav.svelte';
  import { project } from '../state/project.svelte';
  import { settings } from '../state/settings.svelte';
  import { notices } from '../ui/notices.svelte';
  import { currentTag, plural, t, type MessageKey } from '../i18n';
  import { en } from '../i18n/en';
  import type { Id, MapCorner, MapDataDto, MapLayerDto, MapLayerInput, MapScope, MapScopeKind } from '../api/types';
  import { savePicture, type BuiltPicture } from '../chart/pictureExport';
  import { CAPTION_H, pictureFileName, type PicturePlan } from '../chart/picture';
  import { loadBasemap, type Basemap } from './basemap';
  import { DARK, LIGHT, drawMap, themeFor, type DrawLayer, type Mode, type Scene } from './draw';
  import { filterEvents, kindsIn, missingCoordinates, yearRange, type MapFilter } from './filters';
  import { decades, heatPoints } from './heat';
  import { buildPaths, birthLinks, colorScheme, type ColorBy } from './paths';
  import { hitPoint, placePoints, type PlacePoint } from './places';
  import { fitPoints, fromScreen, panBy, toScreen, unmercator, zoomAt, type Camera } from './projection';
  import CoordsHelper from './CoordsHelper.svelte';
  import MapLayersPanel from './MapLayersPanel.svelte';
  import { FULL_MAX, cornersToWorld, defaultCorners, moveAll, moveCorner, screenQuad } from './layers';
  import { planLoads, type Held, type LoadPlan } from './layerLoad';
  import { LayerPlane } from './layerPlane';

  let { onOpenPerson }: { onOpenPerson: (id: Id) => void } = $props();

  const api = getApi();
  const platform = getPlatform();

  let mode = $state<Mode>('places');
  let scopeKind = $state<MapScopeKind>('all');
  let generations = $state(0); // 0: all
  let kinds = $state<Set<string>>(new Set());
  let yFrom = $state<number | null>(null);
  let yTo = $state<number | null>(null);
  let colorBy = $state<ColorBy>('generation');
  let includeLiving = $state(false);

  const homeId = $derived(project.info?.homePerson ?? settings.value.homePerson ?? null);
  const personId = $derived(nav.focusPerson ?? homeId);
  let personName = $state('');

  let data = $state<MapDataDto | null>(null);
  let failed = $state(false);
  let basemap = $state<Basemap | null>(null);
  let basemapFailed = $state(false);
  let cam = $state<Camera>({ cx: 0.5, cy: 0.5, zoom: 0 });
  let size = $state({ w: 800, h: 560 });
  let selected = $state<string | null>(null);
  let helperOpen = $state(false);
  let picking = $state<{ id: Id; label: string } | null>(null);
  let playing = $state(false);
  let busy = $state(false);
  let layersOpen = $state(false);

  let box: HTMLDivElement | undefined = $state();
  let canvas: HTMLCanvasElement | undefined = $state();

  const needsPerson = $derived(scopeKind !== 'all');
  const scope = $derived<MapScope>({
    kind: scopeKind,
    ...(needsPerson && personId ? { person: personId } : {}),
    ...(generations > 0 && (scopeKind === 'ancestors' || scopeKind === 'descendants') ? { generations } : {}),
  });

  // ---------------------------------------------------------------- data

  let asked = 0;
  async function load(keepView = false) {
    const mine = ++asked;
    try {
      const d = await api.mapData(scope);
      if (mine !== asked) return;
      data = d;
      failed = false;
      if (!keepView) fit();
    } catch {
      if (mine === asked) failed = true;
    }
  }

  $effect(() => {
    void scope;
    void project.rev; // edits, undo and redo elsewhere
    void load(true);
  });
  // A new scope shows everything of it.
  let lastScope = '';
  $effect(() => {
    const key = JSON.stringify(scope);
    if (key !== lastScope) {
      lastScope = key;
      selected = null;
      fitSoon = true;
    }
  });
  let fitSoon = true;
  let fittedWithPoints = false;

  $effect(() => {
    const id = personId;
    if (!id) {
      personName = '';
      return;
    }
    void api.getPerson(id).then((p) => (personName = p.ref.name));
  });

  onMount(() => {
    loadBasemap().then(
      (b) => (basemap = b),
      () => (basemapFailed = true),
    );
  });

  // ------------------------------------------------------------ selection

  const allKinds = $derived(data ? kindsIn(data.events) : []);
  const years = $derived(data ? yearRange(data.events) : null);
  const filter = $derived<MapFilter>({ kinds, from: yFrom, to: yTo });
  const events = $derived(data ? filterEvents(data.events, filter) : []);
  const points = $derived(data ? placePoints(data, events) : []);
  const heat = $derived(heatPoints(points));
  /** The places with a point and how many family events happened at each: what the old-maps panel groups the maps by. */
  const layerPlaces = $derived.by(() => {
    if (!data) return [];
    const n: Record<string, number> = {};
    for (const e of data.events) n[e.place] = (n[e.place] ?? 0) + 1;
    return data.places.map((p) => ({ id: p.id, name: p.name, label: p.label, lat: p.lat, lon: p.lon, count: n[p.id] ?? 0 }));
  });
  const missing = $derived(data ? missingCoordinates(data, events) : { events: 0, places: [], noPlace: 0 });
  const scheme = $derived(data ? colorScheme(data.people, colorBy, t('map.legendOther')) : { color: () => '#888', legend: [] });
  const paths = $derived(data && mode === 'movement' ? buildPaths(data, events) : []);
  const links = $derived(data && mode === 'movement' && scopeKind !== 'all' && scopeKind !== 'person' ? birthLinks(data, events) : []);
  const legend = $derived(
    colorBy === 'generation'
      ? scheme.legend.map((l) => ({ ...l, label: t('map.legendGeneration', { n: l.label }) }))
      : scheme.legend,
  );
  const colorOf = $derived.by(() => {
    const byId = new Map(data?.people.map((p) => [p.id, p]) ?? []);
    return (id: string) => (byId.has(id) ? scheme.color(byId.get(id)!) : '#888');
  });
  const lang = $derived<'en' | 'uk'>(currentTag().startsWith('uk') ? 'uk' : 'en');

  // ---------------------------------------------------------- old maps

  const layerList = $derived<MapLayerDto[]>(data?.layers ?? []);
  /** Opacity while its slider moves, before it is saved. */
  let opacityNow = $state<Record<Id, number>>({});
  /** The map being placed and where its corners are now (saved only on Save). */
  let placing = $state<{ media: Id; title: string; corners: MapCorner[] } | null>(null);

  // Pictures of the old maps are held only while they are wanted (shown, placed, near the view): see layerLoad.ts.
  interface Pic {
    img: DrawLayer['img'];
    w: number;
    h: number;
  }
  interface Slot {
    preview?: Pic;
    full?: Pic;
    seen: number;
    fullSeen: number;
  }
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  const slots = new Map<Id, Slot>();
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  const asking = new Set<string>();
  let picsRev = $state(0);
  let lastPlan: LoadPlan | null = null;
  /** The view is being panned or zoomed; the old maps are moved, not drawn again, until it rests. */
  let moving = $state(false);
  let settle: ReturnType<typeof setTimeout> | undefined;
  function nudge() {
    moving = true;
    clearTimeout(settle);
    settle = setTimeout(() => (moving = false), 160);
  }
  const plane = new LayerPlane();
  /** Ticks while the Map is open, so pictures not wanted for a while are let go even when nothing moves. */
  let tick = $state(0);
  onMount(() => {
    const timer = setInterval(() => tick++, 5000);
    return () => {
      clearInterval(timer);
      clearTimeout(settle);
      for (const s of slots.values()) for (const p of [s.preview, s.full]) if (p) letGo(p);
      slots.clear();
      plane.release();
    };
  });

  function letGo(p: Pic) {
    if (typeof ImageBitmap !== 'undefined' && p.img instanceof ImageBitmap) p.img.close();
    else if (typeof HTMLImageElement !== 'undefined' && p.img instanceof HTMLImageElement) p.img.src = '';
  }

  function picture(src: string): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
      const img = new Image();
      img.decoding = 'async';
      img.onload = () => resolve(img);
      img.onerror = () => reject(new Error('picture'));
      img.src = src;
    });
  }

  /** A picture decoded once into a bitmap (at most FULL_MAX across), so drawing it never decodes it again. */
  async function decode(src: string): Promise<Pic> {
    const img = await picture(src);
    const w = img.naturalWidth;
    const h = img.naturalHeight;
    if (typeof createImageBitmap !== 'function') return { img, w, h };
    const k = Math.min(1, FULL_MAX / Math.max(w, h, 1));
    const bmp = await (k < 1 ? createImageBitmap(img, { resizeWidth: Math.round(w * k), resizeHeight: Math.round(h * k), resizeQuality: 'high' }) : createImageBitmap(img));
    img.src = '';
    return { img: bmp, w: bmp.width, h: bmp.height };
  }

  /** Loads a map's preview, or its original. A picture with no preview (none could be made) uses its original instead. */
  async function loadPic(id: Id, full: boolean, original = full): Promise<void> {
    const key = `${id}:${full ? 'full' : 'preview'}`;
    if (asking.has(key) || typeof Image === 'undefined') return;
    asking.add(key);
    let retry = false;
    try {
      const pic = await decode(api.mediaUrl(id, original ? 'original' : 'preview'));
      const slot = slots.get(id);
      // Not wanted any more by the time it came (hidden, or the view moved on): let it go at once.
      const still = full ? lastPlan?.wantedFull.has(id) : lastPlan?.wanted.has(id);
      if (!slot || !still) {
        letGo(pic);
        return;
      }
      const had = full ? slot.full : slot.preview;
      if (had) letGo(had);
      if (full) slot.full = pic;
      else slot.preview = pic;
      picsRev++;
    } catch {
      retry = !full && !original;
    } finally {
      asking.delete(key);
    }
    if (retry) await loadPic(id, false, true);
  }

  // What to load and what to let go of, whenever the maps, the view or the time change.
  $effect(() => {
    void tick;
    void picsRev;
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- a plain input to planLoads, made fresh each run
    const held = new Map<Id, Held>();
    for (const [id, s] of slots) held.set(id, { preview: !!s.preview, previewSide: s.preview ? Math.max(s.preview.w, s.preview.h) : 0, full: !!s.full, seen: s.seen, fullSeen: s.fullSeen });
    const plan = planLoads({
      layers: layerList,
      placing: placing ? { media: placing.media, corners: placing.corners } : null,
      cam,
      w: size.w,
      h: size.h,
      scale: typeof window === 'undefined' ? 1 : Math.max(1, window.devicePixelRatio || 1),
      held,
      now: Date.now(),
      moving,
    });
    untrack(() => applyPlan(plan));
  });

  function applyPlan(plan: LoadPlan) {
    lastPlan = plan;
    const now = Date.now();
    let changed = false;
    for (const id of plan.wanted) {
      const s = slots.get(id) ?? { seen: now, fullSeen: 0 };
      s.seen = now;
      if (plan.wantedFull.has(id)) s.fullSeen = now;
      slots.set(id, s);
    }
    for (const id of plan.drop) {
      const s = slots.get(id);
      for (const p of [s?.preview, s?.full]) if (p) letGo(p);
      slots.delete(id);
      changed = true;
    }
    for (const id of plan.dropFull) {
      const s = slots.get(id);
      if (s?.full) {
        letGo(s.full);
        s.full = undefined;
        changed = true;
      }
    }
    for (const id of plan.preview) void loadPic(id, false);
    for (const id of plan.full) void loadPic(id, true);
    if (changed) picsRev++;
  }

  const drawLayers = $derived.by<DrawLayer[]>(() => {
    void picsRev;
    const out: DrawLayer[] = [];
    for (const l of layerList) {
      const corners = placing?.media === l.media ? placing.corners : l.corners;
      const slot = slots.get(l.media);
      const pic = slot?.full ?? slot?.preview;
      if (!corners || !pic || (!l.visible && placing?.media !== l.media)) continue;
      const op = opacityNow[l.media] ?? l.opacity;
      out.push({ world: cornersToWorld(corners), img: pic.img, iw: pic.w, ih: pic.h, opacity: Math.max(placing?.media === l.media ? 0.6 : 0, op / 100) });
    }
    return out;
  });

  /** The middle of the view in degrees: the panel lists the maps under it. */
  const centre = $derived(unmercator(cam.cx, cam.cy));

  function layerInput(l: MapLayerDto, corners: MapCorner[] | null): MapLayerInput {
    return { series: l.series ?? undefined, year: l.year ?? undefined, source: l.source ?? undefined, corners: corners ?? undefined, opacity: l.opacity, visible: l.visible };
  }

  function fitCorners(c: MapCorner[]) {
    cam = fitPoints(cornersToWorld(c), size.w, size.h, 48, 12);
  }

  /** Places a map by dragging its corners; `guess` (from a .map file that does not fit the picture) is where they start. */
  function startPlacing(l: MapLayerDto, guess?: MapCorner[]) {
    const pic = slots.get(l.media)?.preview;
    const given = guess ?? l.corners;
    const corners = given ?? defaultCorners(cam, size.w, size.h, l.width ?? pic?.w ?? 4, l.height ?? pic?.h ?? 3);
    if (given) fitCorners(given);
    placing = { media: l.media, title: l.title?.trim() || t('map.layers.untitled'), corners };
    selected = null;
  }

  async function savePlacing() {
    const p = placing;
    const l = layerList.find((x) => x.media === p?.media);
    if (!p || !l) return;
    await api.apply({ type: 'setMapLayer', media: p.media, layer: $state.snapshot(layerInput(l, p.corners)) });
    notices.show({ message: t('map.layers.saved', { title: p.title }) });
    placing = null;
    await load(true);
  }

  async function layerAdded(media: Id, placed: boolean, guess?: MapCorner[]) {
    await load(true);
    const l = (data?.layers ?? []).find((x) => x.media === media);
    if (!l) return;
    if (placed && l.corners) fitCorners(l.corners);
    else startPlacing(l, guess);
  }

  /** Corner handles (0 to 3) and the middle (4), on screen. */
  const handles = $derived.by(() => {
    if (!placing) return [];
    const q = screenQuad(cornersToWorld(placing.corners), cam, size.w, size.h);
    const mid: [number, number] = [(q[0][0] + q[1][0] + q[2][0] + q[3][0]) / 4, (q[0][1] + q[1][1] + q[2][1] + q[3][1]) / 4];
    return [...q, mid];
  });
  const handleNames = ['map.layers.tl', 'map.layers.tr', 'map.layers.br', 'map.layers.bl', 'map.layers.moveWhole'] as const;

  let grab: { i: number; x: number; y: number } | null = null;
  function grabDown(i: number, e: PointerEvent) {
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    grab = { i, x: e.clientX, y: e.clientY };
  }
  function grabMove(e: PointerEvent) {
    if (!grab || !placing || !canvas) return;
    const r = canvas.getBoundingClientRect();
    if (grab.i === 4) placing.corners = moveAll(placing.corners, cam, size.w, size.h, e.clientX - grab.x, e.clientY - grab.y);
    else placing.corners = moveCorner(placing.corners, grab.i, cam, size.w, size.h, e.clientX - r.left, e.clientY - r.top);
    grab = { ...grab, x: e.clientX, y: e.clientY };
  }
  function grabUp() {
    grab = null;
  }
  function grabKey(i: number, e: KeyboardEvent) {
    if (!placing) return;
    const step = e.shiftKey ? 10 : 1;
    const d: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    const m = d[e.key];
    if (!m) return;
    e.preventDefault();
    if (i === 4) placing.corners = moveAll(placing.corners, cam, size.w, size.h, m[0], m[1]);
    else {
      const [x, y] = handles[i];
      placing.corners = moveCorner(placing.corners, i, cam, size.w, size.h, x + m[0], y + m[1]);
    }
  }
  void toScreen;

  const scene = $derived<Scene>({ basemap, mode, points, heat, paths, links, colorOf, legend, selected, lang, labels: true, layers: drawLayers, plane, moving });

  const chosenPoint = $derived<PlacePoint | null>(points.find((p) => p.id === selected) ?? null);
  const peopleHere = $derived.by(() => {
    if (!chosenPoint || !data) return [];
    const byId = new Map(data.people.map((p) => [p.id, p]));
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    const rows = new Map<string, { id: string; name: string; living: boolean; events: { kind: string; year: number | null }[] }>();
    for (const e of chosenPoint.events) {
      const p = byId.get(e.person);
      if (!p) continue;
      const row = rows.get(p.id) ?? { id: p.id, name: p.name, living: p.living, events: [] };
      row.events.push({ kind: e.kind, year: e.year });
      rows.set(p.id, row);
    }
    return [...rows.values()].sort((a, b) => a.name.localeCompare(b.name));
  });

  const known = new Set(Object.keys(en.eventKind));
  function kindName(kind: string): string {
    if (known.has(kind)) return t(`eventKind.${kind}` as MessageKey);
    return kind.startsWith('custom:') ? kind.slice(7) : kind;
  }

  // -------------------------------------------------------------- camera

  function fit() {
    fitSoon = false;
    if (!data) return;
    const w = filterEvents(data.events, { kinds: new Set(), from: null, to: null });
    const pts = placePoints(data, w).map((p) => ({ x: p.x, y: p.y }));
    fittedWithPoints = pts.length > 0;
    cam = fitPoints(pts, size.w, size.h);
  }

  $effect(() => {
    // The first points that appear (a coordinate was just set) bring the view to them.
    if ((fitSoon || (!fittedWithPoints && points.length > 0)) && data && size.w > 0) fit();
  });

  onMount(() => {
    if (!box) return;
    const ro = new ResizeObserver(() => {
      if (!box) return;
      const r = box.getBoundingClientRect();
      size = { w: Math.max(200, Math.round(r.width)), h: Math.max(200, Math.round(r.height)) };
    });
    ro.observe(box);
    return () => ro.disconnect();
  });

  // ------------------------------------------------------------- drawing

  let theme = $state(LIGHT);
  $effect(() => {
    if (box) theme = themeFor(box) === DARK ? DARK : LIGHT;
  });

  let frame = 0;
  $effect(() => {
    const c = canvas;
    if (!c) return;
    const { w, h } = size;
    const sc = scene;
    const camera = cam;
    const th = theme;
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => {
      const dpr = Math.max(1, window.devicePixelRatio || 1);
      if (c.width !== Math.round(w * dpr) || c.height !== Math.round(h * dpr)) {
        c.width = Math.round(w * dpr);
        c.height = Math.round(h * dpr);
      }
      const ctx = c.getContext('2d');
      if (!ctx) return;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      drawMap(ctx, camera, w, h, sc, th, dpr);
    });
    return () => cancelAnimationFrame(frame);
  });

  // ---------------------------------------------------------- pointer use

  let drag: { x: number; y: number; moved: number } | null = null;

  function down(e: PointerEvent) {
    canvas?.setPointerCapture?.(e.pointerId);
    drag = { x: e.clientX, y: e.clientY, moved: 0 };
  }
  function move(e: PointerEvent) {
    if (!drag) return;
    const dx = e.clientX - drag.x;
    const dy = e.clientY - drag.y;
    drag = { x: e.clientX, y: e.clientY, moved: drag.moved + Math.abs(dx) + Math.abs(dy) };
    cam = panBy(cam, size.w, size.h, dx, dy);
    nudge();
  }
  async function up(e: PointerEvent) {
    const d = drag;
    drag = null;
    if (!d || d.moved > 4 || !canvas) return;
    const r = canvas.getBoundingClientRect();
    const sx = e.clientX - r.left;
    const sy = e.clientY - r.top;
    if (picking) {
      const w = fromScreen(cam, size.w, size.h, sx, sy);
      const { lat, lon } = unmercator(w.x, w.y);
      const target = picking;
      picking = null;
      await api.apply({ type: 'setPlaceCoordinates', place: target.id, latE6: Math.round(lat * 1e6), lonE6: Math.round(lon * 1e6) });
      notices.show({ message: t('map.helper.saved', { place: target.label }) });
      await load(true);
      return;
    }
    selected = hitPoint(points, cam, size.w, size.h, sx, sy)?.id ?? null;
  }
  function wheel(e: WheelEvent) {
    e.preventDefault();
    if (!canvas) return;
    const r = canvas.getBoundingClientRect();
    cam = zoomAt(cam, size.w, size.h, e.clientX - r.left, e.clientY - r.top, -e.deltaY / 300);
    nudge();
  }
  function key(e: KeyboardEvent) {
    const step = 60;
    const k = e.key;
    if (k === '+' || k === '=') cam = zoomAt(cam, size.w, size.h, size.w / 2, size.h / 2, 1);
    else if (k === '-') cam = zoomAt(cam, size.w, size.h, size.w / 2, size.h / 2, -1);
    else if (k === 'ArrowLeft') cam = panBy(cam, size.w, size.h, step, 0);
    else if (k === 'ArrowRight') cam = panBy(cam, size.w, size.h, -step, 0);
    else if (k === 'ArrowUp') cam = panBy(cam, size.w, size.h, 0, step);
    else if (k === 'ArrowDown') cam = panBy(cam, size.w, size.h, 0, -step);
    else return;
    e.preventDefault();
  }

  // ------------------------------------------------------------- filters

  function toggleKind(k: string) {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    const next = new Set(kinds);
    if (next.has(k)) next.delete(k);
    else next.add(k);
    kinds = next;
  }

  function setYear(which: 'from' | 'to', v: number) {
    if (!years) return;
    if (which === 'from') yFrom = v <= years[0] ? null : Math.min(v, yTo ?? years[1]);
    else yTo = v >= years[1] ? null : Math.max(v, yFrom ?? years[0]);
  }

  let timer: ReturnType<typeof setInterval> | undefined;
  function togglePlay() {
    if (playing) return stop();
    if (!years) return;
    const steps = decades(years[0], years[1]);
    if (steps.length === 0) return;
    playing = true;
    let i = 0;
    const tick = () => {
      if (i >= steps.length) return stop();
      yFrom = steps[i];
      yTo = steps[i] + 9;
      i++;
    };
    tick();
    timer = setInterval(tick, 900);
  }
  function stop() {
    playing = false;
    clearInterval(timer);
    yFrom = null;
    yTo = null;
  }
  $effect(() => () => clearInterval(timer));

  // -------------------------------------------------------------- export

  async function savePng() {
    if (!data) return;
    const d = data;
    const living = new Set(d.people.filter((p) => p.living).map((p) => p.id));
    const keep = includeLiving ? events : events.filter((e) => !living.has(e.person));
    const shown: MapDataDto = includeLiving
      ? d
      : { ...d, people: d.people.filter((p) => !living.has(p.id)), links: d.links.filter((l) => !living.has(l.parent) && !living.has(l.child)) };
    const sc: Scene = {
      ...scene,
      points: placePoints(shown, keep),
      heat: heatPoints(placePoints(shown, keep)),
      paths: mode === 'movement' ? buildPaths(shown, keep) : [],
      links: mode === 'movement' && scopeKind !== 'all' && scopeKind !== 'person' ? birthLinks(shown, keep) : [],
      selected: null,
    };
    const c = { ...cam };
    const { w, h } = size;
    const scale = Math.max(2, Math.min(3, window.devicePixelRatio || 1));
    const date = new Date().toLocaleDateString(currentTag(), { year: 'numeric', month: 'long', day: 'numeric' });
    const title = `${t('map.pictureName')} - ${t(`map.${mode}` as MessageKey)}`;
    const built: BuiltPicture = {
      plan: { cam: { x: 0, y: 0, zoom: 1 }, view: { w, h }, scale, width: Math.round(w * scale), height: Math.round((h + CAPTION_H) * scale), reduced: false, percent: 100 } as PicturePlan,
      theme: { paper: theme.sea, ink2: theme.labelFaint, font: theme.font },
      caption: `${title} · ${date} · Natural Earth, GeoNames`,
      paint: (ctx, plan) => {
        ctx.setTransform(plan.scale, 0, 0, plan.scale, 0, 0);
        drawMap(ctx, c, w, h, sc, theme, plan.scale);
      },
    };
    await savePicture(
      { api, platform },
      { fileName: pictureFileName(title), build: async () => built, setBusy: (b) => (busy = b) },
    );
  }

  const noteForMissing = $derived(missing.events > 0 ? plural('map.noCoords', missing.events) : '');
</script>

<section class="map">
  <header class="bar">
    <h1>{t('map.title')}</h1>
    <Segmented
      legend={t('map.mode')}
      value={mode}
      onchange={(v) => (mode = v as Mode)}
      options={[
        { value: 'places', label: t('map.places') },
        { value: 'heat', label: t('map.heat') },
        { value: 'movement', label: t('map.movement') },
      ]}
    />
  </header>

  <div class="filters">
    <label class="f">
      <span>{t('map.scope')}</span>
      <select bind:value={scopeKind}>
        <option value="all">{t('map.scopeAll')}</option>
        <option value="ancestors">{t('map.scopeAncestors')}</option>
        <option value="descendants">{t('map.scopeDescendants')}</option>
        <option value="person">{t('map.scopePerson')}</option>
      </select>
    </label>
    {#if needsPerson}
      <span class="who" title={t('map.person')}>{personName || t('map.chooseFirst')}</span>
    {/if}
    {#if scopeKind === 'ancestors' || scopeKind === 'descendants'}
      <label class="f">
        <span>{t('map.generations')}</span>
        <select bind:value={generations}>
          <option value={0}>{t('map.allGenerations')}</option>
          {#each [1, 2, 3, 4, 5, 6, 8, 10] as n (n)}<option value={n}>{n}</option>{/each}
        </select>
      </label>
    {/if}
    {#if mode === 'movement'}
      <label class="f">
        <span>{t('map.colorBy')}</span>
        <select bind:value={colorBy}>
          <option value="generation">{t('map.byGeneration')}</option>
          <option value="surname">{t('map.bySurname')}</option>
        </select>
      </label>
    {/if}
  </div>

  {#if allKinds.length > 0}
    <div class="kinds" role="group" aria-label={t('map.events')}>
      <span class="cap">{t('map.events')}</span>
      <button type="button" class="chip" class:on={kinds.size === 0} aria-pressed={kinds.size === 0} onclick={() => (kinds = new Set())}>{t('map.allEvents')}</button>
      {#each allKinds as k (k)}
        <button type="button" class="chip" class:on={kinds.has(k)} aria-pressed={kinds.has(k)} onclick={() => toggleKind(k)}>{kindName(k)}</button>
      {/each}
    </div>
  {/if}

  {#if years && years[0] < years[1]}
    <div class="years">
      <span class="cap">{t('map.years')}</span>
      <input type="range" min={years[0]} max={years[1]} value={yFrom ?? years[0]} aria-label={t('map.yearFrom')} oninput={(e) => setYear('from', Number(e.currentTarget.value))} />
      <output>{yFrom ?? years[0]}</output>
      <input type="range" min={years[0]} max={years[1]} value={yTo ?? years[1]} aria-label={t('map.yearTo')} oninput={(e) => setYear('to', Number(e.currentTarget.value))} />
      <output>{yTo ?? years[1]}</output>
      {#if yFrom !== null || yTo !== null}
        <Button onclick={() => ((yFrom = null), (yTo = null), (playing = false), clearInterval(timer))}>{t('map.allYears')}</Button>
      {/if}
      {#if mode !== 'movement'}
        <Button onclick={togglePlay}>{playing ? t('map.stop') : t('map.play')}</Button>
      {/if}
    </div>
  {/if}

  {#if missing.events > 0 || missing.noPlace > 0}
    <p class="notice" role="status">
      {#if missing.events > 0}
        {noteForMissing}
        <Button variant="secondary" onclick={() => (helperOpen = true)}>{t('map.setThem')}</Button>
      {/if}
      {#if missing.noPlace > 0}<span class="dim">{plural('map.noPlace', missing.noPlace)}</span>{/if}
    </p>
  {/if}

  {#if placing}
    <p class="notice placing" role="status">
      <span>{t('map.layers.placing', { title: placing.title })}</span>
      <Button variant="primary" onclick={savePlacing}>{t('map.layers.save')}</Button>
      <Button onclick={() => (placing = null)}>{t('map.layers.cancel')}</Button>
    </p>
  {/if}

  <div class="stage" class:withSide={helperOpen || chosenPoint || layersOpen}>
    <div class="canvasBox" bind:this={box}>
      {#if failed}
        <div class="over"><EmptyState title={t('map.loadFailed')} /></div>
      {:else if data && data.events.length === 0 && data.eventsWithoutPlace === 0}
        <div class="over"><EmptyState title={t('map.empty')} /></div>
      {/if}
      <canvas
        bind:this={canvas}
        class:picking={!!picking}
        tabindex="0"
        aria-label={t('map.canvasLabel')}
        style:width="{size.w}px"
        style:height="{size.h}px"
        onpointerdown={down}
        onpointermove={move}
        onpointerup={up}
        onwheel={wheel}
        onkeydown={key}
      ></canvas>
      {#if !basemap && !basemapFailed}<p class="loading">{t('map.loading')}</p>{/if}
      <div class="zoom">
        <Button onclick={() => (cam = zoomAt(cam, size.w, size.h, size.w / 2, size.h / 2, 1))} aria-label={t('map.zoomIn')}>+</Button>
        <Button onclick={() => (cam = zoomAt(cam, size.w, size.h, size.w / 2, size.h / 2, -1))} aria-label={t('map.zoomOut')}>−</Button>
        <Button onclick={fit} aria-label={t('map.fitAll')}>⤢</Button>
      </div>
      <p class="credit">{t('map.attribution')}</p>
      {#if placing}
        <svg class="outline" width={size.w} height={size.h} aria-hidden="true">
          <polygon points={handles.slice(0, 4).map((h) => h.join(',')).join(' ')} />
        </svg>
        {#each handles as h, i (i)}
          <button
            type="button"
            class="handle"
            class:mid={i === 4}
            style:left="{h[0]}px"
            style:top="{h[1]}px"
            aria-label={t(handleNames[i])}
            onpointerdown={(e) => grabDown(i, e)}
            onpointermove={grabMove}
            onpointerup={grabUp}
            onkeydown={(e) => grabKey(i, e)}
          ></button>
        {/each}
      {/if}
    </div>

    {#if layersOpen}
      <MapLayersPanel
        layers={layerList.map((l) => (opacityNow[l.media] === undefined ? l : { ...l, opacity: opacityNow[l.media] }))}
        onOpacity={(media, v) => {
          const next = { ...opacityNow };
          if (v === null) delete next[media];
          else next[media] = v;
          opacityNow = next;
        }}
        places={layerPlaces}
        {centre}
        onPlace={(l) => startPlacing(l)}
        onAdded={layerAdded}
        onChanged={() => load(true)}
        onClose={() => ((layersOpen = false), (placing = null))}
      />
    {:else if helperOpen && data}
      <CoordsHelper
        {missing}
        {picking}
        onPick={(p) => (picking = p)}
        onCancelPick={() => (picking = null)}
        onSaved={() => load(true)}
        onClose={() => ((helperOpen = false), (picking = null))}
      />
    {:else if chosenPoint}
      <aside class="side" aria-label={t('map.placeTitle')}>
        <header>
          <h2>{chosenPoint.label}</h2>
          <Button onclick={() => (selected = null)} aria-label={t('map.closePlace')}>×</Button>
        </header>
        <p class="dim">{plural('map.peopleHere', peopleHere.length)} · {plural('map.eventsHere', chosenPoint.count)}</p>
        <ul>
          {#each peopleHere as p (p.id)}
            <li>
              <button type="button" class="link" onclick={() => onOpenPerson(p.id)}>{p.name}</button>
              {#if p.living}<span class="dim"> ({t('map.living')})</span>{/if}
              <div class="dim small">{p.events.map((e) => `${kindName(e.kind)}${e.year === null ? '' : ' ' + e.year}`).join(', ')}</div>
            </li>
          {/each}
        </ul>
      </aside>
    {/if}
  </div>

  <footer class="actions">
    <Button onclick={savePng} disabled={busy || !data}>{t('map.savePicture')}</Button>
    <label class="f inline"><input type="checkbox" bind:checked={includeLiving} /> <span>{t('map.includeLiving')}</span></label>
    {#if !helperOpen}
      <Button onclick={() => ((helperOpen = true), (layersOpen = false))}>{t('map.helper.title')}</Button>
    {/if}
    {#if !layersOpen}
      <Button onclick={() => ((layersOpen = true), (helperOpen = false), (selected = null))}>{t('map.layers.open')}{layerList.length ? ` (${layerList.length})` : ''}</Button>
    {/if}
  </footer>
</section>

<style>
  .map {
    display: flex;
    flex-direction: column;
    gap: var(--s-3);
    /* The shell's page scrolls, so the view sizes itself to the window instead of to its parent. */
    height: calc(100dvh - 7rem);
    min-height: 32rem;
  }
  .bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--s-4);
    flex-wrap: wrap;
  }
  h1 {
    margin: 0;
    font-size: var(--t-lg);
  }
  .filters,
  .kinds,
  .years,
  .actions {
    display: flex;
    gap: var(--s-3);
    align-items: center;
    flex-wrap: wrap;
  }
  .f {
    display: inline-flex;
    gap: var(--s-2);
    align-items: center;
    font-size: var(--t-sm);
  }
  .f select {
    min-height: 2.25rem;
    padding: 0 var(--s-3);
    border: 1px solid var(--ink-2);
    background: var(--paper-1);
    color: var(--ink-0);
    border-radius: var(--r-1);
  }
  .who {
    font-weight: 600;
  }
  .cap {
    font-size: var(--t-sm);
    font-weight: 500;
  }
  .chip {
    min-height: 2rem;
    padding: 0 var(--s-4);
    border: 1px solid var(--rule);
    border-radius: 999px;
    background: var(--paper-1);
    color: var(--ink-0);
    font: inherit;
    font-size: var(--t-sm);
    cursor: pointer;
  }
  .chip.on {
    background: var(--ink-0);
    color: var(--paper-1);
    border-color: var(--ink-0);
  }
  .years input[type='range'] {
    width: 9rem;
  }
  output {
    min-width: 3rem;
    font-variant-numeric: tabular-nums;
  }
  .notice {
    display: flex;
    gap: var(--s-4);
    align-items: center;
    flex-wrap: wrap;
    margin: 0;
    padding: var(--s-2) var(--s-4);
    background: var(--foxing-wash);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
  }
  .dim {
    color: var(--ink-2);
  }
  .small {
    font-size: var(--t-xs);
  }
  .stage {
    flex: 1;
    min-height: 22rem;
    display: grid;
    grid-template-columns: 1fr;
    gap: 0;
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    overflow: hidden;
  }
  .stage.withSide {
    grid-template-columns: 1fr minmax(18rem, 24rem);
  }
  .canvasBox {
    position: relative;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  canvas {
    display: block;
    touch-action: none;
    cursor: grab;
  }
  canvas.picking {
    cursor: crosshair;
  }
  canvas:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: -2px;
  }
  .over {
    position: absolute;
    inset: 0;
    z-index: 1;
    pointer-events: none;
    display: grid;
    place-items: center;
  }
  .zoom {
    position: absolute;
    top: var(--s-3);
    right: var(--s-3);
    display: flex;
    flex-direction: column;
    gap: var(--s-2);
  }
  .credit,
  .loading {
    position: absolute;
    margin: 0;
    padding: 1px var(--s-3);
    font-size: var(--t-xs);
    color: var(--ink-1);
    background: rgb(251 251 248 / 0.8);
  }
  .credit {
    right: 0;
    bottom: 0;
  }
  .loading {
    left: var(--s-3);
    top: var(--s-3);
  }
  .outline {
    position: absolute;
    left: 0;
    top: 0;
    pointer-events: none;
  }
  .outline polygon {
    fill: none;
    stroke: var(--focus);
    stroke-width: 2;
    stroke-dasharray: 6 4;
  }
  .handle {
    position: absolute;
    width: 1.25rem;
    height: 1.25rem;
    margin: -0.625rem 0 0 -0.625rem;
    padding: 0;
    border: 2px solid var(--paper-1);
    border-radius: 50%;
    background: var(--focus);
    box-shadow: 0 0 0 1px var(--ink-0);
    cursor: move;
    touch-action: none;
  }
  .handle.mid {
    width: 1.75rem;
    height: 1.75rem;
    margin: -0.875rem 0 0 -0.875rem;
    border-radius: var(--r-1);
  }
  .handle:focus-visible {
    outline: 3px solid var(--ink-0);
    outline-offset: 2px;
  }
  .side {
    padding: var(--s-4);
    overflow: auto;
    background: var(--paper-1);
    border-left: 1px solid var(--rule);
  }
  .side header {
    display: flex;
    justify-content: space-between;
    align-items: start;
    gap: var(--s-3);
  }
  .side h2 {
    margin: 0;
    font-size: var(--t-md);
  }
  .side ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .side li {
    padding: var(--s-2) 0;
    border-bottom: 1px solid var(--paper-2);
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--focus);
    text-decoration: underline;
    cursor: pointer;
    font: inherit;
  }
</style>
