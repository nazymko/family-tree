import type { ChartEdge, ChartNode, ChartWindow } from '../api/types';
import type { Camera } from './camera';
import { HANDLE_R, handlePos } from './hit';

export type Lod = 'full' | 'boxes' | 'dots';

/** zoom >= 0.55 full, >= 0.25 boxes (no text), else dots. */
export function lodFor(zoom: number): Lod {
  if (zoom >= 0.55) return 'full';
  if (zoom >= 0.25) return 'boxes';
  return 'dots';
}

export interface Theme {
  paper: string;
  rule: string;
  ink: string;
  ink2: string;
  ribbon: string;
  wash: string;
  font: string;
  serif: string;
}

export interface DrawOpts {
  selected: number | null;
  focus: number | null;
  theme: Theme;
  /** Where the "+" handle sits; defaults to pedigree (right edge). */
  kind?: 'pedigree' | 'descendants';
}

/** Theme from the CSS custom properties on <html> (re-read when the colour scheme changes). */
export function themeFromCss(el: Element = document.documentElement): Theme {
  const cs = getComputedStyle(el);
  const v = (name: string, fallback: string) => cs.getPropertyValue(name).trim() || fallback;
  return {
    paper: v('--paper-1', '#FBFBF8'),
    rule: v('--rule', '#C6CABF'),
    ink: v('--ink-0', '#1B2230'),
    ink2: v('--ink-2', '#626A79'),
    ribbon: v('--ribbon', '#9B2C3F'),
    wash: v('--ribbon-wash', '#F1DDE0'),
    font: v('--font-sans', 'system-ui, sans-serif'),
    serif: v('--font-serif', 'Georgia, serif'),
  };
}

function strokeEdges(ctx: CanvasRenderingContext2D, edges: ChartEdge[], color: string, widthPx: number, zoom: number, dash: number[] | null) {
  if (edges.length === 0) return;
  ctx.beginPath();
  for (const e of edges) {
    const p = e.pts;
    if (p.length < 4) continue;
    ctx.moveTo(p[0], p[1]);
    for (let i = 2; i < p.length; i += 2) ctx.lineTo(p[i], p[i + 1]);
  }
  ctx.strokeStyle = color;
  ctx.lineWidth = widthPx / zoom;
  ctx.setLineDash(dash ?? []);
  ctx.stroke();
}

function fit(ctx: CanvasRenderingContext2D, text: string, maxW: number): string {
  if (ctx.measureText(text).width <= maxW) return text;
  let lo = 0;
  let hi = text.length;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (ctx.measureText(text.slice(0, mid) + '…').width <= maxW) lo = mid;
    else hi = mid - 1;
  }
  return text.slice(0, lo) + '…';
}

/**
 * The name lines of a box. A name that fits stays one line. Without a second line (no dates) a long name wraps onto
 * two lines at a word break. With dates, the middle words (a patronymic or middle name) are dropped before cutting
 * with "…", so "Given Patronymic Surname" reads "Given Surname".
 */
export function nameLines(measure: (s: string) => number, name: string, maxW: number, hasSub: boolean): string[] {
  const cut = (t: string) => {
    if (measure(t) <= maxW) return t;
    let lo = 0;
    let hi = t.length;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (measure(t.slice(0, mid) + '…') <= maxW) lo = mid;
      else hi = mid - 1;
    }
    return t.slice(0, lo) + '…';
  };
  if (measure(name) <= maxW) return [name];
  const words = name.split(/\s+/).filter(Boolean);
  if (!hasSub && words.length > 1) {
    let i = 1;
    while (i < words.length - 1 && measure(words.slice(0, i + 1).join(' ')) <= maxW) i++;
    return [cut(words.slice(0, i).join(' ')), cut(words.slice(i).join(' '))];
  }
  if (words.length >= 3) {
    const short = `${words[0]} ${words[words.length - 1]}`;
    if (measure(short) <= maxW) return [short];
  }
  return [cut(name)];
}

function drawHandle(ctx: CanvasRenderingContext2D, n: ChartNode, kind: 'pedigree' | 'descendants', theme: Theme, zoom: number) {
  const h = handlePos(n, kind);
  ctx.beginPath();
  ctx.arc(h.x, h.y, Math.max(HANDLE_R, 6 / zoom), 0, Math.PI * 2);
  ctx.fillStyle = theme.paper;
  ctx.fill();
  ctx.strokeStyle = theme.ribbon;
  ctx.lineWidth = 1.5 / zoom;
  ctx.stroke();
  ctx.beginPath();
  ctx.moveTo(h.x - 4, h.y);
  ctx.lineTo(h.x + 4, h.y);
  ctx.moveTo(h.x, h.y - 4);
  ctx.lineTo(h.x, h.y + 4);
  ctx.stroke();
}

