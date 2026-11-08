import { describe, expect, it } from 'vitest';
import { makeFakeContext } from '../../test-setup';
import type { ChartEdge } from '../api/types';
import { connectorMarks, segKey } from './junctions';
import { draw, type Theme } from './renderer';

const parent = (pts: number[], from = 0, to = 1): ChartEdge => ({ from, to, kind: 'parent', pts });

/** A parent at x=100 with two children at x=40 and x=160 on a bus at y=60. */
const family = (): ChartEdge[] => [parent([100, 0, 100, 60, 40, 60, 40, 100], 0, 1), parent([100, 0, 100, 60, 160, 60, 160, 100], 0, 2)];

describe('connector marks', () => {
  it('puts_a_dot_where_the_trunk_meets_the_bus_and_none_at_the_corners', () => {
    const { dots } = connectorMarks(family());
    expect(dots).toEqual([100, 60]);
  });

  it('puts_a_dot_where_a_middle_child_drops_off_the_bus', () => {
    const edges = [...family(), parent([100, 0, 100, 60, 130, 60, 130, 100], 0, 3)];
    const pairs = connectorMarks(edges).dots;
    const found: string[] = [];
    for (let i = 0; i < pairs.length; i += 2) found.push(`${pairs[i]},${pairs[i + 1]}`);
    expect(found.sort()).toEqual(['100,60', '130,60']);
  });

  it('puts_a_dot_where_the_childrens_line_leaves_the_partner_connector', () => {
    const edges: ChartEdge[] = [
      { from: 0, to: 1, kind: 'partner', pts: [80, 30, 104, 30] },
      parent([92, 30, 92, 60, 40, 60, 40, 100], 0, 2),
      parent([92, 30, 92, 60, 160, 60, 160, 100], 0, 3),
    ];
    const pairs = connectorMarks(edges).dots;
    const found: string[] = [];
    for (let i = 0; i < pairs.length; i += 2) found.push(`${pairs[i]},${pairs[i + 1]}`);
    expect(found.sort()).toEqual(['92,30', '92,60']);
  });

  it('a_single_child_straight_below_has_no_dot', () => {
    expect(connectorMarks([parent([100, 0, 100, 100])]).dots).toEqual([]);
  });

  it('a_crossing_of_two_families_is_a_gap_in_the_horizontal_line_and_not_a_dot', () => {
    const edges = [...family(), parent([70, 20, 70, 100], 5, 6)];
    const m = connectorMarks(edges);
    expect(m.dots).toEqual([100, 60]);
    expect(m.crossings.get(segKey(40, 100, 60))).toEqual([70]);
  });

  it('draws_the_dots_and_leaves_the_gap_in_the_picture_too', () => {
    const theme: Theme = { paper: '#fff', rule: '#ccc', ink: '#000', ink2: '#666', ribbon: '#f00', wash: '#fee', font: 'sans-serif', serif: 'serif' };
    const ctx = makeFakeContext();
    const edges = [...family(), parent([70, 20, 70, 100], 5, 6)];
    draw(ctx, { nodes: [], edges }, { x: 0, y: 0, zoom: 1 }, { w: 800, h: 600, dpr: 1 }, { selected: null, focus: null, theme });
    expect(ctx.calls.filter((c) => c.op === 'arc').length).toBe(1);
    // The bus is cut around x=70: a line stops short of it and the next one starts after it.
    const lines = ctx.calls.filter((c) => c.op === 'lineTo').map((c) => c.args[0]);
    expect(lines.some((x) => x !== 70 && Math.abs(Number(x) - 70) < 5)).toBe(true);
  });
});
