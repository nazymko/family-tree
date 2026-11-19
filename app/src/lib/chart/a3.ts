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
/** The page behind a picture: the app's paper tint, plain white, or nothing (a transparent PNG / SVG). */
export type A3Background = 'paper' | 'white' | 'transparent';

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

function fitTo(b: Rect, o: A3Orientation, title = true) {
  const titleMm = title ? A3_TITLE_MM : 0;
  const wMm = o === 'landscape' ? A3_LONG_MM : A3_SHORT_MM;
  const hMm = o === 'landscape' ? A3_SHORT_MM : A3_LONG_MM;
  const area: Rect = {
    x: mmToPx(A3_MARGIN_MM),
    y: mmToPx(A3_MARGIN_MM + titleMm),
    w: mmToPx(wMm - 2 * A3_MARGIN_MM),
    h: mmToPx(hMm - 2 * A3_MARGIN_MM - titleMm),
  };
  const scale = Math.min(A3_MAX_SCALE, area.w / Math.max(1, b.w), area.h / Math.max(1, b.h));
  return { wMm, hMm, area, scale };
}

/**
 * Lays a chart of the given bounds (world units) out on an A3 page: 10 mm margins, a title block on top, the chart
 * scaled uniformly (never above 1.5x its screen size) and centred in the rest. 'auto' takes the orientation that
 * allows the larger print (landscape on a tie). Without the title block (`title` false) the chart gets its height too.
 */