/** Draws the window. Returns the number of nodes drawn (all of them: the window is the culling). */
export function draw(
  ctx: CanvasRenderingContext2D,
  win: ChartWindow,
  cam: Camera,
  view: { w: number; h: number; dpr: number },
  opts: DrawOpts,
): { drawn: number } {
  const { theme, selected, focus } = opts;
  const kind = opts.kind ?? 'pedigree';
  const z = cam.zoom;
  const lod = lodFor(z);

  ctx.setTransform(view.dpr, 0, 0, view.dpr, 0, 0);
  ctx.clearRect(0, 0, view.w, view.h);
  ctx.setTransform(view.dpr * z, 0, 0, view.dpr * z, -cam.x * view.dpr * z, -cam.y * view.dpr * z);

  // Edges first, one path per kind.
  const parent: ChartEdge[] = [];
  const partner: ChartEdge[] = [];
  const dup: ChartEdge[] = [];
  for (const e of win.edges) (e.kind === 'parent' ? parent : e.kind === 'partner' ? partner : dup).push(e);
  strokeEdges(ctx, parent, theme.ink2, 1, z, null);
  strokeEdges(ctx, partner, theme.ink2, 2, z, null);
  strokeEdges(ctx, dup, theme.ribbon, 1, z, [6 / z, 4 / z]);
  ctx.setLineDash([]);

  if (lod === 'dots') {
    ctx.beginPath();
    for (const n of win.nodes) ctx.rect(n.x, n.y, n.w, n.h);
    ctx.fillStyle = theme.ink2;
    ctx.fill();
    if (selected !== null) {
      const s = win.nodes.find((n) => n.id === selected);
      if (s) {
        ctx.beginPath();
        ctx.rect(s.x, s.y, s.w, s.h);
        ctx.strokeStyle = theme.ribbon;
        ctx.lineWidth = 3 / z;
        ctx.stroke();
      }
    }
    return { drawn: win.nodes.length };
  }

  // Node boxes: one batched fill and stroke for the ordinary ones, dup boxes dashed.
  const plain: ChartNode[] = [];
  const dups: ChartNode[] = [];
  let sel: ChartNode | null = null;
  for (const n of win.nodes) {
    if (n.id === selected) sel = n;
    else if (n.dupOf !== null) dups.push(n);
    else plain.push(n);
  }
  if (plain.length > 0) {
    ctx.beginPath();
    for (const n of plain) ctx.rect(n.x, n.y, n.w, n.h);
    ctx.fillStyle = theme.paper;
    ctx.fill();
    ctx.strokeStyle = theme.rule;
    ctx.lineWidth = 1 / z;
    ctx.stroke();
  }
  if (dups.length > 0) {
    ctx.beginPath();
    for (const n of dups) ctx.rect(n.x, n.y, n.w, n.h);
    ctx.fillStyle = theme.paper;
    ctx.fill();
    ctx.setLineDash([5 / z, 3 / z]);
    ctx.strokeStyle = theme.ink2;
    ctx.stroke();
    ctx.setLineDash([]);
  }
  if (sel) {
    ctx.beginPath();
    ctx.rect(sel.x, sel.y, sel.w, sel.h);
    ctx.fillStyle = theme.wash;
    ctx.fill();
    ctx.strokeStyle = theme.ribbon;
    ctx.lineWidth = 2 / z;
    ctx.stroke();
  }

  // Ribbon bookmark on the home person.
  if (focus !== null) {
    const f = win.nodes.find((n) => n.id === focus);
    if (f) {
      ctx.beginPath();
      ctx.moveTo(f.x + 8, f.y);
      ctx.lineTo(f.x + 8, f.y + 16);
      ctx.lineTo(f.x + 14, f.y + 11);
      ctx.lineTo(f.x + 20, f.y + 16);
      ctx.lineTo(f.x + 20, f.y);
      ctx.closePath();
      ctx.fillStyle = theme.ribbon;
      ctx.fill();
    }
  }

  for (const n of win.nodes) if (kind === 'pedigree' ? n.moreUp : n.moreDown) drawHandle(ctx, n, kind, theme, z);

  if (lod === 'full') {
    ctx.textBaseline = 'alphabetic';
    for (const n of win.nodes) {
      const isDup = n.dupOf !== null;
      const left = n.x + (n.id === focus ? 26 : 10);
      const maxW = n.x + n.w - 10 - left - (n.ahnen !== null ? 22 : 0);
      ctx.textAlign = 'left';
      ctx.fillStyle = isDup ? theme.ink2 : theme.ink;
      ctx.font = `600 13px ${theme.serif}`;
      const sub = isDup ? 'also shown elsewhere' : (n.person?.lifespan ?? '');
      const lines = nameLines((t) => ctx.measureText(t).width, n.person?.name ?? '', maxW, sub !== '');
      lines.forEach((line, i) => ctx.fillText(line, left, n.y + 23 + i * 18));
      ctx.fillStyle = theme.ink2;
      ctx.font = `11px ${theme.font}`;
      if (sub) ctx.fillText(fit(ctx, sub, n.w - 20), n.x + 10, n.y + 43);
      if (n.ahnen !== null) {
        ctx.font = `10px ${theme.font}`;
        ctx.textAlign = 'right';
        ctx.fillText(String(n.ahnen), n.x + n.w - 8, n.y + 14);
      }
    }
  }
  return { drawn: win.nodes.length };
}
