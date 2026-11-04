import { describe, expect, it } from 'vitest';
import type { ChartEdge, ChartNode } from '../api/types';
import {
  CAPTION_H,
  MARGIN,
  branchIds,
  compactX,
  formatOf,
  isKeyboardMenu,
  scalePlan,
  fitScale,
  mergeScenes,
  pictureFileName,
  planPicture,
  sceneBounds,
  sceneFor,
  tileRects,
  type Scene,
} from './picture';

const nav = { parent: null, child: null, prev: null, next: null };

function node(id: number, x: number, y: number, name = `Person ${id}`): ChartNode {
  return { id, person: null, x, y, w: 100, h: 50, gen: 0, ahnen: null, dupOf: null, moreUp: false, moreDown: false, nav, label: name };
}

function edge(from: number, to: number, kind: ChartEdge['kind'] = 'parent'): ChartEdge {
  return { from, to, kind, pts: [from * 10, 0, to * 10, 100] };
}

/**
 * Ivan (1) and Olena (2) are partners; their children are Petro (3) and Maria (4); Petro and Halyna (5) are partners
 * with a child Taras (6). Ivan's father is Stepan (7), Stepan's wife is Hanna (8). Maria's husband is Yuriy (9).
 * Parent edges leave the parent, as in the relatives and descendants charts.
 */
function family(): Scene {
  return {
    nodes: [1, 2, 3, 4, 5, 6, 7, 8, 9].map((i) => node(i, i * 120, i * 5)),
    edges: [
      edge(1, 2, 'partner'),
      edge(7, 8, 'partner'),
      edge(3, 5, 'partner'),
      edge(4, 9, 'partner'),
      edge(1, 3),
      edge(1, 4),
      edge(3, 6),
      edge(7, 1),
    ],
  };
}

const sorted = (s: Set<number>) => [...s].sort((a, b) => a - b);

describe('branchIds', () => {
  it('descendants_keep_the_selected_person_their_descendants_and_the_partners_of_all_of_them', () => {
    // The couple Ivan and Olena, their children, and the partners those children have.
    expect(sorted(branchIds('descendants', family(), 1))).toEqual([1, 2, 3, 4, 5, 6, 9]);
    expect(sorted(branchIds('descendants', family(), 3))).toEqual([3, 5, 6]);
  });

  it('descendants_selected_partner_brings_the_couple_and_their_children', () => {
    // Olena is not the one the child edges leave from, but her children are still hers.
    expect(sorted(branchIds('descendants', family(), 2))).toEqual([1, 2, 3, 4, 5, 6, 9]);
  });

  it('pedigree_keeps_the_selected_person_and_their_ancestors_only', () => {
    // In a pedigree the edges run from the person to the ancestors.
    const s: Scene = {
      nodes: [1, 2, 3, 4, 5, 6, 7].map((i) => node(i, i * 120, 0)),
      edges: [edge(1, 2), edge(1, 3), edge(2, 4), edge(2, 5), edge(3, 6), edge(3, 7)],
    };
    expect(sorted(branchIds('pedigree', s, 2))).toEqual([2, 4, 5]);
    expect(sorted(branchIds('pedigree', s, 1))).toEqual([1, 2, 3, 4, 5, 6, 7]);
    expect(sorted(branchIds('pedigree', s, 7))).toEqual([7]);
  });

  it('relatives_keep_ancestors_descendants_and_partners_but_not_siblings', () => {
    // Petro (3): his father Ivan (1) and Ivan's father Stepan (7), his partner Halyna (5), his child Taras (6).
    // Maria (4) and her husband (9) stay out; the partners of Ivan and Stepan come along.
    expect(sorted(branchIds('relatives', family(), 3))).toEqual([1, 2, 3, 5, 6, 7, 8]);
  });

  it('whole_uses_the_lineage_from_the_backend_plus_partners', () => {
    expect(sorted(branchIds('whole', family(), 3, [3, 1, 6, 7]))).toEqual([1, 2, 3, 5, 6, 7, 8]);
  });

  it('a_selected_person_alone_is_a_branch_of_one', () => {
    const s: Scene = { nodes: [node(1, 0, 0)], edges: [] };
    expect(sorted(branchIds('descendants', s, 1))).toEqual([1]);
  });
});

