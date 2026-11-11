import type { TextAreaInput, TextRegionDto, TextRegionInput } from '../api/api';

/** The smallest side of a marked piece, as a fraction of the picture (the backend refuses less than 0.001). */
export const MIN_SIDE = 0.01;
/** How far one arrow key moves or resizes a piece, as a fraction of the picture. */
export const KEY_STEP = 0.01;

export type Rotation = TextRegionDto['rotation'];
export type Corner = 'nw' | 'ne' | 'sw' | 'se';

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
/** Four decimals are finer than a pixel on any scan and keep the stored numbers short. */
const round = (v: number) => Math.round(v * 10000) / 10000;

/** Keeps an area on the picture and at least {@link MIN_SIDE} on each side. */
export function fitArea(a: TextAreaInput): TextAreaInput {
  const w = clamp(a.w, MIN_SIDE, 1);
  const h = clamp(a.h, MIN_SIDE, 1);
  return { x: round(clamp(a.x, 0, 1 - w)), y: round(clamp(a.y, 0, 1 - h)), w: round(w), h: round(h) };
}

/** The area moved by a fraction of the picture, kept whole on it. */
export function moveArea(a: TextAreaInput, dx: number, dy: number): TextAreaInput {
  return fitArea({ ...a, x: a.x + dx, y: a.y + dy });
}

/** The area with one corner dragged by a fraction of the picture; the opposite corner stays. */
export function resizeArea(a: TextAreaInput, corner: Corner, dx: number, dy: number): TextAreaInput {
  let { x, y } = a;
  let x2 = a.x + a.w;
  let y2 = a.y + a.h;
  if (corner === 'nw' || corner === 'sw') x = clamp(x + dx, 0, x2 - MIN_SIDE);
  else x2 = clamp(x2 + dx, x + MIN_SIDE, 1);
  if (corner === 'nw' || corner === 'ne') y = clamp(y + dy, 0, y2 - MIN_SIDE);
  else y2 = clamp(y2 + dy, y + MIN_SIDE, 1);
  return fitArea({ x, y, w: x2 - x, h: y2 - y });
}

/** The area between two points of the picture (fractions), in whichever direction it was dragged. */
export function areaBetween(x1: number, y1: number, x2: number, y2: number): TextAreaInput {
  const ax = clamp(Math.min(x1, x2), 0, 1);
  const ay = clamp(Math.min(y1, y2), 0, 1);
  return fitArea({ x: ax, y: ay, w: clamp(Math.max(x1, x2), 0, 1) - ax, h: clamp(Math.max(y1, y2), 0, 1) - ay });
}

/** A quarter turn clockwise. */
export function nextRotation(r: Rotation): Rotation {
  return ((r + 90) % 360) as Rotation;
}

/** What the backend takes back: the same ids, so a piece keeps its identity (and its author) across edits. */
export function toInput(regions: TextRegionDto[]): TextRegionInput[] {
  return regions.map((r) => ({ id: r.id, ...(r.area ? { area: r.area } : {}), rotation: r.rotation, text: r.text }));
}

/** The pieces' texts in reading order, one paragraph each; empty pieces are left out (as the backend joins them). */
export function fullText(regions: TextRegionDto[]): string {
  return regions
    .map((r) => r.text.trim())
    .filter(Boolean)
    .join('\n\n');
}

/** A new piece's id; the backend keeps an id the app gives it. */
export function newPieceId(): string {
  return crypto.randomUUID();
}

/** The list with the piece at `from` moved to `to`. */
export function moved<T>(list: T[], from: number, to: number): T[] {
  if (to < 0 || to >= list.length || from === to) return list;
  const out = list.slice();
  const [it] = out.splice(from, 1);
  out.splice(to, 0, it);
  return out;
}
