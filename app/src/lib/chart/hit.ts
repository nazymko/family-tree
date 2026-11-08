import type { ChartKind, ChartNode, ChartWindow } from '../api/types';
import { ancestorsSide, descendantsSide, edgeCentre, type ChartDirection } from './direction';

export type Dir = 'up' | 'down' | 'left' | 'right' | 'parent' | 'child';

export function hitTest(win: ChartWindow, worldX: number, worldY: number): ChartNode | null {
  // Later nodes are drawn on top, so scan backwards.
  for (let i = win.nodes.length - 1; i >= 0; i--) {
    const n = win.nodes[i];
    if (worldX >= n.x && worldX <= n.x + n.w && worldY >= n.y && worldY <= n.y + n.h) return n;
  }
  return null;
}

/** Radius (world units) of the "+" handle drawn on nodes with more ancestors/descendants. */
export const HANDLE_R = 9;

/**
 * Centre of the "+" handle: on the ancestors' side of a pedigree card (default: the right edge), on the descendants'
 * side of a card of a chart of descendants (default: the bottom edge).
 */
export function handlePos(n: ChartNode, kind: ChartKind, dir: ChartDirection = kind === 'pedigree' ? 'right' : 'top'): { x: number; y: number } {
  return edgeCentre(n, kind === 'pedigree' ? ancestorsSide(dir) : descendantsSide(dir));
}

/**
 * Where the "+" handles of a node sit. Pedigree: the ancestors' edge when ancestors are hidden. Descendants: the
 * children's edge when children are hidden. Relatives: the ancestors' edge for hidden ancestors and the opposite edge
 * for hidden children.
 */
export function handleSpots(n: ChartNode, kind: ChartKind, dir: ChartDirection = kind === 'pedigree' ? 'right' : 'top'): { x: number; y: number }[] {
  if (kind === 'relatives' || kind === 'around') {
    const out: { x: number; y: number }[] = [];
    if (n.moreUp) out.push(edgeCentre(n, ancestorsSide(dir)));
    if (n.moreDown) out.push(edgeCentre(n, descendantsSide(dir)));
    return out;
  }
  return (kind === 'pedigree' ? n.moreUp : n.moreDown) ? [handlePos(n, kind, dir)] : [];
}

/** Screen radius (px) a handle must be hittable at, converted to world units. */
export const HANDLE_HIT_PX = 8;

export function hitHandle(win: ChartWindow, kind: ChartKind, wx: number, wy: number, zoom = 1, dir: ChartDirection = kind === 'pedigree' ? 'right' : 'top'): ChartNode | null {
  if (zoom < 0.25) return null; // dots LOD draws no handles
  const r = Math.max(HANDLE_R, HANDLE_HIT_PX / zoom);
  for (const n of win.nodes) {
    for (const h of handleSpots(n, kind, dir)) if ((wx - h.x) ** 2 + (wy - h.y) ** 2 <= r * r) return n;
  }
  return null;
}

/**
 * Next node id from `from` following the nav links. up/down are prev/next siblings; parent/child
 * are tree links (towards / away from the root). left/right pick whichever nav neighbour that is
 * present in the window lies in that direction.
 */
export function neighbor(win: ChartWindow, from: number, dir: Dir): number | null {
  const n = win.nodes.find((x) => x.id === from);
  if (!n) return null;
  switch (dir) {
    case 'up':
      return n.nav.prev;
    case 'down':
      return n.nav.next;
    case 'parent':
      return n.nav.parent;
    case 'child':
      return n.nav.child;
    default: {
      const cx = n.x + n.w / 2;
      const cy = n.y + n.h / 2;
      let best: number | null = null;
      let bestScore = Infinity;
      for (const id of [n.nav.parent, n.nav.child, n.nav.prev, n.nav.next]) {
        if (id === null) continue;
        const o = win.nodes.find((x) => x.id === id);
        if (!o) continue;
        const dx = o.x + o.w / 2 - cx;
        const dy = o.y + o.h / 2 - cy;
        if (dir === 'left' ? dx >= 0 : dx <= 0) continue;
        const score = Math.abs(dy) - Math.abs(dx) * 0.01;
        if (score < bestScore) {
          bestScore = score;
          best = id;
        }
      }
      return best;
    }
  }
}
