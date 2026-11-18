import type { PlacePoint } from './places';

export interface HeatPoint {
  x: number;
  y: number;
  /** Events at the point. */
  w: number;
}

export const heatPoints = (points: readonly PlacePoint[]): HeatPoint[] => points.map((p) => ({ x: p.x, y: p.y, w: p.count }));

/** The decades an animation steps through, from the one holding `from` to the one holding `to`. */
export function decades(from: number, to: number): number[] {
  const out: number[] = [];
  for (let d = Math.floor(from / 10) * 10; d <= to; d += 10) out.push(d);
  return out;
}

/**
 * How strong a point's glow is, 0..1: the square root of its share of the strongest point, so one big village does
 * not wash out the small ones around it.
 */
export function heatStrength(w: number, max: number): number {
  return max <= 0 ? 0 : Math.min(1, Math.sqrt(w / max));
}

/** A colour for a density 0..1: transparent, then blue, green, yellow, red. Returns r, g, b, alpha (0..255). */
export function rampColor(t: number): [number, number, number, number] {
  const v = Math.max(0, Math.min(1, t));
  if (v === 0) return [0, 0, 0, 0];
  const stops: [number, number, number][] = [
    [49, 80, 190],
    [40, 170, 150],
    [240, 210, 60],
    [225, 60, 40],
  ];
  const x = v * (stops.length - 1);
  const i = Math.min(stops.length - 2, Math.floor(x));
  const f = x - i;
  const a = stops[i];
  const b = stops[i + 1];
  return [Math.round(a[0] + (b[0] - a[0]) * f), Math.round(a[1] + (b[1] - a[1]) * f), Math.round(a[2] + (b[2] - a[2]) * f), Math.round(255 * Math.min(0.85, 0.25 + v))];
}

/** 256 colours, ready for a lookup by the accumulated alpha of the glow canvas. */
export function rampTable(): Uint8ClampedArray {
  const t = new Uint8ClampedArray(256 * 4);
  for (let i = 0; i < 256; i++) t.set(rampColor(i / 255), i * 4);
  return t;
}
