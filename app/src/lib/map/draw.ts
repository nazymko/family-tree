// Draws the map. The same functions paint a canvas on screen, a bigger canvas for the PNG and, through the chart
// module's SvgContext, a vector picture, so the context type is the small part of the 2D API all three share.
import type { Basemap, Ring, Shape } from './basemap';
import { heatStrength, rampTable, type HeatPoint } from './heat';
import type { BirthLink, PersonPath } from './paths';
import { pickLabels, radiusFor, type PlacePoint } from './places';
import { drawWarped, onScreen as quadOnScreen, screenQuad, type WarpCtx } from './layers';
import { worldPx, type Camera, type World } from './projection';

export interface MapTheme {
  sea: string;
  land: string;
  coast: string;
  lake: string;
  river: string;
  country: string;
  admin: string;
  label: string;
  labelFaint: string;
  /** Behind a label, so text stays readable over lines. */
  halo: string;
  point: string;
  pointRing: string;
  accent: string;
  font: string;
}

export const LIGHT: MapTheme = {
  sea: '#dbe7ee',
  land: '#f6f3ea',
  coast: '#9db0ba',
  lake: '#dbe7ee',
  river: '#a9c5d6',
  country: '#a39484',
  admin: '#cbbfb2',
  label: '#1b2230',
  labelFaint: '#6b7280',
  halo: 'rgba(251,251,248,0.8)',
  point: '#9b2c3f',
  pointRing: '#ffffff',
  accent: '#2d5ba8',
  font: "'Atkinson Hyperlegible Next', system-ui, sans-serif",
};

export const DARK: MapTheme = {
  sea: '#16202a',
  land: '#26303a',
  coast: '#4a5b69',
  lake: '#16202a',
  river: '#2f4658',
  country: '#7d8a95',
  admin: '#46535e',
  label: '#e8ecf1',
  labelFaint: '#9aa5b1',
  halo: 'rgba(18,22,29,0.75)',
  point: '#ee7b8f',
  pointRing: '#12161d',
  accent: '#7ea6e8',
  font: "'Atkinson Hyperlegible Next', system-ui, sans-serif",
};

/** The pieces of the 2D context the map uses (canvas and SvgContext both have them). */
export type Ctx = Pick<
  CanvasRenderingContext2D,
  | 'fillStyle'
  | 'strokeStyle'
  | 'lineWidth'
  | 'globalAlpha'
  | 'font'
  | 'textAlign'
  | 'textBaseline'
  | 'beginPath'
  | 'closePath'
  | 'moveTo'
  | 'lineTo'
  | 'arc'
  | 'fill'
  | 'stroke'
  | 'fillRect'
  | 'fillText'
  | 'measureText'
  | 'setLineDash'
>;

export type Mode = 'places' | 'heat' | 'movement';

export interface Scene {
  basemap: Basemap | null;
  mode: Mode;
  points: PlacePoint[];
  heat: HeatPoint[];
  paths: PersonPath[];
  links: BirthLink[];
  colorOf: (personId: string) => string;
  /** The colours of the movement mode, bottom left. */
  legend: { label: string; color: string }[];
  selected: string | null;
  /** `uk` shows Ukrainian names of countries, regions and cities where the map has them. */
  lang: 'en' | 'uk';
  /** Draw labels for the dots of the data. */
  labels: boolean;
  /** Old maps, lowest first, drawn over the land and under the labels and the data. */
  layers?: DrawLayer[];
}

/** An old map ready to draw: its corners in world points, its picture, and how opaque (0 to 1). */
export interface DrawLayer {
  world: World[];
  img: CanvasImageSource;
  iw: number;
  ih: number;
  opacity: number;
}

interface View {
  cam: Camera;
  w: number;
  h: number;
  s: number;
  ox: number;
  oy: number;
}

const view = (cam: Camera, w: number, h: number): View => {
  const s = worldPx(cam.zoom);
  return { cam, w, h, s, ox: w / 2 - cam.cx * s, oy: h / 2 - cam.cy * s };
};

