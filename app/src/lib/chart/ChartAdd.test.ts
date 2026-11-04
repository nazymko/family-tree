import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { en } from '../i18n/en';
import { uk } from '../i18n/uk';
import type { MockApi } from '../api/mock/MockApi';
import type { ChartKind, Id } from '../api/types';
import { addOptions, parentFacts } from '../family/addOptions';
import { added } from '../state/added.svelte';
import { project } from '../state/project.svelte';
import { ui } from '../state/ui.svelte';
import ChartCanvas from './ChartCanvas.svelte';
import WholeCanvas from './WholeCanvas.svelte';
import Host from './testing/Host.svelte';
import { allNodes, attachContext, fakeRaf, lastCamera, makeApi, richestRoot } from './testing/helpers';

const RECT = { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600, x: 0, y: 0, toJSON() {} };
const draft = { given: 'Olena', surnames: ['Koval'], sex: 'female' as const };

const menu = () => screen.getByRole('menu', { name: /^(Save as picture|Person and picture options)$/ });
const labels = (m: HTMLElement) => within(m).getAllByRole('menuitem').map((i) => i.textContent?.trim());
const selectedLabel = () => screen.getByRole('option').textContent ?? '';

type Fx = ReturnType<typeof makeApi>['fx'];

async function setupChart(kind: ChartKind = 'pedigree', rootOf?: (api: MockApi, fx: Fx) => Promise<Id>) {
  const mm = window.matchMedia;
  window.matchMedia = ((q: string) => ({ ...mm(q), matches: q.includes('reduce') })) as typeof window.matchMedia;
  const raf = fakeRaf();
  const { api, fx } = makeApi();
  const root = rootOf ? await rootOf(api, fx) : await richestRoot(api, fx, kind);
  const openSpy = vi.spyOn(api, 'chartOpen');
  const winSpy = vi.spyOn(api, 'chartWindow');
  const onOpenPerson = vi.fn();
  render(Host, { props: { api, view: ChartCanvas, props: { kind, root, generations: 5, onRecentre: vi.fn(), onOpenPerson } } });
  const canvas = screen.getByRole('application') as HTMLCanvasElement;
  const ctx = attachContext(canvas);
  canvas.getBoundingClientRect = () => RECT;
  await waitFor(() => expect(openSpy).toHaveBeenCalled());
  await waitFor(() => expect(winSpy.mock.calls.length).toBeGreaterThanOrEqual(2));
  await new Promise((r) => setTimeout(r, 0));
  raf.pump(2);
  return { api, fx, raf, canvas, ctx, openSpy, root, onOpenPerson };
}

/** A person with one partner and at least one child, so a child can be added without choosing a family. */
async function parentOfOne(api: MockApi, fx: Fx): Promise<Id> {
  for (const p of fx.persons) {
    const v = await api.getFamilyView(p.id);
    if (v.partnerships.length === 1 && v.partnerships[0].children.length >= 1) return p.id;
  }
  throw new Error('no suitable parent in the fixture');
}

/** Where the centre of a box is on the 800x600 screen. */
function screenCentre(ctx: ReturnType<typeof attachContext>, n: { x: number; y: number; w: number; h: number }) {
  const cam = lastCamera(ctx)!;
  return { clientX: (n.x + n.w / 2 - cam.x) * cam.zoom, clientY: (n.y + n.h / 2 - cam.y) * cam.zoom };
}

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  ui.closeAdd();
});

