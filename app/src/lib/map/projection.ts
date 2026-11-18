// Web Mercator on a unit world: x and y run 0..1 (x from 180 W to 180 E, y from the north edge down). A camera looks at
// a world point `cx, cy` at zoom `zoom`; the whole world is 256 * 2^zoom pixels wide, as in every web map.

export const MAX_LAT = 85.0511287798;
export const MAX_ZOOM = 13;
const TILE_PX = 256;

export interface World {
  x: number;
  y: number;
}

export interface Camera {
  cx: number;
  cy: number;
  zoom: number;
}

export function mercator(lat: number, lon: number): World {
  const la = Math.max(-MAX_LAT, Math.min(MAX_LAT, lat));
  const s = Math.sin((la * Math.PI) / 180);
  return { x: (lon + 180) / 360, y: 0.5 - Math.log((1 + s) / (1 - s)) / (4 * Math.PI) };
}

export function unmercator(x: number, y: number): { lat: number; lon: number } {
  const lon = x * 360 - 180;
  const lat = (Math.atan(Math.sinh(Math.PI * (1 - 2 * y))) * 180) / Math.PI;
  return { lat, lon };
}

export const worldPx = (zoom: number): number => TILE_PX * 2 ** zoom;

export function toScreen(cam: Camera, w: number, h: number, x: number, y: number): [number, number] {
  const s = worldPx(cam.zoom);
  return [w / 2 + (x - cam.cx) * s, h / 2 + (y - cam.cy) * s];
}

export function fromScreen(cam: Camera, w: number, h: number, sx: number, sy: number): World {
  const s = worldPx(cam.zoom);
  return { x: cam.cx + (sx - w / 2) / s, y: cam.cy + (sy - h / 2) / s };
}

/** The smallest zoom at which the world still fills the view in both directions. */
export function minZoom(w: number, h: number): number {
  return Math.max(0, Math.log2(Math.max(1, w, h) / TILE_PX));
}

/** Keeps the camera inside the world: a zoom within limits, and the view never past an edge when the world is bigger. */
export function clampCamera(cam: Camera, w: number, h: number): Camera {
  const zoom = Math.max(minZoom(w, h), Math.min(MAX_ZOOM, cam.zoom));
  const s = worldPx(zoom);
  const half = (n: number) => Math.min(0.5, n / 2 / s);
  const hx = half(w);
  const hy = half(h);
  return { zoom, cx: Math.max(hx, Math.min(1 - hx, cam.cx)), cy: Math.max(hy, Math.min(1 - hy, cam.cy)) };
}

export function panBy(cam: Camera, w: number, h: number, dx: number, dy: number): Camera {
  const s = worldPx(cam.zoom);
  return clampCamera({ ...cam, cx: cam.cx - dx / s, cy: cam.cy - dy / s }, w, h);
}

/** Zooms by `dz` levels keeping the world point under the screen point (sx, sy) where it is. */
export function zoomAt(cam: Camera, w: number, h: number, sx: number, sy: number, dz: number): Camera {
  const before = fromScreen(cam, w, h, sx, sy);
  const zoom = Math.max(minZoom(w, h), Math.min(MAX_ZOOM, cam.zoom + dz));
  const s = worldPx(zoom);
  return clampCamera({ zoom, cx: before.x - (sx - w / 2) / s, cy: before.y - (sy - h / 2) / s }, w, h);
}

/** A camera that shows all the given world points with `pad` px around them (the whole world when there are none). */
export function fitPoints(points: World[], w: number, h: number, pad = 48, maxZoom = 9): Camera {
  if (points.length === 0) return clampCamera({ cx: 0.5, cy: 0.5, zoom: 0 }, w, h);
  let x0 = Infinity;
  let x1 = -Infinity;
  let y0 = Infinity;
  let y1 = -Infinity;
  for (const p of points) {
    x0 = Math.min(x0, p.x);
    x1 = Math.max(x1, p.x);
    y0 = Math.min(y0, p.y);
    y1 = Math.max(y1, p.y);
  }
  const spanX = Math.max(1e-6, x1 - x0);
  const spanY = Math.max(1e-6, y1 - y0);
  const zx = Math.log2(Math.max(1, w - 2 * pad) / (spanX * TILE_PX));
  const zy = Math.log2(Math.max(1, h - 2 * pad) / (spanY * TILE_PX));
  return clampCamera({ cx: (x0 + x1) / 2, cy: (y0 + y1) / 2, zoom: Math.min(zx, zy, maxZoom) }, w, h);
}
