import type { ChartEdge, ChartNode, ChartWindow, SexKind, WholeBlockDto } from '../api/types';
import type { Camera } from './camera';
import { drawAgentBadges, fit, lodFor, nameLines, themeFromCss, type Theme } from './renderer';
import { kinshipText } from '../family/kinship';

export interface WholeTheme extends Theme {
  male: string;
  female: string;
  other: string;
}

/** The chart theme plus the three bar colours used when people are too small to read. */
export function wholeThemeFromCss(el: Element = document.documentElement): WholeTheme {
  const base = themeFromCss(el);
  const cs = getComputedStyle(el);
  const v = (name: string, fallback: string) => cs.getPropertyValue(name).trim() || fallback;
  return { ...base, male: v('--verdigris', '#2B6B62'), female: base.ribbon, other: v('--foxing', '#8A5A12') };
}

export interface WholeDrawOpts {
  selected: number | null;
  /** The home person's node. */
  home: number | null;
  /** Ancestors, descendants and the selected person; everything else is faded. Null shows everyone normally. */
  lineage: ReadonlySet<number> | null;
  theme: WholeTheme;
  /** People an agent changed and nobody has reviewed; their boxes carry a dot. A saved picture leaves this out. */
  badged?: ReadonlySet<string>;
}

function barColor(sex: SexKind | undefined, t: WholeTheme): string {
  return sex === 'male' ? t.male : sex === 'female' ? t.female : t.other;
}

function stroke(ctx: CanvasRenderingContext2D, edges: ChartEdge[], color: string, widthPx: number, zoom: number) {
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
  ctx.stroke();
}

/**
 * Whether an edge belongs to the highlighted line. A child edge starts at the family's first partner, who is
 * not on the line when the line runs through the second partner; but every child on the line has a parent on
 * it, so a child edge is on the line exactly when its child is. A partner line needs both people.
 */
export function edgeOnLine(e: { from: number; to: number; kind: ChartEdge['kind'] }, lit: (id: number) => boolean): boolean {
  return e.kind === 'partner' ? lit(e.from) && lit(e.to) : lit(e.to);
}

/** Size (world units) of a block heading: 14 px on screen, at least 16 world units. */
export function headingSize(zoom: number): number {
  return Math.max(16, 14 / zoom);
}

/** Zoomed out, a heading is drawn over the chart on a plate at a fixed screen size; zoomed in it sits in its own band. */
export function headingIsOverlay(zoom: number): boolean {
  return headingSize(zoom) > 22;
}

function drawHeadings(ctx: CanvasRenderingContext2D, blocks: WholeBlockDto[], cam: Camera, view: { w: number; h: number }, theme: Theme, overlay: boolean) {
  const z = cam.zoom;
  const size = headingSize(z);
  ctx.font = `600 ${size}px ${theme.serif}`;
  ctx.textAlign = 'left';
  ctx.textBaseline = 'alphabetic';
  const x1 = cam.x + view.w / z;
  const y1 = cam.y + view.h / z;
  if (!overlay) {
    ctx.fillStyle = theme.ink;
    for (const b of blocks) {
      const r = b.rect;
      if (r.x > x1 || r.x + r.w < cam.x || r.y > y1 || r.y + r.h < cam.y) continue;
      ctx.fillText(b.title, r.x, r.y + 21);
    }
    // A rule under each heading keeps a block's top edge visible at any zoom.
    ctx.beginPath();
    for (const b of blocks) {
      const r = b.rect;
      if (r.x > x1 || r.x + r.w < cam.x || r.y > y1 || r.y + r.h < cam.y) continue;
      ctx.moveTo(r.x, r.y + r.h - 2);
      ctx.lineTo(r.x + r.w, r.y + r.h - 2);
    }
    ctx.strokeStyle = theme.rule;
    ctx.lineWidth = 1 / z;
    ctx.stroke();
    return;
  }
  const pad = 4 / z;
  for (const b of blocks) {
    const r = b.rect;
    const top = r.y - size - 2 * pad;
    if (r.x > x1 || r.x + r.w < cam.x || r.y + 2 * pad < cam.y || top > y1) continue;
    const textW = ctx.measureText(b.title).width;
    // The label stays on screen while its block is, so you can tell which family you are over.
    const x = Math.min(Math.max(r.x, cam.x + 8 / z), Math.max(r.x, r.x + r.w - textW - 2 * pad));
    ctx.globalAlpha = 0.92;
    ctx.fillStyle = theme.paper;
    ctx.fillRect(x - pad, top, textW + 2 * pad, size + 2 * pad);
    ctx.globalAlpha = 1;
    ctx.strokeStyle = theme.rule;
    ctx.lineWidth = 1 / z;
    ctx.strokeRect(x - pad, top, textW + 2 * pad, size + 2 * pad);
    ctx.fillStyle = theme.ink;
    ctx.fillText(b.title, x, r.y - pad - size * 0.2);
  }
}

