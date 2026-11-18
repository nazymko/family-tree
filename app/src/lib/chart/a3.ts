import type { Rect } from '../api/types';
import type { Camera } from './camera';
import type { ChartDirection } from './direction';

/** CSS pixels per millimetre (96 dpi). A page is laid out in these units; the SVG scales them to real millimetres. */
export const MM_PX = 96 / 25.4;
/** A3 paper, long and short side (mm). */
export const A3_LONG_MM = 420;
export const A3_SHORT_MM = 297;
/** Empty border around everything on the page (mm). */
export const A3_MARGIN_MM = 10;
/** Height of the title block at the top of the page (mm). */
export const A3_TITLE_MM = 16;
/** Type sizes of the title block (points): the chart name, and the tree name with the date. */
export const A3_TITLE_PT = 18;
export const A3_SUB_PT = 11;
/** Points to layout px (96 dpi). */
export const PT_PX = 96 / 72;
/** Resolution of the PNG. */
export const A3_DPI = 300;
/** Boxes are never printed larger than this multiple of their screen size. */
export const A3_MAX_SCALE = 1.5;
/** Names smaller than this (points) are reported as hard to read. */
export const A3_MIN_PT = 6;
/** Font size (px) of a person's name in a box. */
export const NAME_PX = 13;

export type A3Orientation = 'landscape' | 'portrait';
export type A3Choice = A3Orientation | 'auto';

export interface A3Layout {
  orientation: A3Orientation;
  /** Page size in mm and in layout px (96 dpi). */
  mm: { w: number; h: number };
  px: { w: number; h: number };
  /** Size of the 300 dpi PNG, whole pixels. */
  png: { w: number; h: number };
  /** PNG pixels per layout px (always 300/96). */
  pngScale: number;
  /** Printed size of a screen px of the chart (1 = as on screen). */
  scale: number;
  /** Camera that puts the chart centred in the printable area. */
  cam: Camera;
  /** The printable area below the title block, layout px. */
  area: Rect;
  /** Height of a person's name when printed, in points. */
  namePt: number;
  /** True when names would print below A3_MIN_PT. */
  tooSmall: boolean;
}

export const mmToPx = (mm: number): number => mm * MM_PX;
export const pxToMm = (px: number): number => px / MM_PX;
/** Printed height in points of text that is `px` high on screen, after scaling the chart by `scale`. */
export const printedPt = (px: number, scale: number): number => px * scale * 0.75;

function fitTo(b: Rect, o: A3Orientation) {
  const wMm = o === 'landscape' ? A3_LONG_MM : A3_SHORT_MM;
  const hMm = o === 'landscape' ? A3_SHORT_MM : A3_LONG_MM;
  const area: Rect = {
    x: mmToPx(A3_MARGIN_MM),
    y: mmToPx(A3_MARGIN_MM + A3_TITLE_MM),
    w: mmToPx(wMm - 2 * A3_MARGIN_MM),
    h: mmToPx(hMm - 2 * A3_MARGIN_MM - A3_TITLE_MM),
  };
  const scale = Math.min(A3_MAX_SCALE, area.w / Math.max(1, b.w), area.h / Math.max(1, b.h));
  return { wMm, hMm, area, scale };
}

/**
 * Lays a chart of the given bounds (world units) out on an A3 page: 10 mm margins, a title block on top, the chart
 * scaled uniformly (never above 1.5x its screen size) and centred in the rest. 'auto' takes the orientation that
 * allows the larger print (landscape on a tie).
 */
export function layoutA3(bounds: Rect, choice: A3Choice = 'auto'): A3Layout {
  let orientation: A3Orientation;
  if (choice === 'auto') orientation = fitTo(bounds, 'portrait').scale > fitTo(bounds, 'landscape').scale + 1e-9 ? 'portrait' : 'landscape';
  else orientation = choice;
  const { wMm, hMm, area, scale } = fitTo(bounds, orientation);
  const ox = area.x + (area.w - bounds.w * scale) / 2;
  const oy = area.y + (area.h - bounds.h * scale) / 2;
  const namePt = printedPt(NAME_PX, scale);
  return {
    orientation,
    mm: { w: wMm, h: hMm },
    px: { w: mmToPx(wMm), h: mmToPx(hMm) },
    png: { w: Math.round((wMm / 25.4) * A3_DPI), h: Math.round((hMm / 25.4) * A3_DPI) },
    pngScale: A3_DPI / 96,
    scale,
    cam: { x: bounds.x - ox / scale, y: bounds.y - oy / scale, zoom: scale },
    area,
    namePt,
    tooSmall: namePt < A3_MIN_PT,
  };
}

export interface DirectionFit {
  direction: ChartDirection;
  layout: A3Layout;
}

/**
 * Picks the chart direction for a print: the one (with the page orientation) that allows the largest uniform scale.
 * `bounds` holds the size of the chart laid out in each direction. A direction only wins when it is clearly larger
 * (more than 0.1%), so on a tie the direction the person is looking at stays.
 */
export function bestDirection(bounds: { direction: ChartDirection; bounds: Rect }[], current: ChartDirection, choice: A3Choice = 'auto'): DirectionFit {
  let best: DirectionFit | null = null;
  const ordered = [...bounds].sort((a, b) => Number(b.direction === current) - Number(a.direction === current));
  for (const c of ordered) {
    const layout = layoutA3(c.bounds, choice);
    if (!best || layout.scale > best.layout.scale * 1.001) best = { direction: c.direction, layout };
  }
  if (!best) throw new Error('no chart direction to choose from');
  return best;
}

/** How much of the printable area the chart fills (0..1 each way). */
export function fillRatio(bounds: Rect, l: A3Layout): { w: number; h: number } {
  return { w: (bounds.w * l.scale) / l.area.w, h: (bounds.h * l.scale) / l.area.h };
}

/** The HTML the print dialog gets: one A3 page holding the SVG, no browser margins. */
export function printDocument(svg: string, orientation: A3Orientation): string {
  const body = svg.replace(/^<\?xml[^>]*\?>\s*/, '');
  return (
    `<!doctype html><html><head><meta charset="utf-8"><style>` +
    `@page{size:A3 ${orientation};margin:0}html,body{margin:0;padding:0}svg{display:block}` +
    `</style></head><body>${body}</body></html>`
  );
}

/** Prints an HTML document through a hidden frame (the system print dialog; "Microsoft Print to PDF" makes a PDF). */
export function printHtml(html: string): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    const frame = document.createElement('iframe');
    frame.setAttribute('aria-hidden', 'true');
    frame.style.cssText = 'position:fixed;right:0;bottom:0;width:0;height:0;border:0;visibility:hidden';
    let finished = false;
    const done = () => {
      if (finished) return;
      finished = true;
      setTimeout(() => frame.remove(), 1000);
      resolve();
    };
    frame.onload = () => {
      const w = frame.contentWindow;
      if (!w) {
        frame.remove();
        reject(new Error('no print frame'));
        return;
      }
      try {
        w.addEventListener('afterprint', done, { once: true });
        w.focus();
        w.print();
        // Some webviews never fire afterprint; the call has returned by now in the ones that block.
        setTimeout(done, 3000);
      } catch (e) {
        frame.remove();
        reject(e);
      }
    };
    frame.srcdoc = html;
    document.body.appendChild(frame);
  });
}
