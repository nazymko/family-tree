import { vi, type MockInstance } from 'vitest';
import { MockApi } from '../../api/mock/MockApi';
import type { ChartEdge, ChartMeta, ChartNode, ChartRequest } from '../../api/types';

/**
 * A synthetic "built around" chart big enough to cut: the centred person, parents, four grandparents (surnames Bondar,
 * Hrytsenko, Melnyk, Tkach), and three generations of ancestors above every grandparent.
 */
export function bigAround(): { nodes: ChartNode[]; edges: ChartEdge[]; root: number } {
  const nav = { parent: null, child: null, prev: null, next: null };
  const nodes: ChartNode[] = [];
  const edges: ChartEdge[] = [];
  const add = (gen: number, surname = 'Testenko'): number => {
    const id = nodes.length + 1;
    const name = `Osoba${id} ${surname}`;
    nodes.push({
      id,
      person: { id: `syn-${id}`, name, lifespan: '1900 – 1970', sex: id % 2 ? 'male' : 'female', living: false, private: false, siblingMarker: false },
      x: id * 160,
      y: (gen + 6) * 110,
      w: 140,
      h: 60,
      gen,
      ahnen: null,
      dupOf: null,
      moreUp: false,
      moreDown: false,
      nav,
      label: name,
    });
    return id;
  };
  const edge = (from: number, to: number, kind: ChartEdge['kind']) => {
    const a = nodes[from - 1];
    const b = nodes[to - 1];
    edges.push({ from, to, kind, pts: [a.x + a.w / 2, a.y + a.h, b.x + b.w / 2, b.y] });
  };
  const parents = (c: number, sf = 'Testenko', sm = 'Testenko'): [number, number] => {
    const g = nodes[c - 1].gen - 1;
    const f = add(g, sf);
    const m = add(g, sm);
    edge(f, m, 'partner');
    edge(f, c, 'parent');
    edge(m, c, 'parent');
    return [f, m];
  };
  const chain = (id: number, depth: number): void => {
    if (depth <= 0) return;
    const [f, m] = parents(id);
    chain(f, depth - 1);
    chain(m, depth - 1);
  };
  const root = add(0);
  const [f, m] = parents(root);
  const [ff, fm] = parents(f, 'Bondar', 'Hrytsenko');
  const [mf, mm] = parents(m, 'Melnyk', 'Tkach');
  for (const g of [ff, fm, mf, mm]) chain(g, 3);
  return { nodes, edges, root };
}

/** The HTML documents sent to the print frame (the frame is never added, so the print ends at once). */
export function capturePrints(): string[] {
  const printed: string[] = [];
  const append = document.body.appendChild.bind(document.body);
  vi.spyOn(document.body, 'appendChild').mockImplementation(<T extends Node>(n: T): T => {
    if (n instanceof HTMLIFrameElement) {
      printed.push(n.srcdoc);
      setTimeout(() => n.onload?.(new Event('load')), 0);
      return n;
    }
    return append(n);
  });
  return printed;
}

/**
 * The same people as `big` as a print chart lays them out: other node ids, smaller cards and other coordinates (as the
 * compact print metrics and the best-fit direction give).
 */
export function asPrintChart(big: ReturnType<typeof bigAround>): ReturnType<typeof bigAround> {
  const id = (n: number) => n + 500;
  return {
    nodes: big.nodes.map((n) => ({ ...n, id: id(n.id), x: n.x * 0.7 + 30, y: n.y * 0.8 + 10, w: n.w * 0.7, h: n.h * 0.8, dupOf: n.dupOf === null ? null : id(n.dupOf) })),
    edges: big.edges.map((e) => ({ ...e, from: id(e.from), to: id(e.to), pts: e.pts.map((v, i) => (i % 2 ? v * 0.8 + 10 : v * 0.7 + 30)) })),
    root: id(big.root),
  };
}

/**
 * Serves `big` to every print chart the picture code opens (`req.print`), or what `big(req)` gives (a chart per
 * direction); the chart on screen stays the mock's, or is `screen` when given. `openSpy` is the spy already on
 * `api.chartOpen`.
 */
export function servePrintCharts(
  api: MockApi,
  openSpy: MockInstance<MockApi['chartOpen']>,
  big: ReturnType<typeof bigAround> | ((req: ChartRequest) => ReturnType<typeof bigAround>),
  screen?: ReturnType<typeof bigAround>,
): void {
  const ids = new Map<number, ReturnType<typeof bigAround>>();
  let next = 9000;
  const realOpen = MockApi.prototype.chartOpen;
  const meta = (chartId: number, c: ReturnType<typeof bigAround>): ChartMeta => {
    const w = Math.max(...c.nodes.map((n) => n.x + n.w));
    const h = Math.max(...c.nodes.map((n) => n.y + n.h));
    return { chartId, kind: 'around', bounds: { w, h }, nodeCount: c.nodes.length, personCount: c.nodes.length, rootNode: c.root, truncated: false, version: 0 };
  };
  openSpy.mockImplementation(async (req: ChartRequest): Promise<ChartMeta> => {
    const chart = req.print ? (typeof big === 'function' ? big(req) : big) : screen;
    if (!chart) return realOpen.call(api, req);
    const chartId = next++;
    ids.set(chartId, chart);
    return meta(chartId, chart);
  });
  const realWindow = MockApi.prototype.chartWindow;
  vi.spyOn(api, 'chartWindow').mockImplementation(async (id, rect, zoom) => {
    const c = ids.get(id);
    return c ? structuredClone({ nodes: c.nodes, edges: c.edges }) : realWindow.call(api, id, rect, zoom);
  });
  const realClose = MockApi.prototype.chartClose;
  vi.spyOn(api, 'chartClose').mockImplementation(async (id) => (ids.has(id) ? undefined : realClose.call(api, id)));
}
