import { describe, expect, it } from 'vitest';
import { DEFAULT_DIRECTIONS, DIRECTIONS, arrowStep, camAtSide, choicesFor, directionOf, edgeCentre, edgeStrip, opposite, rootSide } from './direction';

describe('direction', () => {
  it('opposite_sides_pair_up', () => {
    for (const d of DIRECTIONS) expect(opposite(opposite(d))).toBe(d);
    expect(opposite('top')).toBe('bottom');
    expect(opposite('left')).toBe('right');
  });

  it('defaults_are_what_the_charts_drew_before', () => {
    expect(DEFAULT_DIRECTIONS.pedigree).toBe('right');
    for (const k of ['descendants', 'relatives', 'around', 'whole', 'family'] as const) expect(DEFAULT_DIRECTIONS[k]).toBe('top');
    expect(directionOf(undefined, 'pedigree')).toBe('right');
    expect(directionOf({ pedigree: 'left' }, 'pedigree')).toBe('left');
  });

  it('the_family_view_has_two_choices_every_chart_four', () => {
    expect(choicesFor('family')).toEqual(['top', 'bottom']);
    expect(choicesFor('pedigree')).toHaveLength(4);
  });

  it('a_pedigree_starts_on_the_far_side_from_its_ancestors', () => {
    expect(rootSide('pedigree', 'right')).toBe('left');
    expect(rootSide('pedigree', 'top')).toBe('bottom');
    expect(rootSide('descendants', 'top')).toBe('top');
    expect(rootSide('descendants', 'left')).toBe('left');
  });

  it('an_arrow_goes_the_way_it_points_on_every_side', () => {
    const toward = { top: 'ArrowUp', bottom: 'ArrowDown', left: 'ArrowLeft', right: 'ArrowRight' } as const;
    for (const d of DIRECTIONS) {
      expect(arrowStep(toward[d], d)).toBe('ancestors');
      expect(arrowStep(toward[opposite(d)], d)).toBe('descendants');
    }
    // Across: left and right on a chart that grows up or down, up and down on one that grows sideways.
    expect(arrowStep('ArrowLeft', 'top')).toBe('prev');
    expect(arrowStep('ArrowRight', 'bottom')).toBe('next');
    expect(arrowStep('ArrowUp', 'right')).toBe('prev');
    expect(arrowStep('ArrowDown', 'left')).toBe('next');
    expect(arrowStep('Enter', 'top')).toBeNull();
  });

  it('edge_centres_face_the_asked_side', () => {
    const n = { x: 10, y: 20, w: 100, h: 40 };
    expect(edgeCentre(n, 'top')).toEqual({ x: 60, y: 20 });
    expect(edgeCentre(n, 'bottom')).toEqual({ x: 60, y: 60 });
    expect(edgeCentre(n, 'left')).toEqual({ x: 10, y: 40 });
    expect(edgeCentre(n, 'right')).toEqual({ x: 110, y: 40 });
  });

  it('the_camera_puts_the_card_at_the_asked_edge_of_the_view', () => {
    const r = { x: 500, y: 300, w: 176, h: 66 };
    const view = { w: 800, h: 600 };
    const top = camAtSide(r, 'top', 1, view, 40);
    expect(r.y - top.y).toBe(40);
    expect(top.x + view.w / 2).toBe(r.x + r.w / 2);
    const bottom = camAtSide(r, 'bottom', 1, view, 40);
    expect(bottom.y + view.h - (r.y + r.h)).toBe(40);
    const left = camAtSide(r, 'left', 0.5, view, 40);
    expect(r.x - left.x).toBe(40);
    expect(left.y + view.h / 0.5 / 2).toBe(r.y + r.h / 2);
    const right = camAtSide(r, 'right', 1, view, 40);
    expect(right.x + view.w - (r.x + r.w)).toBe(40);
  });

  it('the_probe_strip_hugs_the_asked_edge_of_the_bounds', () => {
    const b = { w: 1000, h: 800 };
    expect(edgeStrip(b, 'left').x).toBeLessThan(0);
    expect(edgeStrip(b, 'right').x + edgeStrip(b, 'right').w).toBeGreaterThanOrEqual(b.w);
    expect(edgeStrip(b, 'top').y).toBeLessThan(0);
    expect(edgeStrip(b, 'bottom').y + edgeStrip(b, 'bottom').h).toBeGreaterThanOrEqual(b.h);
    expect(edgeStrip(b, 'top').w).toBe(b.w);
    expect(edgeStrip(b, 'left').h).toBe(b.h);
  });
});
