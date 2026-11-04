import type { RoleDto, SexKind } from '../types';

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
      if (rel.of === 'sibling') return pick(sex, 'brother-in-law', 'sister-in-law', 'in-law');
      if (rel.of === 'aunt') return pick(sex, 'uncle by marriage', 'aunt by marriage', 'in-law');
      return sex === 'male' || sex === 'female' ? `${generations(rel.of, 'son', 'daughter', 'child', sex)}-in-law` : 'in-law';
  }
}

/** `rel` as the structured role the real backend sends next to the label. */
export function relationRole(rel: Rel, sex: SexKind): RoleDto {
  const r: RoleDto = { kind: 'focus', n: 0, half: false, sex, k: 0 };
  switch (rel.t) {
    case 'ancestor':
      return { ...r, kind: 'ancestor', n: rel.n };
    case 'descendant':
      return { ...r, kind: 'descendant', n: rel.n };
    case 'sibling':
      return { ...r, kind: 'sibling', half: rel.half };
    case 'aunt':
      return { ...r, kind: 'aunt_uncle', half: rel.half };
    case 'nibling':
      return { ...r, kind: 'niece_nephew' };
    case 'cousin':
      return { ...r, kind: 'cousin' };
    case 'partner':
      if (rel.of === 'focus') return { ...r, kind: 'partner' };
      if (rel.of === 'sibling') return { ...r, kind: 'partner_of_sibling' };
      if (rel.of === 'aunt') return { ...r, kind: 'partner_of_aunt_uncle' };
      return { ...r, kind: 'partner_of_descendant', n: rel.of };
    default:
      return r;
  }
}
