import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  A3_MAX_SCALE,
  A3_MIN_PT,
  A3_NAME_FLOOR_PT,
  MM_PX,
  NAME_FLOOR_PX,
  NAME_PX,
  bestDirection,
  ellipsize,
  fillRatio,
  fitPrintName,
  layoutA3,
  nameBaselines,
  nameFloorPx,
  printDirection,
  printHtml,
  mmToPx,
  printDocument,
  printedPt,
  pxToMm,
  type NameLine,
} from './a3';
import type { ChartNode } from '../api/types';
import { draw } from './renderer';
import { SvgContext, parseFont } from './svgContext';
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

  it('as_on_screen_keeps_the_screen_direction_even_when_another_prints_larger', () => {
    const all = sizes({ x: 0, y: 0, w: 7000, h: 600 }, { x: 0, y: 0, w: 1000, h: 1300 });
    // Best fit would turn it to the left; the screen's top stays.
    expect(bestDirection(all, 'top').direction).toBe('left');
    const fit = printDirection(all, 'top', 'screen');
    expect(fit.direction).toBe('top');
    expect(fit.layout).toEqual(layoutA3({ x: 0, y: 0, w: 7000, h: 600 }));
    // Only the screen's own layout is needed.
    expect(printDirection([{ direction: 'right', bounds: tall }], 'right', 'screen').direction).toBe('right');
    expect(() => printDirection([{ direction: 'top', bounds: tall }], 'left', 'screen')).toThrow();
  });

  it('best_fit_still_picks_the_largest', () => {
    const all = sizes({ x: 0, y: 0, w: 7000, h: 600 }, { x: 0, y: 0, w: 1000, h: 1300 });
    const fit = printDirection(all, 'top', 'best');
    expect(fit).toEqual(bestDirection(all, 'top'));
    expect(fit.layout.scale).toBeGreaterThan(printDirection(all, 'top', 'screen').layout.scale * 3);
  });

  it('as_on_screen_turns_the_page_for_that_direction_when_the_orientation_is_auto', () => {
    // The same chart: wide when its ancestors are on top, tall when they are on the left.
    const all = sizes(wide, tall);
    expect(printDirection(all, 'top', 'screen').layout.orientation).toBe('landscape');
    expect(printDirection(all, 'left', 'screen').layout.orientation).toBe('portrait');
    // A chosen orientation is kept.
    expect(printDirection(all, 'left', 'screen', 'landscape').layout.orientation).toBe('landscape');
    expect(printDirection(all, 'top', 'screen', 'portrait').layout.orientation).toBe('portrait');
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

describe('A3 page text and print frame', () => {
  it('ellipsizes_a_long_title_to_the_room_it_has', () => {
    const w = (x: string) => x.length * 10;
    expect(ellipsize(w, 'Short', 100)).toBe('Short');
    const cut = ellipsize(w, 'A very long chart name indeed', 100);
    expect(cut.endsWith('\u2026')).toBe(true);
    expect(w(cut)).toBeLessThanOrEqual(100);
    expect(ellipsize(w, 'Anything', 5)).toBe('');
  });

  it('sets_the_page_size_and_forbids_a_second_page', () => {
    const html = printDocument('<svg></svg>', 'landscape');
    expect(html).toContain('width:420mm;height:297mm;overflow:hidden');
    expect(html).toContain('page-break-after:avoid');
    expect(html).toContain('display:block');
  });
});

describe('printHtml', () => {
  function fakeFrame() {
    const listeners: Record<string, () => void> = {};
    const win = { addEventListener: (n: string, f: () => void) => (listeners[n] = f), focus: vi.fn(), print: vi.fn() };
    const frame = { setAttribute: vi.fn(), style: { cssText: '' }, remove: vi.fn(), contentWindow: win, onload: null as null | (() => void), srcdoc: '' };
    vi.spyOn(document, 'createElement').mockReturnValue(frame as unknown as HTMLElement);
    vi.spyOn(document.body, 'appendChild').mockReturnValue(frame as unknown as HTMLElement);
    return { frame, win, listeners };
  }
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('keeps_the_frame_while_the_dialog_is_open_and_removes_it_after_afterprint', async () => {
    vi.useFakeTimers();
    const { frame, win, listeners } = fakeFrame();
    const done = printHtml('<p/>');
    frame.onload!();
    await done;
    expect(win.print).toHaveBeenCalled();
    vi.advanceTimersByTime(30_000);
    expect(frame.remove).not.toHaveBeenCalled();
    listeners.afterprint();
    expect(frame.remove).toHaveBeenCalledTimes(1);
  });

  it('removes_the_frame_after_the_long_fallback_when_afterprint_never_comes', async () => {
    vi.useFakeTimers();
    const { frame } = fakeFrame();
    const done = printHtml('<p/>', { cleanupMs: 60_000 });
    frame.onload!();
    await done;
    vi.advanceTimersByTime(59_000);
    expect(frame.remove).not.toHaveBeenCalled();
    vi.advanceTimersByTime(2_000);
    expect(frame.remove).toHaveBeenCalled();
  });

  it('rejects_and_removes_the_frame_when_it_never_loads', async () => {
    vi.useFakeTimers();
    const { frame } = fakeFrame();
    const done = printHtml('<p/>', { loadMs: 5_000 });
    const caught = done.catch((e: Error) => e.message);
    vi.advanceTimersByTime(5_001);
    expect(await caught).toMatch(/did not load/);
    expect(frame.remove).toHaveBeenCalled();
  });
});

describe('A3 card names fit without cutting', () => {
  // A monospace stand-in for the print font: each letter is 0.6 of the size wide.
  const m = (s: string, px: number) => s.length * px * 0.6;
  const W = 104;
  const fits = (lines: NameLine[]) => lines.every((l) => m(l.text, l.px) <= W + 1e-9);
  const noEllipsis = (lines: NameLine[]) => lines.every((l) => !l.text.includes('…'));
  const theme = { paper: '#fff', rule: '#ccc', ink: '#000', ink2: '#666', ribbon: '#f00', wash: '#fee', font: 'sans-serif', serif: 'serif' };
  const svgMeasure = (s: string, font: string) => s.length * parseFont(font).size * 0.6;
  const card = (name: string, w: number, h: number): ChartNode => ({
    id: 0,
    person: { id: 'p0', name, lifespan: '1850-1900', sex: 'male', living: false, private: false, siblingMarker: false },
    x: 0,
    y: 0,
    w,
    h,
    gen: 0,
    ahnen: null,
    dupOf: null,
    moreUp: false,
    moreDown: false,
    nav: { parent: null, child: null, prev: null, next: null },
    label: '',
  });
  const texts = (svg: string) => [...svg.matchAll(/<text [^>]*font-size="([\d.]+)"[^>]*>([^<]*)<\/text>/g)].map((x) => ({ px: Number(x[1]), text: x[2] }));

  it('keeps_a_short_name_on_one_line_at_the_normal_size', () => {
    expect(fitPrintName(m, 'Anna Novak', W)).toEqual([{ text: 'Anna Novak', px: NAME_PX }]);
  });

  it('shrinks_only_the_long_surname_and_keeps_the_given_name_size', () => {
    const lines = fitPrintName(m, 'Anna Dennelwenhoffmann', W, NAME_PX, 8);
    expect(lines.map((l) => l.text)).toEqual(['Anna', 'Dennelwenhoffmann']);
    expect(lines[0].px).toBe(NAME_PX);
    expect(lines[1].px).toBeLessThan(NAME_PX);
    expect(lines[1].px).toBeGreaterThanOrEqual(8);
    // Just small enough: a quarter px bigger would not fit.
    expect(m(lines[1].text, lines[1].px + 0.25)).toBeGreaterThan(W);
    expect(fits(lines)).toBe(true);
  });

  it('shrinks_only_a_long_given_name', () => {
    const lines = fitPrintName(m, 'Bartholomaeuswilhelm Novak', W, NAME_PX, 8);
    expect(lines.map((l) => l.text)).toEqual(['Bartholomaeuswilhelm', 'Novak']);
    expect(lines[0].px).toBeLessThan(NAME_PX);
    expect(lines[1].px).toBe(NAME_PX);
    expect(fits(lines)).toBe(true);
  });

  it('wraps_an_extreme_surname_at_the_floor_instead_of_cutting_it', () => {
    const surname = 'Dennelwenhoffmannsdottirovich';
    const lines = fitPrintName(m, `Anna ${surname}`, W, NAME_PX, 8);
    expect(lines[0]).toEqual({ text: 'Anna', px: NAME_PX });
    expect(lines.length).toBe(3);
    expect(lines.slice(1).every((l) => l.px === 8)).toBe(true);
    expect(lines.slice(1).map((l) => l.text).join('')).toBe(surname);
    expect(fits(lines)).toBe(true);
    expect(noEllipsis(lines)).toBe(true);
  });

  it('lets_the_lines_under_the_first_use_the_room_beside_the_ancestor_number', () => {
    const narrow = fitPrintName(m, 'Anna Oberstrassenkehrer', 82, NAME_PX, 8, 3);
    const wide = fitPrintName(m, 'Anna Oberstrassenkehrer', 82, NAME_PX, 8, 3, 104);
    expect(wide.map((l) => l.text)).toEqual(['Anna', 'Oberstrassenkehrer']);
    expect(wide[1].px).toBeGreaterThan(narrow[1].px);
    expect(m(wide[1].text, wide[1].px)).toBeLessThanOrEqual(104);
  });

  it('breaks_a_wrapped_surname_after_its_hyphen', () => {
    const lines = fitPrintName(m, 'Anna Zarzarbarkelmann-Dennelwenhoff', W, NAME_PX, 8);
    expect(lines.slice(1).map((l) => l.text)).toEqual(['Zarzarbarkelmann-', 'Dennelwenhoff']);
  });

  it('never_needs_more_lines_than_the_card_has_and_never_cuts', () => {
    const name = 'Bartholomaeus ' + 'Zarzarbarkelmann'.repeat(6);
    const lines = fitPrintName(m, name, W, NAME_PX, 8, 3);
    expect(lines.length).toBeLessThanOrEqual(3);
    expect(lines.map((l) => l.text).join('').replace(/\s/g, '')).toBe(name.replace(/\s/g, ''));
    expect(fits(lines)).toBe(true);
    expect(noEllipsis(lines)).toBe(true);
  });

  it('keeps_the_floor_at_a_readable_printed_size', () => {
    expect(A3_NAME_FLOOR_PT).toBeGreaterThan(A3_MIN_PT);
    for (const scale of [A3_MAX_SCALE, 1, 0.9]) expect(printedPt(nameFloorPx(scale), scale)).toBeCloseTo(A3_NAME_FLOOR_PT, 9);
    expect(NAME_FLOOR_PX).toBeCloseTo(nameFloorPx(1), 9);
    // A chart printed so small its names are already under that size is never shrunk: long names wrap.
    expect(nameFloorPx(0.4)).toBe(NAME_PX);
  });

  it('keeps_the_card_rows_for_plain_names_and_stacks_mixed_sizes', () => {
    expect(nameBaselines([{ text: 'a', px: NAME_PX }])).toEqual([23]);
    expect(nameBaselines([{ text: 'a', px: NAME_PX }, { text: 'b', px: NAME_PX }])).toEqual([16, 31]);
    // The fullest card: the given name at full size and two surname lines at the floor of a print at screen size.
    const f = NAME_FLOOR_PX;
    const ys = nameBaselines([{ text: 'a', px: NAME_PX }, { text: 'b', px: f }, { text: 'c', px: f }]);
    // Under the card's top, the second line under the ancestor number (baseline 14), no line touching the next.
    expect(ys[0] - 0.75 * NAME_PX).toBeGreaterThanOrEqual(3);
    expect(ys[1] - 0.75 * f).toBeGreaterThan(14);
    expect(ys[1] - 0.75 * f).toBeGreaterThanOrEqual(ys[0] + 0.25 * NAME_PX);
    expect(ys[2] - 0.75 * f).toBeGreaterThanOrEqual(ys[1] + 0.25 * f);
    // The last line clears the dates row (baseline 47, 11 px type).
    expect(ys[2] + 0.25 * f).toBeLessThan(47 - 0.75 * 11);
  });

  it('writes_each_name_line_of_a_print_card_with_its_own_size_in_the_svg', () => {
    const ctx = new SvgContext(400, 200, svgMeasure);
    const opts = { selected: null, focus: null, theme, lod: 'full' as const, compact: true, nameMinPx: 8 };
    draw(ctx as unknown as CanvasRenderingContext2D, { nodes: [card('Bartholomaeus Zarzarbarkelmann', 124, 56)], edges: [] }, { x: 0, y: 0, zoom: 1 }, { w: 400, h: 200, dpr: 1 }, opts);
    const svg = ctx.toSvg('#fff', { w: 400, h: 200 });
    expect(svg).not.toContain('…');
    const t = texts(svg);
    const given = t.find((x) => x.text === 'Bartholomaeus')!;
    const surname = t.find((x) => x.text === 'Zarzarbarkelmann')!;
    expect(given.px).toBe(NAME_PX);
    expect(surname.px).toBeLessThan(NAME_PX);
    expect(surname.text.length * surname.px * 0.6).toBeLessThanOrEqual(104);
    // The dates keep their own size.
    expect(t.find((x) => x.text === '1850-1900')!.px).toBe(11);
  });

  it('leaves_the_screen_cards_as_they_were', () => {
    const ctx = new SvgContext(400, 200, svgMeasure);
    draw(ctx as unknown as CanvasRenderingContext2D, { nodes: [card('Anna Dennelwenhoffmannsdottir', 176, 66)], edges: [] }, { x: 0, y: 0, zoom: 1 }, { w: 400, h: 200, dpr: 1 }, { selected: null, focus: null, theme, lod: 'full' });
    const svg = ctx.toSvg('#fff', { w: 400, h: 200 });
    expect(svg).toContain('…');
    expect(texts(svg).every((x) => x.px === NAME_PX || x.px === 11)).toBe(true);
  });
});
