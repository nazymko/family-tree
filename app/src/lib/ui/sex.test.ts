import { describe, expect, it } from 'vitest';
import { SEX_GLYPH, withSexIcon } from './sex';

describe('the sign of a sex', () => {
  it('stands before the word', () => {
    expect(withSexIcon('male', 'Male')).toBe('♂ Male');
    expect(withSexIcon('female', 'Жіноча')).toBe('♀ Жіноча');
  });

  it('exists for every sex, each different', () => {
    expect(new Set(Object.values(SEX_GLYPH)).size).toBe(5);
  });
});