export function layoutA3(bounds: Rect, choice: A3Choice = 'auto', title = true): A3Layout {
  let orientation: A3Orientation;
  if (choice === 'auto') orientation = fitTo(bounds, 'portrait', title).scale > fitTo(bounds, 'landscape', title).scale + 1e-9 ? 'portrait' : 'landscape';
  else orientation = choice;
  const { wMm, hMm, area, scale } = fitTo(bounds, orientation, title);
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
export function bestDirection(bounds: { direction: ChartDirection; bounds: Rect }[], current: ChartDirection, choice: A3Choice = 'auto', title = true): DirectionFit {
  let best: DirectionFit | null = null;
  const ordered = [...bounds].sort((a, b) => Number(b.direction === current) - Number(a.direction === current));
  for (const c of ordered) {
    const layout = layoutA3(c.bounds, choice, title);
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
export function printDirection(bounds: { direction: ChartDirection; bounds: Rect }[], current: ChartDirection, mode: A3DirectionMode, choice: A3Choice = 'auto', title = true): DirectionFit {
  if (mode === 'best') return bestDirection(bounds, current, choice, title);
  const own = bounds.find((b) => b.direction === current);
  if (!own) throw new Error('the chart was not laid out in its own direction');
  return { direction: current, layout: layoutA3(own.bounds, choice, title) };
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
/** Name lines a print card may take (the print makes its cards taller for them) before the names are drawn smaller. */
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

/** A print name line may always be drawn this much smaller, to keep the given name and patronymic together on one line. */
export const NAME_SLIGHT = 0.85;

/** The largest size (quarter px steps, at most `basePx`) at which `text` fits `maxW`; null when even `minPx` is too big. */
function sizeToFit(measure: Measure, text: string, maxW: number, basePx: number, minPx: number): number | null {
  const w = measure(text, basePx);
  if (w <= maxW) return basePx;
  // Text width grows with the font size, so the size that just fits is close to base * room / width; step down to be sure.
  let px = Math.floor(((basePx * maxW) / w) * 4) / 4;
  while (px >= minPx && measure(text, px) > maxW) px -= 0.25;
  return px >= minPx && px > 0 ? px : null;
}

/** One line of words, drawn smaller when it must (down to `minPx`), wrapped at `minPx` when even that is too wide. */
function fitPart(measure: Measure, text: string, room: Room, basePx: number, minPx: number): NameLine[] {
  const px = sizeToFit(measure, text, room(0), basePx, minPx);
  if (px !== null) return [{ text, px }];
  return wrapAt(measure, text, room, minPx).map((t) => ({ text: t, px: minPx }));
}

/** Whole words packed into lines at `basePx`; a line holding one word that is too wide is drawn smaller or broken (fitPart). */
function packWords(measure: Measure, words: string[], room: Room, basePx: number, minPx: number): NameLine[] {
  const out: NameLine[] = [];
  const push = (text: string) => {
    const at = out.length;
    out.push(...fitPart(measure, text, (n) => room(at + n), basePx, minPx));
  };
  let cur = '';
  for (const word of words) {
    const joined = cur ? `${cur} ${word}` : word;
    if (!cur || measure(joined, basePx) <= room(out.length)) {
      cur = joined;
      continue;
    }
    push(cur);
    cur = word;
  }
  if (cur) push(cur);
  return out;
}

/**
 * The name lines of a print (compact) card, nothing ever cut with "…". A name that fits stays one line at `basePx`.
 * Otherwise it takes two lines when each fits, the line too wide drawn smaller (that line only) down to `minPx`, and
 * always down to NAME_SLIGHT of `basePx`: all words but the last on the first (the given name and the patronymic of a
 * three-word name), the last (the surname) on the second. Otherwise whole words are packed into lines at `basePx`
 * (given name / patronymic / surname when no two fit together), and a single word too wide is drawn smaller, down
 * to `minPx`, then broken (after a hyphen, else between letters). When that needs more than `maxLines` lines the floor
 * is lowered step by step, so an absurdly long name still prints whole. The card is made taller for the extra lines
 * (see planPrintCard and printStretch), never drawn over the dates.
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
  const room: Room = (n) => (n === 0 ? maxW : rest);
  const words = text.split(' ');
  let floor = Math.min(minPx, basePx);
  for (;;) {
    const slight = Math.min(floor, basePx * NAME_SLIGHT);
    if (words.length > 1) {
      // The given name(s) and patronymic, then the surname: each line drawn smaller when it must.
      const a = words.slice(0, -1).join(' ');
      const b = words[words.length - 1];
      const pa = sizeToFit(measure, a, maxW, basePx, slight);
      const pb = pa === null ? null : sizeToFit(measure, b, rest, basePx, slight);
      if (pa !== null && pb !== null) return [{ text: a, px: pa }, { text: b, px: pb }];
    }
    const lines = packWords(measure, words, room, basePx, floor);
    if (lines.length <= Math.max(1, maxLines) || floor <= 2) return lines;
    floor = Math.max(2, floor * 0.85);
  }
}

/** Size (px) of the dates and of the relationship on a card. */
export const CARD_SMALL_PX = 11;
/** Print card text metrics (fractions of the size): room above the baseline (capitals with a breve, Й) and below it (у, д). */
export const TEXT_ASCENT = 0.9;
export const TEXT_DESCENT = 0.25;
/** Print card spacing (px): above the first line and below the last, between the name and the dates, between the dates and a relationship under them. */
const CARD_PAD = 4;
const NAME_GAP = 3;
const REL_GAP = 2;
/** The dates start this far from the card's left edge; the relationship ends this far from its right edge. */
export const CARD_TEXT_X = 10;
export const CARD_TEXT_RIGHT = 8;
/** Room between the dates and a relationship beside them on the same row (px). */
const ROW_GAP = 10;
/** A name line under the first starts below this (px from the top): the ancestor number's baseline (14) and its descent. */
const AHNEN_CLEAR = 17;

/** What a print card shows, and the room it has. */
export interface PrintCardText {
  name: string;
  /** The dates (or "shown elsewhere"); may be empty. */
  sub: string;
  /** How the person is related to the centred one; may be empty. */
  rel: string;
  /** Card width (px). */
  w: number;
  /** Where the name starts, from the card's left edge (after the ribbon of the centre person). */
  left: number;
  /** Room the ancestor number takes at the right of the first name line (0 when there is none). */
  ahnenW: number;
}

/** Width (px) of `text` at `px` in the card's name font, the dates' font or the relationship's (italic). */
export type CardMeasure = (text: string, font: 'name' | 'sub' | 'rel', px: number) => number;

/** Where a print card's text goes: baselines from the card's top. The relationship is right-aligned; the dates left. */
export interface PrintCardPlan {
  lines: NameLine[];
  nameYs: number[];
  sub: { text: string; px: number; y: number } | null;
  rel: { text: string; px: number; y: number } | null;
  /** Height the text takes, padding included. */
  height: number;
}

/** The size (at most `px`) at which `text` fits `maxW`, however small that is (the dates and relationship are never cut). */
function shrinkTo(measure: (px: number) => number, maxW: number, px: number): number {
  const w = measure(px);
  if (w <= maxW || w <= 0) return px;
  let p = Math.floor(((px * maxW) / w) * 4) / 4;
  while (p > 0.5 && measure(p) > maxW) p -= 0.25;
  return Math.max(0.5, p);
}

/** The card's text stacked from the top with every size multiplied by `f`. */
function stackCard(measure: CardMeasure, c: PrintCardText, f: number, minPx: number): PrintCardPlan {
  const base = NAME_PX * f;
  const floor = Math.min(minPx, NAME_PX) * f;
  const small = CARD_SMALL_PX * f;
  const rest = c.w - 10 - c.left;
  const lines = fitPrintName((s, px) => measure(s, 'name', px), c.name, rest - c.ahnenW, base, floor, NAME_MAX_LINES, rest);
  const nameYs: number[] = [];
  let y = CARD_PAD;
  lines.forEach((l, i) => {
    if (i === 0) y += TEXT_ASCENT * l.px;
    else {
      y += TEXT_DESCENT * lines[i - 1].px + TEXT_ASCENT * l.px;
      if (i === 1 && c.ahnenW > 0) y = Math.max(y, AHNEN_CLEAR + TEXT_ASCENT * l.px);
    }
    nameYs.push(y);
  });
  if (lines.length > 0) y += TEXT_DESCENT * lines[lines.length - 1].px + NAME_GAP;
  const room = c.w - CARD_TEXT_X - CARD_TEXT_RIGHT;
  const subPx = c.sub ? shrinkTo((px) => measure(c.sub, 'sub', px), room, small) : small;
  const relPx = c.rel ? shrinkTo((px) => measure(c.rel, 'rel', px), room, small) : small;
  // The relationship shares the dates' row only when both fit there at full size; otherwise it gets a row of its own.
  const onRow =
    c.rel !== '' &&
    relPx === small &&
    (c.sub === '' || (subPx === small && measure(c.sub, 'sub', small) + ROW_GAP + measure(c.rel, 'rel', small) <= room));
  let sub: PrintCardPlan['sub'] = null;
  let rel: PrintCardPlan['rel'] = null;
  if (c.sub || onRow) {
    y += TEXT_ASCENT * small;
    if (c.sub) sub = { text: c.sub, px: subPx, y };
    if (onRow) rel = { text: c.rel, px: small, y };
    y += TEXT_DESCENT * small;
  }
  if (c.rel && !onRow) {
    y += (c.sub ? REL_GAP : 0) + TEXT_ASCENT * relPx;
    rel = { text: c.rel, px: relPx, y };
    y += TEXT_DESCENT * relPx;
  }
  return { lines, nameYs, sub, rel, height: y + CARD_PAD };
}

/** The height (px) a print card needs to show all its text at full size, nothing overlapping or cut. */
export function printCardHeight(measure: CardMeasure, c: PrintCardText, minPx = NAME_FLOOR_PX): number {
  return stackCard(measure, c, 1, minPx).height;
}

/**
 * Lays a print card's text out in a card `h` high: the name lines, the dates under them, the relationship beside the
 * dates when both fit on the row, else on its own row under them; nothing ever overlaps or is cut with "…". The text
 * is centred in the card's height. When the card is too low for it (a print that could not grow its cards), every
 * size is reduced together until it fits.
 */
export function planPrintCard(measure: CardMeasure, c: PrintCardText, h: number, minPx = NAME_FLOOR_PX): PrintCardPlan {
  let f = 1;
  let p = stackCard(measure, c, f, minPx);
  while (p.height > h + 1e-6 && f > 0.2) {
    f *= 0.93;
    p = stackCard(measure, c, f, minPx);
  }
  const dy = Math.max(0, (h - p.height) / 2);
  return {
    lines: p.lines,
    nameYs: p.nameYs.map((y) => y + dy),
    sub: p.sub && { ...p.sub, y: p.sub.y + dy },
    rel: p.rel && { ...p.rel, y: p.rel.y + dy },
    height: p.height,
  };
}

/**
 * A chart with every height multiplied by `k` (card tops and heights, connector points): taller cards, the rows
 * re-spaced in proportion, the connectors still meeting the cards where they did.
 */
export function stretchScene<S extends { nodes: { y: number; h: number }[]; edges: { pts: number[] }[] }>(scene: S, k: number): S {
  if (k === 1) return scene;
  return {
    ...scene,
    nodes: scene.nodes.map((n) => ({ ...n, y: n.y * k, h: n.h * k })),
    edges: scene.edges.map((e) => ({ ...e, pts: e.pts.map((v, i) => (i % 2 === 1 ? v * k : v)) })),
  };
}

/**
 * How much a print stretches its chart's height so the cards are `need` times as tall: all of it when the page has
 * that room left empty (a wide chart on a page it fills across), else as much as fits without printing the chart any
 * smaller (the cards then fit their text by drawing it a little smaller, see planPrintCard).
 */
export function printStretch(bounds: Rect, need: number, choice: A3Choice = 'auto', title = true): number {
  if (!(need > 1)) return 1;
  const s1 = layoutA3(bounds, choice, title).scale;
  const ok = (k: number) => layoutA3({ ...bounds, y: bounds.y * k, h: bounds.h * k }, choice, title).scale >= s1 * (1 - 1e-9);
  if (ok(need)) return need;
  let lo = 1;
  let hi = need;
  for (let i = 0; i < 30; i++) {
    const mid = (lo + hi) / 2;
    if (ok(mid)) lo = mid;
    else hi = mid;
  }
  return lo;
}

/**
 * The HTML the print dialog gets: one A3 page holding the SVG, no browser margins, nothing that could add a second page.
 * `fontCss` is the app's `@font-face` rules: the frame is a document of its own, and without them the names print in
 * a system font (Georgia has no ѣ, ѳ, ѵ).
 */
export function printDocument(svg: string, orientation: A3Orientation, fontCss = ''): string {
  const body = svg.replace(/^<\?xml[^>]*\?>\s*/, '');
  const w = orientation === 'landscape' ? A3_LONG_MM : A3_SHORT_MM;
  const h = orientation === 'landscape' ? A3_SHORT_MM : A3_LONG_MM;
  return (
    `<!doctype html><html><head><meta charset="utf-8"><style>` +
    (fontCss ? `${fontCss.replaceAll('</', '<\\/')}\n` : '') +
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
