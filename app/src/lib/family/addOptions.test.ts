import { describe, expect, it } from 'vitest';
import type { PersonRef } from '../api/types';
import { addOptions, parentFacts } from './addOptions';

const p = { id: 'p' } as PersonRef;
const set = (a: PersonRef | null, b: PersonRef | null, preferred = true) => ({ familyId: 'f', parents: [a, b] as [PersonRef | null, PersonRef | null], rel: ['biological', 'biological'] as never, preferred, summary: null });

describe('addOptions', () => {
  it('a person without parents can add either parent, but no sibling', () => {
    const o = addOptions(parentFacts({ parentSets: [] }));
    expect(o.map((x) => x.relation)).toEqual(['father', 'mother', 'partner', 'child', 'sibling']);
    expect(o.filter((x) => x.disabled).map((x) => x.relation)).toEqual(['sibling']);
    expect(o[4].hint).toBe('family.addParentFirst');
  });

  it('a recorded father disables Add father with a hint, and enables a sibling', () => {
    const o = addOptions(parentFacts({ parentSets: [set(p, null)] }));
    expect(o[0]).toMatchObject({ relation: 'father', disabled: true, hint: 'family.fatherRecorded' });
    expect(o[1]).toMatchObject({ relation: 'mother', disabled: false });
    expect(o[4].disabled).toBe(false);
  });

  it('a recorded mother disables Add mother', () => {
    const o = addOptions(parentFacts({ parentSets: [set(null, p)] }));
    expect(o[1]).toMatchObject({ disabled: true, hint: 'family.motherRecorded' });
  });

  it('with both parents the choice becomes another set of parents', () => {
    const o = addOptions(parentFacts({ parentSets: [set(p, p)] }));
    expect(o.map((x) => x.label)).toEqual(['family.addAnotherParents', 'edit.addPartner', 'edit.addChild', 'edit.addSibling']);
    expect(o[0]).toMatchObject({ relation: 'father', newParentSet: true, disabled: false });
  });

  it('the preferred set decides, not the first', () => {
    const f = parentFacts({ parentSets: [set(p, p, false), set(null, null, true)] });
    expect(f).toEqual({ fatherRecorded: false, motherRecorded: false, hasAnyParent: true });
  });
});
