import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import PersonEditor from './PersonEditor.svelte';
import { findPerson, newApi, renderWithApi } from '../editors/testing/helpers';
import { settings } from '../state/settings.svelte';
import type { MockApi } from '../api/mock/MockApi';

let api: MockApi;
let id: string;

beforeEach(async () => {
  api = newApi();
  id = (await findPerson(api, (d) => d.partnerships.length > 0 && d.names.length >= 1)).ref.id;
  await settings.load(api);
});

async function open() {
  renderWithApi(api, PersonEditor, { id });
  await screen.findByRole('heading', { level: 2, name: 'Names' });
}

describe('PersonEditor', () => {
  it('sex_change_commits_one_command', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    await open();
    await user.selectOptions(screen.getByRole('combobox', { name: 'Sex' }), 'female');
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
    expect(spy).toHaveBeenCalledWith({ type: 'updatePerson', id, sex: 'female' });
    expect(await screen.findByText('Sex saved')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Undo' })).toBeInTheDocument();
  });

  it('living_override_options_show_computed', async () => {
    await open();
    const sel = screen.getByRole('combobox', { name: 'Living status' });
    expect(within(sel).getByRole('option', { name: /^Computed \((living|deceased)\)$/ })).toBeInTheDocument();
    expect(within(sel).getByRole('option', { name: 'Treat as living' })).toBeInTheDocument();
    expect(within(sel).getByRole('option', { name: 'Treat as deceased' })).toBeInTheDocument();
  });

  it('name_dialog_roundtrip', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    await open();
    await user.click(screen.getByRole('button', { name: 'Add name' }));
    const dlg = screen.getByRole('dialog', { name: 'Add name' });
    await user.selectOptions(within(dlg).getByRole('combobox', { name: 'Kind of name' }), 'married');
    await user.type(within(dlg).getByRole('textbox', { name: 'Title or prefix' }), 'Dr');
    await user.type(within(dlg).getByRole('textbox', { name: 'Surnames' }), 'Müller, Schmidt');
    await user.click(within(dlg).getByRole('button', { name: 'Save name' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    const cmd = spy.mock.calls[0][0] as unknown as { type: string; person: string; name: Record<string, unknown> };
    expect(cmd.type).toBe('saveName');
    expect(cmd.person).toBe(id);
    expect(cmd.name).toMatchObject({ kind: 'married', prefix: 'Dr', surnames: ['Müller', 'Schmidt'] });
    expect(await screen.findByText('Name saved')).toBeInTheDocument();
  });

  it('discard_prompt_only_when_dirty', async () => {
    const user = userEvent.setup();
    await open();
    await user.click(screen.getByRole('button', { name: 'Add name' }));
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Add name' }));
    await user.type(screen.getByRole('textbox', { name: 'Given name' }), 'Zed');
    await user.keyboard('{Escape}');
    expect(screen.getByText('Discard changes?')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Discard changes' }));
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('delete_person_requires_confirmation_and_lists_consequences', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    const onDeleted = vi.fn();
    renderWithApi(api, PersonEditor, { id, onDeleted });
    await screen.findByRole('heading', { level: 2, name: 'Names' });
    const d = await api.getPerson(id);
    await user.click(screen.getByRole('button', { name: 'Delete person' }));
    expect(spy).not.toHaveBeenCalled();
    const n = d.partnerships.length + d.parentSets.length;
    expect(screen.getByRole('dialog').textContent).toMatch(
      new RegExp(`This removes .+ from ${n} famil(y|ies) and deletes \\d+ events? only they took part in\\.`),
    );
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Delete person' }));
    await waitFor(() => expect(onDeleted).toHaveBeenCalled());
    expect(spy).toHaveBeenCalledWith({ type: 'deletePerson', id });
  });

  it('make_home_person_sets_setting', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'setSettings');
    await open();
    await user.click(screen.getByRole('button', { name: /Make home person/ }));
    await waitFor(() => expect(spy).toHaveBeenCalledWith({ homePerson: id }));
  });

  it('note_markdown_saved_verbatim', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    await open();
    const text = `Met [Anna](genetree://person/${id}) in **Hamburg**.\n\n- one\n- two`;
    await user.click(screen.getByRole('button', { name: 'Add note' }));
    const box = screen.getByRole('textbox', { name: 'Note' });
    await user.click(box);
    await user.paste(text);
    await user.click(screen.getByRole('button', { name: 'Save note' }));
    await waitFor(() => expect(spy).toHaveBeenCalled());
    const cmd = spy.mock.calls[0][0] as unknown as { note: { text: string } };
    expect(cmd.note.text).toBe(text);
  });

  it('history_lists_entries', async () => {
    await api.apply({ type: 'updatePerson', id, private: true });
    await open();
    expect(await screen.findByText('Edit person')).toBeInTheDocument();
  });

  it('save_error_is_shown_not_swallowed', async () => {
    const user = userEvent.setup();
    await open();
    vi.spyOn(api, 'apply').mockRejectedValue({ code: 'io', message: 'The disk is full. Free some space and save again.' });
    await user.click(screen.getByRole('button', { name: 'Add name' }));
    await user.type(screen.getByRole('textbox', { name: 'Given name' }), 'Kept');
    await user.click(screen.getByRole('button', { name: 'Save name' }));
    expect(await screen.findByText('The disk is full. Free some space and save again.')).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: 'Given name' })).toHaveValue('Kept');
  });

  it('add_event_opens_event_editor_and_saves', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    await open();
    await user.click(screen.getByRole('button', { name: 'Add event' }));
    const dlg = screen.getByRole('dialog', { name: 'Add event' });
    await user.type(within(dlg).getByRole('textbox', { name: 'Date' }), '1901');
    await user.click(within(dlg).getByRole('button', { name: 'Save event' }));
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(await screen.findByText('Event saved')).toBeInTheDocument();
  });

  it('deleting_an_event_toasts_deleted_not_saved', async () => {
    const user = userEvent.setup();
    await open();
    await user.click(screen.getAllByRole('button', { name: /^Edit (Birth|Death|Occupation|.+)$/ }).find((b) => b.getAttribute('aria-label')?.match(/^Edit [A-Z]/) && !/name|note|family/i.test(b.getAttribute('aria-label')!))!);
    await user.click(screen.getByRole('button', { name: 'Delete event' }));
    expect(await screen.findByText('Event deleted')).toBeInTheDocument();
    expect(screen.queryByText('Event saved')).toBeNull();
  });
});
