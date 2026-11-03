import type { Rect } from '../api/types';

/** World point at the viewport's top-left; zoom = CSS pixels per world unit. */
export interface Camera {
  x: number;
  y: number;
  zoom: number;
}
export const ZOOM_MIN = 0.1;
export const ZOOM_MAX = 2.5;

interface Size {
  w: number;
  h: number;
}

const clampZoom = (z: number): number => Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, z));

/** Zoom by `factor` keeping the world point under `anchorPx` (viewport pixels) fixed. */
export function zoomAt(c: Camera, factor: number, anchorPx: { x: number; y: number }): Camera {
  const zoom = clampZoom(c.zoom * factor);
  const wx = c.x + anchorPx.x / c.zoom;
  const wy = c.y + anchorPx.y / c.zoom;
  return { x: wx - anchorPx.x / zoom, y: wy - anchorPx.y / zoom, zoom };
}

function clampAxis(pos: number, viewWorld: number, bound: number): number {
  const need = Math.min(0.25 * bound, viewWorld);
  // The visible span [pos, pos + viewWorld] must overlap [0, bound] by at least `need`.
  return Math.min(bound - need, Math.max(need - viewWorld, pos));
}

/** Drag by a pixel delta (content follows the pointer); keeps at least 25% of the bounds in reach. */
export function panBy(c: Camera, dxPx: number, dyPx: number, bounds: Size, view: Size): Camera {
  return {
    zoom: c.zoom,
    x: clampAxis(c.x - dxPx / c.zoom, view.w / c.zoom, bounds.w),
    y: clampAxis(c.y - dyPx / c.zoom, view.h / c.zoom, bounds.h),
  };
}

export function viewRect(c: Camera, view: Size): Rect {
  return { x: c.x, y: c.y, w: view.w / c.zoom, h: view.h / c.zoom };
}

export function centerOn(c: Camera, node: Rect, view: Size): Camera {
  return {
    zoom: c.zoom,
    x: node.x + node.w / 2 - view.w / c.zoom / 2,
    y: node.y + node.h / 2 - view.h / c.zoom / 2,
  };
}

/** Camera that shows all of `bounds` with a margin (zoom capped at 1 so small charts stay readable). */
export function fit(bounds: Size, view: Size, margin = 24): Camera {
  const raw = Math.min((view.w - 2 * margin) / Math.max(1, bounds.w), (view.h - 2 * margin) / Math.max(1, bounds.h));
  const zoom = clampZoom(Math.min(1, raw));
  return {
    zoom,
    x: bounds.w / 2 - view.w / zoom / 2,
    y: bounds.h / 2 - view.h / zoom / 2,
  };
}

const ease = (t: number): number => 1 - (1 - t) ** 3;

export function tween(
  from: Camera,
  to: Camera,
  ms: number,
  step: (c: Camera) => void,
  reduced: boolean,
): { cancel(): void } {
  if (reduced || ms <= 0) {
    step(to);
    return { cancel() {} };
  }
  let raf = 0;
  let done = false;
  const t0 = performance.now();
  const tick = (now: number) => {
    if (done) return;
    const t = Math.min(1, Math.max(0, (now - t0) / ms));
    const k = ease(t);
    step({ x: from.x + (to.x - from.x) * k, y: from.y + (to.y - from.y) * k, zoom: from.zoom + (to.zoom - from.zoom) * k });
    if (t < 1) raf = requestAnimationFrame(tick);
    else done = true;
  };
  raf = requestAnimationFrame(tick);
  return {
    cancel() {
      done = true;
      cancelAnimationFrame(raf);
    },
  };
}
