import { describe, expect, it } from 'vitest';
import { MockApi } from './MockApi';
import { fixtureTree } from './fixtures';

describe('MockApi edit semantics (Task 21)', () => {
  const draft = { given: 'New', surnames: ['Person'], sex: 'unknown' as const };
  const fresh = () => new MockApi(fixtureTree(500));
  const specials = fixtureTree(500).specials;
  const q = { sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false } as const;

  it('quick_add_father_without_parents_creates_family_and_preferred_link', async () => {
    const api = fresh();
    const orphan = specials.undated!;
    const res = await api.apply({ type: 'quickAdd', anchor: orphan, relation: 'father', draft });
    expect(res.label).toBe('Add father');
    const view = await api.getFamilyView(orphan);
    expect(view.parentSets).toHaveLength(1);
    expect(view.parentSets[0].parents[0]?.id).toBe(res.focus);
    expect(view.parentSets[0].parents[0]?.sex).toBe('male');
    expect(view.parentSets[0].preferred).toBe(true);
  });

  it('quick_add_father_when_full_conflicts_unless_new_parent_set', async () => {
    const api = fresh();
    const home = (await api.getSettings()).homePerson!;
    const sets = (await api.getFamilyView(home)).parentSets;
    expect(sets[0].parents[0] && sets[0].parents[1]).toBeTruthy();
    await expect(api.apply({ type: 'quickAdd', anchor: home, relation: 'father', draft })).rejects.toMatchObject({ code: 'conflict' });
    await api.apply({ type: 'quickAdd', anchor: home, relation: 'father', draft, newParentSet: true });
    const after = (await api.getFamilyView(home)).parentSets;
    expect(after).toHaveLength(2);
    expect(after.filter((p) => p.preferred)).toHaveLength(1);
  });

  it('quick_add_child_multiple_families_requires_choice', async () => {
    const api = fresh();
    const many = specials.manyPartners!;
    await expect(api.apply({ type: 'quickAdd', anchor: many, relation: 'child', draft })).rejects.toMatchObject({ code: 'invalid' });
    const fam = (await api.getFamilyView(many)).partnerships[3].familyId;
    await api.apply({ type: 'quickAdd', anchor: many, relation: 'child', draft, family: fam });
    expect((await api.getFamilyView(many)).partnerships[3].children).toHaveLength(1);
  });

  it('quick_add_sibling_without_parents_creates_empty_family_with_both_children', async () => {
    const api = fresh();
    const u = specials.undated!;
    const res = await api.apply({ type: 'quickAdd', anchor: u, relation: 'sibling', draft });
    const view = await api.getFamilyView(u);
    expect(view.siblings.map((x) => x.id)).toEqual([res.focus]);
    expect(view.parentSets[0].parents).toEqual([null, null]);
  });

  it('quick_add_creates_birth_event_with_place_chain_reused', async () => {
    const api = fresh();
    const anchor = (await api.listPersons(q, 0, 1)).items[0];
    const d = { ...draft, birthDateRaw: '3 Apr 1901', birthPlaceText: 'Kleinstadt, Neuland, Nordreich' };
    const a = await api.apply({ type: 'quickAdd', anchor: anchor.id, relation: 'child', draft: d });
    expect((await api.suggestPlaces('Neuland', 10)).map((p) => p.full)).toContain('Neuland, Nordreich');
    const b = await api.apply({ type: 'quickAdd', anchor: anchor.id, relation: 'sibling', draft: d });
    const pa = (await api.getPerson(a.focus!)).events[0].place!;
    const pb = (await api.getPerson(b.focus!)).events[0].place!;
    expect(pa.id).toBe(pb.id);
    expect(await api.suggestPlaces('Kleinstadt', 10)).toHaveLength(1);
  });

  it('self_child_and_same_partner_rejected', async () => {
    const api = fresh();
    const many = specials.manyPartners!;
    const fam = (await api.getFamilyView(many)).partnerships[0].familyId;
    await expect(api.apply({ type: 'linkChild', family: fam, child: many, rel: ['birth', 'birth'], preferred: true })).rejects.toMatchObject({ code: 'invalid' });
    await expect(api.apply({ type: 'saveFamily', partner1: many, partner2: many, kind: 'married' })).rejects.toMatchObject({ code: 'invalid' });
  });

  it('delete_person_cleans_family_slots_and_undo_restores', async () => {
    const api = fresh();
    const home = (await api.getSettings()).homePerson!;
    const parent = (await api.getFamilyView(home)).parentSets[0].parents[0]!;
    await api.apply({ type: 'deletePerson', id: parent.id });
    expect((await api.getFamilyView(home)).parentSets[0].parents[0]).toBeNull();
    await api.undo();
    expect((await api.getFamilyView(home)).parentSets[0].parents[0]?.id).toBe(parent.id);
  });

  it('reuse_last_citation_copies_with_new_id', async () => {
    const api = fresh();
    const p = (await api.listPersons(q, 0, 1)).items[0];
    const target = { kind: 'person', id: p.id } as const;
    await expect(api.apply({ type: 'addCitation', target, citation: { reuseLast: true } })).rejects.toMatchObject({ code: 'invalid' });
    await api.apply({ type: 'addCitation', target, citation: { source: { title: 'Fresh source' }, page: '1' } });
    await api.apply({ type: 'addCitation', target, citation: { reuseLast: true } });
    const cits = (await api.getPerson(p.id)).citations;
    expect(cits).toHaveLength(2);
    expect(cits[0].id).not.toBe(cits[1].id);
    expect((await api.lastCitation())?.page).toBe('1');
    expect((await api.suggestSources('fresh', 5))[0].citationCount).toBe(2);
  });

  it('reorder_children_changes_only_order', async () => {
    const api = fresh();
    const many = specials.manyPartners!;
    const fam = (await api.getFamilyView(many)).partnerships[0].familyId;
    for (const g of ['A', 'B', 'C']) await api.apply({ type: 'quickAdd', anchor: many, relation: 'child', family: fam, draft: { ...draft, given: g } });
    const kids = (await api.getFamilyView(many)).partnerships[0].children.map((c) => c.person.id);
    await api.apply({ type: 'reorderChildren', family: fam, order: [...kids].reverse() });
    expect((await api.getFamilyView(many)).partnerships[0].children.map((c) => c.person.id)).toEqual([...kids].reverse());
    await expect(api.apply({ type: 'reorderChildren', family: fam, order: kids.slice(1) })).rejects.toMatchObject({ code: 'invalid' });
    expect((await api.history(fam, 10))[0].label).toBe('Reorder children');
  });

  it('attach_files_dedups_and_rejects_bad_names', async () => {
    const api = fresh();
    const p = (await api.listPersons(q, 0, 1)).items[0];
    const t = { kind: 'person', id: p.id } as const;
    const r = await api.attachFiles(t, ['C:/x/a.jpg', 'C:/x/run.exe', 'C:/x/folder', 'C:/x/a.jpg']);
    expect(r.attached).toHaveLength(1);
    expect(r.failed.map((f) => f.reason)).toEqual(expect.arrayContaining(['This file type cannot be attached.', 'Already attached']));
    const again = await api.attachFiles(t, ['D:/other/A.JPG']);
    expect(again.attached).toHaveLength(0);
    expect(api.mediaUrl(r.attached[0].id, 'thumb')).toMatch(/^data:image\/svg\+xml/);
  });
});
