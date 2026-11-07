import type { RoleDto, SexKind } from '../types';

// How a person on the relatives chart is related to the centred one. Mirrors crates/gt-session/src/relation.rs:
// all the chart's relationship wording is in these two files.

/** Whose partner a partner box is: the centred person, a sibling, a parent's sibling, or a descendant n generations down. */
export type PartnerOf = 'focus' | 'sibling' | 'aunt' | number | { up: number; down: number };

export type Rel =
  | { t: 'focus' }
  | { t: 'ancestor'; n: number }
  | { t: 'descendant'; n: number }
  | { t: 'sibling'; half: boolean }
  | { t: 'aunt'; half: boolean }
  | { t: 'nibling' }
  | { t: 'cousin' }
  /** A blood relative on the all-relatives chart: `up` generations to the common ancestor, `down` from it. */
  | { t: 'kin'; up: number; down: number }
  | { t: 'partner'; of: PartnerOf };

/** English words for a blood relative `up` above the common ancestor and `down` below it (plain-text fallback). */
function kinLabel(up: number, down: number, sex: SexKind): string {
  const min = Math.min(up, down);
  if (min === 1) {
    if (up > down) {
      const w = pick(sex, 'uncle', 'aunt', 'aunt or uncle');
      return up === 2 ? w : up === 3 ? `great-${w}` : `${up - 2}x great-${w}`;
    }
    const w = pick(sex, 'nephew', 'niece', 'nephew or niece');
    return down === 2 ? w : down === 3 ? `grand${w}` : `${down - 3}x great-grand${w}`;
  }
  const degree = ['', 'first', 'second', 'third'][min - 1] ?? `${min - 1}th`;
  const diff = up - down;
  return diff === 0 ? `${degree} cousin` : `${degree} cousin, ${Math.abs(diff)} removed`;
}

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
    case 'kin':
      return kinLabel(rel.up, rel.down, sex);
    case 'nibling':
      return pick(sex, 'nephew', 'niece', 'nephew or niece');
    case 'partner':
      if (typeof rel.of === 'object') return 'relative by marriage';
      if (rel.of === 'focus') return 'partner';
      if (rel.of === 'sibling') return pick(sex, 'brother-in-law', 'sister-in-law', 'in-law');
      if (rel.of === 'aunt') return pick(sex, 'uncle by marriage', 'aunt by marriage', 'in-law');
      return sex === 'male' || sex === 'female' ? `${generations(rel.of, 'son', 'daughter', 'child', sex)}-in-law` : 'in-law';
  }
}

/** `rel` as the structured role the real backend sends next to the label. */
export function relationRole(rel: Rel, sex: SexKind): RoleDto {
  const r: RoleDto = { kind: 'focus', n: 0, half: false, sex, k: 0, married: false };
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
    case 'kin':
      return { ...r, kind: 'kin', n: rel.up, k: rel.down };
    case 'partner':
      if (typeof rel.of === 'object') return { ...r, kind: 'partner_of_kin', n: rel.of.up, k: rel.of.down };
      if (rel.of === 'focus') return { ...r, kind: 'partner' };
      if (rel.of === 'sibling') return { ...r, kind: 'partner_of_sibling' };
      if (rel.of === 'aunt') return { ...r, kind: 'partner_of_aunt_uncle' };
      return { ...r, kind: 'partner_of_descendant', n: rel.of };
    default:
      return r;
  }
}