/** Adds a ring to the current path, leaving out what is off screen and thinning points that fall within one pixel. Returns whether anything was added. */
function trace(ctx: Ctx, r: Ring, v: View, closed: boolean): boolean {
  const sx0 = r.x0 * v.s + v.ox;
  const sx1 = r.x1 * v.s + v.ox;
  const sy0 = r.y0 * v.s + v.oy;
  const sy1 = r.y1 * v.s + v.oy;
  if (sx1 < -4 || sx0 > v.w + 4 || sy1 < -4 || sy0 > v.h + 4) return false;
  const size = sx1 - sx0 + (sy1 - sy0);
  if (size < 0.8) return false;
  const n = r.pts.length / 2;
  const stride = Math.max(1, Math.floor(n / Math.max(16, size * 1.5)));
  ctx.moveTo(r.pts[0] * v.s + v.ox, r.pts[1] * v.s + v.oy);
  for (let i = stride; i < n; i += stride) ctx.lineTo(r.pts[2 * i] * v.s + v.ox, r.pts[2 * i + 1] * v.s + v.oy);
  ctx.lineTo(r.pts[2 * (n - 1)] * v.s + v.ox, r.pts[2 * (n - 1) + 1] * v.s + v.oy);
  if (closed) ctx.closePath();
  return true;
}

const onScreen = (sh: Shape, v: View): boolean => !(sh.x1 * v.s + v.ox < -4 || sh.x0 * v.s + v.ox > v.w + 4 || sh.y1 * v.s + v.oy < -4 || sh.y0 * v.s + v.oy > v.h + 4);

function fillShapes(ctx: Ctx, shapes: Shape[], v: View): void {
  ctx.beginPath();
  for (const sh of shapes) if (onScreen(sh, v)) for (const r of sh.rings) trace(ctx, r, v, true);
  ctx.fill();
}

function strokeShapes(ctx: Ctx, shapes: Shape[], v: View, closed: boolean, show: (sh: Shape) => boolean = () => true): void {
  ctx.beginPath();
  for (const sh of shapes) if (show(sh) && onScreen(sh, v)) for (const r of sh.rings) trace(ctx, r, v, closed);
  ctx.stroke();
}

function name(sh: { n?: string; u?: string }, lang: 'en' | 'uk'): string {
  return (lang === 'uk' && sh.u ? sh.u : sh.n) ?? '';
}

