import type { SexKind } from '../api/types';

/** The sign of each sex, shared by the person cards and everywhere the word is written. */
export const SEX_GLYPH: Record<SexKind, string> = {
  male: '♂',
  female: '♀',
  intersex: '⚥',
  unknown: '?',
  other: '•',
};

/** The word with its sign in front of it. Plain text, so it also works inside a native select. */
export const withSexIcon = (sex: SexKind, word: string): string => `${SEX_GLYPH[sex]} ${word}`;
