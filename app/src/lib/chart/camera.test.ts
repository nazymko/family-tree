import { describe, expect, it, vi } from 'vitest';
import { ZOOM_MAX, ZOOM_MIN, centerOn, panBy, tween, viewRect, zoomAt, type Camera } from './camera';

const view = { w: 800, h: 600 };

describe('camera', () => {
  it('zoom_keeps_anchor_fixed', () => {
    const c: Camera = { x: 120, y: -40, zoom: 0.8 };
    const a = { x: 310, y: 222 };
    const before = { x: c.x + a.x / c.zoom, y: c.y + a.y / c.zoom };
    const z = zoomAt(c, 1.7, a);
    expect(z.x + a.x / z.zoom).toBeCloseTo(before.x, 6);
    expect(z.y + a.y / z.zoom).toBeCloseTo(before.y, 6);
  });

  it('zoom_clamped', () => {
    let c: Camera = { x: 0, y: 0, zoom: 1 };
    for (let i = 0; i < 50; i++) c = zoomAt(c, 1.5, { x: 5, y: 5 });
    expect(c.zoom).toBe(ZOOM_MAX);
    for (let i = 0; i < 50; i++) c = zoomAt(c, 0.5, { x: 5, y: 5 });
    expect(c.zoom).toBe(ZOOM_MIN);
  });

  it('pan_keeps_content_in_reach', () => {
    const bounds = { w: 2000, h: 1000 };
    const far = panBy({ x: 0, y: 0, zoom: 1 }, -1e6, -1e6, bounds, view);
    const r = viewRect(far, view);
    expect(r.x).toBeLessThanOrEqual(bounds.w - 0.25 * bounds.w);
    expect(r.y).toBeLessThanOrEqual(bounds.h - 0.25 * bounds.h);
    const back = panBy({ x: 0, y: 0, zoom: 1 }, 1e6, 1e6, bounds, view);
    const rb = viewRect(back, view);
    expect(rb.x + rb.w).toBeGreaterThanOrEqual(Math.min(0.25 * bounds.w, view.w) - 1e-6);
    expect(rb.y + rb.h).toBeGreaterThanOrEqual(Math.min(0.25 * bounds.h, view.h) - 1e-6);
  });

  it('pan_moves_content_with_the_pointer', () => {
    const c = panBy({ x: 100, y: 100, zoom: 2 }, 20, -10, { w: 5000, h: 5000 }, view);
    expect(c).toEqual({ x: 90, y: 105, zoom: 2 });
  });

  it('center_on_puts_node_in_middle', () => {
    const c = centerOn({ x: 0, y: 0, zoom: 0.5 }, { x: 1000, y: 400, w: 176, h: 56 }, view);
    const r = viewRect(c, view);
    expect(r.x + r.w / 2).toBeCloseTo(1088, 6);
    expect(r.y + r.h / 2).toBeCloseTo(428, 6);
  });

  it('tween_reduced_motion_jumps', () => {
    const step = vi.fn();
    const to = { x: 5, y: 6, zoom: 1 };
    tween({ x: 0, y: 0, zoom: 1 }, to, 280, step, true);
    expect(step).toHaveBeenCalledTimes(1);
    expect(step).toHaveBeenCalledWith(to);
  });

  it('tween_animates_to_target', () => {
    vi.useFakeTimers();
    const step = vi.fn();
    const to = { x: 100, y: 0, zoom: 1 };
    tween({ x: 0, y: 0, zoom: 1 }, to, 100, step, false);
    vi.advanceTimersByTime(400);
    expect(step.mock.calls.length).toBeGreaterThan(1);
    expect(step.mock.calls.at(-1)![0].x).toBeCloseTo(100, 6);
    vi.useRealTimers();
  });
});
