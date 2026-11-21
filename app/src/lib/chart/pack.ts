import type { Rect } from '../api/types';
import { A3_MAX_SCALE, a3Area, mmToPx, type A3Choice, type A3Orientation } from './a3';

/**
 * Packing the pieces of a "sheets by branch" set onto A3 pages (spec amendment 21.11.2026, "Packing"): first-fit
 * decreasing shelf packing at the target scale, then each page's one common scale grown as far as its pieces still pack.
 */

/** Height of the caption band above a packed piece (mm), and its text (points). */
export const CAPTION_MM = 6;
export const CAPTION_PT = 9;
/** A caption longer than this (mm) ends in an ellipsis. */
export const CAPTION_MAX_MM = 140;
/** Room between packed pieces, side by side and between shelves (mm). */
export const PIECE_GAP_MM = 6;

/** A piece to pack: its drawing `w` x `h` (world units) and its caption's width (layout px; 0 = no caption). */
export interface PackItem {
  id: number;
  w: number;
  h: number;
  capW: number;
}

/** Where a piece lands on its page (layout px): the top-left of its drawing, and the top of its caption band. */
export interface PackedSlot {
  id: number;
  x: number;
  y: number;
  capY: number | null;
}

export interface PackedPage {
  orientation: A3Orientation;
  /** The page's one scale (printed size of a world unit, layout px). */
  scale: number;
  slots: PackedSlot[];
  /** One piece too big for a page at the target, at its own best scale. */
  alone: boolean;
}

const GROW_STEPS = 24;
const EPS = 1e-9;

const band = (it: PackItem) => (it.capW > 0 ? mmToPx(CAPTION_MM) : 0);
const slotW = (it: PackItem, s: number) => Math.max(it.w * s, Math.min(it.capW, mmToPx(CAPTION_MAX_MM)));
const slotH = (it: PackItem, s: number) => it.h * s + band(it);

interface Placed {
  item: PackItem;
  x: number;
  y: number;
}

/** Shelf-packs `items` in order into one area at scale `s`; null when they do not all fit. Positions relative to the area. */
function shelfPack(items: PackItem[], s: number, area: Rect): { placed: Placed[]; w: number; h: number } | null {
  const gap = mmToPx(PIECE_GAP_MM);
  const shelves: { y: number; h: number; x: number }[] = [];
  let used = 0;
  let maxW = 0;
  const placed: Placed[] = [];
  for (const it of items) {
    const w = slotW(it, s);
    const h = slotH(it, s);
    if (w > area.w + EPS || h > area.h + EPS) return null;
    let shelf = shelves.find((sh) => h <= sh.h + EPS && sh.x + w <= area.w + EPS);
    if (!shelf) {
      const y = shelves.length ? used + gap : 0;
      if (y + h > area.h + EPS) return null;
      shelf = { y, h, x: 0 };
      shelves.push(shelf);
      used = y + h;
    }
    placed.push({ item: it, x: shelf.x, y: shelf.y });
    maxW = Math.max(maxW, shelf.x + w);
    shelf.x += w + gap;
  }
  return { placed, w: maxW, h: used };
}

/** The slots of `items` packed at `s` on a page of orientation `o`, the packed block centred in the printable area. */
function slotsAt(items: PackItem[], s: number, o: A3Orientation, header: boolean): PackedSlot[] {
  const area = a3Area(o, header);
  const r = shelfPack(items, s, area)!;
  const ox = area.x + (area.w - r.w) / 2;
  const oy = area.y + (area.h - r.h) / 2;
  return r.placed.map((p) => {
    const b = band(p.item);
    return { id: p.item.id, x: ox + p.x, y: oy + p.y + b, capY: b > 0 ? oy + p.y : null };
  });
}

/** The largest scale in [lo, A3_MAX_SCALE] at which `items` pack on one page of orientation `o` (lo itself must). */
function grow(items: PackItem[], lo: number, o: A3Orientation, header: boolean): number {
  const area = a3Area(o, header);
  if (shelfPack(items, A3_MAX_SCALE, area)) return A3_MAX_SCALE;
  let a = lo;
  let b = A3_MAX_SCALE;
  for (let i = 0; i < GROW_STEPS; i++) {
    const mid = (a + b) / 2;
    if (shelfPack(items, mid, area)) a = mid;
    else b = mid;
  }
  return a;
}

/**
 * Packs pieces onto A3 pages. `items[0]` is the cover: the first slot of page 1. A piece whose slot does not fit any
 * allowed orientation at `target` gets a page of its own at the largest scale that fits it (`alone`). The others, the
 * cover first and then by slot height (tallest first, ties by id), go on the first shelf of the first page with room,
 * else on a new shelf of the first page with height left, else on a new page (landscape unless only portrait fits; the
 * chosen orientation when one is fixed). Then each page's scale grows to the largest (at most `A3_MAX_SCALE`) at which
 * its pieces, in the same order, still pack on it, in each allowed orientation (the larger wins, landscape on a tie),
 * and the packed block is centred on the page. Pages are in the order they were opened.
 */
export function packPages(items: PackItem[], o: { target: number; choice: A3Choice; header: boolean }): PackedPage[] {
  const allowed: A3Orientation[] = o.choice === 'auto' ? ['landscape', 'portrait'] : [o.choice];
  const fitsAt = (it: PackItem, or: A3Orientation) => {
    const area = a3Area(or, o.header);
    return slotW(it, o.target) <= area.w + EPS && slotH(it, o.target) <= area.h + EPS;
  };
  const order = items.length ? [items[0], ...items.slice(1).sort((a, b) => slotH(b, o.target) - slotH(a, o.target) || a.id - b.id)] : [];

  type Draft = { orientation: A3Orientation; items: PackItem[]; alone: boolean };
  const drafts: Draft[] = [];
  for (const it of order) {
    const ok = allowed.filter((or) => fitsAt(it, or));
    if (!ok.length) {
      drafts.push({ orientation: allowed[0], items: [it], alone: true });
      continue;
    }
    let home = drafts.find((d) => !d.alone && shelfPack([...d.items, it], o.target, a3Area(d.orientation, o.header)));
    if (home) home.items.push(it);
    else {
      home = { orientation: ok[0], items: [it], alone: false };
      drafts.push(home);
    }
  }

  return drafts.map((d) => {
    if (d.alone) {
      const it = d.items[0];
      let best: { or: A3Orientation; s: number } | null = null;
      for (const or of allowed) {
        const area = a3Area(or, o.header);
        const s = Math.min(A3_MAX_SCALE, area.w / Math.max(1e-9, it.w), (area.h - band(it)) / Math.max(1e-9, it.h));
        if (!best || s > best.s + EPS) best = { or, s };
      }
      return { orientation: best!.or, scale: best!.s, slots: slotsAt(d.items, best!.s, best!.or, o.header), alone: true };
    }
    let best: { or: A3Orientation; s: number } | null = null;
    for (const or of allowed) {
      if (!shelfPack(d.items, o.target, a3Area(or, o.header))) continue;
      const s = grow(d.items, o.target, or, o.header);
      if (!best || s > best.s + EPS) best = { or, s };
    }
    return { orientation: best!.or, scale: best!.s, slots: slotsAt(d.items, best!.s, best!.or, o.header), alone: false };
  });
}
