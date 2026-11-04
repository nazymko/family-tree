import { t, type MessageKey } from '../i18n';
import type { Kinship, SexKind } from '../api/types';

/**
 * Words for how one person is related to another, in the active language, built from the structured
 * `Kinship` the backend sends. The texts live in the `kinship.*` catalogue entries.
 */

type S = 'M' | 'F' | 'N';
const sexOf = (sex: SexKind): S => (sex === 'male' ? 'M' : sex === 'female' ? 'F' : 'N');
const word = (base: string, sex: SexKind, params?: Record<string, string | number>) => t(`kinship.${base}${sexOf(sex)}` as MessageKey, params);

/** Longest word table (generations) before a relation is called just "relative". */
const MAX_DEGREE = 7;

/** Up the line: parent, grandparent, great-grandparent, then "3× great-grandfather". */
function ancestor(n: number, sex: SexKind): string {
  if (n <= 1) return word('parent', sex);
  if (n === 2) return word('grand', sex);
  if (n === 3) return word('great', sex);
  if (n === 4) return word('great2', sex);
  return word('far', sex, { m: n - 2 });
}

function descendant(n: number, sex: SexKind): string {
  if (n <= 1) return word('child', sex);
  if (n === 2) return word('grandchild', sex);
  if (n === 3) return word('greatChild', sex);
  if (n === 4) return word('great2Child', sex);
  return word('farChild', sex, { m: n - 2 });
}

function removed(r: number): string {
  return r === 2 ? t('kinship.removed2') : t('kinship.removedN', { r });
}

function bloodText(up: number, down: number, half: boolean, sex: SexKind): string {
  let text: string | null;
  let canHalf = false;
  if (up === 0 && down === 0) return t('kinship.self');
  if (down === 0) text = ancestor(up, sex);
  else if (up === 0) text = descendant(down, sex);
  else if (up === 1 && down === 1) {
    text = word('sibling', sex);
    canHalf = true;
  } else if (up === 1) {
    // Children of a sibling, then their children, ...
    text = down === 2 ? word('nibling', sex) : down === 3 ? word('grandNibling', sex) : down === 4 ? word('great2Nibling', sex) : null;
    canHalf = true;
  } else if (down === 1) {
    text = up === 2 ? word('auntUncle', sex) : up === 3 ? word('grandAuntUncle', sex) : up === 4 ? word('great2AuntUncle', sex) : null;
    canHalf = true;
  } else {
    const k = Math.min(up, down) - 1;
    const r = Math.abs(up - down);
    if (k > MAX_DEGREE) return t('kinship.relative');
    const deg = word(`deg${k}`, sex);
    if (r === 0) text = word('cousin', sex, { deg });
    else if (r === 1) text = word(up > down ? 'cousinUp' : 'cousinDown', sex, { deg });
    else return t('kinship.cousinFar', { deg: word(`deg${k}`, 'male'), removed: removed(r) });
    canHalf = true;
  }
  if (text === null) return t('kinship.relative');
  return half && canHalf ? word('half', sex, { t: text }) : text;
}

/** Husband or wife of the partner a relation goes through, from the partner's sex. */
function viaKey(via: SexKind): 'wife' | 'husband' | 'partner' {
  return via === 'female' ? 'wife' : via === 'male' ? 'husband' : 'partner';
}

function partnerOfRelative(k: Kinship): string {
  const { up, down, sex, via } = k;
  // A parent's partner who is not linked as this person's parent: a stepparent, or a parent whose link is
  // missing (common in imported files where a birth record names one parent). Say only what the data shows.
  if (up === 1 && down === 0) {
    if (k.married && via === 'male' && sex === 'female') return t('kinship.fathersWife');
    if (k.married && via === 'female' && sex === 'male') return t('kinship.mothersHusband');
    return t(via === 'male' ? 'kinship.fathersPartner' : via === 'female' ? 'kinship.mothersPartner' : 'kinship.parentsPartner');
  }
  if (up === 0 && down === 1) return word('childInLaw', sex);
  if (up === 0 && down === 2) return word('grandchildInLaw', sex);
  if (up === 0 && down === 3) return word('greatChildInLaw', sex);
  if (up === 1 && down === 1) {
    // A brother's wife or a sister's husband; two women or two men have no common word.
    if (via === 'male' && sex === 'female') return word('siblingInLaw', sex);
    if (via === 'female' && sex === 'male') return word('siblingInLaw', sex);
    if (via !== 'male' && via !== 'female') return word('siblingInLaw', sex);
    return t('kinship.byMarriage');
  }
  if (up === 2 && down === 1) return word('auntUncleByMarriage', sex);
  return t('kinship.byMarriage');
}

function relativeOfPartner(k: Kinship): string {
  const { up, down, sex, via } = k;
  const side = viaKey(via);
  if (up === 1 && down === 0) return word(`${side}Parent`, sex);
  if (up === 1 && down === 1) return word(`${side}Sibling`, sex);
  // A partner's child not linked to this person (stepchild, or own child with a missing link).
  if (up === 0 && down === 1) return word(`${side}Child`, sex);
  return t(side === 'wife' ? 'kinship.relOfWife' : side === 'husband' ? 'kinship.relOfHusband' : 'kinship.relOfPartner');
}

/** "brother", "half sister", "mother-in-law", "first cousin once removed"; empty for no relation. */
export function kinshipText(k: Kinship | null | undefined): string {
  if (!k) return '';
  switch (k.kind) {
    case 'self_person':
      return t('kinship.self');
    case 'blood':
      return bloodText(k.up, k.down, k.half, k.sex);
    case 'partner':
      return k.married ? word('spouse', k.sex) : word('partner', k.sex);
    case 'partner_of_relative':
      return partnerOfRelative(k);
    case 'relative_of_partner':
      return relativeOfPartner(k);
  }
}
