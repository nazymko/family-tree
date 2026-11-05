import { describe, expect, it } from 'vitest';
import { makeFakeContext } from '../../test-setup';
import type { ChartEdge, ChartNode, ChartWindow } from '../api/types';
import { draw, lodFor, nameLines, type Theme } from './renderer';

const theme: Theme = { paper: '#fff', rule: '#ccc', ink: '#000', ink2: '#666', ribbon: '#f00', wash: '#fee', font: 'sans-serif', serif: 'serif' };
const view = { w: 800, h: 600, dpr: 2 };

function node(id: number, over: Partial<ChartNode> = {}): ChartNode {
  return {
    id,
    person: { id: `p${id}`, name: `Person ${id}`, lifespan: '1850-1900', sex: 'male', living: false, private: false, siblingMarker: false },
    x: id * 216,
    y: 0,
    w: 176,
    h: 56,
    gen: id,
    ahnen: id + 1,
    dupOf: null,
    moreUp: false,
    moreDown: false,
    nav: { parent: null, child: null, prev: null, next: null },
    label: '',
    ...over,
  };
}
const edge = (i: number, kind: ChartEdge['kind'] = 'parent'): ChartEdge => ({ from: i, to: i + 1, kind, pts: [0, i, 10, i, 10, i + 5, 20, i + 5] });
const ops = (ctx: ReturnType<typeof makeFakeContext>, op: string) => ctx.calls.filter((c) => c.op === op);

describe('renderer', () => {
  it('lod_thresholds', () => {
    expect(lodFor(0.6)).toBe('full');
    expect(lodFor(0.55)).toBe('full');
    expect(lodFor(0.3)).toBe('boxes');
    expect(lodFor(0.2)).toBe('dots');
  });

  const win: ChartWindow = { nodes: [node(0), node(1), node(2)], edges: [] };

  it('draws_text_when_full', () => {
    const ctx = makeFakeContext();
    draw(ctx, win, { x: 0, y: 0, zoom: 1 }, view, { selected: null, focus: null, theme });
    expect(ops(ctx, 'fillText').length).toBeGreaterThanOrEqual(6);
  });

  it('draws_how_each_person_is_related_to_the_starting_person', () => {
    const kin = (up: number): ChartNode['kinship'] => ({ kind: 'blood', up, down: 0, half: false, married: false, sex: 'male', via: 'unknown' });
    const related: ChartWindow = { nodes: [node(0), node(1, { kinship: kin(1) }), node(2, { kinship: kin(2) })], edges: [] };
    const ctx = makeFakeContext();
    draw(ctx, related, { x: 0, y: 0, zoom: 1 }, view, { selected: null, focus: 0, theme });
    const texts = ops(ctx, 'fillText').map((c) => c.args[0]);
    expect(texts).toContain('father');
    expect(texts).toContain('grandfather');
    const plain = makeFakeContext();
    draw(plain, win, { x: 0, y: 0, zoom: 1 }, view, { selected: null, focus: 0, theme });
    expect(ops(plain, 'fillText').map((c) => c.args[0])).not.toContain('father');
  });

  it('draws_the_more_people_handle_unless_asked_not_to', () => {
    const more: ChartWindow = { nodes: [node(0, { moreUp: true })], edges: [] };
    const withHandle = makeFakeContext();
    draw(withHandle, more, { x: 0, y: 0, zoom: 1 }, view, { selected: null, focus: null, theme });
    expect(ops(withHandle, 'arc')).toHaveLength(1);
    const without = makeFakeContext();
    draw(without, more, { x: 0, y: 0, zoom: 1 }, view, { selected: null, focus: null, theme, handles: false });
    expect(ops(without, 'arc')).toHaveLength(0);
  });

  it('no_text_below_0_55', () => {
    const ctx = makeFakeContext();
    draw(ctx, win, { x: 0, y: 0, zoom: 0.3 }, view, { selected: null, focus: null, theme });
    expect(ops(ctx, 'fillText')).toHaveLength(0);
  });

  it('draws_only_given_window', () => {
    const ctx = makeFakeContext();
    expect(draw(ctx, win, { x: 0, y: 0, zoom: 0.3 }, view, { selected: null, focus: null, theme }).drawn).toBe(win.nodes.length);
  });

  it('sets_world_transform', () => {
    const ctx = makeFakeContext();
    draw(ctx, win, { x: 10, y: 20, zoom: 0.5 }, view, { selected: null, focus: null, theme });
    const t = ops(ctx, 'setTransform').at(-1)!.args;
    expect(t).toEqual([1, 0, 0, 1, -10, -20]);
  });

  it('selected_node_has_ribbon_stroke', () => {
    const ctx = makeFakeContext();
    draw(ctx, win, { x: 0, y: 0, zoom: 1 }, view, { selected: 1, focus: null, theme });
    const idx = ctx.calls.findIndex((c) => c.op === 'set:strokeStyle' && c.args[0] === theme.ribbon);
    expect(idx).toBeGreaterThan(-1);
    expect(ctx.calls.slice(idx).some((c) => c.op === 'stroke')).toBe(true);
  });

  it('edges_batched', () => {
    const ctx = makeFakeContext();
    const edges = Array.from({ length: 50 }, (_, i) => edge(i));
    draw(ctx, { nodes: [], edges }, { x: 0, y: 0, zoom: 1 }, view, { selected: null, focus: null, theme });
    expect(ops(ctx, 'beginPath')).toHaveLength(1);
    expect(ops(ctx, 'stroke')).toHaveLength(1);
    expect(ops(ctx, 'moveTo')).toHaveLength(50);
  });

  it('dup_edges_are_dashed', () => {
    const ctx = makeFakeContext();
    draw(ctx, { nodes: [], edges: [edge(0, 'dup')] }, { x: 0, y: 0, zoom: 1 }, view, { selected: null, focus: null, theme });
    expect(ops(ctx, 'setLineDash').some((c) => (c.args[0] as number[]).length > 0)).toBe(true);
  });

  it('more_up_draws_a_plus_handle', () => {
    const ctx = makeFakeContext();
    draw(ctx, { nodes: [node(0, { moreUp: true })], edges: [] }, { x: 0, y: 0, zoom: 1 }, view, { selected: null, focus: null, theme });
    expect(ops(ctx, 'arc')).toHaveLength(1);
  });
});

