// Old scanned maps laid on the Map. A picture is placed by its four corners (top left, top right, bottom right, bottom
// left); between them it is stretched by the projective transform those corners give, drawn as a mesh of triangles
// (canvas 2D draws only affine pieces). Everything here works in Web Mercator world coordinates or screen pixels.
import type { MapCorner } from '../api/types';
import { fromScreen, mercator, toScreen, unmercator, type Camera, type World } from './projection';

export type Pt = [number, number];
export type Quad = [Pt, Pt, Pt, Pt];

/** The corners of a layer as world points. */
export function cornersToWorld(c: MapCorner[]): World[] {
  return c.map((p) => mercator(p.latE6 / 1e6, p.lonE6 / 1e6));
}

/** World points as the corners a layer keeps (millionths of a degree). */
export function worldToCorners(w: World[]): MapCorner[] {
  return w.map((p) => {
    const { lat, lon } = unmercator(p.x, p.y);
    return {
      latE6: Math.round(lat * 1e6),
      lonE6: Math.round(Math.max(-180, Math.min(180, lon)) * 1e6),
    };
  });
}

/**
 * Where a new map goes before anyone places it: in the middle of the view, the picture's own shape, about 60 % of the
 * view across.
 */
export function defaultCorners(cam: Camera, w: number, h: number, imgW: number, imgH: number): MapCorner[] {
  const aspect = imgW > 0 && imgH > 0 ? imgW / imgH : 4 / 3;
  let bw = w * 0.6;
  let bh = bw / aspect;
  if (bh > h * 0.6) {
    bh = h * 0.6;
    bw = bh * aspect;
  }
  const x0 = (w - bw) / 2;
  const y0 = (h - bh) / 2;
  const screen: Pt[] = [
    [x0, y0],
    [x0 + bw, y0],
    [x0 + bw, y0 + bh],
    [x0, y0 + bh],
  ];
  return worldToCorners(screen.map(([x, y]) => fromScreen(cam, w, h, x, y)));
}

/** The corners moved together by a screen offset (the picture dragged as a whole). */
export function moveAll(c: MapCorner[], cam: Camera, w: number, h: number, dx: number, dy: number): MapCorner[] {
  const pts = cornersToWorld(c).map((p) => {
    const [sx, sy] = toScreen(cam, w, h, p.x, p.y);
    return fromScreen(cam, w, h, sx + dx, sy + dy);
  });
  return worldToCorners(pts);
}

/** One corner moved to a screen point. */
export function moveCorner(c: MapCorner[], i: number, cam: Camera, w: number, h: number, sx: number, sy: number): MapCorner[] {
  const out = c.slice();
  out[i] = worldToCorners([fromScreen(cam, w, h, sx, sy)])[0];
  return out;
}

/**
 * The 3 x 3 projective transform (row major, h[8] = 1) taking the four `src` points to the four `dst` points, or null
 * when three of them lie on one line.
 */
export function homography(src: Pt[], dst: Pt[]): number[] | null {
  const a: number[][] = [];
  for (let i = 0; i < 4; i++) {
    const [x, y] = src[i];
    const [u, v] = dst[i];
    a.push([x, y, 1, 0, 0, 0, -x * u, -y * u, u]);
    a.push([0, 0, 0, x, y, 1, -x * v, -y * v, v]);
  }
  const s = solve(a);
  return s ? [...s, 1] : null;
}

/** Gaussian elimination with partial pivoting on an n x (n + 1) matrix. */
export function solve(m: number[][]): number[] | null {
  const a = m.map((r) => {
    const n = m.length;
    const k = Math.max(...r.slice(0, n).map(Math.abs)) || 1;
    return r.map((v) => v / k);
  });
  const n = a.length;
  for (let c = 0; c < n; c++) {
    let p = c;
    for (let r = c + 1; r < n; r++) if (Math.abs(a[r][c]) > Math.abs(a[p][c])) p = r;
    if (Math.abs(a[p][c]) < 1e-12) return null;
    [a[c], a[p]] = [a[p], a[c]];
    for (let r = 0; r < n; r++) {
      if (r === c) continue;
      const f = a[r][c] / a[c][c];
      for (let j = c; j <= n; j++) a[r][j] -= f * a[c][j];
    }
  }
  return a.map((r, i) => r[n] / r[i]);
}

