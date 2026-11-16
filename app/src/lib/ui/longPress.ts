/**
 * Touch gestures that stand in for the mouse: a long press opens the same menu as a right-click, and two quick
 * taps stand in for a double-click. Mouse input is never touched: a right-click still raises `contextmenu` and
 * the callers handle it as before.
 */

export interface PressPoint {
  x: number;
  y: number;
  target: EventTarget | null;
}

export interface LongPressOptions {
  /** Runs when the finger has stayed down long enough; open the menu at (x, y). */
  onLongPress: (p: PressPoint) => void;
  /** Hold time in ms. */
  delay?: number;
  /** Movement in px that turns the press into a drag and cancels it. */
  slop?: number;
  /** Return false to ignore a press that starts on this target. */
  accept?: (target: EventTarget | null) => boolean;
}

export const LONG_PRESS_MS = 500;
export const LONG_PRESS_SLOP = 10;
/** How long after a touch the native `contextmenu` that Android raises for the same press is still ignored. */
const NATIVE_MENU_WINDOW_MS = 1200;

const isTouchLike = (e: { pointerType?: string }) => e.pointerType === 'touch' || e.pointerType === 'pen';

export interface LongPress {
  pointerdown(e: PointerEvent): void;
  pointermove(e: PointerEvent): void;
  /** Returns true when this release ended a press that already opened the menu (so it is not also a click). */
  pointerup(e: PointerEvent): boolean;
  pointercancel(e: PointerEvent): void;
  /**
   * For the `contextmenu` handler: true when the event is the browser's own menu for a touch press, which this
   * helper already handled (or took over). The caller should `preventDefault()` and do nothing else.
   */
  isNativeTouchMenu(e: Event): boolean;
  cancel(): void;
}

/** One controller per surface. Feed it the surface's pointer events; it calls `onLongPress` once per press. */
export function createLongPress(opts: LongPressOptions): LongPress {
  const delay = opts.delay ?? LONG_PRESS_MS;
  const slop = opts.slop ?? LONG_PRESS_SLOP;
  const down = new Set<number>();
  let timer: ReturnType<typeof setTimeout> | null = null;
  let id = -1;
  let start = { x: 0, y: 0 };
  let target: EventTarget | null = null;
  let firedFor = -1;
  let lastTouchAt = -Infinity;

  const stop = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
    id = -1;
  };

  const fire = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
    firedFor = id;
    id = -1;
    try {
      navigator.vibrate?.(10);
    } catch {
      // Not every browser lets a page vibrate.
    }
    opts.onLongPress({ x: start.x, y: start.y, target });
  };

  return {
    pointerdown(e) {
      if (!isTouchLike(e)) return;
      lastTouchAt = Date.now();
      down.add(e.pointerId);
      // A second finger is a pinch, not a press.
      if (down.size > 1) return stop();
      if (opts.accept && !opts.accept(e.target)) return;
      stop();
      id = e.pointerId;
      firedFor = -1;
      start = { x: e.clientX, y: e.clientY };
      target = e.target;
      timer = setTimeout(fire, delay);
    },
    pointermove(e) {
      if (e.pointerId !== id) return;
      if (Math.hypot(e.clientX - start.x, e.clientY - start.y) > slop) stop();
    },
    pointerup(e) {
      if (!isTouchLike(e)) return false;
      lastTouchAt = Date.now();
      down.delete(e.pointerId);
      const fired = firedFor === e.pointerId;
      if (e.pointerId === id) stop();
      if (fired) firedFor = -1;
      return fired;
    },
    pointercancel(e) {
      down.delete(e.pointerId);
      if (e.pointerId === id) stop();
    },
    isNativeTouchMenu(e) {
      const pt = (e as { pointerType?: string }).pointerType;
      if (pt === 'mouse') return false;
      const m = e as MouseEvent;
      const keyboardLike = pt === '' || (pt === undefined && m.button === 0 && m.detail === 0 && m.clientX === 0 && m.clientY === 0);
      const recent = Date.now() - lastTouchAt < NATIVE_MENU_WINDOW_MS;
      if (!(pt === 'touch' || pt === 'pen') && !(recent && !keyboardLike)) return false;
      // The browser got there first: its menu event is the signal, so open ours now instead of a moment later.
      if (timer !== null) fire();
      return true;
    },
    cancel() {
      stop();
      down.clear();
    },
  };
}

/** Svelte action: `use:longPress={{ onLongPress }}` on an element that has no pointer code of its own. */
export function longPress(node: HTMLElement, options: LongPressOptions) {
  let lp = createLongPress(options);
  const handlers: [string, (e: Event) => void][] = [
    ['pointerdown', (e) => lp.pointerdown(e as PointerEvent)],
    ['pointermove', (e) => lp.pointermove(e as PointerEvent)],
    [
      'pointerup',
      (e) => {
        // The press already opened the menu: the lift must not also click through to the item beneath.
        if (lp.pointerup(e as PointerEvent)) swallowNextClick(node);
      },
    ],
    ['pointercancel', (e) => lp.pointercancel(e as PointerEvent)],
    [
      'contextmenu',
      (e) => {
        if (lp.isNativeTouchMenu(e)) e.preventDefault();
      },
    ],
  ];
  for (const [n, h] of handlers) node.addEventListener(n, h);
  return {
    update(next: LongPressOptions) {
      lp.cancel();
      lp = createLongPress(next);
    },
    destroy() {
      lp.cancel();
      for (const [n, h] of handlers) node.removeEventListener(n, h);
    },
  };
}

function swallowNextClick(node: HTMLElement): void {
  const eat = (ev: Event) => {
    ev.stopPropagation();
    ev.preventDefault();
  };
  node.addEventListener('click', eat, { capture: true, once: true });
  setTimeout(() => node.removeEventListener('click', eat, { capture: true }), 400);
}

export interface DoubleTapOptions {
  /** Longest gap between the two taps, ms. */
  ms?: number;
  /** Farthest the second tap may land from the first, px. */
  slop?: number;
  now?: () => number;
}

/**
 * Detects two quick taps close together. Call `tap(x, y)` for each completed touch tap; it answers true for the
 * second one (and forgets, so a third tap starts over).
 */
export function createDoubleTap(opts: DoubleTapOptions = {}): { tap(x: number, y: number): boolean; reset(): void } {
  const ms = opts.ms ?? 300;
  const slop = opts.slop ?? 20;
  const now = opts.now ?? Date.now;
  let last: { x: number; y: number; at: number } | null = null;
  return {
    tap(x, y) {
      const at = now();
      if (last && at - last.at <= ms && Math.hypot(x - last.x, y - last.y) <= slop) {
        last = null;
        return true;
      }
      last = { x, y, at };
      return false;
    },
    reset() {
      last = null;
    },
  };
}

/** Whether the main input is a finger (a phone or tablet), for showing visible stand-ins for hover and right-click. */
export function isCoarsePointer(): boolean {
  try {
    return typeof window !== 'undefined' && !!window.matchMedia?.('(pointer: coarse)').matches;
  } catch {
    return false;
  }
}

/** True for the pointer types a finger or a pen produce. */
export const isTouchPointer = isTouchLike;
