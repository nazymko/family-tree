import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import type { ChartKind, ChartNode } from '../api/types';
import type { MockApi } from '../api/mock/MockApi';
import ChartCanvas from './ChartCanvas.svelte';
import WholeCanvas from './WholeCanvas.svelte';
import Host from './testing/Host.svelte';
import { allNodes, attachContext, fakeRaf, lastCamera, makeApi, richestRoot } from './testing/helpers';

const RECT = { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600, x: 0, y: 0, toJSON() {} };

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

/** A tree where one family carries the "more brothers and sisters" entry. */
async function withMarker(kind: ChartKind) {
  const { api, fx } = makeApi(30);
  const root = await richestRoot(api, fx, kind);
  const view = await api.getFamilyView(root);
  const fam = view.partnerships[0]!.familyId;
  await api.apply({ type: 'addSiblingMarker', family: fam });
  const marker = (await api.getFamily(fam)).children.find((c) => c.person.siblingMarker)!.person.id;
  return { api, fx, root, marker };
}

/** The screen point at the middle of a node, from the camera the canvas last drew with. */
function centreOf(n: ChartNode, ctx: ReturnType<typeof attachContext>) {
  const cam = lastCamera(ctx)!;
  return { clientX: (n.x + n.w / 2 - cam.x) * cam.zoom, clientY: (n.y + n.h / 2 - cam.y) * cam.zoom };
}

async function openChart() {
  const raf = fakeRaf();
  const s = await withMarker('descendants');
  const onRecentre = vi.fn();
  const onOpenPerson = vi.fn();
  const openSpy = vi.spyOn(s.api, 'chartOpen');
  const viewSpy = vi.spyOn(s.api, 'getFamilyView');
  render(Host, { props: { api: s.api, view: ChartCanvas, props: { kind: 'descendants', root: s.root, generations: 6, onRecentre, onOpenPerson } } });
  const canvas = screen.getByRole('application') as HTMLCanvasElement;
  const ctx = attachContext(canvas);
  canvas.getBoundingClientRect = () => RECT;
  await waitFor(() => expect(openSpy).toHaveBeenCalled());
  await waitFor(() => expect(screen.queryByRole('option')).not.toBeNull());
  await new Promise((r) => setTimeout(r, 0));
  raf.pump(2);
  const id = (await openSpy.mock.results[0].value).chartId;
  const nodes = await allNodes(s.api as MockApi, id);
  const markerNode = nodes.find((n) => n.person?.siblingMarker)!;
  expect(markerNode).toBeTruthy();
  const real = nodes.find((n) => n.person && !n.person.siblingMarker && n.id !== 0 && n.dupOf === null)!;
  return { ...s, raf, canvas, ctx, onRecentre, onOpenPerson, viewSpy, markerNode, real };
}

describe('the unnamed-siblings marker on the chart', () => {
  it('a_real_person_next_to_it_does_recentre', async () => {
    const s = await openChart();
    const at = centreOf(s.real, s.ctx);
    await fireEvent.pointerDown(s.canvas, { ...at, pointerId: 1 });
    await fireEvent.pointerUp(s.canvas, { ...at, pointerId: 1 });
    await fireEvent.dblClick(s.canvas, at);
    expect(s.onRecentre).toHaveBeenCalledWith(s.real.person!.id);
  });

  it('double_click_and_enter_do_not_recentre_on_it', async () => {
    const s = await openChart();
    const m = centreOf(s.markerNode, s.ctx);
    // A right-click selects the box under it; that proves the point below is on the marker.
    await fireEvent.contextMenu(s.canvas, { button: 2, ...m });
    await waitFor(() => expect(screen.getByRole('option').textContent).toContain('brothers and sisters'));
    await fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
    s.onRecentre.mockClear();
    await fireEvent.dblClick(s.canvas, m);
    await fireEvent.keyDown(s.canvas, { key: 'Enter' });
    await fireEvent.keyDown(s.canvas, { key: 'e' });
    expect(screen.getByRole('option').textContent).toContain('brothers and sisters');
    expect(s.onRecentre).not.toHaveBeenCalled();
    expect(s.onOpenPerson).not.toHaveBeenCalled();
  });

  it('right_click_offers_no_person_items_and_reads_no_family', async () => {
    const s = await openChart();
    const m = centreOf(s.markerNode, s.ctx);
    s.viewSpy.mockClear();
    await fireEvent.contextMenu(s.canvas, { button: 2, ...m });
    await waitFor(() => expect(screen.getByRole('option').textContent).toContain('brothers and sisters'));
    const menu = await screen.findByRole('menu', { name: 'Save as picture' });
    expect(within(menu).queryByRole('menuitem', { name: /Open person/ })).toBeNull();
    expect(s.viewSpy).not.toHaveBeenCalled();
  });
});

describe('the unnamed-siblings marker on the whole-tree chart', () => {
  it('right_click_offers_no_person_items_and_reads_no_family', async () => {
    const mm = window.matchMedia;
    window.matchMedia = ((q: string) => ({ ...mm(q), matches: q.includes('reduce') })) as typeof window.matchMedia;
    const raf = fakeRaf();
    const s = await withMarker('descendants');
    const openSpy = vi.spyOn(s.api, 'chartOpenWhole');
    const viewSpy = vi.spyOn(s.api, 'getFamilyView');
    render(Host, { props: { api: s.api, view: WholeCanvas, props: { homeId: null, onOpenPerson: vi.fn(), onOpenInPedigree: vi.fn() } } });
    const canvas = screen.getByRole('application', { name: 'Whole tree chart' }) as HTMLCanvasElement;
    const ctx = attachContext(canvas);
    canvas.getBoundingClientRect = () => RECT;
    const mini = (await screen.findByRole('application', { name: /Minimap/ })) as HTMLCanvasElement;
    mini.getBoundingClientRect = () => ({ ...RECT, width: 220, height: mini.height, right: 220, bottom: mini.height });
    attachContext(mini);
    await waitFor(() => expect(openSpy).toHaveBeenCalled());
    await new Promise((r) => setTimeout(r, 0));
    raf.pump(2);
    const id = (await openSpy.mock.results[0].value).meta.chartId;
    const node = (await allNodes(s.api as MockApi, id)).find((n) => n.person?.siblingMarker)!;
    expect(node).toBeTruthy();
    viewSpy.mockClear();
    await fireEvent.contextMenu(canvas, { button: 2, ...centreOf(node, ctx) });
    const menu = await screen.findByRole('menu', { name: 'Save as picture' });
    expect(menu).toBeTruthy();
    expect(screen.getByRole('option').textContent).toContain('brothers and sisters');
    expect(viewSpy).not.toHaveBeenCalled();
    window.matchMedia = mm;
  });
});
