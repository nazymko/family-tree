import { describe, expect, it } from 'vitest';
import type { WholeBlockDto } from '../api/types';
import { founderOf } from './wholeStart';

const block = (x: number, y: number, w: number, unconnected = false): WholeBlockDto => ({ title: 't', surname: '', people: 1, unconnected, rect: { x, y, w, h: 28 }, bodyH: 400 });
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

  it('bottom_picks_the_bottom_row_of_the_family_not_a_block_below', () => {
    // The family's body spans y 40..440; another block starts below it. Reversed, its oldest generation is the bottom row.
    const blocks = [{ ...block(0, 0, 80000), bodyH: 400 }, block(0, 9000, 240, true)];
    const cards = [card(1, 60000, 40), card(2, 100, 384), card(3, 61000, 384), card(4, 10, 9040)];
    expect(founderOf(blocks, cards, 'bottom')?.id).toBe(2);
    expect(founderOf(blocks, cards)?.id).toBe(1);
  });

  it('left_and_right_pick_the_first_column_of_the_family', () => {
    // A turned family: its generations are columns inside the block's body (y 40..440), siblings stack down.
    const blocks = [{ ...block(0, 0, 900), bodyH: 400 }, block(0, 9000, 240, true)];
    const cards = [card(1, 700, 100), card(2, 0, 300), card(3, 0, 40), card(4, 350, 40), card(5, 10, 9040)];
    expect(founderOf(blocks, cards, 'left')?.id).toBe(3);
    expect(founderOf(blocks, cards, 'right')?.id).toBe(1);
  });
});
