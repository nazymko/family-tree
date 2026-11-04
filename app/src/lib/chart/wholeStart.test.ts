import { describe, expect, it } from 'vitest';
import type { WholeBlockDto } from '../api/types';
import { founderOf } from './wholeStart';

const block = (x: number, y: number, w: number, unconnected = false): WholeBlockDto => ({ title: 't', people: 1, unconnected, rect: { x, y, w, h: 28 } });
const card = (id: number, x: number, y: number) => ({ id, x, y, w: 176, h: 56 });

describe('founderOf', () => {
  it('picks_the_top_row_card_even_when_it_is_far_from_the_heading_corner', () => {
    // The family's oldest generation sits at x = 60000, nowhere near the heading at x = 0.
    const blocks = [block(0, 0, 80000), block(0, 9000, 240, true)];
    const cards = [card(1, 60000, 40), card(2, 100, 400), card(3, 61000, 40), card(4, 10, 9040)];
    expect(founderOf(blocks, cards)?.id).toBe(1);
  });

  it('is_null_for_an_empty_tree', () => {
    expect(founderOf([], [])).toBeNull();
  });
});
