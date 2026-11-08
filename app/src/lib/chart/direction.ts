import type { ChartDirection, ChartDirections } from '../api/generated';
import type { ChartKind } from '../api/types';
import type { MessageKey } from '../i18n';

export type { ChartDirection, ChartDirections };

/** Every direction, in the order the choice shows them. */
export const DIRECTIONS: readonly ChartDirection[] = ['top', 'bottom', 'left', 'right'];

/** The charts that remember a direction of their own (the family view too). */
export type DirectionKey = keyof ChartDirections;

/** The words for each direction (the control and the settings) and for each kind of chart (the settings). */
export const DIRECTION_LABEL: Record<ChartDirection, MessageKey> = {
  top: 'tree.top',
  bottom: 'tree.bottom',
  left: 'tree.left',
  right: 'tree.right',
};
export const CHART_LABEL: Record<DirectionKey, MessageKey> = {
  pedigree: 'settings.dirPedigree',
  descendants: 'settings.dirDescendants',
  relatives: 'settings.dirRelatives',
  around: 'settings.dirAround',
  whole: 'settings.dirWhole',
  family: 'settings.dirFamily',
};
/** Every kind that remembers a direction, in the order the settings list them. */
export const DIRECTION_KEYS: readonly DirectionKey[] = ['pedigree', 'descendants', 'relatives', 'around', 'whole', 'family'];

/** The choices a kind of chart offers: the family view is a page of rows, so it only goes up or down. */
export const choicesFor = (key: DirectionKey): readonly ChartDirection[] => (key === 'family' ? DIRECTIONS.filter((d) => d === 'top' || d === 'bottom') : DIRECTIONS);

export const isHorizontal = (d: ChartDirection): boolean => d === 'left' || d === 'right';

export const opposite = (d: ChartDirection): ChartDirection => (d === 'top' ? 'bottom' : d === 'bottom' ? 'top' : d === 'left' ? 'right' : 'left');

/** The direction every kind of chart had before it could be chosen. */
export const DEFAULT_DIRECTIONS: ChartDirections = {
  pedigree: 'right',
  descendants: 'top',
  relatives: 'top',
  around: 'top',
  whole: 'top',
  family: 'top',
};

/** The direction of a chart kind in the settings value (a settings value from an older app has none: the defaults). */
export function directionOf(dirs: Partial<ChartDirections> | undefined, kind: DirectionKey): ChartDirection {
  return dirs?.[kind] ?? DEFAULT_DIRECTIONS[kind];
}

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** The middle of the side of `n` that faces `side`. */
export function edgeCentre(n: Box, side: ChartDirection): { x: number; y: number } {
  switch (side) {
    case 'top':
      return { x: n.x + n.w / 2, y: n.y };
    case 'bottom':
      return { x: n.x + n.w / 2, y: n.y + n.h };
    case 'left':
      return { x: n.x, y: n.y + n.h / 2 };
    default:
      return { x: n.x + n.w, y: n.y + n.h / 2 };
  }
}

/**
 * The side of a chart where the person it starts from is drawn: the ancestors' side for a chart of descendants (it
 * starts at the oldest generation) and the other end for a pedigree (it starts at the youngest).
 */
export function rootSide(kind: ChartKind, dir: ChartDirection): ChartDirection {
  return kind === 'pedigree' ? opposite(dir) : dir;
}

/** The side a card's hidden people sit on: its ancestors on `dir`'s side, its descendants on the other. */
export const ancestorsSide = (dir: ChartDirection): ChartDirection => dir;
export const descendantsSide = (dir: ChartDirection): ChartDirection => opposite(dir);

export type ArrowKey = 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight';

const ARROW_SIDE: Record<ArrowKey, ChartDirection> = { ArrowUp: 'top', ArrowDown: 'bottom', ArrowLeft: 'left', ArrowRight: 'right' };

export type Step = 'ancestors' | 'descendants' | 'prev' | 'next';

/**
 * What an arrow key does on a turned chart: the arrow that points at the ancestors' side steps towards the ancestors,
 * the opposite one towards the descendants, and the two across move to the previous / next brother, sister or partner
 * (left and right on a chart that grows up or down, up and down on one that grows sideways).
 */
export function arrowStep(key: string, dir: ChartDirection): Step | null {
  const side = ARROW_SIDE[key as ArrowKey];
  if (!side) return null;
  if (side === dir) return 'ancestors';
  if (side === opposite(dir)) return 'descendants';
  return side === 'left' || side === 'top' ? 'prev' : 'next';
}

/**
 * How a chart's own frame (the part that stays when it is turned) is laid out: a camera that puts `r` at the `side` edge of a
 * view of `view` pixels at `zoom`, `margin` world units from it, centred on the other axis.
 */
export function camAtSide(r: Box, side: ChartDirection, zoom: number, view: { w: number; h: number }, margin: number): { zoom: number; x: number; y: number } {
  const vw = view.w / zoom;
  const vh = view.h / zoom;
  switch (side) {
    case 'top':
      return { zoom, x: r.x + r.w / 2 - vw / 2, y: r.y - margin };
    case 'bottom':
      return { zoom, x: r.x + r.w / 2 - vw / 2, y: r.y + r.h + margin - vh };
    case 'left':
      return { zoom, x: r.x - margin, y: r.y + r.h / 2 - vh / 2 };
    default:
      return { zoom, x: r.x + r.w + margin - vw, y: r.y + r.h / 2 - vh / 2 };
  }
}

/**
 * A thin strip of the chart along the `side` edge of its bounds, wide enough to hold the cards on that edge: where the
 * person a chart starts from is found without asking for the whole chart.
 */
export function edgeStrip(bounds: { w: number; h: number }, side: ChartDirection): Box {
  const cardW = 178;
  const cardH = 58;
  switch (side) {
    case 'top':
      return { x: 0, y: -1, w: Math.max(1, bounds.w), h: cardH };
    case 'bottom':
      return { x: 0, y: bounds.h - (cardH - 1), w: Math.max(1, bounds.w), h: cardH };
    case 'left':
      return { x: -1, y: 0, w: cardW, h: Math.max(1, bounds.h) };
    default:
      return { x: bounds.w - (cardW - 1), y: 0, w: cardW, h: Math.max(1, bounds.h) };
  }
}