describe('sceneFor', () => {
  it('whole_keeps_everything', () => {
    const s = family();
    expect(sceneFor('whole', 'descendants', s, null)).toEqual(s);
  });

  it('branch_keeps_only_edges_between_branch_people', () => {
    const out = sceneFor('branch', 'descendants', family(), 3);
    expect(out.nodes.map((n) => n.id)).toEqual([3, 5, 6]);
    expect(out.edges.map((e) => `${e.from}>${e.to}`).sort()).toEqual(['3>5', '3>6']);
  });

  it('branch_without_a_selection_is_empty', () => {
    const out = sceneFor('branch', 'pedigree', family(), null);
    expect(out.nodes).toEqual([]);
  });
});

describe('sceneBounds', () => {
  it('is_the_box_around_nodes_and_edge_points', () => {
    const b = sceneBounds({ nodes: [node(1, 10, 20), node(2, 300, 90)], edges: [{ from: 1, to: 2, kind: 'parent', pts: [0, 5, 400, 5] }] });
    expect(b).toEqual({ x: 0, y: 5, w: 400, h: 135 });
  });

  it('includes_extra_rectangles_such_as_block_headings', () => {
    const b = sceneBounds({ nodes: [node(1, 100, 100)], edges: [] }, [{ x: 80, y: 60, w: 50, h: 10 }]);
    expect(b).toEqual({ x: 80, y: 60, w: 120, h: 90 });
  });
});

describe('fitScale', () => {
  it('leaves_a_normal_picture_alone', () => {
    expect(fitScale(3000, 2000)).toEqual({ scale: 1, w: 3000, h: 2000, reduced: false, percent: 100 });
  });

  it('keeps_each_side_within_16384', () => {
    const f = fitScale(40960, 1000);
    expect(f.w).toBeLessThanOrEqual(16384);
    expect(f.h).toBeLessThanOrEqual(16384);
    expect(f.reduced).toBe(true);
    expect(f.percent).toBe(40);
  });

  it('keeps_the_area_within_200_megapixels', () => {
    const f = fitScale(16000, 16000);
    expect(f.w * f.h).toBeLessThanOrEqual(200_000_000);
    expect(f.reduced).toBe(true);
    expect(f.percent).toBeLessThan(100);
  });

  it('an_allowed_size_is_not_reduced', () => {
    expect(fitScale(16384, 12000).reduced).toBe(false);
  });

  it('never_makes_a_picture_larger_or_empty', () => {
    expect(fitScale(10, 10).scale).toBe(1);
    const f = fitScale(1e9, 1e9);
    expect(f.w).toBeGreaterThanOrEqual(1);
    expect(f.h).toBeGreaterThanOrEqual(1);
  });
});

describe('tileRects', () => {
  it('covers_the_bounds', () => {
    const tiles = tileRects({ x: -1, y: -1, w: 9002, h: 4002 }, 4000);
    expect(tiles).toHaveLength(3 * 2);
    expect(Math.min(...tiles.map((t) => t.x))).toBe(-1);
    expect(Math.max(...tiles.map((t) => t.x + t.w))).toBe(9001);
    expect(Math.max(...tiles.map((t) => t.y + t.h))).toBe(4001);
  });

  it('a_small_chart_is_one_tile', () => {
    expect(tileRects({ x: 0, y: 0, w: 500, h: 400 }, 4000)).toEqual([{ x: 0, y: 0, w: 500, h: 400 }]);
  });
});

