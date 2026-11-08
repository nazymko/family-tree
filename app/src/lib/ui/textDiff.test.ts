import { describe, expect, it } from 'vitest';
import { collapse, diffText, sideOf, tokenize } from './textDiff';

const joined = (segs: { kind: string; text: string }[], kinds: string[]) => segs.filter((s) => kinds.includes(s.kind)).map((s) => s.text).join('');

describe('diffText', () => {
  it('marks_an_inserted_word', () => {
    const d = diffText('The red house', 'The big red house');
    expect(d.segs.filter((s) => s.kind === 'ins').map((s) => s.text.trim())).toEqual(['big']);
    expect(d.addedWords).toBe(1);
    expect(d.removedWords).toBe(0);
  });

  it('marks_a_deleted_phrase', () => {
    const d = diffText('She moved to the coast (in the spring) and stayed.', 'She moved to the coast and stayed.');
    expect(d.removedWords).toBe(3);
    expect(d.addedWords).toBe(0);
    expect(joined(d.segs, ['eq', 'del'])).toBe('She moved to the coast (in the spring) and stayed.');
    expect(joined(d.segs, ['eq', 'ins'])).toBe('She moved to the coast and stayed.');
  });

  it('marks_a_replaced_word_on_both_sides', () => {
    const d = diffText('one two three', 'one 2 three');
    expect(d.removedWords).toBe(1);
    expect(d.addedWords).toBe(1);
    expect(d.sharedWords).toBe(2);
  });

  it('handles_ukrainian_text_with_guillemets_and_apostrophes', () => {
    const before = 'Він жив у селі «Зоряне» і мав пів’сотні курей.';
    const after = 'Він жив у місті «Зоряне» і мав пів’сотні курей.';
    const d = diffText(before, after);
    expect(d.segs.filter((s) => s.kind === 'del').map((s) => s.text)).toEqual(['селі']);
    expect(d.segs.filter((s) => s.kind === 'ins').map((s) => s.text)).toEqual(['місті']);
    expect(tokenize('пів’сотні')).toEqual(['пів’сотні']);
    expect(tokenize("it's «ok»")).toEqual(["it's", ' ', '«', 'ok', '»']);
  });

  it('reports_nothing_for_identical_texts', () => {
    const d = diffText('same words here', 'same words here');
    expect(d.segs).toEqual([{ kind: 'eq', text: 'same words here' }]);
    expect(d.removedWords + d.addedWords).toBe(0);
  });

  it('handles_an_empty_side', () => {
    expect(diffText('', 'new text').segs).toEqual([{ kind: 'ins', text: 'new text' }]);
    expect(diffText('old text', '').segs).toEqual([{ kind: 'del', text: 'old text' }]);
    expect(diffText('', '').segs).toEqual([]);
  });

  it('keeps_both_sides_whole_for_a_very_long_text_that_hits_the_guard', () => {
    const sentence = (n: number) => Array.from({ length: 8 }, (_, k) => `w${n}x${k}`).join(' ') + '.';
    const a = Array.from({ length: 1500 }, (_, n) => sentence(n));
    const b = a.map((s, n) => (n % 2 === 0 ? s : s.replace('x3', 'y3')));
    const t0 = Date.now();
    const d = diffText(a.join(' '), b.join(' '));
    expect(Date.now() - t0).toBeLessThan(5000);
    expect(joined(d.segs, ['eq', 'del'])).toBe(a.join(' '));
    expect(joined(d.segs, ['eq', 'ins'])).toBe(b.join(' '));
    expect(d.sharedWords).toBeGreaterThan(0);
  });
});

describe('collapse', () => {
  const words = (n: number, p: string) => Array.from({ length: n }, (_, k) => `${p}${k}`).join(' ');

  it('keeps_context_around_a_change_and_a_gap_for_the_rest', () => {
    const before = `${words(40, 'a')} (extra phrase) ${words(40, 'b')}`;
    const after = `${words(40, 'a')} ${words(40, 'b')}`;
    const view = collapse(sideOf(diffText(before, after).segs, 'before'));
    const text = view.map((s) => s.text).join('');
    expect(view.filter((s) => s.kind === 'gap')).toHaveLength(2);
    expect(text.startsWith('…')).toBe(true);
    expect(text.endsWith('…')).toBe(true);
    expect(text).toContain('a39');
    expect(text).toContain('(extra phrase)');
    expect(text).not.toContain('a5 ');
  });

  it('leaves_short_unchanged_stretches_whole', () => {
    const view = collapse(sideOf(diffText('one two X three four', 'one two three four').segs, 'before'));
    expect(view.some((s) => s.kind === 'gap')).toBe(false);
  });
});
