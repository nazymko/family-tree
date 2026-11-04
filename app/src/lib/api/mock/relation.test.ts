import { describe, expect, it } from 'vitest';
import { relationLabel as l } from './relation';

describe('relationLabel', () => {
  it('names_ancestors_by_sex_and_distance', () => {
    expect(l({ t: 'ancestor', n: 1 }, 'male')).toBe('father');
    expect(l({ t: 'ancestor', n: 1 }, 'female')).toBe('mother');
    expect(l({ t: 'ancestor', n: 1 }, 'unknown')).toBe('parent');
    expect(l({ t: 'ancestor', n: 2 }, 'female')).toBe('grandmother');
    expect(l({ t: 'ancestor', n: 3 }, 'male')).toBe('great-grandfather');
    expect(l({ t: 'ancestor', n: 4 }, 'female')).toBe('2x great-grandmother');
  });

  it('names_descendants', () => {
    expect(l({ t: 'descendant', n: 1 }, 'male')).toBe('son');
    expect(l({ t: 'descendant', n: 1 }, 'other')).toBe('child');
    expect(l({ t: 'descendant', n: 2 }, 'female')).toBe('granddaughter');
    expect(l({ t: 'descendant', n: 3 }, 'unknown')).toBe('great-grandchild');
  });

  it('names_siblings_aunts_and_nieces', () => {
    expect(l({ t: 'sibling', half: false }, 'male')).toBe('brother');
    expect(l({ t: 'sibling', half: true }, 'female')).toBe('half sister');
    expect(l({ t: 'sibling', half: true }, 'unknown')).toBe('half sibling');
    expect(l({ t: 'aunt', half: false }, 'female')).toBe('aunt');
    expect(l({ t: 'aunt', half: true }, 'male')).toBe('half uncle');
    expect(l({ t: 'aunt', half: false }, 'unknown')).toBe('aunt or uncle');
    expect(l({ t: 'nibling' }, 'female')).toBe('niece');
    expect(l({ t: 'nibling' }, 'male')).toBe('nephew');
    expect(l({ t: 'cousin' }, 'female')).toBe('cousin');
  });

  it('names_partners_and_the_starting_person', () => {
    expect(l({ t: 'focus' }, 'male')).toBe('starting person');
    expect(l({ t: 'partner', of: 'focus' }, 'female')).toBe('partner');
    expect(l({ t: 'partner', of: 'sibling' }, 'female')).toBe('sister-in-law');
    expect(l({ t: 'partner', of: 'sibling' }, 'male')).toBe('brother-in-law');
    expect(l({ t: 'partner', of: 'sibling' }, 'unknown')).toBe('in-law');
    expect(l({ t: 'partner', of: 'aunt' }, 'male')).toBe('uncle by marriage');
    expect(l({ t: 'partner', of: 'aunt' }, 'female')).toBe('aunt by marriage');
    expect(l({ t: 'partner', of: 1 }, 'male')).toBe('son-in-law');
    expect(l({ t: 'partner', of: 1 }, 'female')).toBe('daughter-in-law');
    expect(l({ t: 'partner', of: 2 }, 'male')).toBe('grandson-in-law');
    expect(l({ t: 'partner', of: 2 }, 'female')).toBe('granddaughter-in-law');
    expect(l({ t: 'partner', of: 1 }, 'unknown')).toBe('in-law');
  });
});