function drawBasemap(ctx: Ctx, bm: Basemap, sc: Scene, th: MapTheme, v: View, obstacles: { x: number; y: number }[], under: () => void): void {
  const z = v.cam.zoom;
  ctx.fillStyle = th.land;
  fillShapes(ctx, bm.land, v);
  ctx.strokeStyle = th.coast;
  ctx.lineWidth = 0.8;
  strokeShapes(ctx, bm.land, v, true);
  ctx.fillStyle = th.lake;
  fillShapes(ctx, bm.lakes, v);
  ctx.strokeStyle = th.coast;
  ctx.lineWidth = 0.6;
  strokeShapes(ctx, bm.lakes, v, true);
  ctx.strokeStyle = th.river;
  ctx.lineWidth = 1.1;
  strokeShapes(ctx, bm.rivers, v, false, (sh) => z + 1.5 >= sh.z);
  ctx.strokeStyle = th.admin;
  ctx.lineWidth = 0.7;
  ctx.setLineDash([4, 3]);
  strokeShapes(ctx, bm.admin1, v, true, (sh) => z >= 3.2 && z + 2 >= sh.z);
  ctx.setLineDash([]);
  ctx.strokeStyle = th.country;
  ctx.lineWidth = 1;
  strokeShapes(ctx, bm.countries, v, true);
  under();

  ctx.textAlign = 'left';
  ctx.textBaseline = 'middle';
  const placed: { x0: number; x1: number; y0: number; y1: number }[] = [];
  const free = (x0: number, y0: number, x1: number, y1: number): boolean => {
    for (const q of placed) if (x0 < q.x1 && x1 > q.x0 && y0 < q.y1 && y1 > q.y0) return false;
    for (const o of obstacles) if (o.x > x0 - 12 && o.x < x1 + 12 && o.y > y0 - 12 && o.y < y1 + 12) return false;
    placed.push({ x0, x1, y0, y1 });
    return true;
  };
  const text = (s: string, x: number, y: number, size: number, color: string, centered: boolean): void => {
    ctx.font = `${size}px ${th.font}`;
    const tw = ctx.measureText(s).width;
    const x0 = centered ? x - tw / 2 : x;
    if (x0 > v.w || x0 + tw < 0 || y < 0 || y > v.h || !free(x0 - 2, y - size / 2, x0 + tw + 2, y + size / 2)) return;
    ctx.fillStyle = th.halo;
    ctx.fillRect(x0 - 2, y - size / 2 - 1, tw + 4, size + 2);
    ctx.fillStyle = color;
    ctx.fillText(s, x0, y);
  };
  const centre = (sh: Shape): [number, number] => [((sh.x0 + sh.x1) / 2) * v.s + v.ox, ((sh.y0 + sh.y1) / 2) * v.s + v.oy];
  // Country names while the countries are big on screen; regions once the countries are.
  if (z < 7) {
    for (const sh of [...bm.countries].sort((a, b) => (b.x1 - b.x0) * (b.y1 - b.y0) - (a.x1 - a.x0) * (a.y1 - a.y0))) {
      const label = name(sh, sc.lang);
      const sw = (sh.x1 - sh.x0) * v.s;
      if (!label || sw < label.length * 6 || !onScreen(sh, v)) continue;
      const [x, y] = centre(sh);
      text(label.toUpperCase(), x, y, 11, th.labelFaint, true);
    }
  }
  if (z >= 4.5 && z < 10) {
    for (const sh of bm.admin1) {
      const label = name(sh, sc.lang);
      const sw = (sh.x1 - sh.x0) * v.s;
      if (!label || sw < label.length * 5.5 || !onScreen(sh, v)) continue;
      const [x, y] = centre(sh);
      text(label, x, y, 11, th.labelFaint, true);
    }
  }
  for (const p of bm.places) {
    if (z + 1.2 < p.z) continue;
    const x = p.x * v.s + v.ox;
    const y = p.y * v.s + v.oy;
    if (x < -20 || y < -20 || x > v.w + 20 || y > v.h + 20) continue;
    ctx.fillStyle = th.labelFaint;
    ctx.beginPath();
    ctx.arc(x, y, 2, 0, Math.PI * 2);
    ctx.fill();
    text(name(p, sc.lang), x + 5, y, 11, th.label, false);
  }
}

function arrow(ctx: Ctx, x0: number, y0: number, x1: number, y1: number, size: number, at = 0.6): void {
  const a = Math.atan2(y1 - y0, x1 - x0);
  const x = x0 + (x1 - x0) * at;
  const y = y0 + (y1 - y0) * at;
  ctx.beginPath();
  ctx.moveTo(x + Math.cos(a) * size, y + Math.sin(a) * size);
  ctx.lineTo(x + Math.cos(a + 2.5) * size, y + Math.sin(a + 2.5) * size);
  ctx.lineTo(x + Math.cos(a - 2.5) * size, y + Math.sin(a - 2.5) * size);
  ctx.closePath();
  ctx.fill();
}

const yearText = (from: number | null, to: number | null): string => (from === null ? '' : to !== null && to !== from ? `${from}-${to}` : String(from));

