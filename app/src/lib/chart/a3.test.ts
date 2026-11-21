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
  NAME_SLIGHT,
  TEXT_ASCENT,
  TEXT_DESCENT,
  CARD_TEXT_X,
  CARD_TEXT_RIGHT,
  planPrintCard,
  printCardHeight,
  printStretch,
  stretchScene,
  type CardMeasure,
  type PrintCardPlan,
  type PrintCardText,
  nameFloorPx,
  printDirection,
  printHtml,
  mmToPx,
  printDocument,
  printDocumentPages,
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

  it('carries_the_app_fonts_so_old_letters_print_in_them', () => {
    const face = "@font-face { font-family: 'GT Old Cyrillic'; src: url(/assets/x.woff2); unicode-range: U+460-52F; }";
    const html = printDocument('<svg><text>Ѳеодоръ</text></svg>', 'portrait', `${face}</style><script>`);
    expect(html).toContain("font-family: 'GT Old Cyrillic'");
    expect(html).not.toContain('</style><script>');
    expect(html.indexOf('GT Old Cyrillic')).toBeLessThan(html.indexOf('@page'));
  });

  it('prints_a_set_of_sheets_as_one_document_one_page_each', () => {
    const html = printDocumentPages(
      [
        { svg: '<?xml version="1.0"?>\n<svg id="a"></svg>', orientation: 'landscape' },
        { svg: '<svg id="b"></svg>', orientation: 'portrait' },
        { svg: '<svg id="c"></svg>', orientation: 'landscape' },
      ],
      "@font-face { font-family: 'GT Old Cyrillic'; }",
    );
    expect(html.match(/<svg/g)).toHaveLength(3);
    expect(html).not.toContain('<?xml');
    // A break after every page but the last.
    expect(html.match(/break-after:page/g)).toHaveLength(2);
    expect(html).toContain('@page land{size:A3 landscape;margin:0}');
    expect(html).toContain('@page port{size:A3 portrait;margin:0}');
    // Each page takes its own paper: the portrait one names the portrait page.
    const blocks = html.split('<div class="sheet').slice(1);
    expect(blocks.map((b) => /page:(\w+)/.exec(b)?.[1])).toEqual(['land', 'port', 'land']);
    expect(blocks[1]).toContain('width:297mm;height:420mm');
    expect(blocks[2]).not.toContain('break-after:page');
    expect(html.indexOf('GT Old Cyrillic')).toBeLessThan(html.indexOf('@page'));
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

  it('keeps_the_given_name_and_patronymic_together_when_a_slight_shrink_fits', () => {
    // 14 letters: 109 px at 13 px, a little over the 104 px line; 12.25 px fits.
    const lines = fitPrintName(m, 'Іван Антонович Тестенко', W, NAME_PX, NAME_PX);
    expect(lines.map((l) => l.text)).toEqual(['Іван Антонович', 'Тестенко']);
    expect(lines[0].px).toBeLessThan(NAME_PX);
    expect(lines[0].px).toBeGreaterThanOrEqual(NAME_PX * NAME_SLIGHT);
    expect(lines[1].px).toBe(NAME_PX);
  });

  it('puts_a_long_three_word_name_on_three_whole_word_lines_at_full_size_on_a_small_print', () => {
    // A print so small its names are under the floor: nothing is shrunk past NAME_SLIGHT, each word gets its own line.
    const lines = fitPrintName(m, 'Святослав Олександрович Тестенко', W, NAME_PX, nameFloorPx(0.47));
    expect(lines).toEqual([
      { text: 'Святослав', px: NAME_PX },
      { text: 'Олександрович', px: NAME_PX },
      { text: 'Тестенко', px: NAME_PX },
    ]);
  });

  describe('print cards never overlap or cut their text', () => {
  // Synthetic three-word Ukrainian-style names (given name, patronymic, surname), dates and relationships.
  const NAMES = [
    'Святослав Олександрович Тестенко',
    'Ярослава Костянтинівна Прикладенко',
    'Олександра Володимирівна Вигаданчук',
    'Мирослав Святославович Зразковський',
    'Іван Петрович Тестенко',
    'Христина Мирославівна Довгопрізвищенко-Вигаданчук',
    'Анна Тестенко',
  ];
  const SUBS = ['бл. 1774 – ?', 'нар. бл. 1862', '1889 – 1970', '1915 – ?', ''];
  const RELS = ['прадідусь', 'бабуся', '2× прабабуся', 'двоюрідна прабабуся', 'мати', ''];
  // Stand-in for the fonts: a name letter is 0.6 of the size wide, a dates or relationship letter 0.55.
  const cm: CardMeasure = (s, font, px) => s.length * px * (font === 'name' ? 0.6 : 0.55);
  const CARD_W = 124;
  /** A print scaled like the owner's (cards about a third of an A3 width apart): names under the floor already. */
  const SMALL = nameFloorPx(0.47);

  interface Box {
    what: string;
    x0: number;
    x1: number;
    y0: number;
    y1: number;
  }
  const boxes = (p: PrintCardPlan, c: PrintCardText): Box[] => {
    const out: Box[] = p.lines.map((l, i) => ({
      what: `name ${i}`,
      x0: c.left,
      x1: c.left + cm(l.text, 'name', l.px),
      y0: p.nameYs[i] - TEXT_ASCENT * l.px,
      y1: p.nameYs[i] + TEXT_DESCENT * l.px,
    }));
    if (p.sub) out.push({ what: 'dates', x0: CARD_TEXT_X, x1: CARD_TEXT_X + cm(p.sub.text, 'sub', p.sub.px), y0: p.sub.y - TEXT_ASCENT * p.sub.px, y1: p.sub.y + TEXT_DESCENT * p.sub.px });
    if (p.rel) out.push({ what: 'relation', x0: c.w - CARD_TEXT_RIGHT - cm(p.rel.text, 'rel', p.rel.px), x1: c.w - CARD_TEXT_RIGHT, y0: p.rel.y - TEXT_ASCENT * p.rel.px, y1: p.rel.y + TEXT_DESCENT * p.rel.px });
    // The ancestor number: 10 px type at baseline 14, right-aligned.
    if (c.ahnenW > 0) out.push({ what: 'number', x0: c.w - 8 - (c.ahnenW - 8), x1: c.w - 8, y0: 14 - 7.5, y1: 14 + 2.5 });
    return out;
  };
  const overlaps = (a: Box, b: Box) => a.x0 < b.x1 - 1e-9 && b.x0 < a.x1 - 1e-9 && a.y0 < b.y1 - 1e-9 && b.y0 < a.y1 - 1e-9;
  /** Every problem of a laid-out card: boxes that intersect, text outside the card, text cut or missing. */
  const problems = (p: PrintCardPlan, c: PrintCardText, h: number): string[] => {
    const out: string[] = [];
    const bs = boxes(p, c);
    for (let i = 0; i < bs.length; i++) {
      const b = bs[i];
      if (b.x0 < -1e-9 || b.x1 > c.w + 1e-9 || b.y0 < -1e-9 || b.y1 > h + 1e-9) out.push(`${b.what} outside the card`);
      for (let j = i + 1; j < bs.length; j++) if (overlaps(b, bs[j])) out.push(`${b.what} overlaps ${bs[j].what}`);
    }
    const all = [...p.lines.map((l) => l.text), p.sub?.text ?? '', p.rel?.text ?? ''].join('');
    if (all.includes('…')) out.push('ellipsis');
    if (p.lines.map((l) => l.text).join('').replace(/\s/g, '') !== c.name.replace(/\s/g, '')) out.push('name not whole');
    if ((p.sub?.text ?? '') !== c.sub) out.push('dates not whole');
    if ((p.rel?.text ?? '') !== c.rel) out.push('relation not whole');
    return out;
  };
  const cards = (): PrintCardText[] => {
    const out: PrintCardText[] = [];
    for (const name of NAMES)
      for (const sub of SUBS)
        for (const rel of RELS)
          for (const [left, ahnenW] of [
            [10, 0],
            [26, 0],
            [10, 22],
            [10, 38],
          ])
            out.push({ name, sub, rel, w: CARD_W, left, ahnenW });
    return out;
  };

  it('reproduces_the_overlap_the_old_card_rows_had_with_a_three_line_name', () => {
    // The card is 56 px high; three name lines at 13 px and the dates do not fit in it.
    const c: PrintCardText = { name: 'Святослав Олександрович Тестенко', sub: 'бл. 1774 – ?', rel: 'прадідусь', w: CARD_W, left: 10, ahnenW: 0 };
    expect(fitPrintName((s, px) => cm(s, 'name', px), c.name, 104, NAME_PX, SMALL)).toHaveLength(3);
    expect(printCardHeight(cm, c, SMALL)).toBeGreaterThan(56);
  });

  it('grown_to_the_height_it_needs_a_card_keeps_every_size_and_nothing_overlaps', () => {
    for (const c of cards()) {
      const h = Math.max(56, printCardHeight(cm, c, SMALL));
      const p = planPrintCard(cm, c, h, SMALL);
      expect(problems(p, c, h), JSON.stringify(c)).toEqual([]);
      // Grown, nothing is drawn smaller than it has to be.
      expect(p.lines).toEqual(fitPrintName((s, px) => cm(s, 'name', px), c.name, CARD_W - 10 - c.left - c.ahnenW, NAME_PX, SMALL, 3, CARD_W - 10 - c.left));
      if (p.sub) expect(p.sub.px).toBe(11);
    }
  });

  it('a_card_that_cannot_grow_draws_its_text_smaller_and_still_nothing_overlaps', () => {
    for (const c of cards()) {
      const p = planPrintCard(cm, c, 56, SMALL);
      expect(p.height).toBeLessThanOrEqual(56 + 1e-9);
      expect(problems(p, c, 56), JSON.stringify(c)).toEqual([]);
    }
  });

  it('puts_the_relationship_beside_the_dates_only_when_both_fit_else_on_its_own_row', () => {
    const beside = planPrintCard(cm, { name: 'Анна Тестенко', sub: '1915 – ?', rel: 'мати', w: CARD_W, left: 10, ahnenW: 0 }, 56);
    expect(beside.rel!.y).toBe(beside.sub!.y);
    const under = planPrintCard(cm, { name: 'Анна Тестенко', sub: 'бл. 1774 – ?', rel: '2× прабабуся', w: CARD_W, left: 10, ahnenW: 0 }, 70);
    expect(under.rel!.text).toBe('2× прабабуся');
    expect(under.rel!.y).toBeGreaterThan(under.sub!.y);
  });

  it('keeps_a_short_name_card_at_its_old_place_in_the_card', () => {
    const p = planPrintCard(cm, { name: 'Анна Тестенко', sub: '1915 – ?', rel: '', w: CARD_W, left: 10, ahnenW: 0 }, 56);
    expect(p.lines).toEqual([{ text: 'Анна Тестенко', px: NAME_PX }]);
    expect(Math.abs(p.nameYs[0] - 23)).toBeLessThan(2);
    expect(Math.abs(p.sub!.y - 43)).toBeLessThan(3);
  });

  it('stretches_the_chart_height_with_the_connectors_still_meeting_the_cards', () => {
    const scene = {
      nodes: [
        { id: 0, y: 0, h: 56 },
        { id: 1, y: 100, h: 56 },
      ],
      edges: [{ pts: [10, 56, 10, 78, 40, 78, 40, 100] }],
    };
    const s = stretchScene(scene, 1.5);
    expect(s.nodes.map((n) => [n.y, n.h])).toEqual([
      [0, 84],
      [150, 84],
    ]);
    const pts = s.edges[0].pts;
    // From the bottom of the upper card to the top of the lower one; x untouched.
    expect(pts[1]).toBe(s.nodes[0].y + s.nodes[0].h);
    expect(pts[pts.length - 1]).toBe(s.nodes[1].y);
    expect(pts.filter((_, i) => i % 2 === 0)).toEqual([10, 10, 40, 40]);
    expect(stretchScene(scene, 1)).toBe(scene);
  });

  it('grows_the_cards_only_into_the_room_the_page_leaves_empty', () => {
    // A wide, low chart fills the page across: there is room below it, so the cards grow all the way.
    const wide = { x: 0, y: 0, w: 3000, h: 600 };
    expect(printStretch(wide, 1.5, 'landscape')).toBe(1.5);
    expect(layoutA3({ ...wide, h: 900 }, 'landscape').scale).toBeCloseTo(layoutA3(wide, 'landscape').scale, 9);
    // A tall chart fills the page from top to bottom: growing would print it smaller, so it does not grow.
    const tall = { x: 0, y: 0, w: 600, h: 3000 };
    expect(printStretch(tall, 1.5, 'landscape')).toBeCloseTo(1, 6);
    // In between it grows as far as the page allows, and the chart prints as large as before.
    const mid = { x: 0, y: 0, w: 3000, h: 1500 };
    const k = printStretch(mid, 1.5, 'landscape');
    expect(k).toBeGreaterThan(1);
    expect(k).toBeLessThan(1.5);
    expect(layoutA3({ ...mid, h: mid.h * k }, 'landscape').scale).toBeCloseTo(layoutA3(mid, 'landscape').scale, 6);
    expect(printStretch(wide, 1, 'landscape')).toBe(1);
  });
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
