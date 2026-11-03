import { describe, expect, it } from 'vitest';
import { lifespanText } from './mockDate';
import { toDateDto } from './mockParse';

const d = (raw: string) => toDateDto(raw, 'dmy');

describe('lifespanText', () => {
  it('uses_stated_years_with_qualifiers', () => {
    expect(lifespanText(d('abt 1968'), d('2021'))).toBe('c. 1968 \u2013 2021');
    expect(lifespanText(d('bef 1968'), d('aft 2001'))).toBe('bef. 1968 \u2013 aft. 2001');
    expect(lifespanText(d('between 1850 and 1860'), d('3 Apr 1921'))).toBe('c. 1850 \u2013 1921');
    expect(lifespanText(d('1850'), null)).toBe('b. 1850');
    expect(lifespanText(null, d('2021'))).toBe('? \u2013 2021');
    expect(lifespanText(null, null)).toBe('');
  });
});
