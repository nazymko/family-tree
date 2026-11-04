import { afterEach, describe, expect, it } from 'vitest';
import { setLanguage } from '../i18n';
import type { Kinship, SexKind } from '../api/types';
import { kinshipText } from './kinship';

const blood = (up: number, down: number, sex: SexKind, half = false): Kinship => ({ kind: 'blood', up, down, half, married: false, sex, via: 'unknown' });
const partner = (sex: SexKind, married: boolean): Kinship => ({ kind: 'partner', up: 0, down: 0, half: false, married, sex, via: 'unknown' });
const ofRelative = (up: number, down: number, sex: SexKind, via: SexKind): Kinship => ({ kind: 'partner_of_relative', up, down, half: false, married: true, sex, via });
const ofPartner = (up: number, down: number, sex: SexKind, via: SexKind): Kinship => ({ kind: 'relative_of_partner', up, down, half: false, married: true, sex, via });

const m: SexKind = 'male';
const f: SexKind = 'female';
const u: SexKind = 'unknown';

afterEach(() => setLanguage('en'));

describe('kinshipText in English', () => {
  it('words_the_direct_line', () => {
    setLanguage('en');
    expect(kinshipText(blood(1, 0, m))).toBe('father');
    expect(kinshipText(blood(1, 0, f))).toBe('mother');
    expect(kinshipText(blood(1, 0, u))).toBe('parent');
    expect(kinshipText(blood(2, 0, f))).toBe('grandmother');
    expect(kinshipText(blood(3, 0, m))).toBe('great-grandfather');
    expect(kinshipText(blood(4, 0, m))).toBe('great-great-grandfather');
    expect(kinshipText(blood(5, 0, m))).toBe('3× great-grandfather');
    expect(kinshipText(blood(0, 1, m))).toBe('son');
    expect(kinshipText(blood(0, 2, f))).toBe('granddaughter');
    expect(kinshipText(blood(0, 3, u))).toBe('great-grandchild');
    expect(kinshipText(blood(0, 6, m))).toBe('4× great-grandson');
  });

  it('words_siblings_and_their_branches', () => {
    setLanguage('en');
    expect(kinshipText(blood(1, 1, m))).toBe('brother');
    expect(kinshipText(blood(1, 1, f, true))).toBe('half-sister');
    expect(kinshipText(blood(1, 1, u, true))).toBe('half-sibling');
    expect(kinshipText(blood(1, 2, f))).toBe('niece');
    expect(kinshipText(blood(1, 3, m))).toBe('great-nephew');
    expect(kinshipText(blood(2, 1, m))).toBe('uncle');
    expect(kinshipText(blood(2, 1, f))).toBe('aunt');
    expect(kinshipText(blood(2, 1, u))).toBe('aunt or uncle');
    expect(kinshipText(blood(3, 1, m))).toBe('great-uncle');
  });

  it('words_cousins_and_removals', () => {
    setLanguage('en');
    expect(kinshipText(blood(2, 2, m))).toBe('first cousin');
    expect(kinshipText(blood(3, 3, f))).toBe('second cousin');
    expect(kinshipText(blood(2, 3, f))).toBe('first cousin once removed');
    expect(kinshipText(blood(4, 3, m))).toBe('second cousin once removed');
    expect(kinshipText(blood(2, 4, m))).toBe('first cousin twice removed');
    expect(kinshipText(blood(2, 6, m))).toBe('first cousin 4 times removed');
  });

  it('words_partners_and_in_laws', () => {
    setLanguage('en');
    expect(kinshipText(partner(m, true))).toBe('husband');
    expect(kinshipText(partner(f, true))).toBe('wife');
    expect(kinshipText(partner(u, true))).toBe('spouse');
    expect(kinshipText(partner(f, false))).toBe('partner');
    expect(kinshipText(ofRelative(1, 0, f, m))).toBe('stepmother');
    expect(kinshipText(ofRelative(0, 1, m, f))).toBe('son-in-law');
    expect(kinshipText(ofRelative(0, 2, f, m))).toBe('granddaughter-in-law');
    expect(kinshipText(ofRelative(1, 1, m, f))).toBe('brother-in-law');
    expect(kinshipText(ofRelative(2, 1, f, m))).toBe('aunt by marriage');
    expect(kinshipText(ofPartner(1, 0, m, f))).toBe('father-in-law');
    expect(kinshipText(ofPartner(1, 1, f, m))).toBe('sister-in-law');
    expect(kinshipText(ofPartner(0, 1, f, m))).toBe('stepdaughter');
  });

  it('falls_back_to_relative_words', () => {
    setLanguage('en');
    expect(kinshipText(blood(1, 6, m))).toBe('relative');
    expect(kinshipText(ofPartner(2, 2, m, f))).toBe('relative by marriage');
    expect(kinshipText(ofRelative(3, 3, m, f))).toBe('relative by marriage');
    expect(kinshipText(undefined)).toBe('');
  });
});