describe('nameLines', () => {
  // One unit per character keeps the arithmetic obvious.
  const m = (s: string) => s.length;

  it('keeps a name that fits on one line', () => {
    expect(nameLines(m, 'Ivan Koval', 20, true)).toEqual(['Ivan Koval']);
  });

  it('wraps a long name onto two lines when there are no dates', () => {
    expect(nameLines(m, 'Ivan Petrovych Koval', 15, false)).toEqual(['Ivan Petrovych', 'Koval']);
  });

  it('drops the patronymic before cutting when dates take the second line', () => {
    expect(nameLines(m, 'Ivan Petrovych Koval', 15, true)).toEqual(['Ivan Koval']);
  });

  it('cuts with an ellipsis when even the short form does not fit', () => {
    const [line] = nameLines(m, 'Ivan Petrovych Kovalenko-Shevchenko', 12, true);
    expect(line.endsWith('…')).toBe(true);
    expect(line.length).toBeLessThanOrEqual(12);
  });

  it('cuts each wrapped line that is still too long', () => {
    const lines = nameLines(m, 'Oleksandrivna Kovalenko-Shevchenko', 14, false);
    expect(lines).toHaveLength(2);
    for (const l of lines) expect(l.length).toBeLessThanOrEqual(14);
  });
});

describe('renderer unnamed-siblings marker far out', () => {
  const mk = node(1, { person: { id: 'm', name: '', lifespan: '', sex: 'unknown', living: false, private: false, siblingMarker: true } });
  it('is_dashed_and_labelled_not_a_solid_block_at_every_zoom', () => {
    for (const zoom of [0.2, 0.3, 1]) {
      const ctx = makeFakeContext();
      draw(ctx, { nodes: [node(0), mk], edges: [] }, { x: 0, y: 0, zoom }, view, { selected: null, focus: null, theme });
      expect(ctx.calls.some((c) => c.op === 'setLineDash' && (c.args[0] as number[]).length > 0), String(zoom)).toBe(true);
      expect(ops(ctx, 'fillText').some((c) => String(c.args[0]).startsWith('<Some more')), String(zoom)).toBe(true);
    }
  });
});