describe('add a relative from a chart', () => {
  it('a_right_click_on_a_box_starts_with_the_add_items_then_open_person_then_the_save_items', async () => {
    const s = await setupChart();
    const cam = lastCamera(s.ctx)!;
    const nodes = await allNodes(s.api as MockApi, (await s.openSpy.mock.results[0].value).chartId);
    const first = selectedLabel();
    const other = nodes.find(
      (n) => n.label !== first && n.dupOf === null && n.person && n.x >= cam.x && (n.x + n.w - cam.x) * cam.zoom < 780 && (n.y - cam.y) * cam.zoom > 0 && (n.y + n.h - cam.y) * cam.zoom < 590,
    )!;
    expect(other).toBeTruthy();
    await fireEvent.contextMenu(s.canvas, { button: 2, ...screenCentre(s.ctx, other) });
    await waitFor(() => expect(menu()).toBeTruthy());
    const items = labels(menu());
    // A disabled item carries its hint in the same row, so match on the start of the text.
    const at = (text: string) => items.findIndex((i) => i?.startsWith(text));
    expect(at('Add partner')).toBeGreaterThanOrEqual(0);
    expect(at('Add child')).toBeGreaterThan(at('Add partner'));
    expect(at('Open person')).toBe(at('Add sibling') + 1);
    expect(items.slice(items.indexOf('Open person') + 1)).toEqual([
      'Save the whole chart as a picture…',
      "Save the selected person's branch as a picture…",
      'Save what is on the screen as a picture…',
    ]);
    expect(menu().querySelectorAll('hr')).toHaveLength(1);
  });

  it('empty_space_shows_only_the_save_items', async () => {
    const s = await setupChart();
    await fireEvent.contextMenu(s.canvas, { button: 2, clientX: 5, clientY: 5 });
    await waitFor(() => expect(menu()).toBeTruthy());
    expect(labels(menu())).toHaveLength(3);
    expect(screen.queryByRole('menuitem', { name: /Add mother/ })).toBeNull();
  });

  it('follows_the_disable_rules_of_the_family_page', async () => {
    const s = await setupChart();
    const expected = addOptions(parentFacts(await s.api.getFamilyView(s.root)));
    const names: Record<string, string> = {
      'family.addAnotherParents': 'Add another set of parents',
      'edit.addFather': 'Add father',
      'edit.addMother': 'Add mother',
      'edit.addPartner': 'Add partner',
      'edit.addChild': 'Add child',
      'edit.addSibling': 'Add sibling',
    };
    s.canvas.focus();
    await fireEvent.keyDown(s.canvas, { key: 'ContextMenu' });
    await waitFor(() => expect(menu()).toBeTruthy());
    for (const o of expected) {
      const item = within(menu()).getByRole('menuitem', { name: new RegExp(`^${names[o.label]}`) });
      if (o.disabled) expect(item).toHaveAttribute('aria-disabled', 'true');
      else expect(item).not.toHaveAttribute('aria-disabled', 'true');
    }
  });

  it('a_person_without_parents_cannot_add_a_sibling_and_is_told_why', async () => {
    const s = await setupChart('pedigree', async (api, fx) => {
      for (const p of fx.persons) if (!parentFacts(await api.getFamilyView(p.id)).hasAnyParent) return p.id;
      throw new Error('no person without parents in the fixture');
    });
    s.canvas.focus();
    await fireEvent.keyDown(s.canvas, { key: 'F10', shiftKey: true });
    await waitFor(() => expect(menu()).toBeTruthy());
    const sibling = within(menu()).getByRole('menuitem', { name: /^Add sibling/ });
    expect(sibling).toHaveAttribute('aria-disabled', 'true');
    expect(within(menu()).getByText('Add a parent first, so the sibling has someone to belong to')).toBeInTheDocument();
    const spy = vi.spyOn(ui, 'requestAdd');
    await fireEvent.click(sibling);
    expect(spy).not.toHaveBeenCalled();
    expect(within(menu()).getByRole('menuitem', { name: /^Add father/ })).not.toHaveAttribute('aria-disabled', 'true');
  });

  it('choosing_an_item_asks_for_the_dialog_with_the_relation_and_the_person', async () => {
    const s = await setupChart();
    const spy = vi.spyOn(ui, 'requestAdd');
    s.canvas.focus();
    await fireEvent.keyDown(s.canvas, { key: 'ContextMenu' });
    await waitFor(() => expect(menu()).toBeTruthy());
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /^Add partner/ }));
    expect(spy).toHaveBeenCalledWith('partner', s.root, undefined);
    expect(screen.queryByRole('menu')).toBeNull();
    expect(s.canvas).toHaveFocus();
    await fireEvent.keyDown(s.canvas, { key: 'ContextMenu' });
    await waitFor(() => expect(menu()).toBeTruthy());
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /^Add child/ }));
    expect(spy).toHaveBeenLastCalledWith('child', s.root, undefined);
  });

  it('another_set_of_parents_asks_for_a_new_parent_set', async () => {
    const s = await setupChart('pedigree', async (api, fx) => {
      for (const p of fx.persons) {
        const f = parentFacts(await api.getFamilyView(p.id));
        if (f.fatherRecorded && f.motherRecorded) return p.id;
      }
      throw new Error('no person with both parents in the fixture');
    });
    const spy = vi.spyOn(ui, 'requestAdd');
    s.canvas.focus();
    await fireEvent.keyDown(s.canvas, { key: 'ContextMenu' });
    await waitFor(() => expect(menu()).toBeTruthy());
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /^Add another set of parents/ }));
    expect(spy).toHaveBeenCalledWith('father', s.root, true);
  });

  it('open_person_opens_the_person_page', async () => {
    const s = await setupChart();
    s.canvas.focus();
    await fireEvent.keyDown(s.canvas, { key: 'F10', shiftKey: true });
    await waitFor(() => expect(menu()).toBeTruthy());
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: 'Open person' }));
    expect(s.onOpenPerson).toHaveBeenCalledWith(s.root);
  });

  it('the_toolbar_has_an_add_relative_button_for_the_selected_person', async () => {
    const s = await setupChart();
    const spy = vi.spyOn(ui, 'requestAdd');
    const button = await screen.findByRole('button', { name: 'Add relative' });
    await fireEvent.click(button);
    const m = screen.getByRole('menu', { name: 'Add relative' });
    expect(labels(m)).toEqual(expect.arrayContaining(['Add partner', 'Add child', 'Add sibling']));
    await fireEvent.click(within(m).getByRole('menuitem', { name: 'Add child' }));
    expect(spy).toHaveBeenCalledWith('child', s.root, undefined);
  });

  it('the_chart_reloads_after_an_edit_keeps_the_camera_and_selects_the_new_person', async () => {
    const s = await setupChart('descendants', parentOfOne);
    const camBefore = lastCamera(s.ctx)!;
    const opens = s.openSpy.mock.calls.length;
    const fam = (await s.api.getFamilyView(s.root)).partnerships[0].familyId;
    const res = await s.api.apply({ type: 'quickAdd', anchor: s.root, relation: 'child', draft, family: fam });
    project.rev++;
    await waitFor(() => expect(s.openSpy.mock.calls.length).toBe(opens + 1));
    added.note(res.focus!);
    await waitFor(() => expect(selectedLabel()).toContain('Olena Koval'));
    s.raf.pump(3);
    expect(lastCamera(s.ctx)).toEqual(camBefore);
  });

  it('a_note_that_arrives_before_the_chart_has_reloaded_still_selects_the_new_person', async () => {
    const s = await setupChart('descendants', parentOfOne);
    const fam = (await s.api.getFamilyView(s.root)).partnerships[0].familyId;
    const res = await s.api.apply({ type: 'quickAdd', anchor: s.root, relation: 'child', draft, family: fam });
    added.note(res.focus!);
    await new Promise((r) => setTimeout(r, 0));
    project.rev++;
    await waitFor(() => expect(selectedLabel()).toContain('Olena Koval'));
  });

  it('the_selected_person_stays_selected_after_an_edit_elsewhere', async () => {
    const s = await setupChart();
    s.canvas.focus();
    await fireEvent.keyDown(s.canvas, { key: 'ArrowRight' });
    const sel = selectedLabel();
    const opens = s.openSpy.mock.calls.length;
    project.rev++;
    await waitFor(() => expect(s.openSpy.mock.calls.length).toBe(opens + 1));
    await waitFor(() => expect(selectedLabel()).toBe(sel));
  });
});

