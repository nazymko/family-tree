import { describe, expect, it, vi } from 'vitest';
import type { ChartWindow, Rect } from '../api/types';
import { WindowCache } from './window';

const empty = (tag = 0): ChartWindow => ({ nodes: [], edges: [{ from: tag, to: 0, kind: 'parent', pts: [] }] });
const flush = () => new Promise((r) => setTimeout(r, 0));
const view: Rect = { x: 1000, y: 1000, w: 800, h: 600 };

describe('WindowCache', () => {
  it('no_refetch_while_inside', async () => {
    const fetch = vi.fn(async () => empty());
    const c = new WindowCache(fetch);
    c.need(view, 1);
    for (let i = 0; i < 100; i++) c.need({ ...view, x: view.x + i * 5, y: view.y - i * 3 }, 1);
    await flush();
    for (let i = 0; i < 100; i++) c.need({ ...view, x: view.x - i * 5 }, 1);
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(c.current()).not.toBeNull();
  });

  it('fetches_view_inflated_by_100_percent', async () => {
    const fetch = vi.fn(async (r: Rect, z: number) => (void r, void z, empty()));
    new WindowCache(fetch).need(view, 1);
    expect(fetch).toHaveBeenCalledWith({ x: 200, y: 400, w: 2400, h: 1800 }, 1);
  });

  it('refetch_when_leaving', async () => {
    const fetch = vi.fn(async () => empty());
    const c = new WindowCache(fetch);
    c.need(view, 1);
    await flush();
    c.need({ ...view, x: view.x + 5000 }, 1);
    expect(fetch).toHaveBeenCalledTimes(2);
  });

  it('refetch_on_lod_change', async () => {
    const fetch = vi.fn(async () => empty());
    const c = new WindowCache(fetch);
    c.need(view, 1);
    await flush();
    c.need(view, 0.3);
    expect(fetch).toHaveBeenCalledTimes(2);
    await flush();
    c.need(view, 0.35);
    expect(fetch).toHaveBeenCalledTimes(2);
  });

  it('stale_response_dropped', async () => {
    const resolvers: ((w: ChartWindow) => void)[] = [];
    const fetch = vi.fn(() => new Promise<ChartWindow>((res) => resolvers.push(res)));
    const c = new WindowCache(fetch);
    c.need(view, 1);
    c.need({ ...view, x: view.x + 9000 }, 1);
    expect(fetch).toHaveBeenCalledTimes(2);
    resolvers[1](empty(2));
    await flush();
    resolvers[0](empty(1));
    await flush();
    expect(c.current()!.win.edges[0].from).toBe(2);
  });

  it('at_most_two_in_flight', () => {
    const fetch = vi.fn(() => new Promise<ChartWindow>(() => {}));
    const c = new WindowCache(fetch);
    for (let i = 0; i < 5; i++) c.need({ ...view, x: i * 10000 }, 1);
    expect(fetch).toHaveBeenCalledTimes(2);
  });

  it('notifies_on_change', async () => {
    const c = new WindowCache(async () => empty());
    const cb = vi.fn();
    c.onChange(cb);
    c.need(view, 1);
    await flush();
    expect(cb).toHaveBeenCalledTimes(1);
  });

  it('invalidate_drops_cache_and_inflight', async () => {
    const fetch = vi.fn(async () => empty());
    const c = new WindowCache(fetch);
    c.need(view, 1);
    c.invalidate();
    await flush();
    expect(c.current()).toBeNull();
    c.need(view, 1);
    expect(fetch).toHaveBeenCalledTimes(2);
  });
});
