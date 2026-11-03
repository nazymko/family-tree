import { describe, expect, it, vi } from 'vitest';
import { flipMove, snapshotRects } from './flip';

function rect(left: number, top: number): DOMRect {
  return { left, top, right: left + 10, bottom: top + 10, width: 10, height: 10, x: left, y: top, toJSON: () => ({}) } as DOMRect;
}

function setup() {
  const container = document.createElement('div');
  const a = document.createElement('div');
  a.dataset.flipKey = 'a';
  const b = document.createElement('div');
  b.dataset.flipKey = 'b';
  container.append(a, b);
  a.getBoundingClientRect = () => rect(100, 50);
  b.getBoundingClientRect = () => rect(0, 0);
  const animate = vi.fn();
  (a as unknown as { animate: unknown }).animate = animate;
  const animateB = vi.fn();
  (b as unknown as { animate: unknown }).animate = animateB;
  return { container, animate, animateB };
}

describe('flip', () => {
  it('animates_moved_elements_from_old_to_new', () => {
    const { container, animate, animateB } = setup();
    const before = new Map([
      ['a', rect(0, 0)],
      ['b', rect(0, 0)],
    ]);
    flipMove(container, before, 260, false);
    expect(animate).toHaveBeenCalledTimes(1);
    const [frames, opts] = animate.mock.calls[0];
    expect(frames[0].transform).toBe('translate(-100px, -50px)');
    expect(opts.duration).toBe(260);
    expect(animateB).not.toHaveBeenCalled();
  });

  it('noop_when_reduced', () => {
    const { container, animate } = setup();
    flipMove(container, new Map([['a', rect(0, 0)]]), 260, true);
    expect(animate).not.toHaveBeenCalled();
  });

  it('snapshot_collects_keys', () => {
    const { container } = setup();
    expect([...snapshotRects(container).keys()]).toEqual(['a', 'b']);
  });
});