export function project(h: number[], x: number, y: number): Pt {
  const d = h[6] * x + h[7] * y + h[8];
  return [(h[0] * x + h[1] * y + h[2]) / d, (h[3] * x + h[4] * y + h[5]) / d];
}

/** How far the fourth corner is from where a parallelogram through the other three puts it, in pixels. */
export function skew(q: Quad): number {
  const [a, b, c, d] = q;
  return Math.hypot(a[0] + c[0] - b[0] - d[0], a[1] + c[1] - b[1] - d[1]);
}

/** The affine transform [a, b, c, d, e, f] (canvas order) taking triangle s to triangle d, or null when s is flat. */
export function affine(s: [Pt, Pt, Pt], d: [Pt, Pt, Pt]): number[] | null {
  const [[x0, y0], [x1, y1], [x2, y2]] = s;
  const det = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
  if (Math.abs(det) < 1e-12) return null;
  const [[u0, v0], [u1, v1], [u2, v2]] = d;
  const a = ((u1 - u0) * (y2 - y0) - (u2 - u0) * (y1 - y0)) / det;
  const c = ((u2 - u0) * (x1 - x0) - (u1 - u0) * (x2 - x0)) / det;
  const b = ((v1 - v0) * (y2 - y0) - (v2 - v0) * (y1 - y0)) / det;
  const dd = ((v2 - v0) * (x1 - x0) - (v1 - v0) * (x2 - x0)) / det;
  return [a, b, c, dd, u0 - a * x0 - c * y0, v0 - b * x0 - dd * y0];
}

/** One triangle of the mesh: where it is on the picture (pixels) and where it goes on screen. */
export interface Piece {
  src: [Pt, Pt, Pt];
  dst: [Pt, Pt, Pt];
}

/** How many cells across the mesh needs: 1 for a near parallelogram, more the further the corners are from one. */
export function meshSize(q: Quad): number {
  const k = skew(q);
  if (k < 1) return 1;
  return Math.max(2, Math.min(16, Math.ceil(Math.sqrt(k) * 1.5)));
}

/** The triangles a picture of iw x ih pixels is drawn as onto the screen quad `q`. */
export function mesh(q: Quad, iw: number, ih: number, n = meshSize(q)): Piece[] {
  const unit: Pt[] = [
    [0, 0],
    [1, 0],
    [1, 1],
    [0, 1],
  ];
  const h = homography(unit, q);
  if (!h) return [];
  const out: Piece[] = [];
  const at = (i: number, j: number): Pt => project(h, i / n, j / n);
  const px = (i: number, j: number): Pt => [(i / n) * iw, (j / n) * ih];
  for (let j = 0; j < n; j++) {
    for (let i = 0; i < n; i++) {
      out.push({
        src: [px(i, j), px(i + 1, j), px(i + 1, j + 1)],
        dst: [at(i, j), at(i + 1, j), at(i + 1, j + 1)],
      });
      out.push({
        src: [px(i, j), px(i + 1, j + 1), px(i, j + 1)],
        dst: [at(i, j), at(i + 1, j + 1), at(i, j + 1)],
      });
    }
  }
  return out;
}

/** A triangle pushed out from its middle by `by` pixels, so that neighbours overlap and no hairline shows. */
function grow(t: [Pt, Pt, Pt], by: number): [Pt, Pt, Pt] {
  const cx = (t[0][0] + t[1][0] + t[2][0]) / 3;
  const cy = (t[0][1] + t[1][1] + t[2][1]) / 3;
  return t.map(([x, y]) => {
    const d = Math.hypot(x - cx, y - cy) || 1;
    return [x + ((x - cx) / d) * by, y + ((y - cy) / d) * by];
  }) as [Pt, Pt, Pt];
}

