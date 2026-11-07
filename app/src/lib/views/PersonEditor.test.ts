import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import PersonEditor from './PersonEditor.svelte';
import { findPerson, newApi, renderWithApi } from '../editors/testing/helpers';
import { settings } from '../state/settings.svelte';
import type { MockApi } from '../api/mock/MockApi';
import { observeApi } from '../state/observe';
import { toasts } from '../state/toast.svelte';
import { ui } from '../state/ui.svelte';
import { nav } from '../state/nav.svelte';
import { setLanguage } from '../i18n';
import { KEYLESS_AGENT_ID, KEYLESS_AGENT_NAME } from '../i18n/backend';

const toastTexts = () => toasts.items.map((t) => t.text);

let api: MockApi;
let id: string;

beforeEach(async () => {
  api = newApi();
  id = (await findPerson(api, (d) => d.partnerships.length > 0 && d.names.length >= 1)).ref.id;
  await settings.load(api);
});

async function open() {
  toasts.clear();
  renderWithApi(observeApi(api), PersonEditor, { id });
  await screen.findByRole('heading', { level: 2, name: 'Names' });
}

async function open2() {
  toasts.clear();
  renderWithApi(observeApi(api), PersonEditor, { id });
  await screen.findByRole('heading', { level: 1 });
}

