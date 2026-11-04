import type { Rect, WholeBlockDto } from '../api/types';

/**
 * The card to show first when there is no focus or home person: the left-most card of the top row of the
 * largest family (its oldest generation). `cards` is any window of the chart, for example the minimap's.
 * Block coordinates are not card coordinates: a big family's top row can lie far from the heading's corner.
 */
export function founderOf(blocks: WholeBlockDto[], cards: { id: number; x: number; y: number; w: number; h: number }[]): { id: number; rect: Rect } | null {
  const b = blocks.find((x) => !x.unconnected) ?? blocks[0];
  if (!b) return null;
  let best: (typeof cards)[number] | null = null;
  for (const c of cards) {
    if (c.x + c.w <= b.rect.x || c.x >= b.rect.x + b.rect.w || c.y < b.rect.y + b.rect.h) continue;
    if (!best || c.y < best.y || (c.y === best.y && c.x < best.x)) best = c;
  }
  return best ? { id: best.id, rect: { x: best.x, y: best.y, w: best.w, h: best.h } } : null;
}
