import { t, type MessageKey } from '.';
import type { ChartNode, OutlineItem, RoleDto } from '../api/types';
import { bloodText, kinshipText } from '../family/kinship';
import { birthSurnameText } from '../ui/birthSurname';

/**
 * Wording of how a chart person is related to the centred one, built from the structured `role` the backend sends
 * (kind + number of generations + sex), so the words come from the catalogue instead of backend English.
 */

const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

const sexSuffix = (sex: RoleDto['sex']): 'M' | 'F' | 'N' => (sex === 'male' ? 'M' : sex === 'female' ? 'F' : 'N');

const key = (base: string, sex: RoleDto['sex']) => `role.${base}${sexSuffix(sex)}` as MessageKey;

/** The generation word for an ancestor `n` steps up (1 parent, 2 grandparent, 3 great-grandparent, then "2x ..."). */
function ancestor(n: number, sex: RoleDto['sex']): string {
  if (n <= 1) return t(key('parent', sex));
  if (n === 2) return t(key('grand', sex));
  if (n === 3) return t(key('great', sex));
  return t(key('far', sex), { m: n - 2 });
}

/** Husband or wife for a marriage, partner otherwise (the neutral spouse words when the sex is unknown). */
function partnerText(r: RoleDto): string {
  return t(key(r.married ? 'spouse' : 'partner', r.sex));
}

function descendant(n: number, sex: RoleDto['sex']): string {
  if (n <= 1) return t(key('child', sex));
  if (n === 2) return t(key('grandchild', sex));
  if (n === 3) return t(key('greatChild', sex));
  return t(key('farChild', sex), { m: n - 2 });
}

function childInLaw(n: number, sex: RoleDto['sex']): string {
  if (n <= 1) return t(key('childInLaw', sex));
  if (n === 2) return t(key('grandchildInLaw', sex));
  if (n === 3) return t(key('greatChildInLaw', sex));
  return t(key('farChildInLaw', sex), { m: n - 2 });
}

/** "father's mother" for an Ahnentafel number: up to three steps by catalogue phrase, further out as a far ancestor. */
export function ahnenText(a: number): string {
  if (a <= 1) return t('role.selfPerson');
  const steps = Math.floor(Math.log2(a));
  if (steps <= 3) return t(`ahnen.n${a}` as MessageKey);
  return ancestor(steps, a & 1 ? 'female' : 'male');
}

/** The relationship in lower case ("brother", "half sister", "mother-in-law"), in the active language. */
export function roleText(r: RoleDto): string {
  switch (r.kind) {
    case 'self_person':
      return t('role.selfPerson');
    case 'focus':
      return t('role.focus');
    case 'ancestor':
      return ancestor(r.n, r.sex);
    case 'descendant':
      return descendant(r.n, r.sex);
    case 'sibling':
      return t(key(r.half ? 'halfSibling' : 'sibling', r.sex));
    case 'aunt_uncle':
      return t(key(r.half ? 'halfAuntUncle' : 'auntUncle', r.sex));
    case 'niece_nephew':
      return t(key('nibling', r.sex));
    case 'cousin':
      return t(key('cousin', r.sex));
    case 'kin':
      return bloodText(r.n, r.k, false, r.sex);
    case 'partner_of_kin':
      return t('kinship.byMarriage');
    case 'partner':
      return partnerText(r);
    case 'partner_of_sibling':
      return t(key('sibInLaw', r.sex));
    case 'partner_of_aunt_uncle':
      return t(key('auntInLaw', r.sex));
    case 'partner_of_descendant':
      return r.sex === 'male' || r.sex === 'female' ? childInLaw(r.n, r.sex) : t('role.childInLawN');
    case 'ancestor_no':
      return t('role.ancestorNo', { n: r.n });
    case 'descendant_no': {
      const k = r.k;
      if (r.n <= 1) return t('role.childNo', { k });
      if (r.n === 2) return t('role.grandchildNo', { k });
      if (r.n === 3) return t('role.greatChildNo', { k });
      return t('role.farChildNo', { m: r.n - 2, k });
    }
    case 'generation':
      return t('role.generation', { n: r.n });
  }
}

const RELATION_ROLES: ReadonlySet<RoleDto['kind']> = new Set(['ancestor', 'descendant', 'sibling', 'aunt_uncle', 'niece_nephew', 'cousin', 'kin', 'partner_of_kin', 'partner_of_sibling', 'partner_of_aunt_uncle', 'partner_of_descendant']);

/** The text a screen reader gets for a chart person: "Name, lifespan, role, appears earlier". */
export function nodeLabel(n: ChartNode): string {
  if (!n.role || !n.person) return n.label;
  const p = n.person;
  const head = [p.name, birthSurnameText(p), p.lifespan].filter(Boolean).join(', ');
  const tail = n.dupOf !== null ? `, ${t('role.appearsEarlier')}` : '';
  // The relation chart's role already says how the person is related; the others add it.
  const kin = n.kinship && !RELATION_ROLES.has(n.role.kind) ? kinshipText(n.kinship) : '';
  return `${head}, ${roleText(n.role)}${kin ? `, ${kin}` : ''}${tail}`;
}

const GROUPS: Record<NonNullable<OutlineItem['group']>, MessageKey> = {
  parents: 'role.groupParents',
  siblings: 'role.groupSiblings',
  partners: 'role.groupPartners',
  children: 'role.groupChildren',
  aunts_uncles: 'role.groupAuntsUncles',
};

/** One row of the chart outline, e.g. "Brother (also shown above)", "2 Father", "Parents". */
export function outlineLabel(it: OutlineItem): string {
  if (it.group) return t(GROUPS[it.group]);
  const r = it.role;
  if (!r) return it.label;
  let text: string;
  if (r.kind === 'ancestor_no') text = `${r.n} ${cap(ahnenText(r.n))}`;
  else if (r.kind === 'partner') text = cap(partnerText(r));
  else text = cap(roleText(r));
  // Pedigree and descendants rows: say how the person is related to the starting person unless the row already does.
  const kin = it.kinship ? kinshipText(it.kinship) : '';
  if (kin && !text.toLowerCase().includes(kin.toLowerCase())) text += `, ${kin}`;
  if (it.appearsEarlier) text += ` (${t('role.appearsEarlier')})`;
  if (it.alsoAbove) text += ` (${t('role.alsoAbove')})`;
  return text;
}

/** The short relation shown on a relatives box and in the breadcrumb, as plain lower-case words. */
export function relationText(n: { role?: RoleDto; relation?: string }): string {
  return n.role ? roleText(n.role) : (n.relation ?? '');
}