describe('mergeScenes', () => {
  it('drops_nodes_and_edges_seen_in_two_tiles', () => {
    const a: Scene = { nodes: [node(1, 0, 0), node(2, 5000, 0)], edges: [edge(1, 2)] };
    const b: Scene = { nodes: [node(2, 5000, 0), node(3, 6000, 0)], edges: [edge(1, 2), edge(2, 3)] };
    const m = mergeScenes([a, b]);
    expect(m.nodes.map((n) => n.id)).toEqual([1, 2, 3]);
    expect(m.edges).toHaveLength(2);
  });
});

describe('planPicture', () => {
  const scene: Scene = { nodes: [node(1, 1000, 500), node(2, 1300, 700)], edges: [] };

  it('whole_and_branch_are_drawn_at_zoom_1_with_a_40_px_margin_and_a_caption_strip_at_twice_the_pixels', () => {
    for (const scope of ['whole', 'branch'] as const) {
      const p = planPicture(scope, scene, null);
      expect(p.cam).toEqual({ x: 1000 - MARGIN, y: 500 - MARGIN, zoom: 1 });
      expect(p.view.w).toBe(400 + 2 * MARGIN);
      expect(p.view.h).toBe(250 + 2 * MARGIN);
      expect(p.scale).toBe(2);
      expect(p.width).toBe(p.view.w * 2);
      expect(p.height).toBe((p.view.h + CAPTION_H) * 2);
      expect(p.reduced).toBe(false);
    }
  });

  it('uses_the_screen_pixel_ratio_when_it_is_above_2', () => {
    expect(planPicture('whole', scene, null, [], { dpr: 3 }).scale).toBe(3);
  });

  it('falls_back_to_1x_before_scaling_below_1x', () => {
    // 9000 px wide at 1x is fine, at 2x it is over 16384.
    const wide: Scene = { nodes: [node(1, 0, 0), node(2, 8900, 0)], edges: [] };
    const p = planPicture('whole', wide, null);
    expect(p.scale).toBe(1);
    expect(p.reduced).toBe(false);
    const huge: Scene = { nodes: [node(1, 0, 0), node(2, 60000, 0)], edges: [] };
    const q = planPicture('whole', huge, null);
    expect(q.scale).toBeLessThan(1);
    expect(q.reduced).toBe(true);
  });

  it('a_vector_picture_is_never_scaled', () => {
    const huge: Scene = { nodes: [node(1, 0, 0), node(2, 60000, 0)], edges: [] };
    const p = planPicture('whole', huge, null, [], { vector: true });
    expect(p.scale).toBe(1);
    expect(p.reduced).toBe(false);
    expect(p.width).toBe(p.view.w);
    expect(p.height).toBe(p.view.h + CAPTION_H);
  });

  it('scalePlan_halves_the_size_for_a_retry', () => {
    const p = scalePlan(planPicture('whole', scene, null), 0.5);
    expect(p.scale).toBe(1);
    expect(p.reduced).toBe(false);
    const q = scalePlan(p, 0.5);
    expect(q.scale).toBe(0.5);
    expect(q.reduced).toBe(true);
    expect(q.percent).toBe(50);
  });

  it('screen_is_exactly_the_viewport_at_the_current_zoom', () => {
    const p = planPicture('screen', scene, { cam: { x: 70, y: 80, zoom: 0.5 }, size: { w: 800, h: 600 }, dpr: 2 });
    expect(p.cam).toEqual({ x: 70, y: 80, zoom: 0.5 });
    expect(p.view).toEqual({ w: 800, h: 600 });
    expect(p.scale).toBe(2);
    expect(p.width).toBe(1600);
    expect(p.height).toBe(1200 + CAPTION_H * 2);
  });

  it('a_huge_chart_is_scaled_down_and_says_so', () => {
    const wide: Scene = { nodes: [node(1, 0, 0), node(2, 60000, 0)], edges: [] };
    const p = planPicture('whole', wide, null);
    expect(p.reduced).toBe(true);
    expect(p.width).toBeLessThanOrEqual(16384);
    expect(p.height).toBeLessThanOrEqual(16384);
    expect(p.percent).toBe(Math.round(p.scale * 100));
  });
});

