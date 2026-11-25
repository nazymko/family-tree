import type { TextAreaInput, TextRegionDto, TextRegionInput } from '../api/api';

/** The smallest side of a marked piece, as a fraction of the picture (the backend refuses less, too). */
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

/** `r` turned by a quarter: 90 clockwise, -90 the other way. */
export function turnedBy(r: Rotation, by: 90 | -90): Rotation {
  return ((((r + by) % 360) + 360) % 360) as Rotation;
}

/**
 * How a piece's writing looks once the picture is shown turned `picture` degrees clockwise. A piece's rotation is
 * how far its writing is turned on the picture upright by EXIF; turning the picture turns the writing with it, so a
 * line running down the page (90) reads upright on a picture turned left once (270).
 */
export function shownTurn(piece: Rotation, picture: Rotation): Rotation {
  return ((piece + picture) % 360) as Rotation;
}

/**
 * A move on screen, as a move on the picture upright by EXIF, when the picture is shown turned `turn` degrees
 * clockwise: the screen's x and y turned back the other way.
 */
export function unturn(turn: Rotation, x: number, y: number): [number, number] {
  const [a, b] = turn === 90 ? [y, -x] : turn === 180 ? [-x, -y] : turn === 270 ? [-y, x] : [x, y];
  // No negative zero: it would only confuse comparisons.
  return [a + 0, b + 0];
}

/** A box on screen: what getBoundingClientRect gives of the turned picture. */
export type ScreenBox = { left: number; top: number; w: number; h: number };

/** The picture's own width and height on screen: across a quarter turn they swap. */
function upright(turn: Rotation, b: ScreenBox): { w: number; h: number } {
  return turn % 180 === 0 ? { w: b.w, h: b.h } : { w: b.h, h: b.w };
}

/** A pointer move in pixels, as fractions of the picture upright by EXIF (where the pieces are stored). */
export function moveOnPicture(turn: Rotation, b: ScreenBox, dx: number, dy: number): { dx: number; dy: number } {
  const [x, y] = unturn(turn, dx, dy);
  const s = upright(turn, b);
  return { dx: x / s.w, dy: y / s.h };
}

/** A point on screen, as fractions of the picture upright by EXIF; the turn is about the picture's middle. */
export function pointOnPicture(turn: Rotation, b: ScreenBox, cx: number, cy: number): { x: number; y: number } {
  const d = moveOnPicture(turn, b, cx - (b.left + b.w / 2), cy - (b.top + b.h / 2));
  return { x: 0.5 + d.dx, y: 0.5 + d.dy };
}

/** Which corner of a stored area shows at the corner `shown` of the screen. */
export function cornerShownAt(turn: Rotation, shown: Corner): Corner {
  const v: Record<Corner, [number, number]> = { nw: [-1, -1], ne: [1, -1], sw: [-1, 1], se: [1, 1] };
  const [x, y] = unturn(turn, ...v[shown]);
  return `${y < 0 ? 'n' : 's'}${x < 0 ? 'w' : 'e'}` as Corner;
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
