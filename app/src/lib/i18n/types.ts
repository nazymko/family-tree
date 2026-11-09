/** Plural forms of one message, keyed by `Intl.PluralRules` category. `{n}` is filled in with the count. */
export interface Plural {
  one: string;
  few?: string;
  many?: string;
  other: string;
}

/**
 * The shape of a catalogue, derived from the English one: every leaf is a string, or a Plural when the
 * English entry has an `other` form. Another language must have exactly the same keys.
 */
export type Widen<T> = T extends string ? string : T extends { one: string; other: string } ? Plural : { [K in keyof T]: Widen<T[K]> };

type Join<P extends string, K extends string> = P extends '' ? K : `${P}.${K}`;

/** Dotted paths of the leaves of `T` whose widened value is `L`. */
export type LeafPaths<T, L, P extends string = ''> = {
  [K in keyof T & string]: T[K] extends string
    ? string extends L
      ? Join<P, K>
      : never
    : T[K] extends { one: string; other: string }
      ? Plural extends L
        ? Join<P, K>
        : never
      : LeafPaths<T[K], L, Join<P, K>>;
}[keyof T & string];

/** Every CLDR plural category; a language uses only some of them (Arabic all six, Japanese only `other`). */
export type PluralCategory = 'zero' | 'one' | 'two' | 'few' | 'many' | 'other';

/** Plural forms as a partial catalogue may hold them: any categories the language needs. */
export type PluralForms = Partial<Record<PluralCategory, string>>;

/**
 * The shape of a partial catalogue: the English shape with every key optional and every plural entry allowed any
 * categories. A key a partial catalogue lacks falls back to English (see `find` / `plural` in index.ts).
 */
export type WidenPartial<T> = T extends string
  ? string
  : T extends { one: string; other: string }
    ? PluralForms
    : { [K in keyof T]?: WidenPartial<T[K]> };
