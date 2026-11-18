import { describe, expect, it } from 'vitest';
import { A3_MAX_SCALE, A3_MIN_PT, MM_PX, bestDirection, fillRatio, layoutA3, mmToPx, printDocument, printedPt, pxToMm } from './a3';
import { renderSvg } from './pictureExport';
import type { PicturePlan } from './picture';

describe('A3 layout', () => {
  it('converts_between_mm_and_px', () => {
    expect(mmToPx(25.4)).toBeCloseTo(96, 9);
    expect(pxToMm(mmToPx(420))).toBeCloseTo(420, 9);
    expect(MM_PX).toBeCloseTo(3.7795, 3);
  });

  it('picks_landscape_for_a_wide_chart_and_portrait_for_a_tall_one', () => {
    expect(layoutA3({ x: 0, y: 0, w: 4000, h: 1500 }).orientation).toBe('landscape');
    expect(layoutA3({ x: 0, y: 0, w: 1500, h: 4000 }).orientation).toBe('portrait');
    // A tie (a tiny chart is capped at the same scale either way) goes to landscape.
    expect(layoutA3({ x: 0, y: 0, w: 10, h: 10 }).orientation).toBe('landscape');
  });

  it('honours_the_manual_orientation', () => {
    const l = layoutA3({ x: 0, y: 0, w: 4000, h: 1500 }, 'portrait');
    expect(l.orientation).toBe('portrait');
    expect(l.mm).toEqual({ w: 297, h: 420 });
    expect(l.png).toEqual({ w: 3508, h: 4961 });
  });

  it('gives_the_300_dpi_png_size_of_an_a3_page', () => {
    const l = layoutA3({ x: 0, y: 0, w: 3000, h: 1000 });
    expect(l.png).toEqual({ w: 4961, h: 3508 });
    expect(l.pngScale).toBeCloseTo(3.125, 9);
  });

  it('scales_uniformly_to_fill_the_printable_area_and_centres_it', () => {
    const b = { x: 100, y: 50, w: 3000, h: 1000 };
    const l = layoutA3(b);
    expect(l.scale).toBeCloseTo(mmToPx(400) / 3000, 9);
    // The chart's left edge lands on the left margin, its right edge on the right margin.
    expect((b.x - l.cam.x) * l.scale).toBeCloseTo(mmToPx(10), 6);
    expect((b.x + b.w - l.cam.x) * l.scale).toBeCloseTo(mmToPx(410), 6);
    // Vertically centred in the area below the title block.
    const top = (b.y - l.cam.y) * l.scale;
    const bottom = l.px.h - (b.y + b.h - l.cam.y) * l.scale;
    expect(top - l.area.y).toBeCloseTo(bottom - mmToPx(10), 6);
    expect(l.cam.zoom).toBe(l.scale);
  });

  it('never_prints_boxes_larger_than_one_and_a_half_times_the_screen_size', () => {
    const l = layoutA3({ x: 0, y: 0, w: 200, h: 100 });
    expect(l.scale).toBe(A3_MAX_SCALE);
    expect(l.tooSmall).toBe(false);
  });

  it('reports_names_below_six_points', () => {
    expect(printedPt(13, 1)).toBeCloseTo(9.75, 9);
    const big = layoutA3({ x: 0, y: 0, w: 12000, h: 5000 });
    expect(big.namePt).toBeLessThan(A3_MIN_PT);
    expect(big.tooSmall).toBe(true);
    expect(layoutA3({ x: 0, y: 0, w: 1200, h: 800 }).tooSmall).toBe(false);
  });
});

describe('A3 svg', () => {
  it('has_mm_units_and_a_matching_viewbox', () => {
    const lay = layoutA3({ x: 0, y: 0, w: 3000, h: 1000 });
    const plan: PicturePlan = { cam: lay.cam, view: { ...lay.px }, scale: 1, width: lay.px.w, height: lay.px.h, reduced: false, percent: 100 };
    const bytes = renderSvg({
      plan,
      theme: { paper: '#fff', ink2: '#666', font: 'sans-serif' },
      caption: '',
      pageMm: { ...lay.mm, orientation: lay.orientation },
      paint: () => {},
    });
    const svg = new TextDecoder().decode(bytes);
    const root = /<svg [^>]*>/.exec(svg)![0];
    expect(root).toContain('width="420mm"');
    expect(root).toContain('height="297mm"');
    const vb = /viewBox="0 0 ([\d.]+) ([\d.]+)"/.exec(root)!;
    expect(Number(vb[1]) / MM_PX).toBeCloseTo(420, 1);
    expect(Number(vb[2]) / MM_PX).toBeCloseTo(297, 1);
    expect(svg).not.toContain('<text');
  });

  it('wraps_the_svg_in_an_a3_print_page', () => {
    const html = printDocument('<?xml version="1.0"?>\n<svg></svg>', 'portrait');
    expect(html).toContain('@page{size:A3 portrait;margin:0}');
    expect(html).not.toContain('<?xml');
  });
});

describe('A3 direction choice', () => {
  const wide = { x: 0, y: 0, w: 7000, h: 600 };
  const tall = { x: 0, y: 0, w: 1200, h: 3000 };
  const sizes = (a: typeof wide, b: typeof wide) => [
    { direction: 'top' as const, bounds: a },
    { direction: 'bottom' as const, bounds: a },
    { direction: 'left' as const, bounds: b },
    { direction: 'right' as const, bounds: b },
  ];

  it('takes_the_direction_that_prints_largest', () => {
    const wideTall = bestDirection(sizes({ x: 0, y: 0, w: 7000, h: 600 }, { x: 0, y: 0, w: 1000, h: 1300 }), 'top');
    expect(wideTall.direction).toBe('left');
    expect(wideTall.layout.scale).toBeGreaterThan(layoutA3(wide).scale * 3);
    expect(bestDirection(sizes({ x: 0, y: 0, w: 1000, h: 800 }, tall), 'left').direction).toBe('top');
  });

  it('keeps_the_current_direction_on_a_tie', () => {
    expect(bestDirection(sizes(wide, wide), 'bottom').direction).toBe('bottom');
    expect(bestDirection(sizes(wide, wide), 'right').direction).toBe('right');
    // A mirror image (same size) never beats the current side.
    expect(bestDirection(sizes(tall, tall), 'top').direction).toBe('top');
  });

  it('honours_a_manual_orientation_when_choosing', () => {
    const fit = bestDirection(sizes(wide, tall), 'top', 'portrait');
    expect(fit.layout.orientation).toBe('portrait');
  });

  it('measures_how_much_of_the_printable_area_is_filled', () => {
    const l = layoutA3(wide, 'landscape');
    const f = fillRatio(wide, l);
    expect(f.w).toBeCloseTo(1, 6);
    expect(f.h).toBeLessThan(0.2);
    const t = layoutA3(tall, 'portrait');
    expect(fillRatio(tall, t).h).toBeCloseTo(1, 6);
  });
});
