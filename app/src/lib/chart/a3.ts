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

/** Which direction an A3 print uses: the one on the screen, or the one that prints largest. */
export type A3DirectionMode = 'screen' | 'best';

/**
 * The direction and page layout of an A3 print. 'screen' keeps `current` (the page orientation still follows `choice`,
 * so 'auto' turns the page for that direction); 'best' is `bestDirection`. `bounds` must hold `current` for 'screen'.
 */
export function printDirection(bounds: { direction: ChartDirection; bounds: Rect }[], current: ChartDirection, mode: A3DirectionMode, choice: A3Choice = 'auto'): DirectionFit {
  if (mode === 'best') return bestDirection(bounds, current, choice);
  const own = bounds.find((b) => b.direction === current);
  if (!own) throw new Error('the chart was not laid out in its own direction');
  return { direction: current, layout: layoutA3(own.bounds, choice) };
}

/** How much of the printable area the chart fills (0..1 each way). */
export function fillRatio(bounds: Rect, l: A3Layout): { w: number; h: number } {
  return { w: (bounds.w * l.scale) / l.area.w, h: (bounds.h * l.scale) / l.area.h };
}

/** Shortens `text` with an ellipsis until `measure` says it fits in `maxW`. */
export function ellipsize(measure: (s: string) => number, text: string, maxW: number): string {
  if (measure(text) <= maxW) return text;
  let lo = 0;
  let hi = text.length;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (measure(text.slice(0, mid).trimEnd() + '\u2026') <= maxW) lo = mid;
    else hi = mid - 1;
  }
  return lo === 0 ? '' : text.slice(0, lo).trimEnd() + '\u2026';
}

/**
 * Smallest printed size (points) a name line on a print card is shrunk to before it wraps instead: a point above
 * A3_MIN_PT, the size below which names count as hard to read.
 */
export const A3_NAME_FLOOR_PT = A3_MIN_PT + 1;
/** Name lines a print card has room for above the dates. */
export const NAME_MAX_LINES = 3;

/**
 * The smallest size (px, card units) a name line may be shrunk to on a print laid out at `scale`: A3_NAME_FLOOR_PT on
 * paper, and never above the normal name size (a chart printed so small that its names are already under that size
 * does not shrink them at all; a long one wraps instead).
 */
export function nameFloorPx(scale: number): number {
  return Math.min(NAME_PX, A3_NAME_FLOOR_PT / (0.75 * Math.max(1e-6, scale)));
}
/** The floor for a print at the size of the screen (scale 1). */
export const NAME_FLOOR_PX = nameFloorPx(1);

/** One line of a name on a print card and the size (px) it is drawn at. */
export interface NameLine {
  text: string;
  px: number;
}

type Measure = (s: string, px: number) => number;
/** The room of the n-th line of a part of a name (the first line may be shorter, beside the ancestor number). */
type Room = (line: number) => number;

/** Splits a word that is too wide at `px` into pieces that fit, after a hyphen when there is one, else between letters. */
function breakWord(measure: Measure, word: string, room: Room, first: number, px: number): string[] {
  const out: string[] = [];
  let rest = word;
  while (rest && measure(rest, px) > room(first + out.length)) {
    const maxW = room(first + out.length);
    // The longest start that fits (at least one letter, so this always ends).
    let lo = 1;
    let hi = rest.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (measure(rest.slice(0, mid), px) <= maxW) lo = mid;
      else hi = mid - 1;
    }
    const hyphen = rest.lastIndexOf('-', lo - 1);
    const at = hyphen > 0 ? hyphen + 1 : lo;
    out.push(rest.slice(0, at));
    rest = rest.slice(at);
  }
  if (rest) out.push(rest);
  return out;
}

/** Lays `text` out in lines that fit at `px`: whole words where they fit, a too-long word broken. */
function wrapAt(measure: Measure, text: string, room: Room, px: number): string[] {
  const lines: string[] = [];
  let cur = '';
  for (const word of text.split(/\s+/).filter(Boolean)) {
    const joined = cur ? `${cur} ${word}` : word;
    if (measure(joined, px) <= room(lines.length)) {
      cur = joined;
      continue;
    }
    if (cur) lines.push(cur);
    const pieces = breakWord(measure, word, room, lines.length, px);
    cur = pieces.pop() ?? '';
    lines.push(...pieces);
  }
  if (cur) lines.push(cur);
  return lines;
}

/** One part of a name (the given names, or the surname) in as few lines as it needs, never cut. */
function fitPart(measure: Measure, text: string, room: Room, basePx: number, minPx: number): NameLine[] {
  const maxW = room(0);
  const w = measure(text, basePx);
  if (w <= maxW) return [{ text, px: basePx }];
  // Text width grows with the font size, so the size that just fits is close to base * room / width; step down to be sure.
  let px = Math.floor(((basePx * maxW) / w) * 4) / 4;
  while (px >= minPx && measure(text, px) > maxW) px -= 0.25;
  if (px >= minPx) return [{ text, px }];
  return wrapAt(measure, text, room, minPx).map((t) => ({ text: t, px: minPx }));
}

