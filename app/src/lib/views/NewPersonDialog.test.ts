import { describe, expect, it, vi } from 'vitest';
import { screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import NewPersonDialog from './NewPersonDialog.svelte';
import NameEditor from '../editors/NameEditor.svelte';
import { newApi, renderWithApi } from '../editors/testing/helpers';

const FATHERS_NAME = 'Father’s name (patronymic)';

describe('NewPersonDialog', () => {
  it('sends_given_name_surname_and_fathers_name_as_separate_parts', async () => {
    const user = userEvent.setup();
    const api = newApi(10);
    const spy = vi.spyOn(api, 'apply');
    const ondone = vi.fn();
    renderWithApi(api, NewPersonDialog, { open: true, onclose: vi.fn(), ondone });
    await user.type(screen.getByRole('textbox', { name: 'Given names' }), 'Ivan');
    await user.type(screen.getByRole('textbox', { name: 'Surname' }), 'Koval');
    await user.type(screen.getByRole('textbox', { name: FATHERS_NAME }), 'Petrovych');
    await user.click(screen.getByRole('button', { name: 'Add person' }));
    await waitFor(() => expect(ondone).toHaveBeenCalled());
    expect(spy.mock.calls[0][0]).toMatchObject({ type: 'createPerson', draft: { given: 'Ivan', surnames: ['Koval'], patronymic: 'Petrovych' } });
    expect((await api.getPerson(ondone.mock.calls[0][0] as string)).ref.name).toBe('Ivan Petrovych Koval');
  });

  it('fathers_name_is_optional', async () => {
    const user = userEvent.setup();
    const api = newApi(10);
    const spy = vi.spyOn(api, 'apply');
    const ondone = vi.fn();
    renderWithApi(api, NewPersonDialog, { open: true, onclose: vi.fn(), ondone });
    await user.type(screen.getByRole('textbox', { name: 'Given names' }), 'Olena');
    await user.click(screen.getByRole('button', { name: 'Add person' }));
    await waitFor(() => expect(ondone).toHaveBeenCalled());
    const cmd = spy.mock.calls[0][0] as unknown as { draft: Record<string, unknown> };
    expect(cmd.draft.patronymic).toBeUndefined();
  });

  it('the_name_editor_shows_and_saves_the_fathers_name_added_here', async () => {
    const user = userEvent.setup();
    const api = newApi(10);
    const id = (await api.apply({ type: 'createPerson', draft: { given: 'Ivan', surnames: ['Koval'], patronymic: 'Petrovych', sex: 'male' } })).focus!;
    const name = (await api.getPerson(id)).names[0];
    const onSaved = vi.fn();
    renderWithApi(api, NameEditor, { person: id, name, onSaved, onCancel: vi.fn() });
    const field = screen.getByRole('textbox', { name: FATHERS_NAME });
    expect(field).toHaveValue('Petrovych');
    await user.clear(field);
    await user.type(field, 'Stepanovych');
    await user.click(screen.getByRole('button', { name: 'Save name' }));
    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect((await api.getPerson(id)).ref.name).toBe('Ivan Stepanovych Koval');
  });
});