describe('kinshipText in Ukrainian', () => {
  it('words_the_direct_line', () => {
    setLanguage('uk');
    expect(kinshipText(blood(1, 0, m))).toBe('батько');
    expect(kinshipText(blood(1, 0, f))).toBe('мати');
    expect(kinshipText(blood(2, 0, m))).toBe('дідусь');
    expect(kinshipText(blood(3, 0, f))).toBe('прабаба');
    expect(kinshipText(blood(4, 0, m))).toBe('прапрадід');
    expect(kinshipText(blood(0, 1, f))).toBe('донька');
    expect(kinshipText(blood(0, 2, m))).toBe('онук');
    expect(kinshipText(blood(0, 3, f))).toBe('правнучка');
  });

  it('words_siblings_aunts_and_cousins', () => {
    setLanguage('uk');
    expect(kinshipText(blood(1, 1, m))).toBe('брат');
    expect(kinshipText(blood(1, 1, f, true))).toBe('зведена сестра');
    expect(kinshipText(blood(1, 1, m, true))).toBe('зведений брат');
    expect(kinshipText(blood(1, 2, f))).toBe('племінниця');
    expect(kinshipText(blood(2, 1, m))).toBe('дядько');
    expect(kinshipText(blood(2, 1, f))).toBe('тітка');
    expect(kinshipText(blood(2, 2, m))).toBe('двоюрідний брат');
    expect(kinshipText(blood(2, 2, f))).toBe('двоюрідна сестра');
    expect(kinshipText(blood(3, 3, m))).toBe('троюрідний брат');
    expect(kinshipText(blood(2, 3, f))).toBe('двоюрідна племінниця');
    expect(kinshipText(blood(3, 2, m))).toBe('двоюрідний дядько');
    expect(kinshipText(blood(2, 4, m))).toBe('двоюрідний родич, на два покоління');
  });

  it('words_partners_and_in_laws_by_side', () => {
    setLanguage('uk');
    expect(kinshipText(partner(m, true))).toBe('чоловік');
    expect(kinshipText(partner(f, true))).toBe('дружина');
    expect(kinshipText(partner(f, false))).toBe('партнерка');
    expect(kinshipText(ofPartner(1, 0, m, f))).toBe('тесть');
    expect(kinshipText(ofPartner(1, 0, f, f))).toBe('теща');
    expect(kinshipText(ofPartner(1, 0, m, m))).toBe('свекор');
    expect(kinshipText(ofPartner(1, 0, f, m))).toBe('свекруха');
    expect(kinshipText(ofPartner(1, 1, m, f))).toBe('шурин');
    expect(kinshipText(ofPartner(1, 1, f, f))).toBe('своячениця');
    expect(kinshipText(ofPartner(1, 1, m, m))).toBe('дівер');
    expect(kinshipText(ofPartner(1, 1, f, m))).toBe('зовиця');
    expect(kinshipText(ofRelative(0, 1, m, f))).toBe('зять');
    expect(kinshipText(ofRelative(0, 1, f, m))).toBe('невістка');
    expect(kinshipText(ofRelative(1, 0, f, m))).toBe('мачуха');
    expect(kinshipText(ofPartner(0, 1, m, m))).toBe('пасинок');
  });

  it('falls_back_to_the_side_of_the_partner', () => {
    setLanguage('uk');
    expect(kinshipText(blood(1, 6, m))).toBe('родич');
    expect(kinshipText(ofPartner(2, 2, m, m))).toBe('родич чоловіка');
    expect(kinshipText(ofPartner(2, 2, m, f))).toBe('родич дружини');
    expect(kinshipText(ofPartner(2, 2, m, u))).toBe('родич партнера');
    expect(kinshipText(ofRelative(3, 3, m, f))).toBe('родич за шлюбом');
  });
});
