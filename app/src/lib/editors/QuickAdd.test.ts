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
    const spy = vi.spyOn(api, 'apply').mockResolvedValue({
      label: 'Add partner',
      focus: null,
      touched: [],
      undo: { canUndo: true, canRedo: false, undoLabel: null, redoLabel: null },
    });
    const { onDone } = show(single, 'partner');
    expect(screen.getByText('No name entered: saved as Unknown')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Add partner' }));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(spy).toHaveBeenCalledTimes(1);
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
