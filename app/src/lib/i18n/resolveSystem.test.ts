import { describe, expect, it, vi } from 'vitest';

// Pretend the partial languages are translated (tl just under the auto-pick share, sw exactly at it), to test the mapping alone.
vi.mock('./locales/coverage.json', () => ({
  default: Object.fromEntries(
    ['zh-Hans', 'hi', 'es', 'ar', 'fr', 'bn', 'pt', 'id', 'ur', 'de', 'ja', 'mr', 'vi', 'te', 'ha', 'tr', 'ta'].map((c) => [c, 100]).concat([['tl', 49], ['sw', 50]]),
  ),
}));

// The test setup imported the real module already; load a fresh copy that sees the mocked coverage.
vi.resetModules();
const { resolveSystemLanguage } = await import('./index');

describe('resolveSystemLanguage', () => {
  it('maps_regional_tags_to_the_catalogue_language', () => {
    expect(resolveSystemLanguage('zh-CN')).toBe('zh-Hans');
    expect(resolveSystemLanguage('zh')).toBe('zh-Hans');
    expect(resolveSystemLanguage('pt-BR')).toBe('pt');
    expect(resolveSystemLanguage('es-MX')).toBe('es');
    expect(resolveSystemLanguage('uk-UA')).toBe('uk');
    expect(resolveSystemLanguage('en-GB')).toBe('en');
  });

  it('gives_English_for_unknown_or_traditional_chinese', () => {
    expect(resolveSystemLanguage('ru-RU')).toBe('en');
    expect(resolveSystemLanguage('zh-TW')).toBe('en');
    expect(resolveSystemLanguage('')).toBe('en');
    expect(resolveSystemLanguage(undefined)).toBe('en');
  });

  it('skips_a_language_less_than_half_translated', () => {
    expect(resolveSystemLanguage('fil')).toBe('en');
    expect(resolveSystemLanguage('sw')).toBe('sw');
  });

  it('tries_the_navigator_list_in_order', () => {
    expect(resolveSystemLanguage(['fil-PH', 'de-DE', 'en'])).toBe('de');
    expect(resolveSystemLanguage(['ru', 'fr'])).toBe('fr');
  });
});
