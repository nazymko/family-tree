import { describe, expect, it } from 'vitest';
import { handleSpots, hitHandle, hitTest, neighbor } from './hit';
import { synthChart } from './synth';

describe('hit', () => {
  const s = synthChart(15, 7);
  const win = s.query({ x: -10, y: -10, w: 1e6, h: 1e6 });

  it('hit_node_and_miss', () => {
    const n = win.nodes[3];
    expect(hitTest(win, n.x + 5, n.y + 5)?.id).toBe(n.id);
    expect(hitTest(win, n.x + n.w + 10, n.y + n.h + 10)).toBeNull();
  });

  it('handle_ignored_at_dots_lod_and_grows_when_zoomed_out', () => {
    const n = { ...win.nodes[0], moreUp: true };
    const py = n.y + n.h / 2;
    expect(hitHandle({ nodes: [n], edges: [] }, 'pedigree', n.x + n.w + 4, py, 0.2)).toBeNull();
    expect(hitHandle({ nodes: [n], edges: [] }, 'pedigree', n.x + n.w + 12, py, 0.5)?.id).toBe(n.id);
  });

  it('neighbor_follows_nav_links', () => {
    expect(neighbor(win, 0, 'child')).toBe(1);
    expect(neighbor(win, 1, 'parent')).toBe(0);
    expect(neighbor(win, 1, 'down')).toBe(2);
    expect(neighbor(win, 2, 'up')).toBe(1);
    expect(neighbor(win, 0, 'parent')).toBeNull();
    expect(neighbor(win, 99, 'child')).toBeNull();
  });

  it('neighbor_left_right_are_geometric', () => {
    expect(neighbor(win, 1, 'right')).toBe(3);
    expect(neighbor(win, 1, 'left')).toBe(0);
  });

  it('handle_hit', () => {
    const n = { ...win.nodes[0], moreUp: true };
    expect(hitHandle({ nodes: [n], edges: [] }, 'pedigree', n.x + n.w + 2, n.y + n.h / 2)?.id).toBe(n.id);
    expect(hitHandle({ nodes: [n], edges: [] }, 'pedigree', n.x + 5, n.y + 5)).toBeNull();
  });

  it('handles_sit_on_the_opposite_edge_when_reversed', () => {
    const n = { ...win.nodes[0], moreUp: true, moreDown: true };
    expect(handleSpots(n, 'pedigree', true)).toEqual([{ x: n.x, y: n.y + n.h / 2 }]);
    expect(handleSpots(n, 'descendants', true)).toEqual([{ x: n.x + n.w / 2, y: n.y }]);
    expect(handleSpots(n, 'relatives', true)).toEqual([
      { x: n.x + n.w / 2, y: n.y + n.h },
      { x: n.x + n.w / 2, y: n.y },
    ]);
    expect(hitHandle({ nodes: [n], edges: [] }, 'pedigree', n.x - 2, n.y + n.h / 2, 1, true)?.id).toBe(n.id);
    expect(hitHandle({ nodes: [n], edges: [] }, 'pedigree', n.x + n.w + 2, n.y + n.h / 2, 1, true)).toBeNull();
  });
});
