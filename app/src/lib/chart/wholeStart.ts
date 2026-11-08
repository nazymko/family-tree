import type { Rect, WholeBlockDto } from '../api/types';
import type { ChartDirection } from './direction';

/**
 * The card to show first when there is no focus or home person: the first card of the oldest generation of the
 * largest family (the top row, left-most card, when the ancestors are on top). `cards` is any window of the chart, for example the minimap's.
 * Block coordinates are not card coordinates: a big family's top row can lie far from the heading's corner.
 */
/** Gap between a heading and the first row of its block (the layout's HEADING_GAP). */
const HEADING_GAP = 12;

export function founderOf(
  blocks: WholeBlockDto[],
  cards: { id: number; x: number; y: number; w: number; h: number }[],
  dir: ChartDirection = 'top',
): { id: number; rect: Rect } | null {
  const b = blocks.find((x) => !x.unconnected) ?? blocks[0];
  if (!b) return null;
  let best: (typeof cards)[number] | null = null;
  // The oldest generation is the first row (column) in the direction of the ancestors; across it, the first card.
  const key = (c: (typeof cards)[number]) => (dir === 'top' ? c.y : dir === 'bottom' ? -c.y : dir === 'left' ? c.x : -c.x);
  const across = (c: (typeof cards)[number]) => (dir === 'left' || dir === 'right' ? c.y : c.x);
  for (const c of cards) {
    const bottom = b.rect.y + b.rect.h + HEADING_GAP + b.bodyH;
    if (c.x + c.w <= b.rect.x || c.x >= b.rect.x + b.rect.w || c.y < b.rect.y + b.rect.h) continue;
    // A block below this one starts under its body.
    if (c.y + c.h > bottom + 1) continue;
    if (!best || key(c) < key(best) || (key(c) === key(best) && across(c) < across(best))) best = c;
  }
  return best ? { id: best.id, rect: { x: best.x, y: best.y, w: best.w, h: best.h } } : null;
}
