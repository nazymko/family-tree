import { beforeEach, describe, expect, it } from 'vitest';
import { setLanguage } from '.';
import { ahnenText, nodeLabel, outlineLabel, relationText, roleText } from './relation';
import type { ChartNode, OutlineItem, RoleDto } from '../api/types';

const role = (kind: RoleDto['kind'], sex: RoleDto['sex'], over: Partial<RoleDto> = {}): RoleDto => ({ kind, n: 1, half: false, sex, k: 0, ...over });

describe('relationship words in Ukrainian', () => {
  beforeEach(() => setLanguage('uk'));

  it('names_close_family_by_sex', () => {
    expect(roleText(role('ancestor', 'male'))).toBe('батько');
    expect(roleText(role('ancestor', 'female'))).toBe('мати');
    expect(roleText(role('ancestor', 'male', { n: 2 }))).toBe('дідусь');
    expect(roleText(role('ancestor', 'female', { n: 2 }))).toBe('бабуся');
    expect(roleText(role('ancestor', 'male', { n: 3 }))).toBe('прадідусь');
    expect(roleText(role('descendant', 'female', { n: 2 }))).toBe('онука');
    expect(roleText(role('descendant', 'male', { n: 3 }))).toBe('правнук');
    expect(roleText(role('sibling', 'male'))).toBe('брат');
    expect(roleText(role('sibling', 'female'))).toBe('сестра');
  });

  it('names_distant_relatives_by_generations_up_and_down', () => {
    const kin = (sex: RoleDto['sex'], up: number, down: number) => roleText(role('kin', sex, { n: up, k: down }));
    expect(kin('male', 3, 1)).toBe('двоюрідний дідусь');
    expect(kin('female', 3, 1)).toBe('двоюрідна бабуся');
    expect(kin('male', 4, 1)).toBe('двоюрідний прадідусь');
    expect(kin('female', 2, 2)).toBe('двоюрідна сестра');
    expect(kin('male', 3, 3)).toBe('троюрідний брат');
    expect(kin('male', 1, 3)).toBe('внучатий племінник');
    expect(kin('male', 3, 2)).toBe('двоюрідний дядько');
    expect(kin('female', 2, 3)).toBe('двоюрідна племінниця');
    // No exact word: the general one, never a wrong one.
    expect(kin('male', 9, 1)).toBe('родич');
    expect(roleText(role('partner_of_kin', 'female', { n: 2, k: 2 }))).toBe('родич за шлюбом');
  });

  it('names_half_siblings_aunts_uncles_and_cousins', () => {
    expect(roleText(role('sibling', 'male', { half: true }))).toBe('зведений брат');
    expect(roleText(role('sibling', 'female', { half: true }))).toBe('зведена сестра');
    expect(roleText(role('aunt_uncle', 'male'))).toBe('дядько');
    expect(roleText(role('aunt_uncle', 'female'))).toBe('тітка');
    expect(roleText(role('niece_nephew', 'female'))).toBe('племінниця');
    expect(roleText(role('cousin', 'male'))).toBe('двоюрідний брат');
    expect(roleText(role('cousin', 'female'))).toBe('двоюрідна сестра');
  });

  it('names_in_laws', () => {
    expect(roleText(role('partner_of_descendant', 'male'))).toBe('зять');
    expect(roleText(role('partner_of_descendant', 'female'))).toBe('невістка');
    expect(roleText(role('partner_of_descendant', 'unknown'))).toBe('родич по шлюбу');
    expect(roleText(role('partner_of_sibling', 'female'))).not.toBe('');
    expect(roleText(role('partner_of_aunt_uncle', 'male'))).not.toBe('');
  });

  it('counts_far_generations_with_a_multiplier', () => {
    expect(roleText(role('ancestor', 'male', { n: 5 }))).toBe('3× прадідусь');
    expect(roleText(role('descendant', 'female', { n: 5 }))).toContain('3×');
  });

  it('spells_out_ahnentafel_chains', () => {
    expect(ahnenText(3)).toBe('мати');
    expect(ahnenText(6)).toBe('батько матері');
  });

  it('uses_the_same_structured_role_in_english', () => {
    setLanguage('en');
    expect(roleText(role('sibling', 'female', { half: true }))).toBe('half sister');
    expect(roleText(role('cousin', 'male'))).toBe('cousin');
  });

  it('builds_screen_reader_and_outline_labels_from_the_role', () => {
    const node = {
      id: 1,
      person: { id: 'p1', name: 'Олена Петренко', lifespan: '1950–2010' },
      role: role('sibling', 'female'),
      dupOf: null,
      label: 'ignored English label',
    } as unknown as ChartNode;
    expect(nodeLabel(node)).toBe('Олена Петренко, 1950–2010, сестра');
    expect(nodeLabel({ ...node, dupOf: 3 })).toContain('з’являється раніше');
    // A pedigree or descendants person also gets how they are related to the starting person.
    const ancestorNo = { ...node, role: role('ancestor_no', 'male', { n: 2 }), kinship: { kind: 'blood', up: 1, down: 0, half: false, married: false, sex: 'male', via: 'unknown' } } as unknown as ChartNode;
    expect(nodeLabel(ancestorNo)).toBe('Олена Петренко, 1950–2010, предок № 2, батько');
    // The relatives chart already says it in the role.
    expect(nodeLabel({ ...node, kinship: { kind: 'blood', up: 1, down: 1, half: false, married: false, sex: 'female', via: 'unknown' } } as ChartNode)).toBe('Олена Петренко, 1950–2010, сестра');
    const item = { nodeId: 1, person: null, label: 'English', hasChildren: false, appearsEarlier: false, alsoAbove: true, role: role('sibling', 'male') } as OutlineItem;
    expect(outlineLabel(item)).toMatch(/^Брат \(/);
    expect(outlineLabel({ ...item, role: undefined, group: 'siblings' })).toBe('Брати й сестри');
    expect(relationText({ role: role('aunt_uncle', 'female'), relation: 'aunt' })).toBe('тітка');
    expect(relationText({ relation: 'fallback' })).toBe('fallback');
  });
});
