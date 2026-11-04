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
