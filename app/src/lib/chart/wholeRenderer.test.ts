import { describe, expect, it } from 'vitest';
import type { ChartEdge, ChartNode, ChartWindow } from '../api/types';
import { makeFakeContext } from '../../test-setup';
import { drawWhole, edgeOnLine, headingSize, type WholeTheme } from './wholeRenderer';

const theme: WholeTheme = {
  paper: '#fff', rule: '#ccc', ink: '#000', ink2: '#555', ribbon: '#900', wash: '#fdd', font: 'sans', serif: 'serif',
  male: '#060', female: '#900', other: '#850',
};

function node(id: number, x: number, y: number, sex: 'male' | 'female'): ChartNode {
  return {
    id, x, y, w: 176, h: 56, gen: 0, ahnen: null, dupOf: null, moreUp: false, moreDown: false, label: `n${id}`,
    nav: { parent: null, child: null, prev: null, next: null },
    person: { id: `p${id}`, name: `Person ${id}`, lifespan: '', sex, living: false, private: false },
  };
}

// Family 2 + 3 has children 4 and 5; every child line starts at the first partner (node 2).
const edge = (from: number, to: number, kind: ChartEdge['kind'], y: number): ChartEdge => ({ from, to, kind, pts: [from * 10, y, to * 10, y + 40] });
const win: ChartWindow = {
  nodes: [node(2, 0, 0, 'male'), node(3, 200, 0, 'female'), node(4, 0, 200, 'male'), node(5, 200, 200, 'female'), node(6, 600, 200, 'male')],
  edges: [edge(2, 3, 'partner', 1), edge(2, 4, 'parent', 2), edge(2, 5, 'parent', 3), edge(6, 6, 'parent', 4)],
};

describe('edgeOnLine', () => {
  const lit = (ids: number[]) => (id: number) => ids.includes(id);

  it('keeps_a_child_line_when_the_line_runs_through_the_second_partner', () => {
    // Selected: the mother (3). The line to her children starts at the father (2), who is not on the line.
    const l = lit([3, 4, 5]);
    expect(edgeOnLine(edge(2, 4, 'parent', 0), l)).toBe(true);
    expect(edgeOnLine(edge(2, 5, 'parent', 0), l)).toBe(true);
  });

  it('keeps_every_line_through_a_daughter', () => {
    // Selected: a grandparent (1) down through daughter 3 (second partner of 2) to grandchild 5.
    const l = lit([1, 3, 5]);
    expect(edgeOnLine(edge(0, 3, 'parent', 0), l)).toBe(true);
    expect(edgeOnLine(edge(2, 5, 'parent', 0), l)).toBe(true);
    expect(edgeOnLine(edge(2, 4, 'parent', 0), l)).toBe(false);
  });

  it('needs_both_people_for_a_partner_line', () => {
    expect(edgeOnLine(edge(2, 3, 'partner', 0), lit([2, 3]))).toBe(true);
    expect(edgeOnLine(edge(2, 3, 'partner', 0), lit([3]))).toBe(false);
  });
});

describe('drawWhole highlight', () => {
  it('draws_the_selected_mothers_child_lines_at_full_strength', () => {
    const ctx = makeFakeContext();
    drawWhole(ctx as unknown as CanvasRenderingContext2D, win, [], { x: 0, y: 0, zoom: 1 }, { w: 800, h: 600, dpr: 1 }, {
      selected: 3, home: null, lineage: new Set([3, 4, 5]), theme,
    });
    const calls = ctx.calls;
    const faded = calls.findIndex((c) => c.op === 'set:globalAlpha' && c.args[0] === 0.25);
    const restored = calls.findIndex((c, i) => i > faded && c.op === 'set:globalAlpha' && c.args[0] === 1);
    const dimmed = calls.slice(faded, restored).filter((c) => c.op === 'moveTo').map((c) => c.args[1]);
    // Only the unrelated edge (6 -> 6) and the partner line to the unlit father are dim; the child lines from node 2 are not.
    expect(dimmed).toEqual(expect.arrayContaining([1, 4]));
    expect(dimmed).not.toContain(2);
    expect(dimmed).not.toContain(3);
    const hot = calls.slice(restored).filter((c) => c.op === 'set:strokeStyle' && c.args[0] === theme.ribbon);
    expect(hot.length).toBeGreaterThan(0);
  });
});

describe('drawWhole kinship', () => {
  it('writes_how_each_card_is_related_to_the_reference_person', () => {
    const kin: ChartNode['kinship'] = { kind: 'blood', up: 1, down: 1, half: false, married: false, sex: 'female', via: 'unknown' };
    const cards: ChartWindow = { nodes: [{ ...node(2, 0, 0, 'male'), kinship: kin }, node(3, 200, 0, 'female')], edges: [] };
    const ctx = makeFakeContext();
    drawWhole(ctx as unknown as CanvasRenderingContext2D, cards, [], { x: 0, y: 0, zoom: 1 }, { w: 800, h: 600, dpr: 1 }, {
      selected: null, home: null, lineage: null, theme,
    });
    const texts = ctx.calls.filter((c) => c.op === 'fillText').map((c) => c.args[0]);
    expect(texts).toContain('sister');
    expect(texts.filter((x) => x === 'sister')).toHaveLength(1);
  });
});

describe('block headings', () => {
  it('keep_a_fixed_screen_size_when_zoomed_out', () => {
    expect(headingSize(0.1) * 0.1).toBeCloseTo(14, 5);
    expect(headingSize(0.05) * 0.05).toBeCloseTo(14, 5);
  });

  it('are_drawn_on_a_plate_at_low_zoom_and_kept_on_screen', () => {
    const ctx = makeFakeContext();
    const blocks = [{ title: 'Koval family (9 people)', people: 9, unconnected: false, rect: { x: 0, y: 1000, w: 4000, h: 28 } }];
    // The view starts well inside the block, so the label slides right to stay visible.
    drawWhole(ctx as unknown as CanvasRenderingContext2D, { nodes: [], edges: [] }, blocks, { x: 1500, y: 800, zoom: 0.1 }, { w: 800, h: 600, dpr: 1 }, {
      selected: null, home: null, lineage: null, theme,
    });
    const text = ctx.calls.find((c) => c.op === 'fillText' && String(c.args[0]).startsWith('Koval'));
    expect(text).toBeTruthy();
    expect(text!.args[1] as number).toBeGreaterThanOrEqual(1500);
    expect(ctx.calls.some((c) => c.op === 'fillRect')).toBe(true);
  });
});

