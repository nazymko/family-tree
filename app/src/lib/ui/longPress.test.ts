import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createDoubleTap, createLongPress, longPress } from './longPress';

const ptr = (pointerType: string, pointerId: number, x = 100, y = 100): PointerEvent =>
  ({ pointerType, pointerId, clientX: x, clientY: y, target: null }) as unknown as PointerEvent;

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('createLongPress', () => {
  it('fires after the hold time at the finger position, with a short vibration', () => {
    const vibrate = vi.fn();
    Object.defineProperty(navigator, 'vibrate', { value: vibrate, configurable: true });
    const onLongPress = vi.fn();
    const lp = createLongPress({ onLongPress });
    lp.pointerdown(ptr('touch', 1, 40, 60));
    vi.advanceTimersByTime(499);
    expect(onLongPress).not.toHaveBeenCalled();
    vi.advanceTimersByTime(2);
    expect(onLongPress).toHaveBeenCalledTimes(1);
    expect(onLongPress.mock.calls[0][0]).toMatchObject({ x: 40, y: 60 });
    expect(vibrate).toHaveBeenCalledWith(10);
    // The lift that ends a handled press is not a click.
    expect(lp.pointerup(ptr('touch', 1))).toBe(true);
  });

  it('a lift before the hold time cancels, and the lift is an ordinary tap', () => {
    const onLongPress = vi.fn();
    const lp = createLongPress({ onLongPress });
    lp.pointerdown(ptr('touch', 1));
    vi.advanceTimersByTime(200);
    expect(lp.pointerup(ptr('touch', 1))).toBe(false);
    vi.advanceTimersByTime(1000);
    expect(onLongPress).not.toHaveBeenCalled();
  });

  it('moving more than the slop cancels (a pan), a small tremor does not', () => {
    const onLongPress = vi.fn();
    const lp = createLongPress({ onLongPress });
    lp.pointerdown(ptr('touch', 1, 100, 100));
    lp.pointermove(ptr('touch', 1, 104, 103));
    vi.advanceTimersByTime(600);
    expect(onLongPress).toHaveBeenCalledTimes(1);

    const onSecond = vi.fn();
    const lp2 = createLongPress({ onLongPress: onSecond });
    lp2.pointerdown(ptr('touch', 2, 100, 100));
    lp2.pointermove(ptr('touch', 2, 100, 115));
    vi.advanceTimersByTime(600);
    expect(onSecond).not.toHaveBeenCalled();
  });

  it('a second finger (a pinch) cancels', () => {
    const onLongPress = vi.fn();
    const lp = createLongPress({ onLongPress });
    lp.pointerdown(ptr('touch', 1));
    vi.advanceTimersByTime(300);
    lp.pointerdown(ptr('touch', 2, 200, 200));
    vi.advanceTimersByTime(1000);
    expect(onLongPress).not.toHaveBeenCalled();
  });

  it('pointercancel cancels', () => {
    const onLongPress = vi.fn();
    const lp = createLongPress({ onLongPress });
    lp.pointerdown(ptr('touch', 1));
    lp.pointercancel(ptr('touch', 1));
    vi.advanceTimersByTime(1000);
    expect(onLongPress).not.toHaveBeenCalled();
  });

  it('the mouse is left alone: no timer, no suppression of its right-click', () => {
    const onLongPress = vi.fn();
    const lp = createLongPress({ onLongPress });
    lp.pointerdown(ptr('mouse', 1));
    vi.advanceTimersByTime(1000);
    expect(onLongPress).not.toHaveBeenCalled();
    expect(lp.pointerup(ptr('mouse', 1))).toBe(false);
    expect(lp.isNativeTouchMenu({ pointerType: 'mouse' } as unknown as Event)).toBe(false);
  });

  it('the native contextmenu after a touch press is ignored, and cannot open a second menu', () => {
    const onLongPress = vi.fn();
    const lp = createLongPress({ onLongPress });
    lp.pointerdown(ptr('touch', 1));
    vi.advanceTimersByTime(520);
    expect(onLongPress).toHaveBeenCalledTimes(1);
    expect(lp.isNativeTouchMenu({ pointerType: 'touch' } as unknown as Event)).toBe(true);
    // An older WebView gives a plain MouseEvent with no pointerType: still ignored right after a touch.
    expect(lp.isNativeTouchMenu({ button: 0, detail: 1, clientX: 100, clientY: 100 } as unknown as Event)).toBe(true);
    vi.advanceTimersByTime(2000);
    expect(onLongPress).toHaveBeenCalledTimes(1);
  });

  it('when the native menu event comes first it opens ours at once, and only once', () => {
    const onLongPress = vi.fn();
    const lp = createLongPress({ onLongPress });
    lp.pointerdown(ptr('touch', 1, 7, 8));
    vi.advanceTimersByTime(300);
    expect(lp.isNativeTouchMenu({ pointerType: 'touch' } as unknown as Event)).toBe(true);
    expect(onLongPress).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(1000);
    expect(onLongPress).toHaveBeenCalledTimes(1);
  });

  it('the keyboard menu key is not mistaken for a touch', () => {
    const lp = createLongPress({ onLongPress: vi.fn() });
    lp.pointerdown(ptr('touch', 1));
    lp.pointerup(ptr('touch', 1));
    expect(lp.isNativeTouchMenu({ pointerType: '' } as unknown as Event)).toBe(false);
  });

  it('a press outside accepted targets does nothing', () => {
    const onLongPress = vi.fn();
    const lp = createLongPress({ onLongPress, accept: () => false });
    lp.pointerdown(ptr('touch', 1));
    vi.advanceTimersByTime(1000);
    expect(onLongPress).not.toHaveBeenCalled();
  });
});

