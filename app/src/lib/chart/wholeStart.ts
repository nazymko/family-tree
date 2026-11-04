import type { Rect, WholeBlockDto } from '../api/types';

/**
 * The card to show first when there is no focus or home person: the left-most card of the top row of the
 * largest family (its oldest generation). `cards` is any window of the chart, for example the minimap's.
 * Block coordinates are not card coordinates: a big family's top row can lie far from the heading's corner.
 */
/** Gap between a heading and the first row of its block (the layout's HEADING_GAP). */
const HEADING_GAP = 12;

export function founderOf(
  blocks: WholeBlockDto[],
  cards: { id: number; x: number; y: number; w: number; h: number }[],
  reversed = false,
): { id: number; rect: Rect } | null {
  const b = blocks.find((x) => !x.unconnected) ?? blocks[0];
  if (!b) return null;
  let best: (typeof cards)[number] | null = null;
  for (const c of cards) {
    const bottom = b.rect.y + b.rect.h + HEADING_GAP + b.bodyH;
    if (c.x + c.w <= b.rect.x || c.x >= b.rect.x + b.rect.w || c.y < b.rect.y + b.rect.h) continue;
    // Reversed, the oldest generation is the bottom row of the block.
    if (reversed && c.y + c.h > bottom + 1) continue;
    const older = reversed ? best !== null && c.y > best.y : best !== null && c.y < best.y;
    if (!best || older || (c.y === best.y && c.x < best.x)) best = c;
  }
  return best ? { id: best.id, rect: { x: best.x, y: best.y, w: best.w, h: best.h } } : null;
}