/** The part of the 2D context a warp needs. */
export type WarpCtx = Pick<CanvasRenderingContext2D, 'save' | 'restore' | 'transform' | 'beginPath' | 'moveTo' | 'lineTo' | 'closePath' | 'clip' | 'drawImage'>;

/** Draws a picture (iw x ih pixels) stretched onto the screen quad q, fully opaque. */
export function drawWarped(ctx: WarpCtx, img: CanvasImageSource, iw: number, ih: number, q: Quad): void {
  const n = meshSize(q);
  if (n === 1) {
    const m = affine(
      [
        [0, 0],
        [iw, 0],
        [0, ih],
      ],
      [q[0], q[1], q[3]],
    );
    if (!m) return;
    ctx.save();
    ctx.transform(m[0], m[1], m[2], m[3], m[4], m[5]);
    ctx.drawImage(img, 0, 0, iw, ih);
    ctx.restore();
    return;
  }
  for (const p of mesh(q, iw, ih, n)) {
    const m = affine(p.src, p.dst);
    if (!m) continue;
    const d = grow(p.dst, 0.6);
    ctx.save();
    ctx.beginPath();
    ctx.moveTo(d[0][0], d[0][1]);
    ctx.lineTo(d[1][0], d[1][1]);
    ctx.lineTo(d[2][0], d[2][1]);
    ctx.closePath();
    ctx.clip();
    ctx.transform(m[0], m[1], m[2], m[3], m[4], m[5]);
    ctx.drawImage(img, 0, 0, iw, ih);
    ctx.restore();
  }
}

/** The screen quad of world corners. */
export function screenQuad(world: World[], cam: Camera, w: number, h: number): Quad {
  return world.map((p) => toScreen(cam, w, h, p.x, p.y)) as Quad;
}

/** Whether a screen quad touches the view at all. */
export function onScreen(q: Quad, w: number, h: number): boolean {
  const xs = q.map((p) => p[0]);
  const ys = q.map((p) => p[1]);
  return Math.max(...xs) >= 0 && Math.min(...xs) <= w && Math.max(...ys) >= 0 && Math.min(...ys) <= h;
}

/** The longest side of a screen quad, in pixels: how many picture pixels the view could use across. */
export function screenSpan(q: Quad): number {
  let m = 0;
  for (let i = 0; i < 4; i++) m = Math.max(m, Math.hypot(q[(i + 1) % 4][0] - q[i][0], q[(i + 1) % 4][1] - q[i][1]));
  return m;
}

/** The largest side a full-resolution copy is kept at: big enough for a village, small enough for the GPU. */
export const FULL_MAX = 8192;

/** Whether the view needs more of the picture than a copy `have` pixels across gives. */
export function wantsMore(q: Quad, scale: number, have: number, full: number): boolean {
  return have < Math.min(full, FULL_MAX) && screenSpan(q) * scale > have * 1.25;
}

/** A layer list in drawing order (lowest first) with one moved up (+1) or down (-1); the ids for ReorderMapLayers. */
export function reordered(ids: string[], id: string, by: 1 | -1): string[] {
  const i = ids.indexOf(id);
  const j = i + by;
  if (i < 0 || j < 0 || j >= ids.length) return ids;
  const out = ids.slice();
  [out[i], out[j]] = [out[j], out[i]];
  return out;
}

/** Decimal degrees typed by the owner ("50.45", "50,45") as millionths, or null when it is not a number in range. */
export function parseDegrees(text: string, max: number): number | null {
  const v = Number(text.trim().replace(',', '.'));
  if (!text.trim() || !Number.isFinite(v) || Math.abs(v) > max) return null;
  return Math.round(v * 1e6);
}
