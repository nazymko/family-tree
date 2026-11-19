import { describe, expect, it } from 'vitest';
import { bestDirection, fillRatio, layoutA3, mmToPx, printDirection, printStretch } from './a3';

describe('A3 without the title block', () => {
  const bounds = { x: 0, y: 0, w: 400, h: 2000 };

  it('gives_the_chart_the_height_of_the_title_block', () => {
    const withTitle = layoutA3(bounds, 'portrait');
    const without = layoutA3(bounds, 'portrait', false);
    expect(without.area.h - withTitle.area.h).toBeCloseTo(mmToPx(16), 6);
    // The margin stays: the chart starts 10 mm from the top.
    expect(without.area.y).toBeCloseTo(mmToPx(10), 6);
    expect(withTitle.area.y).toBeCloseTo(mmToPx(26), 6);
    // A chart that is limited by the height prints larger.
    expect(without.scale).toBeGreaterThan(withTitle.scale);
    expect(fillRatio(bounds, without).h).toBeCloseTo(1, 6);
  });

  it('is_used_by_the_direction_choice_and_the_card_stretch', () => {
    const list = [{ direction: 'right' as const, bounds }];
    expect(printDirection(list, 'right', 'screen', 'auto', false).layout.area.y).toBeCloseTo(mmToPx(10), 6);
    expect(bestDirection(list, 'right', 'auto', false).layout.area.h).toBeGreaterThan(bestDirection(list, 'right', 'auto').layout.area.h);
    // A wide chart that fills the page across has empty room above and below: the cards still stretch into it.
    const wide = { x: 0, y: 0, w: 1600, h: 300 };
    expect(printStretch(wide, 1.3, 'landscape', false)).toBeCloseTo(1.3, 6);
  });
});