/** Draws the window: full cards when zoomed in, plain boxes in the middle, coloured bars with the surname far out. */
export function drawWhole(
  ctx: CanvasRenderingContext2D,
  win: ChartWindow,
  blocks: WholeBlockDto[],
  cam: Camera,
  view: { w: number; h: number; dpr: number },
  opts: WholeDrawOpts,
): { drawn: number } {
  const { theme, selected, home, lineage } = opts;
  const z = cam.zoom;
  const lod = lodFor(z);

  ctx.setTransform(view.dpr, 0, 0, view.dpr, 0, 0);
  ctx.clearRect(0, 0, view.w, view.h);
  ctx.setTransform(view.dpr * z, 0, 0, view.dpr * z, -cam.x * view.dpr * z, -cam.y * view.dpr * z);

  const overlay = headingIsOverlay(z);
  if (!overlay) drawHeadings(ctx, blocks, cam, view, theme, false);

  const lit = (id: number) => lineage === null || lineage.has(id);
  const parent: ChartEdge[] = [];
  const partner: ChartEdge[] = [];
  const hot: ChartEdge[] = [];
  const dim: ChartEdge[] = [];
  for (const e of win.edges) {
    if (lineage === null) (e.kind === 'partner' ? partner : parent).push(e);
    else if (edgeOnLine(e, lit)) hot.push(e);
    else dim.push(e);
  }
  if (lineage === null) {
    stroke(ctx, parent, theme.ink2, 1, z);
    stroke(ctx, partner, theme.ink2, 2, z);
  } else {
    ctx.globalAlpha = 0.25;
    stroke(ctx, dim, theme.ink2, 1, z);
    ctx.globalAlpha = 1;
    stroke(ctx, hot, theme.ribbon, 2, z);
  }

  const faded = win.nodes.filter((n) => !lit(n.id));
  const bright = lineage === null ? win.nodes : win.nodes.filter((n) => lit(n.id));
  const sel = selected === null ? null : (win.nodes.find((n) => n.id === selected) ?? null);

  const paint = (nodes: ChartNode[]) => {
    if (lod === 'dots') {
      // One batch per bar colour.
      for (const sex of ['male', 'female', 'other'] as const) {
        ctx.beginPath();
        for (const n of nodes) {
          const s = n.person?.sex;
          if ((s === 'male' ? 'male' : s === 'female' ? 'female' : 'other') === sex) ctx.rect(n.x, n.y, n.w, n.h);
        }
        ctx.fillStyle = barColor(sex, theme);
        ctx.fill();
      }
      return;
    }
    ctx.beginPath();
    for (const n of nodes) ctx.rect(n.x, n.y, n.w, n.h);
    ctx.fillStyle = theme.paper;
    ctx.fill();
    ctx.strokeStyle = theme.rule;
    ctx.lineWidth = 1 / z;
    ctx.stroke();
    // A thin sex-coloured edge on the left of every card.
    for (const sex of ['male', 'female', 'other'] as const) {
      ctx.beginPath();
      for (const n of nodes) {
        const s = n.person?.sex;
        if ((s === 'male' ? 'male' : s === 'female' ? 'female' : 'other') === sex) ctx.rect(n.x, n.y, 4, n.h);
      }
      ctx.fillStyle = barColor(sex, theme);
      ctx.fill();
    }
  };

  if (faded.length > 0) {
    ctx.globalAlpha = 0.3;
    paint(faded);
    ctx.globalAlpha = 1;
  }
  paint(bright);

  if (sel) {
    ctx.beginPath();
    ctx.rect(sel.x, sel.y, sel.w, sel.h);
    if (lod !== 'dots') {
      ctx.fillStyle = theme.wash;
      ctx.fill();
    }
    ctx.strokeStyle = theme.ribbon;
    ctx.lineWidth = Math.max(2, 3 / z);
    ctx.stroke();
  }

  if (home !== null && lod !== 'dots') {
    const f = win.nodes.find((n) => n.id === home);
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

  ctx.textBaseline = 'alphabetic';
  ctx.textAlign = 'left';
  if (lod === 'full') {
    for (const n of win.nodes) {
      if (!lit(n.id)) ctx.globalAlpha = 0.3;
      const left = n.x + (n.id === home ? 26 : 12);
      const maxW = n.x + n.w - 10 - left;
      ctx.fillStyle = theme.ink;
      ctx.font = `600 13px ${theme.serif}`;
      const sub = n.person?.lifespan ?? '';
      // How this person is related to the reference person, at the right of the second line.
      const rel = kinshipText(n.kinship);
      const lines = nameLines((t) => ctx.measureText(t).width, n.person?.name ?? '', maxW, sub !== '' || rel !== '');
      lines.forEach((line, i) => ctx.fillText(line, left, n.y + 23 + i * 18));
      let relW = 0;
      if (rel) {
        ctx.fillStyle = theme.ink2;
        ctx.font = `italic 11px ${theme.font}`;
        const text = fit(ctx, rel, n.w * 0.5);
        relW = ctx.measureText(text).width + 8;
        ctx.textAlign = 'right';
        ctx.fillText(text, n.x + n.w - 8, n.y + 43);
        ctx.textAlign = 'left';
      }
      if (sub) {
        ctx.fillStyle = theme.ink2;
        ctx.font = `11px ${theme.font}`;
        ctx.fillText(fit(ctx, sub, n.w - 20 - relW), n.x + 12, n.y + 43);
      }
      ctx.globalAlpha = 1;
    }
  } else if (lod === 'dots') {
    // The backend sends only the surname here. Write it on the bar when the bar is wide enough on screen to read it.
    const px = 11;
    const fontWorld = px / z;
    if (win.nodes.length > 0 && win.nodes[0].w * z >= 34) {
      ctx.font = `600 ${fontWorld}px ${theme.font}`;
      ctx.fillStyle = theme.paper;
      for (const n of win.nodes) {
        const name = n.person?.name ?? '';
        if (!name) continue;
        const maxW = n.w - 4 / z;
        const [line] = nameLines((t) => ctx.measureText(t).width, name, maxW, true);
        if (!lit(n.id)) ctx.globalAlpha = 0.3;
        ctx.fillText(line, n.x + 2 / z, n.y + n.h / 2 + fontWorld * 0.35);
        ctx.globalAlpha = 1;
      }
    }
  }
  if (overlay) drawHeadings(ctx, blocks, cam, view, theme, true);
  drawAgentBadges(ctx, win.nodes, opts.badged, theme, z);
  return { drawn: win.nodes.length };
}
