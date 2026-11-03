import type { ChartNode, ChartWindow } from '../api/types';

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

/** Centre of the "+" handle: right edge for pedigrees (ancestors go right), bottom edge for descendants. */
export function handlePos(n: ChartNode, kind: 'pedigree' | 'descendants'): { x: number; y: number } {
  return kind === 'pedigree' ? { x: n.x + n.w, y: n.y + n.h / 2 } : { x: n.x + n.w / 2, y: n.y + n.h };
}

/** Screen radius (px) a handle must be hittable at, converted to world units. */
export const HANDLE_HIT_PX = 8;

export function hitHandle(win: ChartWindow, kind: 'pedigree' | 'descendants', wx: number, wy: number, zoom = 1): ChartNode | null {
  if (zoom < 0.25) return null; // dots LOD draws no handles
  const r = Math.max(HANDLE_R, HANDLE_HIT_PX / zoom);
  for (const n of win.nodes) {
    if (!(kind === 'pedigree' ? n.moreUp : n.moreDown)) continue;
    const h = handlePos(n, kind);
    if ((wx - h.x) ** 2 + (wy - h.y) ** 2 <= r * r) return n;
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
