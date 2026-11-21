import { describe, expect, it } from 'vitest';
import type { Rect } from '../api/types';
import { A3_MAX_SCALE, a3Area, mmToPx, targetScale, type A3Choice } from './a3';
import { CAPTION_MAX_MM, CAPTION_MM, PIECE_GAP_MM, packPages, type PackItem, type PackedPage } from './pack';

const T = targetScale();
const GAP = mmToPx(PIECE_GAP_MM);
const opts = (choice: A3Choice = 'auto') => ({ target: T, choice, header: true });

/** The slot of an item on a page: its drawing at the page scale and the caption band above it. */
function slotRect(page: PackedPage, item: PackItem): Rect {
  const s = page.slots.find((x) => x.id === item.id)!;
  const band = item.capW > 0 ? mmToPx(CAPTION_MM) : 0;
  const w = Math.max(item.w * page.scale, Math.min(item.capW, mmToPx(CAPTION_MAX_MM)));
  return { x: s.x, y: s.y - band, w, h: item.h * page.scale + band };
}

function rng(seed: number) {
  let x = seed >>> 0;
  return () => {
    x = (x * 1664525 + 1013904223) >>> 0;
    return x / 2 ** 32;
  };
}

function randomItems(n: number, seed: number): PackItem[] {
  const r = rng(seed);
  return Array.from({ length: n }, (_, id) => ({ id, w: 50 + r() * 2950, h: 50 + r() * 2950, capW: id === 0 ? 0 : Math.round(r() * 400) }));
}

function expectNoOverlap(pages: PackedPage[], items: PackItem[]) {
  const byId = new Map(items.map((i) => [i.id, i]));
  for (const p of pages) {
    const area = a3Area(p.orientation, true);
    const rects = p.slots.map((s) => slotRect(p, byId.get(s.id)!));
    for (const r of rects) {
      expect(r.x >= area.x - 1e-6 && r.y >= area.y - 1e-6, 'inside').toBe(true);
      expect(r.x + r.w <= area.x + area.w + 1e-6 && r.y + r.h <= area.y + area.h + 1e-6, 'inside').toBe(true);
    }
    for (let i = 0; i < rects.length; i++)
      for (let j = i + 1; j < rects.length; j++) {
        const a = rects[i];
        const b = rects[j];
        const g = GAP / 2 - 1e-6;
        const hit = a.x - g < b.x + b.w + g && b.x - g < a.x + a.w + g && a.y - g < b.y + b.h + g && b.y - g < a.y + a.h + g;
        if (hit) expect.fail(`slots ${p.slots[i].id} and ${p.slots[j].id} overlap`);
      }
  }
}

describe('packPages', () => {
  it('every_item_once', () => {
    const items = randomItems(200, 7);
    const pages = packPages(items, opts());
    const seen = pages.flatMap((p) => p.slots.map((s) => s.id)).sort((a, b) => a - b);
    expect(seen).toEqual(items.map((i) => i.id));
  });

  it('no_overlap_random', () => {
    for (const seed of [1, 2, 3]) {
      const items = randomItems(200, seed);
      expectNoOverlap(packPages(items, opts()), items);
    }
  });

  it('page_scale_at_least_target', () => {
    const pages = packPages(randomItems(200, 5), opts());
    for (const p of pages) if (!p.alone) expect(p.scale).toBeGreaterThanOrEqual(T - 1e-9);
    for (const p of pages) expect(p.scale).toBeLessThanOrEqual(A3_MAX_SCALE + 1e-9);
  });

  it('scale_grows_to_fill', () => {
    const one = packPages([{ id: 0, w: 400, h: 300, capW: 0 }], opts());
    expect(one.length).toBe(1);
    expect(one[0].scale).toBeCloseTo(A3_MAX_SCALE, 9);
    const area = a3Area('landscape', true);
    const w = ((area.w - GAP) / 2 - 1) / T;
    const two = packPages(
      [
        { id: 0, w, h: 50 / T, capW: 0 },
        { id: 1, w, h: 50 / T, capW: 0 },
      ],
      opts(),
    );
    expect(two.length).toBe(1);
    expect(two[0].scale).toBeGreaterThan(T);
  });

  it('cover_first', () => {
    const items: PackItem[] = [{ id: 0, w: 10, h: 10, capW: 0 }, ...randomItems(30, 9).slice(1)];
    const pages = packPages(items, opts());
    expect(pages[0].slots[0].id).toBe(0);
    expect(pages[0].slots[0].capY).toBeNull();
  });

  it('ffd_count', () => {
    const area = a3Area('landscape', true);
    const tall = Array.from({ length: 6 }, (_, id) => ({ id, w: (area.w * 0.9) / T, h: (area.h / 2 + 5) / T, capW: 0 }));
    expect(packPages(tall, opts()).length).toBe(6);
    const third = Array.from({ length: 6 }, (_, id) => ({ id, w: (area.w * 0.99) / T, h: ((area.h - 2 * GAP) / 3 - 1) / T, capW: 0 }));
    expect(packPages(third, opts()).length).toBe(2);
  });

  it('fixed_orientation_everywhere', () => {
    const land = a3Area('landscape', true);
    const port = a3Area('portrait', true);
    const items: PackItem[] = [
      { id: 0, w: (port.w * 0.5) / T, h: 300, capW: 0 },
      // Wider than a portrait page at the target: alone.
      { id: 1, w: (land.w * 0.95) / T, h: 200, capW: 120 },
      { id: 2, w: (port.w * 0.6) / T, h: 900, capW: 120 },
      { id: 3, w: (land.w * 0.8) / T, h: 100, capW: 120 },
    ];
    const pages = packPages(items, opts('portrait'));
    for (const p of pages) expect(p.orientation).toBe('portrait');
    const where = (id: number) => pages.find((p) => p.slots.some((s) => s.id === id))!;
    expect(where(1).alone).toBe(true);
    expect(where(3).alone).toBe(true);
    expect(where(0).alone).toBe(false);
    expect(pages.flatMap((p) => p.slots).length).toBe(4);
    expectNoOverlap(pages, items);
  });

  it('oversize_alone', () => {
    const area = a3Area('landscape', true);
    const items: PackItem[] = [
      { id: 0, w: 200, h: 200, capW: 0 },
      { id: 1, w: (3 * area.w) / T, h: (3 * area.h) / T, capW: 200 },
    ];
    const pages = packPages(items, opts());
    const p = pages.find((x) => x.slots.some((s) => s.id === 1))!;
    expect(p.alone).toBe(true);
    expect(p.slots.length).toBe(1);
    expect(p.scale).toBeLessThan(T);
    expectNoOverlap(pages, items);
  });

  it('caption_wider_than_piece', () => {
    const items: PackItem[] = [
      { id: 0, w: 2000, h: 2000, capW: 0 },
      { id: 1, w: 50, h: 100, capW: 300 },
      { id: 2, w: 50, h: 100, capW: 300 },
    ];
    const pages = packPages(items, opts());
    const p = pages.find((x) => x.slots.some((s) => s.id === 1))!;
    expect(slotRect(p, items[1]).w).toBe(300);
    expectNoOverlap(pages, items);
  });

  it('deterministic', () => {
    const items = randomItems(120, 11);
    expect(packPages(items, opts())).toEqual(packPages(items, opts()));
  });
});
