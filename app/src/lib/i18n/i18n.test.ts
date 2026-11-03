import { describe, expect, it } from 'vitest';
import { plural, resolveSystemLanguage, setLanguage, t, currentLocale, formatNumber, LANGUAGES } from '.';

describe('i18n', () => {
  it('t_returns_english_and_fills_placeholders', () => {
    expect(t('common.save')).toBe('Save');
    expect(t('settings.livingThresholdHint', { age: 90 })).toContain('more than 90 years');
  });

  it('t_switches_to_ukrainian_at_once', () => {
    setLanguage('uk');
    expect(t('common.save')).toBe('Зберегти');
    expect(document.documentElement.lang).toBe('uk');
    setLanguage('en');
    expect(t('common.save')).toBe('Save');
    expect(document.documentElement.lang).toBe('en');
  });

  it('unknown_placeholder_is_left_alone_and_unknown_key_returns_the_key', () => {
    expect(t('settings.livingThresholdHint')).toContain('{age}');
    // @ts-expect-error not a key
    expect(t('no.such.key')).toBe('no.such.key');
  });

  it('system_language_maps_uk_to_Ukrainian_and_anything_else_to_English', () => {
    expect(resolveSystemLanguage('uk')).toBe('uk');
    expect(resolveSystemLanguage('uk-UA')).toBe('uk');
    expect(resolveSystemLanguage('en-GB')).toBe('en');
    expect(resolveSystemLanguage('de-DE')).toBe('en');
    expect(resolveSystemLanguage('')).toBe('en');
    setLanguage('system');
    expect(currentLocale()).toBe('en');
  });

  it('every_language_is_listed_by_its_own_name', () => {
    expect(LANGUAGES.map((l) => l.name)).toEqual(['English', 'Українська']);
  });

  it('plural_in_ukrainian_gives_one_few_many', () => {
    setLanguage('uk');
    const word = (n: number) => plural('common.people', n);
    expect(word(1)).toBe('1 особа');
    expect(word(2)).toBe('2 особи');
    expect(word(5)).toBe('5 осіб');
    expect(word(21)).toBe('21 особа');
    expect(word(11)).toBe('11 осіб');
    expect(word(24)).toBe('24 особи');
  });

  it('plural_in_english_gives_one_and_other', () => {
    expect(plural('common.people', 1)).toBe('1 person');
    expect(plural('common.people', 2)).toBe('2 people');
    expect(plural('common.people', 0)).toBe('0 people');
  });

  it('numbers_use_the_language_grouping', () => {
    expect(formatNumber(1234)).toBe(new Intl.NumberFormat('en').format(1234));
    setLanguage('uk');
    expect(formatNumber(1234)).toBe(new Intl.NumberFormat('uk').format(1234));
    expect(plural('common.people', 1234)).toContain(new Intl.NumberFormat('uk').format(1234));
  });
});