describe('PersonEditor', () => {
  it('shows_the_birth_surname_under_the_name_and_above_the_dates', async () => {
    const married = await findPerson(api, (d) => !!d.ref.birthSurname);
    id = married.ref.id;
    setLanguage('uk');
    try {
      await open2();
      // The same line also sits on the person's box in the family tree below: the one under the page heading is the first.
      const heading = (await screen.findAllByRole('heading', { level: 1 }))[0];
      const maiden = heading.nextElementSibling as HTMLElement;
      // The heading comes first, then the birth surname line, then the dates.
      expect(maiden.textContent).toBe(`дів. ${married.ref.birthSurname}`);
      expect(maiden.className).toContain('maiden');
      expect(maiden.nextElementSibling?.className).toContain('life');
    } finally {
      setLanguage('en');
    }
  });

  it('a_surname_prefix_on_the_birth_name_reads_the_same_in_the_line_and_the_surname_row', async () => {
    const married = await findPerson(api, (d) => !!d.ref.birthSurname);
    id = married.ref.id;
    const birth = married.names.find((n) => n.kind === 'birth')!;
    await api.apply({ type: 'saveName', person: id, name: { id: birth.id, kind: 'birth', preferred: birth.preferred, private: birth.private, given: birth.given ?? undefined, surnames: birth.surnames, surnamePrefix: 'van' } });
    await open2();
    const heading = (await screen.findAllByRole('heading', { level: 1 }))[0];
    const line = (heading.nextElementSibling as HTMLElement).textContent!;
    const row = screen.getByText('Surname at birth').closest('p')!.textContent!.replace('Surname at birth', '').trim();
    expect(line).toBe(`née van ${birth.surnames.join(' ')}`);
    expect(row).toBe(`van ${birth.surnames.join(', ')}`);
  });

  it('the_name_editor_shows_a_surname_prefix_that_is_set', async () => {
    const user = userEvent.setup();
    const married = await findPerson(api, (d) => !!d.ref.birthSurname);
    id = married.ref.id;
    const birth = married.names.find((n) => n.kind === 'birth')!;
    await api.apply({ type: 'saveName', person: id, name: { id: birth.id, kind: 'birth', preferred: birth.preferred, private: birth.private, given: birth.given ?? undefined, surnames: birth.surnames, surnamePrefix: 'van' } });
    await open();
    const shown = (await api.getPerson(id)).names.find((n) => n.kind === 'birth')!;
    await user.click(screen.getByRole('button', { name: `Edit name ${shown.display}` }));
    const dlg = await screen.findByRole('dialog', { name: 'Edit name' });
    // A value already there keeps the folded fields open, so a stray prefix can be seen and cleared.
    expect(within(dlg).getByRole('textbox', { name: 'Surname prefix' })).toHaveValue('van');
  });

  it('shows_no_birth_surname_line_when_there_is_none', async () => {
    const plain = await findPerson(api, (d) => !d.ref.birthSurname && d.names.length >= 1);
    id = plain.ref.id;
    setLanguage('uk');
    try {
      await open2();
      await screen.findByRole('heading', { level: 1 });
      expect(screen.queryByText(/^дів\. /)).toBeNull();
      expect(screen.queryByText(/^при нар\. /)).toBeNull();
    } finally {
      setLanguage('en');
    }
  });

  it('a_family_note_shows_on_the_partners_page_and_can_be_edited', async () => {
    const user = userEvent.setup();
    const person = await api.getPerson(id);
    const family = person.partnerships[0].familyId;
    await api.apply({ type: 'saveNote', target: { kind: 'family', id: family }, note: { kind: 'general', text: 'They moved to the village in spring.' } });
    await open();
    const text = await screen.findByText('They moved to the village in spring.');
    expect(screen.getByRole('heading', { level: 3, name: /Shared family notes/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Open family' })).toBeInTheDocument();
    expect(screen.queryByText('No notes yet.')).toBeNull();
    const article = text.closest('article') as HTMLElement;
    await user.click(within(article).getByRole('button', { name: /Edit/ }));
    expect(await screen.findByRole('dialog')).toBeInTheDocument();
  });

  it('event_rows_show_what_the_description_says', async () => {
    const person = await findPerson(api, (d) => d.events.some((e) => !!e.description));
    const text = person.events.find((e) => !!e.description)!.description!;
    id = person.ref.id;
    await open();
    expect(screen.getAllByText(text).length).toBeGreaterThan(0);
  });

  it('sex_change_commits_one_command', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    await open();
    await user.selectOptions(screen.getByRole('combobox', { name: 'Sex' }), 'female');
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
    expect(spy).toHaveBeenCalledWith({ type: 'updatePerson', id, sex: 'female' }, undefined);
    // The shell's toaster (observeApi) is the only one: exactly one toast, with Undo.
    await waitFor(() => expect(toasts.items).toHaveLength(1));
    expect(toasts.items[0].action?.label).toBe('Undo');
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
    // The rarely used fields are folded: the surname comes right after the given name and patronymic.
    expect(within(dlg).queryByRole('textbox', { name: 'Surname prefix' })).toBeNull();
    await user.click(within(dlg).getByRole('button', { name: /^More fields/ }));
    await user.type(within(dlg).getByRole('textbox', { name: 'Title or prefix' }), 'Dr');
    await user.type(within(dlg).getByRole('textbox', { name: 'Surnames' }), 'Müller, Schmidt');
    await user.click(within(dlg).getByRole('button', { name: 'Save name' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    const cmd = spy.mock.calls[0][0] as unknown as { type: string; person: string; name: Record<string, unknown> };
    expect(cmd.type).toBe('saveName');
    expect(cmd.person).toBe(id);
    expect(cmd.name).toMatchObject({ kind: 'married', prefix: 'Dr', surnames: ['Müller', 'Schmidt'] });
    await waitFor(() => expect(toastTexts()).toEqual(['Name saved']));
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

  it('note_voice_samples_play_on_the_page_without_the_editor', async () => {
    await api.apply({ type: 'saveNote', target: { kind: 'person', id }, note: { kind: 'general', text: 'Voice of Ivan Koval.' } });
    const d = await api.getPerson(id);
    const note = d.notes[d.notes.length - 1];
    await api.attachFiles({ kind: 'note', id: note.id }, ['C:/Mock/ivan.wav']);
    await open();
    await waitFor(() => expect(document.querySelector('audio')).not.toBeNull());
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('history_lists_entries', async () => {
    await api.apply({ type: 'updatePerson', id, private: true });
    await open();
    expect(await screen.findByText('Edit person')).toBeInTheDocument();
  });

  /** The newest row of the history section (its list is inside a closed <details>, still in the page). */
  async function historyRow(): Promise<HTMLElement> {
    return await waitFor(() => {
      const li = document.querySelector('section[aria-labelledby="h-history"] ul.rows li');
      if (!li) throw new Error('no history row yet');
      return li as HTMLElement;
    });
  }

  async function renameSurname(to: string[]) {
    const d = await api.getPerson(id);
    const n = d.names[0];
    await api.apply({
      type: 'saveName',
      person: id,
      name: { id: n.id, kind: n.kind, preferred: n.preferred, private: n.private, given: n.given ?? undefined, surnames: to, patronymic: n.patronymic ?? undefined },
    });
    return d;
  }

  /** Each changed field of a history row as `field before after`. */
  const changedRows = (row: Element) => [...row.querySelectorAll('tbody tr')].map((r) => [...r.children].map((c) => c.textContent?.trim()).join(' '));

  it('history_says_what_a_name_change_changed_in_english', async () => {
    await renameSurname(['Koval']);
    await renameSurname(['Kovalchuk']);
    await open();
    const row = await historyRow();
    expect(changedRows(row)).toContain('Surname Koval Kovalchuk');
    expect(row.querySelectorAll('bdi[dir="auto"]').length).toBeGreaterThanOrEqual(1);
  });

  it('history_says_empty_for_a_missing_side_in_english', async () => {
    const d = await api.getPerson(id);
    const n = d.names[0];
    await api.apply({ type: 'saveName', person: id, name: { id: n.id, kind: n.kind, preferred: n.preferred, private: n.private, given: n.given ?? 'Ivan', surnames: n.surnames, patronymic: undefined, suffix: 'Jr' } });
    await open();
    const row = await historyRow();
    expect(changedRows(row)).toContain('Suffix empty Jr');
  });

  it('history_says_what_a_name_change_changed_in_ukrainian', async () => {
    const d = await api.getPerson(id);
    const n = d.names[0];
    await api.apply({ type: 'saveName', person: id, name: { id: n.id, kind: n.kind, preferred: n.preferred, private: n.private, given: n.given ?? 'Ivan', surnames: n.surnames, suffix: 'Jr' } });
    setLanguage('uk');
    try {
      renderWithApi(observeApi(api), PersonEditor, { id });
      const row = await historyRow();
      expect(changedRows(row)).toContain('Суфікс порожньо Jr');
    } finally {
      setLanguage('en');
    }
  });

  it('history_words_the_agent_without_a_key_in_the_active_language_and_keeps_other_names', async () => {
    api.recordAgentBatch({ agent: KEYLESS_AGENT_NAME, agentId: KEYLESS_AGENT_ID, changes: [{ person: id, note: 'Told' }] });
    api.recordAgentBatch({ agent: 'Transcript reader', changes: [{ person: id, note: 'Read' }] });
    setLanguage('uk');
    try {
      renderWithApi(observeApi(api), PersonEditor, { id });
      await waitFor(() => {
        const metas = [...document.querySelectorAll('section[aria-labelledby="h-history"] .meta')].map((m) => m.textContent);
        expect(metas).toContain('Агент без ключа');
        expect(metas).toContain('Transcript reader');
        expect(metas).not.toContain(KEYLESS_AGENT_NAME);
      });
    } finally {
      setLanguage('en');
    }
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
    await user.type(within(dlg).getByRole('textbox', { name: 'Year' }), '1901');
    await user.click(within(dlg).getByRole('button', { name: 'Save event' }));
    await waitFor(() => expect(spy).toHaveBeenCalled());
    await waitFor(() => expect(toastTexts()).toEqual(['Event saved']));
  });

  it('deleting_an_event_toasts_deleted_not_saved', async () => {
    const user = userEvent.setup();
    await open();
    await user.click(screen.getAllByRole('button', { name: /^Edit (Birth|Death|Occupation|.+)$/ }).find((b) => b.getAttribute('aria-label')?.match(/^Edit [A-Z]/) && !/name|note|family/i.test(b.getAttribute('aria-label')!))!);
    await user.click(screen.getByRole('button', { name: 'Delete event' }));
    await waitFor(() => expect(toastTexts()).toEqual(['Event deleted']));
  });

  it('shows_the_closest_relatives_tree_with_add_slots', async () => {
    const user = userEvent.setup();
    await open();
    const name = (await api.getPerson(id)).ref.name;
    const view = await api.getFamilyView(id);
    const section = (await screen.findByRole('heading', { level: 2, name: 'Family tree' })).closest('section')!;
    const tree = within(section as HTMLElement);
    // The person and every partner appear as boxes in the tree.
    expect(tree.getAllByRole('button', { name: new RegExp(name) }).length).toBeGreaterThan(0);
    for (const u of view.partnerships) if (u.partner) expect(tree.getAllByRole('button', { name: new RegExp(u.partner.name) }).length).toBeGreaterThan(0);
    const spy = vi.spyOn(ui, 'requestAdd');
    await user.click(tree.getByRole('button', { name: `Add partner of ${name}` }));
    expect(spy).toHaveBeenLastCalledWith('partner', id);
    ui.closeAdd();
  });

  it('clicking_a_relative_in_the_tree_opens_their_page', async () => {
    const user = userEvent.setup();
    await open();
    const view = await api.getFamilyView(id);
    const partner = view.partnerships.find((u) => u.partner)!.partner!;
    const section = (await screen.findByRole('heading', { level: 2, name: 'Family tree' })).closest('section')!;
    const go = vi.spyOn(nav, 'go');
    await user.click(within(section as HTMLElement).getAllByRole('button', { name: new RegExp(`^${partner.name}`) })[0]);
    expect(go).toHaveBeenCalledWith({ name: 'person', id: partner.id });
  });
});