describe('compactX', () => {
  it('shrinks_empty_gaps_to_one_gap_and_keeps_rows_aligned_and_edges_connected', () => {
    const a = node(1, 0, 0);
    const b = node(2, 5000, 100);
    const c = node(3, 9000, 0);
    const s: Scene = {
      nodes: [a, b, c],
      edges: [
        { from: 1, to: 2, kind: 'parent', pts: [50, 50, 50, 75, 5050, 75, 5050, 100] },
        { from: 1, to: 3, kind: 'parent', pts: [50, 50, 50, 60, 9050, 60, 9050, 0] },
      ],
    };
    const out = compactX(s, 40);
    // Boxes are 100 wide: 0..100, then a 40 gap, 140..240, then a 40 gap.
    expect(out.nodes.map((n) => n.x)).toEqual([0, 140, 280]);
    expect(out.nodes.map((n) => n.w)).toEqual([100, 100, 100]);
    expect(out.nodes.map((n) => n.y)).toEqual([0, 100, 0]);
    expect(out.edges[0].pts).toEqual([50, 50, 50, 75, 190, 75, 190, 100]);
    expect(out.edges[1].pts).toEqual([50, 50, 50, 60, 330, 60, 330, 0]);
    // The input is untouched.
    expect(s.nodes[1].x).toBe(5000);
  });

  it('leaves_gaps_that_are_already_small_and_keeps_the_order', () => {
    const s: Scene = { nodes: [node(1, 0, 0), node(2, 120, 0), node(3, 300, 0)], edges: [] };
    expect(compactX(s, 40).nodes.map((n) => n.x)).toEqual([0, 120, 260]);
    expect(compactX({ nodes: [], edges: [] }).nodes).toEqual([]);
  });

  it('boxes_in_different_rows_that_overlap_in_x_close_no_gap_between_them', () => {
    const s: Scene = { nodes: [node(1, 0, 0), node(2, 50, 100), node(3, 2000, 0)], edges: [] };
    expect(compactX(s, 40).nodes.map((n) => n.x)).toEqual([0, 50, 190]);
  });
});

describe('small helpers', () => {
  it('formatOf_reads_the_ending_the_user_chose', () => {
    expect(formatOf('C:/x/Whole tree.svg')).toBe('svg');
    expect(formatOf('C:/x/Whole tree.SVG')).toBe('svg');
    expect(formatOf('C:/x/Whole tree.png')).toBe('png');
    expect(formatOf('C:/x/Whole tree')).toBe('png');
  });

  it('isKeyboardMenu_tells_a_key_press_from_a_click', () => {
    expect(isKeyboardMenu({ pointerType: '', button: 0, detail: 0, clientX: 5, clientY: 5 })).toBe(true);
    expect(isKeyboardMenu({ pointerType: undefined, button: 0, detail: 0, clientX: 0, clientY: 0 })).toBe(true);
    expect(isKeyboardMenu({ pointerType: 'mouse', button: 2, detail: 0, clientX: 40, clientY: 50 })).toBe(false);
    expect(isKeyboardMenu({ pointerType: undefined, button: 2, detail: 0, clientX: 0, clientY: 0 })).toBe(false);
  });
});

describe('pictureFileName', () => {
  it('removes_characters_a_file_name_cannot_hold', () => {
    expect(pictureFileName('Pedigree - Ivan "Vanya" Koval / Jr.')).toBe('Pedigree - Ivan Vanya Koval Jr..png');
    expect(pictureFileName('  Whole tree ')).toBe('Whole tree.png');
    expect(pictureFileName('???')).toBe('Chart.png');
  });
});
