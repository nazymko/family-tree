import { vi } from 'vitest';
import { MockApi } from '../../api/mock/MockApi';
import { fixtureTree } from '../../api/mock/fixtures';
import type { ChartNode, Id } from '../../api/types';
import { makeFakeContext } from '../../../test-setup';

export const BIG = { x: -1e6, y: -1e6, w: 2e6, h: 2e6 };

export function makeApi(n = 500) {
  const fx = fixtureTree(n);
  return { fx, api: new MockApi(fx) };
}

/** The person whose pedigree has the most nodes (the mock tree is shallow). */
export async function richestRoot(api: MockApi, fx: ReturnType<typeof fixtureTree>, kind: 'pedigree' | 'descendants'): Promise<Id> {
  let best = fx.persons[0].id;
  let count = 0;
  for (const p of fx.persons) {
    const m = await api.chartOpen({ kind, root: p.id, generations: 5 });
    await api.chartClose(m.chartId);
    if (m.nodeCount > count) {
      count = m.nodeCount;
      best = p.id;
    }
  }
  return best;
}

/** Controllable requestAnimationFrame. */
export function fakeRaf() {
  let q: FrameRequestCallback[] = [];
  vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
    q.push(cb);
    return q.length;
  });
  vi.stubGlobal('cancelAnimationFrame', () => {});
  return {
    pump(times = 1) {
      for (let i = 0; i < times; i++) {
        const run = q;
        q = [];
        for (const cb of run) cb(performance.now());
      }
    },
    pending: () => q.length,
  };
}

export function attachContext(canvas: HTMLCanvasElement) {
  const ctx = makeFakeContext();
  canvas.getContext = (() => ctx) as unknown as typeof canvas.getContext;
  return ctx;
}

/** Recovers the camera from the last world transform the renderer set. */
export function lastCamera(ctx: ReturnType<typeof makeFakeContext>, dpr = 1) {
  const t = ctx.calls.filter((c) => c.op === 'setTransform').at(-1)?.args as number[] | undefined;
  if (!t) return null;
  const z = t[0] / dpr;
  return { zoom: z, x: -t[4] / (dpr * z), y: -t[5] / (dpr * z) };
}

export async function allNodes(api: MockApi, chartId: number): Promise<ChartNode[]> {
  return (await api.chartWindow(chartId, BIG, 1)).nodes;
}
