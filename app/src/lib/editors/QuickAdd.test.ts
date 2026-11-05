import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import QuickAdd from './QuickAdd.svelte';
import { findPerson, newApi, renderWithApi } from './testing/helpers';
import type { MockApi } from '../api/mock/MockApi';
import type { PersonDetail } from '../api/types';

let api: MockApi;
let multi: PersonDetail;
let single: PersonDetail;

beforeEach(async () => {
  api = newApi(500);
  multi = await findPerson(api, (d) => d.partnerships.length > 1);
  single = await findPerson(api, (d) => d.partnerships.length === 1 && d.parentSets.length === 0 && d.names[0].surnames.length > 0);
});

function show(d: PersonDetail, relation: string, extra: Record<string, unknown> = {}) {
  const onDone = vi.fn();
  const onCancel = vi.fn();
  renderWithApi(api, QuickAdd, { anchor: d.ref, relation, onDone, onCancel, ...extra });
  return { onDone, onCancel };
}

describe('QuickAdd', () => {
  it('father_prefills_surname_and_sex', async () => {
    show(single, 'father');
    expect(screen.getByRole('dialog', { name: `Add father of ${single.ref.name}` })).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: 'Surname(s)' })).toHaveValue(single.names[0].surnames.join(', ')),
    );
    expect(screen.getByRole('combobox', { name: 'Sex' })).toHaveValue('male');
  });

  it('mother_prefills_female', async () => {
    show(single, 'mother');
    expect(screen.getByRole('combobox', { name: 'Sex' })).toHaveValue('female');
    expect(screen.getByRole('textbox', { name: 'Surname(s)' })).toHaveValue('');
  });

  it('child_with_two_partnerships_requires_choice', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    show(multi, 'child');
    const sel = await screen.findByRole('combobox', { name: 'With which partner?' });
    await user.type(screen.getByRole('textbox', { name: 'Given name' }), 'Neu');
    const save = screen.getByRole('button', { name: 'Add child' });
    expect(save).toBeDisabled();
    const fam = multi.partnerships[1].familyId;
    await user.selectOptions(sel, fam);
    expect(save).toBeEnabled();
    await user.click(save);
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(spy.mock.calls[0][0]).toMatchObject({ type: 'quickAdd', relation: 'child', family: fam });
  });

  it('sibling_without_parents_creates_command_with_no_family', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    const d = await findPerson(api, (x) => x.parentSets.length === 0);
    show(d, 'sibling');
    await user.type(screen.getByRole('textbox', { name: 'Given name' }), 'Geschwister');
    await user.click(screen.getByRole('button', { name: 'Add sibling' }));
    await waitFor(() => expect(spy).toHaveBeenCalled());
    const cmd = spy.mock.calls[0][0] as Record<string, unknown>;
    expect(cmd.relation).toBe('sibling');
    expect('family' in cmd).toBe(false);
  });

  it('submit_with_enter', async () => {
    const user = userEvent.setup();
    const { onDone } = show(single, 'partner');
    await user.type(screen.getByRole('textbox', { name: 'Given name' }), 'Enter{Enter}');
    await waitFor(() => expect(onDone).toHaveBeenCalledTimes(1));
  });

  it('empty_name_allowed_but_warns', async () => {
    const user = userEvent.setup();
    const before = (await api.getPerson(single.ref.id)).partnerships.length;
    const { onDone } = show(single, 'partner');
    expect(screen.getByText('No name entered: saved as Unknown')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Add partner' }));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    const r = onDone.mock.calls[0][0] as { focus: string };
    expect((await api.getPerson(r.focus)).ref.name).toBe('Unknown');
    expect((await api.getPerson(single.ref.id)).partnerships.length).toBe(before + 1);
  });

  it('surname_only_is_dirty_and_esc_asks', async () => {
    const user = userEvent.setup();
    show(single, 'mother');
    await user.type(screen.getByRole('textbox', { name: 'Surname(s)' }), 'Neumann');
    await user.keyboard('{Escape}');
    expect(screen.getByText('Discard changes?')).toBeInTheDocument();
  });

  /** A child whose birth names only the father, while the father has a wife: what an import can leave behind. */
  async function fatherOnly() {
    const dad = (await api.apply({ type: 'createPerson', draft: { given: 'Petro', surnames: ['Koval'], sex: 'male' } })).focus!;
    const kid = (await api.apply({ type: 'quickAdd', anchor: dad, relation: 'child', draft: { given: 'Ivan', surnames: ['Koval'], sex: 'male' } })).focus!;
    const wife = (await api.apply({ type: 'quickAdd', anchor: dad, relation: 'partner', draft: { given: 'Hanna', surnames: ['Koval'], sex: 'female' } })).focus!;
    return { dad, kid: await api.getPerson(kid), wife };
  }

  it('an_existing_person_can_be_linked_and_the_fathers_wife_is_suggested_as_mother', async () => {
    const user = userEvent.setup();
    const { dad, kid, wife } = await fatherOnly();
    const { onDone } = show(kid, 'mother');
    expect(screen.getByRole('button', { name: 'Add mother' })).toBeEnabled();
    await user.click(screen.getByRole('radio', { name: 'Someone already in the tree' }));
    expect(screen.queryByRole('textbox', { name: 'Given name' })).toBeNull();
    // Nothing picked yet: nothing to save.
    expect(screen.getByRole('button', { name: 'Add mother' })).toBeDisabled();
    await user.click(await screen.findByRole('button', { name: /^Hanna Koval/ }));
    await user.click(screen.getByRole('button', { name: 'Add mother' }));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    const sets = (await api.getPerson(kid.ref.id)).parentSets;
    expect(sets).toHaveLength(1);
    expect(sets[0].parents.map((p) => p?.id).sort()).toEqual([dad, wife].sort());
    // The couple is recorded once: the father-only family went away.
    expect((await api.getPerson(dad)).partnerships).toHaveLength(1);
  });

  it('the_person_search_finds_someone_already_in_the_tree', async () => {
    const user = userEvent.setup();
    const { kid } = await fatherOnly();
    const other = (await api.apply({ type: 'createPerson', draft: { given: 'Zinovia', surnames: ['Ostapenko'], sex: 'female' } })).focus!;
    const { onDone } = show(kid, 'mother');
    await user.click(screen.getByRole('radio', { name: 'Someone already in the tree' }));
    await user.type(screen.getByRole('combobox', { name: /Find the person/ }), 'Zinovia');
    await user.click(await screen.findByRole('option', { name: /Zinovia Ostapenko/ }));
    await user.click(screen.getByRole('button', { name: 'Add mother' }));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    const set = (await api.getPerson(kid.ref.id)).parentSets[0];
    expect(set.parents.some((p) => p?.id === other)).toBe(true);
  });

  it('a_stepfather_joins_the_mother_and_the_birth_parents_stay', async () => {
    const user = userEvent.setup();
    const dad = (await api.apply({ type: 'createPerson', draft: { given: 'Petro', surnames: ['Koval'], sex: 'male' } })).focus!;
    const mum = (await api.apply({ type: 'quickAdd', anchor: dad, relation: 'partner', draft: { given: 'Maria', surnames: ['Koval'], sex: 'female' } })).focus!;
    const fam = (await api.getPerson(dad)).partnerships[0].familyId;
    const kid = (await api.apply({ type: 'quickAdd', anchor: dad, relation: 'child', family: fam, draft: { given: 'Ivan', surnames: ['Koval'], sex: 'male' } })).focus!;
    const { onDone } = show(await api.getPerson(kid), 'father', { parentRel: 'step' });
    expect(screen.getByRole('dialog', { name: /^Step father of Ivan Koval/ })).toBeInTheDocument();
    await user.type(screen.getByRole('textbox', { name: 'Given name' }), 'Stepan');
    await user.click(screen.getByRole('button', { name: 'Add: Step father' }));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    const sets = (await api.getPerson(kid)).parentSets;
    expect(sets).toHaveLength(2);
    const birth = sets.find((s) => s.preferred)!;
    expect(birth.parents.map((x) => x?.id)).toContain(dad);
    const step = sets.find((s) => !s.preferred)!;
    expect(step.parents.map((x) => x?.id)).toContain(mum);
    const slot = step.parents.findIndex((x) => x?.id !== mum);
    expect(step.rel[slot]).toBe('step');
    expect(step.rel[1 - slot]).toBe('birth');
  });

  it('the_relationship_can_be_chosen', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    const { kid } = await fatherOnly();
    show(kid, 'mother');
    await user.selectOptions(screen.getByRole('combobox', { name: 'How are they related?' }), 'adopted');
    await user.type(screen.getByRole('textbox', { name: 'Given name' }), 'Nina');
    await user.click(screen.getByRole('button', { name: 'Add: Adopted mother' }));
    await waitFor(() => expect(spy).toHaveBeenLastCalledWith(expect.objectContaining({ type: 'quickAdd', relation: 'mother', parentRel: 'adopted' })));
  });

  it('a_partner_has_no_relationship_choice', () => {
    show(single, 'partner');
    expect(screen.queryByRole('combobox', { name: 'How are they related?' })).toBeNull();
  });

  it('cancel_does_not_call_api', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    const { onCancel } = show(single, 'child');
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onCancel).toHaveBeenCalled();
    expect(spy).not.toHaveBeenCalled();
  });
});