/**
 * The name lines of a print (compact) card, nothing ever cut with "…". A name that fits stays one line at `basePx`.
 * Otherwise it goes on two lines as on the cards: the given names, then the rest (the surname); each line that is too
 * wide is drawn smaller (that line only), down to `minPx`; a line still too wide at `minPx` wraps onto extra lines at
 * `minPx` (at spaces, then after a hyphen, then between letters). When that needs more than `maxLines` lines the
 * floor is lowered step by step, so an absurdly long name still prints whole.
 * `measure(text, px)` is the text's width at that size in the card's font. The first line has `maxW`; the lines under
 * it have `restW` (they are below the ancestor number, so they may use the whole card width).
 */
export function fitPrintName(
  measure: Measure,
  name: string,
  maxW: number,
  basePx = NAME_PX,
  minPx = NAME_FLOOR_PX,
  maxLines = NAME_MAX_LINES,
  restW = maxW,
): NameLine[] {
  const text = name.trim().replace(/\s+/g, ' ');
  if (!text) return [];
  if (maxW <= 0) return [{ text, px: basePx }];
  if (measure(text, basePx) <= maxW) return [{ text, px: basePx }];
  const rest = Math.max(maxW, restW);
  const words = text.split(' ');
  const parts: string[] = [];
  if (words.length > 1) {
    // As many leading words as fit on the first line, at least the last word on the second (the cards' own split).
    let i = 1;
    while (i < words.length - 1 && measure(words.slice(0, i + 1).join(' '), basePx) <= maxW) i++;
    parts.push(words.slice(0, i).join(' '), words.slice(i).join(' '));
  } else {
    parts.push(text);
  }
  let floor = Math.min(minPx, basePx);
  for (;;) {
    const lines: NameLine[] = [];
    for (const p of parts) {
      const at = lines.length;
      lines.push(...fitPart(measure, p, (n) => (at + n === 0 ? maxW : rest), basePx, floor));
    }
    if (lines.length <= Math.max(1, maxLines) || floor <= 2) return lines;
    floor = Math.max(2, floor * 0.85);
  }
}

/**
 * Baselines (from the card's top) of a print card's name lines. One or two lines at the normal size keep the card's
 * own places (23; 16 and 31); other mixes are stacked (a quarter of the line above for its descenders, then 0.9 of the
 * line itself: 1.15 times the size when both are equal) around the same middle, never above the card's top padding,
 * and the second line starts below the ancestor number's row (baseline 14).
 */
export function nameBaselines(lines: NameLine[], basePx = NAME_PX): number[] {
  if (lines.length === 0) return [];
  const plain = lines.every((l) => l.px === basePx);
  if (plain && lines.length === 1) return [23];
  if (plain && lines.length === 2) return [16, 31];
  const steps = lines.slice(1).map((l, i) => 0.25 * lines[i].px + 0.9 * l.px);
  const span = steps.reduce((a, b) => a + b, 0);
  let y = Math.max(23.5 - span / 2, 3 + 0.75 * lines[0].px);
  const out = [y];
  lines.slice(1).forEach((l, i) => {
    y = i === 0 ? Math.max(y + steps[0], 15 + 0.8 * l.px) : y + steps[i];
    out.push(y);
  });
  return out;
}

/** The HTML the print dialog gets: one A3 page holding the SVG, no browser margins, nothing that could add a second page. */
export function printDocument(svg: string, orientation: A3Orientation): string {
  const body = svg.replace(/^<\?xml[^>]*\?>\s*/, '');
  const w = orientation === 'landscape' ? A3_LONG_MM : A3_SHORT_MM;
  const h = orientation === 'landscape' ? A3_SHORT_MM : A3_LONG_MM;
  return (
    `<!doctype html><html><head><meta charset="utf-8"><style>` +
    `@page{size:A3 ${orientation};margin:0}` +
    `html,body{margin:0;padding:0;width:${w}mm;height:${h}mm;overflow:hidden}` +
    `svg{display:block;page-break-after:avoid;break-after:avoid}` +
    `</style></head><body>${body}</body></html>`
  );
}

/**
 * Prints an HTML document through a hidden frame (the system print dialog; "Microsoft Print to PDF" makes a PDF).
 * Resolves when the print call has returned; rejects when the frame does not load within `loadMs`. The frame is removed
 * after the print finished (`afterprint`), or after `cleanupMs` when the webview never says so: never while the dialog is open.
 */
export function printHtml(html: string, opts: { loadMs?: number; cleanupMs?: number } = {}): Promise<void> {
  const loadMs = opts.loadMs ?? 15_000;
  const cleanupMs = opts.cleanupMs ?? 120_000;
  return new Promise<void>((resolve, reject) => {
    const frame = document.createElement('iframe');
    frame.setAttribute('aria-hidden', 'true');
    frame.style.cssText = 'position:fixed;right:0;bottom:0;width:0;height:0;border:0;visibility:hidden';
    let cleanup: ReturnType<typeof setTimeout> | undefined;
    const remove = () => {
      if (cleanup !== undefined) clearTimeout(cleanup);
      frame.remove();
    };
    const loadTimer = setTimeout(() => {
      remove();
      reject(new Error('The print page did not load.'));
    }, loadMs);
    frame.onload = () => {
      clearTimeout(loadTimer);
      const w = frame.contentWindow;
      if (!w) {
        remove();
        reject(new Error('The print page did not load.'));
        return;
      }
      try {
        w.addEventListener('afterprint', remove, { once: true });
        cleanup = setTimeout(remove, cleanupMs);
        w.focus();
        w.print();
        resolve();
      } catch (e) {
        remove();
        reject(e);
      }
    };
    frame.srcdoc = html;
    document.body.appendChild(frame);
  });
}