function drawMovement(ctx: Ctx, sc: Scene, th: MapTheme, v: View): void {
  const px = (x: number) => x * v.s + v.ox;
  const py = (y: number) => y * v.s + v.oy;
  // Parent to child birth places first, under the lives.
  ctx.setLineDash([6, 4]);
  for (const l of sc.links) {
    const c = sc.colorOf(l.child);
    ctx.strokeStyle = c;
    ctx.fillStyle = c;
    ctx.globalAlpha = 0.55;
    ctx.lineWidth = 1.6;
    const x0 = px(l.from.x);
    const y0 = py(l.from.y);
    const x1 = px(l.to.x);
    const y1 = py(l.to.y);
    ctx.beginPath();
    ctx.moveTo(x0, y0);
    ctx.lineTo(x1, y1);
    ctx.stroke();
    ctx.setLineDash([]);
    arrow(ctx, x0, y0, x1, y1, 6, 0.5);
    ctx.setLineDash([6, 4]);
  }
  ctx.setLineDash([]);
  ctx.globalAlpha = 1;
  const marks: { x: number; y: number; text: string; color: string }[] = [];
  for (const path of sc.paths) {
    const c = sc.colorOf(path.person.id);
    ctx.strokeStyle = c;
    ctx.fillStyle = c;
    ctx.lineWidth = path.person.id === sc.selected ? 3.2 : 2;
    ctx.globalAlpha = 0.9;
    for (let i = 1; i < path.stops.length; i++) {
      const a = path.stops[i - 1];
      const b = path.stops[i];
      ctx.beginPath();
      ctx.moveTo(px(a.x), py(a.y));
      ctx.lineTo(px(b.x), py(b.y));
      ctx.stroke();
      arrow(ctx, px(a.x), py(a.y), px(b.x), py(b.y), 6.5);
    }
    for (const s of path.stops) {
      ctx.beginPath();
      ctx.arc(px(s.x), py(s.y), 4, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = th.pointRing;
      ctx.lineWidth = 1.2;
      ctx.stroke();
      ctx.strokeStyle = c;
      const y = yearText(s.from, s.to);
      if (y) marks.push({ x: px(s.x) + 7, y: py(s.y) - 7, text: y, color: c });
    }
  }
  ctx.globalAlpha = 1;
  ctx.font = `11px ${th.font}`;
  ctx.textAlign = 'left';
  ctx.textBaseline = 'middle';
  const taken: { x0: number; x1: number; y0: number; y1: number }[] = [];
  for (const m of marks) {
    if (m.x < -50 || m.y < -20 || m.x > v.w + 50 || m.y > v.h + 20) continue;
    const tw = ctx.measureText(m.text).width;
    const box = { x0: m.x - 2, x1: m.x + tw + 2, y0: m.y - 7, y1: m.y + 7 };
    if (taken.some((q) => box.x0 < q.x1 && box.x1 > q.x0 && box.y0 < q.y1 && box.y1 > q.y0)) continue;
    taken.push(box);
    ctx.fillStyle = th.halo;
    ctx.fillRect(box.x0, box.y0, tw + 4, 14);
    ctx.fillStyle = m.color;
    ctx.fillText(m.text, m.x, m.y);
  }
}

function drawPlaces(ctx: Ctx, sc: Scene, th: MapTheme, v: View): void {
  const order = [...sc.points].sort((a, b) => b.count - a.count);
  for (const p of order) {
    const x = p.x * v.s + v.ox;
    const y = p.y * v.s + v.oy;
    if (x < -30 || y < -30 || x > v.w + 30 || y > v.h + 30) continue;
    const r = radiusFor(p.count);
    ctx.globalAlpha = 0.82;
    ctx.fillStyle = th.point;
    ctx.beginPath();
    ctx.arc(x, y, r, 0, Math.PI * 2);
    ctx.fill();
    ctx.globalAlpha = 1;
    ctx.strokeStyle = p.id === sc.selected ? th.accent : th.pointRing;
    ctx.lineWidth = p.id === sc.selected ? 3 : 1.5;
    ctx.stroke();
    if (p.count > 1 && r >= 8) {
      ctx.fillStyle = '#ffffff';
      ctx.font = `600 ${r >= 12 ? 11 : 10}px ${th.font}`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(String(p.count), x, y);
    }
  }
  if (!sc.labels) return;
  ctx.font = `12px ${th.font}`;
  ctx.textAlign = 'left';
  ctx.textBaseline = 'middle';
  const cam = v.cam;
  for (const l of pickLabels(sc.points, cam, v.w, v.h, (s) => ctx.measureText(s).width)) {
    ctx.fillStyle = th.halo;
    ctx.fillRect(l.x - 2, l.y - 8, l.w + 4, 16);
    ctx.fillStyle = th.label;
    ctx.fillText(l.text, l.x, l.y);
  }
}

function drawLegend(ctx: Ctx, sc: Scene, th: MapTheme, v: View): void {
  if (sc.legend.length === 0) return;
  ctx.font = `12px ${th.font}`;
  ctx.textAlign = 'left';
  ctx.textBaseline = 'middle';
  const rows = sc.legend.slice(0, 10);
  const wide = Math.max(...rows.map((r) => ctx.measureText(r.label).width));
  const h = rows.length * 18 + 10;
  const x = 12;
  const y = v.h - h - 12;
  ctx.fillStyle = th.halo;
  ctx.fillRect(x, y, wide + 40, h);
  rows.forEach((r, i) => {
    ctx.fillStyle = r.color;
    ctx.fillRect(x + 8, y + 9 + i * 18 + 3, 14, 6);
    ctx.fillStyle = th.label;
    ctx.fillText(r.label, x + 30, y + 14 + i * 18);
  });
}

/** A glow canvas for the heat layer, when the context belongs to a real canvas. */
function glowCanvas(ctx: Ctx, w: number, h: number): { c: HTMLCanvasElement; g: CanvasRenderingContext2D } | null {
  const real = (ctx as { canvas?: HTMLCanvasElement }).canvas;
  if (!real || typeof document === 'undefined') return null;
  const c = document.createElement('canvas');
  c.width = Math.max(1, Math.ceil(w));
  c.height = Math.max(1, Math.ceil(h));
  const g = c.getContext('2d');
  return g ? { c, g } : null;
}

let ramp: Uint8ClampedArray | null = null;

function drawHeat(ctx: Ctx, sc: Scene, th: MapTheme, v: View, scale: number): void {
  const max = sc.heat.reduce((m, p) => Math.max(m, p.w), 0);
  const radius = 26 + Math.min(30, v.cam.zoom * 3);
  const glow = glowCanvas(ctx, v.w * scale, v.h * scale);
  if (glow) {
    const g = glow.g;
    g.scale(scale, scale);
    for (const p of sc.heat) {
      const x = p.x * v.s + v.ox;
      const y = p.y * v.s + v.oy;
      if (x < -radius || y < -radius || x > v.w + radius || y > v.h + radius) continue;
      const a = 0.18 + 0.7 * heatStrength(p.w, max);
      const grad = g.createRadialGradient(x, y, 0, x, y, radius);
      grad.addColorStop(0, `rgba(0,0,0,${a})`);
      grad.addColorStop(1, 'rgba(0,0,0,0)');
      g.fillStyle = grad;
      g.fillRect(x - radius, y - radius, radius * 2, radius * 2);
    }
    const img = g.getImageData(0, 0, glow.c.width, glow.c.height) as ImageData | undefined;
    if (!img?.data) return;
    const d = img.data;
    ramp ??= rampTable();
    for (let i = 0; i < d.length; i += 4) {
      const a = d[i + 3];
      if (a === 0) continue;
      d[i] = ramp[a * 4];
      d[i + 1] = ramp[a * 4 + 1];
      d[i + 2] = ramp[a * 4 + 2];
      d[i + 3] = ramp[a * 4 + 3];
    }
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.putImageData(img, 0, 0);
    (ctx as unknown as CanvasRenderingContext2D).drawImage(glow.c, 0, 0, v.w, v.h);
    return;
  }
  // A vector picture: concentric translucent discs stand in for the gradient.
  for (const p of sc.heat) {
    const x = p.x * v.s + v.ox;
    const y = p.y * v.s + v.oy;
    if (x < -radius || y < -radius || x > v.w + radius || y > v.h + radius) continue;
    const t = heatStrength(p.w, max);
    const colors = ['#3150be', '#28aa96', '#f0d23c', '#e13c28'];
    for (let i = 0; i < 4; i++) {
      if (t < i / 4) break;
      ctx.fillStyle = colors[i];
      ctx.globalAlpha = 0.35;
      ctx.beginPath();
      ctx.arc(x, y, radius * (1 - i * 0.22), 0, Math.PI * 2);
      ctx.fill();
    }
  }
  ctx.globalAlpha = 1;
  void th;
}

let scratch: HTMLCanvasElement | null = null;

/**
 * The old maps. Each is stretched onto a canvas of its own fully opaque and laid on the map with its opacity in one go, so
 * the seams of its triangles never show through. Only a bitmap target can draw pictures; the vector picture leaves them out.
 */
function drawLayers(ctx: Ctx, sc: Scene, v: View, scale: number): void {
  const list = sc.layers ?? [];
  if (list.length === 0 || !('drawImage' in ctx) || typeof document === 'undefined') return;
  const main = ctx as unknown as CanvasRenderingContext2D;
  const pw = Math.max(1, Math.round(v.w * scale));
  const ph = Math.max(1, Math.round(v.h * scale));
  for (const l of list) {
    if (l.opacity <= 0 || l.world.length !== 4) continue;
    const q = screenQuad(l.world, v.cam, v.w, v.h);
    if (!quadOnScreen(q, v.w, v.h)) continue;
    scratch ??= document.createElement('canvas');
    if (scratch.width !== pw || scratch.height !== ph) {
      scratch.width = pw;
      scratch.height = ph;
    }
    const g = scratch.getContext('2d');
    if (!g) return;
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.clearRect(0, 0, pw, ph);
    g.setTransform(scale, 0, 0, scale, 0, 0);
    g.imageSmoothingQuality = 'high';
    drawWarped(g as WarpCtx, l.img, l.iw, l.ih, q);
    main.save();
    main.globalAlpha = l.opacity;
    main.drawImage(scratch, 0, 0, v.w, v.h);
    main.restore();
  }
  ctx.globalAlpha = 1;
}

/** Paints the whole map: the basemap, then the layer of the mode. `scale` is the pixel ratio of a bitmap target. */
export function drawMap(ctx: Ctx, cam: Camera, w: number, h: number, sc: Scene, th: MapTheme, scale = 1): void {
  const v = view(cam, w, h);
  ctx.globalAlpha = 1;
  ctx.fillStyle = th.sea;
  ctx.fillRect(0, 0, w, h);
  const obstacles = sc.points.map((p) => ({ x: p.x * v.s + v.ox, y: p.y * v.s + v.oy }));
  const layers = () => drawLayers(ctx, sc, v, scale);
  if (sc.basemap) drawBasemap(ctx, sc.basemap, sc, th, v, obstacles, layers);
  else layers();
  if (sc.mode === 'heat') drawHeat(ctx, sc, th, v, scale);
  if (sc.mode === 'movement') {
    drawMovement(ctx, sc, th, v);
    drawLegend(ctx, sc, th, v);
  }
  if (sc.mode === 'places') drawPlaces(ctx, sc, th, v);
  ctx.globalAlpha = 1;
}

/** Light or dark colours, by the app's own paper colour. */
export function themeFor(el: Element): MapTheme {
  const paper = getComputedStyle(el).getPropertyValue('--paper-0').trim();
  const m = /^#([0-9a-f]{6})$/i.exec(paper);
  if (!m) return LIGHT;
  const n = parseInt(m[1], 16);
  const lum = 0.2126 * ((n >> 16) & 255) + 0.7152 * ((n >> 8) & 255) + 0.0722 * (n & 255);
  return lum < 110 ? DARK : LIGHT;
}