describe('add a relative on the whole tree', () => {
  async function setupWhole() {
    const mm = window.matchMedia;
    window.matchMedia = ((q: string) => ({ ...mm(q), matches: q.includes('reduce') })) as typeof window.matchMedia;
    const raf = fakeRaf();
    const { api, fx } = makeApi(300);
    const openSpy = vi.spyOn(api, 'chartOpenWhole');
    const winSpy = vi.spyOn(api, 'chartWindow');
    render(Host, { props: { api, view: WholeCanvas, props: { homeId: fx.persons[0].id, onOpenPerson: vi.fn(), onOpenInPedigree: vi.fn() } } });
    const canvas = screen.getByRole('application', { name: 'Whole tree chart' }) as HTMLCanvasElement;
    const ctx = attachContext(canvas);
    canvas.getBoundingClientRect = () => RECT;
    const mini = (await screen.findByRole('application', { name: /Minimap/ })) as HTMLCanvasElement;
    mini.getBoundingClientRect = () => ({ ...RECT, width: 220, height: mini.height, right: 220, bottom: mini.height });
    attachContext(mini);
    await waitFor(() => expect(openSpy).toHaveBeenCalled());
    await waitFor(() => expect(winSpy.mock.calls.length).toBeGreaterThanOrEqual(2));
    await new Promise((r) => setTimeout(r, 0));
    raf.pump(2);
    return { api, fx, raf, canvas, ctx, openSpy };
  }

  it('shows_the_add_items_for_a_selected_card_and_not_for_empty_space', async () => {
    const s = await setupWhole();
    const spy = vi.spyOn(ui, 'requestAdd');
    s.canvas.focus();
    await fireEvent.keyDown(s.canvas, { key: 'ContextMenu' });
    await waitFor(() => expect(menu()).toBeTruthy());
    expect(labels(menu())).toEqual(expect.arrayContaining(['Add partner', 'Add child', 'Open person']));
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /^Add child/ }));
    expect(spy).toHaveBeenCalledWith('child', s.fx.persons[0].id, undefined);
    await fireEvent.keyDown(s.canvas, { key: 'Escape' }); // clears the selection
    await fireEvent.contextMenu(s.canvas, { button: 2, clientX: 2, clientY: 2 });
    await waitFor(() => expect(menu()).toBeTruthy());
    expect(labels(menu())).toHaveLength(3);
  });

  it('reloads_after_an_edit_keeps_the_camera_and_selects_the_new_person', async () => {
    const s = await setupWhole();
    const id = s.fx.persons[0].id;
    await fireEvent.keyDown(s.canvas, { key: 'Escape' });
    s.raf.pump(2);
    const camBefore = lastCamera(s.ctx)!;
    const opens = s.openSpy.mock.calls.length;
    const res = await s.api.apply({ type: 'quickAdd', anchor: id, relation: 'partner', draft });
    project.rev++;
    added.note(res.focus!);
    await waitFor(() => expect(s.openSpy.mock.calls.length).toBe(opens + 1));
    await waitFor(() => expect(selectedLabel()).toContain('Olena Koval'));
    s.raf.pump(3);
    expect(lastCamera(s.ctx)).toEqual(camBefore);
  });
});

describe('add a relative: text', () => {
  it('has_english_and_ukrainian_keys', () => {
    for (const k of ['addRelative', 'menuLabel', 'openPerson'] as const) {
      expect(en.chartAdd[k]).toBeTruthy();
      expect(uk.chartAdd[k]).toBeTruthy();
      expect(uk.chartAdd[k]).not.toBe(en.chartAdd[k]);
    }
  });
});
