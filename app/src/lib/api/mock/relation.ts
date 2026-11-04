import type { SexKind } from '../types';

// How a person on the relatives chart is related to the centred one. Mirrors crates/gt-session/src/relation.rs:
// all the chart's relationship wording is in these two files.

/** Whose partner a partner box is: the centred person, a sibling, a parent's sibling, or a descendant n generations down. */
export type PartnerOf = 'focus' | 'sibling' | 'aunt' | number;

export type Rel =
  | { t: 'focus' }
  | { t: 'ancestor'; n: number }
  | { t: 'descendant'; n: number }
  | { t: 'sibling'; half: boolean }
  | { t: 'aunt'; half: boolean }
  | { t: 'nibling' }
  | { t: 'cousin' }
  | { t: 'partner'; of: PartnerOf };

function pick(sex: SexKind, male: string, female: string, neutral: string): string {
  return sex === 'male' ? male : sex === 'female' ? female : neutral;
}

function generations(steps: number, male: string, female: string, neutral: string, sex: SexKind): string {
  const word = pick(sex, male, female, neutral);
  if (steps <= 1) return word;
  if (steps === 2) return `grand${word}`;
  if (steps === 3) return `great-grand${word}`;
  return `${steps - 2}x great-grand${word}`;
}

/** The label for `rel`; `sex` is the sex of the person being labelled. */
export function relationLabel(rel: Rel, sex: SexKind): string {
  switch (rel.t) {
    case 'focus':
      return 'starting person';
    case 'ancestor':
      return generations(rel.n, 'father', 'mother', 'parent', sex);
    case 'descendant':
      return generations(rel.n, 'son', 'daughter', 'child', sex);
    case 'sibling': {
      const w = pick(sex, 'brother', 'sister', 'sibling');
      return rel.half ? `half ${w}` : w;
    }
    case 'aunt': {
      const w = pick(sex, 'uncle', 'aunt', 'aunt or uncle');
      return rel.half ? `half ${w}` : w;
    }
    case 'cousin':
      return 'cousin';
    case 'nibling':
      return pick(sex, 'nephew', 'niece', 'nephew or niece');
    case 'partner':
      if (rel.of === 'focus') return 'partner';
      if (rel.of === 'sibling') return "sibling's partner";
      if (rel.of === 'aunt') return "aunt or uncle's partner";
      return `${generations(rel.of, 'child', 'child', 'child', 'unknown')}'s partner`;
  }
}
