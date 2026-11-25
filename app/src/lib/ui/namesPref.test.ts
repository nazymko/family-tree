import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { makeFakeContext } from '../../test-setup';
import type { ChartNode } from '../api/types';
import { draw, type Theme } from '../chart/renderer';
import { NAMES_AS_RECORDED_KEY, cardName, namesAsRecorded, shownName } from './namesPref.svelte';

// Invented people only.
const theme: Theme = { paper: '#fff', rule: '#ccc', ink: '#000', ink2: '#666', ribbon: '#f00', wash: '#fee', font: 'sans-serif', serif: 'serif' };
const view = { w: 800, h: 600, dpr: 2 };
const fedir = { name: 'Fedir Petrovych Vyhadanenko', asRecorded: 'Ѳеодоръ Петровъ Вигаданенко' };

beforeEach(() => {
  localStorage.removeItem(NAMES_AS_RECORDED_KEY);
  namesAsRecorded.reload();
});
afterEach(() => {
  localStorage.removeItem(NAMES_AS_RECORDED_KEY);
  namesAsRecorded.reload();
});

describe('names as recorded', () => {
  it('shows_the_modern_name_by_default', () => {
    expect(namesAsRecorded.value).toBe(false);
    expect(shownName(fedir)).toEqual({ text: fedir.name, recorded: false, modern: fedir.name });
  });

  it('shows_the_recorded_name_when_on_and_falls_back_to_the_modern_one_without_it', () => {
    expect(shownName(fedir, true)).toEqual({ text: fedir.asRecorded, recorded: true, modern: fedir.name });
    expect(shownName({ name: 'Olena Vyhadanenko' }, true).text).toBe('Olena Vyhadanenko');
    expect(shownName({ name: 'Olena Vyhadanenko', asRecorded: '  ' }, true).recorded).toBe(false);
    expect(shownName(null, true).text).toBe('');
  });

  it('is_remembered', () => {
    namesAsRecorded.set(true);
    expect(localStorage.getItem(NAMES_AS_RECORDED_KEY)).toBe('1');
    namesAsRecorded.value = false;
    namesAsRecorded.reload();
    expect(namesAsRecorded.value).toBe(true);
    namesAsRecorded.set(false);
    namesAsRecorded.reload();
    expect(namesAsRecorded.value).toBe(false);
  });

  it('marks_a_recorded_name_on_the_card_only_when_on', () => {
    expect(cardName(fedir)).toBe(fedir.name);
    namesAsRecorded.set(true);
    expect(cardName(fedir)).toBe(`«${fedir.asRecorded}»`);
    expect(cardName({ name: 'Olena Vyhadanenko' })).toBe('Olena Vyhadanenko');
  });

  it('a_chart_card_draws_the_recorded_name', () => {
    const n: ChartNode = {
      id: 0,
      person: { id: 'p0', ...fedir, lifespan: '', sex: 'male', living: false, private: false, siblingMarker: false },
      x: 0,
      y: 0,
      w: 300,
      h: 66,
      gen: 0,
      ahnen: 1,
      dupOf: null,
      moreUp: false,
      moreDown: false,
      nav: { parent: null, child: null, prev: null, next: null },
      label: '',
    };
    const texts = () => {
      const ctx = makeFakeContext();
      draw(ctx, { nodes: [n], edges: [] }, { x: 0, y: 0, zoom: 1 }, view, { selected: null, focus: null, theme });
      return ctx.calls.filter((c) => c.op === 'fillText').map((c) => String(c.args[0]));
    };
    expect(texts()).toContain(fedir.name);
    namesAsRecorded.set(true);
    const on = texts();
    expect(on).toContain(`«${fedir.asRecorded}»`);
    expect(on).not.toContain(fedir.name);
  });
});