describe('longPress action', () => {
  it('opens on a held touch, swallows the click that follows, and ignores the native menu', () => {
    const node = document.createElement('div');
    document.body.append(node);
    const onLongPress = vi.fn();
    const act = longPress(node, { onLongPress });
    const fire = (type: string, extra: Record<string, unknown>) => {
      const e = new MouseEvent(type, { bubbles: true, cancelable: true });
      for (const [k, v] of Object.entries(extra)) Object.defineProperty(e, k, { value: v });
      node.dispatchEvent(e);
      return e;
    };
    fire('pointerdown', { pointerType: 'touch', pointerId: 1, clientX: 5, clientY: 6 });
    vi.advanceTimersByTime(600);
    expect(onLongPress).toHaveBeenCalledTimes(1);
    const menu = fire('contextmenu', { pointerType: 'touch' });
    expect(menu.defaultPrevented).toBe(true);
    fire('pointerup', { pointerType: 'touch', pointerId: 1 });
    const onClick = vi.fn();
    node.addEventListener('click', onClick);
    node.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    expect(onClick).not.toHaveBeenCalled();
    // A real right-click stays with the page's own handler.
    const mouseMenu = fire('contextmenu', { pointerType: 'mouse' });
    expect(mouseMenu.defaultPrevented).toBe(false);
    act.destroy();
    node.remove();
  });
});

describe('createDoubleTap', () => {
  it('two taps close in time and place are a double tap, a third starts over', () => {
    let t = 0;
    const d = createDoubleTap({ now: () => t });
    expect(d.tap(10, 10)).toBe(false);
    t = 250;
    expect(d.tap(15, 12)).toBe(true);
    t = 300;
    expect(d.tap(15, 12)).toBe(false);
  });

  it('a slow second tap, or one far away, is a new first tap', () => {
    let t = 0;
    const d = createDoubleTap({ now: () => t });
    d.tap(10, 10);
    t = 400;
    expect(d.tap(10, 10)).toBe(false);
    t = 500;
    expect(d.tap(80, 10)).toBe(false);
  });
});
