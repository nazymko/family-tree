import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import PersonEditor from './PersonEditor.svelte';
import Sources from './Sources.svelte';
import CoupleEndDialog from './CoupleEndDialog.svelte';
import { apiContext } from '../api/context';
import type { PersonDetail, PersonQuery } from '../api/types';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { findPerson, renderWithApi } from '../editors/testing/helpers';
import CitationEditor from '../editors/CitationEditor.svelte';
import EventEditor from '../editors/EventEditor.svelte';
import FamilyEditor from '../editors/FamilyEditor.svelte';
import NoteEditor from '../editors/NoteEditor.svelte';
import { agentChanges } from '../state/agentChanges.svelte';
import { nav } from '../state/nav.svelte';
import { observeApi } from '../state/observe';
import { project } from '../state/project.svelte';
import { settings } from '../state/settings.svelte';
import { toasts } from '../state/toast.svelte';

const ALL: PersonQuery = { sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false };
const PERSON_BANNER = 'An agent changed this person. Reload to continue.';
const BANNER = 'An agent changed this. Reload to continue.';
const fixture = fixtureTree(60);

let stop: (() => void) | null = null;

async function world() {
  const raw = new MockApi(fixture);
  const api = observeApi(raw);
  const platform = new MockPlatform();
  project.open(await api.openProject('C:/Genealogy/Test'));
  nav.reset();
  stop = agentChanges.start(api, platform);
  await settings.load(api);
  const people = (await raw.listPersons(ALL, 0, 3)).items;
  return { raw, api, platform, people, user: userEvent.setup() };
}

/** Holds the next `getPerson` until released; the calls after it go straight through. */
function holdNextLoad(raw: MockApi) {
  const real = raw.getPerson.bind(raw);
  let release: () => void = () => {};
  let fail: (e: unknown) => void = () => {};
  const held = new Promise<void>((res, rej) => {
    release = res;
    fail = rej;
  });
  vi.spyOn(raw, 'getPerson').mockImplementationOnce(async (id) => {
    await held;
    return real(id);
  });
  return { release, fail };
}

const settle = () => new Promise((r) => setTimeout(r, 10));

async function openOwnEditor(w: Awaited<ReturnType<typeof world>>) {
  render(PersonEditor, { props: { id: w.people[0].id }, context: apiContext(w.api, w.platform) });
  await w.user.click(await screen.findByRole('button', { name: 'Add name' }));
  return screen.findByRole('dialog', { name: 'Add name' });
}

beforeEach(() => {
  toasts.clear();
  project.close();
  agentChanges.reset();
});

afterEach(() => {
  stop?.();
  stop = null;
});

describe('Reload on an editor with typed text', () => {
  it('keeps_the_text_until_the_owner_says_yes', async () => {
    const w = await world();
    const dlg = await openOwnEditor(w);
    const given = within(dlg).getByRole('textbox', { name: 'Given name' });
    await w.user.type(given, 'Typed');
    w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.people[0].id, note: 'Underneath you' }] });
    await w.user.click(await within(dlg).findByRole('button', { name: 'Reload' }));
    expect(within(dlg).getByText('Discard changes?')).toBeInTheDocument();
    expect(screen.getByText(PERSON_BANNER)).toBeInTheDocument();
    expect(within(screen.getByRole('dialog')).getByRole('textbox', { name: 'Given name' })).toHaveValue('Typed');

    await w.user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Keep editing' }));
    await settle();
    expect(screen.getByText(PERSON_BANNER)).toBeInTheDocument();
    expect(within(screen.getByRole('dialog')).getByRole('textbox', { name: 'Given name' })).toHaveValue('Typed');

    await w.user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Reload' }));
    await w.user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Discard changes' }));
    await waitFor(() => expect(screen.queryByText(PERSON_BANNER)).toBeNull());
    expect(within(screen.getByRole('dialog')).getByRole('textbox', { name: 'Given name' })).toHaveValue('');
  });

  it('reloads_at_once_when_nothing_was_typed', async () => {
    const w = await world();
    const dlg = await openOwnEditor(w);
    w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.people[0].id, note: 'Underneath you' }] });
    await w.user.click(await within(dlg).findByRole('button', { name: 'Reload' }));
    await waitFor(() => expect(screen.queryByText(PERSON_BANNER)).toBeNull());
    expect(screen.queryByText('Discard changes?')).toBeNull();
  });
});

describe('the basics of the person page while an agent writes', () => {
  async function page() {
    const w = await world();
    const id = w.people[0].id;
    await w.raw.apply({ type: 'updatePerson', id, sex: 'other', sexText: 'As recorded' });
    render(PersonEditor, { props: { id }, context: apiContext(w.api, w.platform) });
    const field = await screen.findByRole('textbox', { name: 'Describe sex' });
    return { ...w, id, field: field as HTMLInputElement };
  }

  it('text_being_typed_survives_an_event_and_is_then_committed', async () => {
    const { raw, platform, user, id, field } = await page();
    const apply = vi.spyOn(raw, 'apply');
    await user.clear(field);
    await user.type(field, 'Half typed');
    const reads = vi.spyOn(raw, 'getPerson');
    platform.simulateAgentBatch(raw, { changes: [{ person: id, note: 'Underneath you' }] });
    await waitFor(() => expect(reads).toHaveBeenCalled());
    await settle();
    expect(field).toHaveValue('Half typed');
    await user.tab();
    await waitFor(() => expect(apply).toHaveBeenCalledWith({ type: 'updatePerson', id, sex: 'other', sexText: 'Half typed' }, undefined));
    await settle();
    expect(screen.getByRole('textbox', { name: 'Describe sex' })).toHaveValue('Half typed');
  });

  it('a_focused_field_with_no_typing_waits_and_then_catches_up', async () => {
    const { raw, platform, user, id, field } = await page();
    field.focus();
    await raw.apply({ type: 'updatePerson', id, sexText: 'From the agent' });
    platform.simulateAgentBatch(raw, { changes: [{ person: id, note: 'Underneath you' }] });
    await settle();
    expect(field).toHaveValue('As recorded');
    await user.tab();
    await waitFor(() => expect(field).toHaveValue('From the agent'));
  });

  it('a_field_nobody_is_in_follows_the_tree', async () => {
    const { raw, platform, id, field } = await page();
    await raw.apply({ type: 'updatePerson', id, sexText: 'From the agent', private: true });
    platform.simulateAgentBatch(raw, { changes: [{ person: id, note: 'Underneath you' }] });
    await waitFor(() => expect(field).toHaveValue('From the agent'));
    expect(screen.getByRole('checkbox', { name: 'Private' })).toBeChecked();
  });

  it('a_change_of_sex_does_not_take_the_field_away_from_under_the_typing', async () => {
    const { raw, platform, user, id, field } = await page();
    await user.type(field, ' and more');
    await raw.apply({ type: 'updatePerson', id, sex: 'male' });
    platform.simulateAgentBatch(raw, { changes: [{ person: id, note: 'Underneath you' }] });
    await settle();
    expect(screen.getByRole('textbox', { name: 'Describe sex' })).toHaveValue('As recorded and more');
  });

  it('a_focused_select_is_left_alone_until_focus_moves', async () => {
    const { raw, platform, user, id } = await page();
    const living = screen.getByRole('combobox', { name: 'Living status' });
    living.focus();
    await raw.apply({ type: 'updatePerson', id, livingOverride: 'deceased' });
    platform.simulateAgentBatch(raw, { changes: [{ person: id, note: 'Underneath you' }] });
    await settle();
    expect(living).toHaveValue('');
    await user.tab();
    await waitFor(() => expect(living).toHaveValue('deceased'));
  });

  it('a_commit_that_fails_goes_back_to_the_tree_even_with_focus', async () => {
    const { raw, user, id } = await page();
    const box = screen.getByRole('combobox', { name: 'Living status' });
    vi.spyOn(raw, 'apply').mockRejectedValueOnce({ code: 'invalid', message: 'No.' });
    await user.selectOptions(box, 'deceased');
    expect(raw.apply).toHaveBeenCalledWith(expect.objectContaining({ id, livingOverride: 'deceased' }), undefined);
    await waitFor(() => expect(box).toHaveValue(''));
  });
});

describe('what an editor counts as seen', () => {
  it('a_reload_overtaken_by_a_newer_load_stays_stale', async () => {
    const w = await world();
    const dlg = await openOwnEditor(w);
    w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.people[0].id, note: 'Underneath you' }] });
    const reload = await within(dlg).findByRole('button', { name: 'Reload' });
    await settle();
    const held = holdNextLoad(w.raw);
    await w.user.click(reload);
    // A newer load starts and finishes while the reload's own is held.
    project.rev++;
    await settle();
    held.release();
    await settle();
    expect(screen.getByText(PERSON_BANNER)).toBeInTheDocument();
    expect(within(screen.getByRole('dialog')).getByRole('button', { name: 'Save name' })).toBeDisabled();
    // Asking again works.
    await w.user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Reload' }));
    await waitFor(() => expect(screen.queryByText(PERSON_BANNER)).toBeNull());
  });

  it('a_reload_that_fails_stays_stale', async () => {
    const w = await world();
    const dlg = await openOwnEditor(w);
    const given = within(dlg).getByRole('textbox', { name: 'Given name' });
    w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.people[0].id, note: 'Underneath you' }] });
    const reload = await within(dlg).findByRole('button', { name: 'Reload' });
    await settle();
    vi.spyOn(w.raw, 'getPerson').mockRejectedValueOnce({ code: 'io', message: 'The disk is gone.' });
    await w.user.click(reload);
    await settle();
    expect(screen.getByText(PERSON_BANNER)).toBeInTheDocument();
    expect(within(screen.getByRole('dialog')).getByRole('button', { name: 'Save name' })).toBeDisabled();
    expect(given.isConnected).toBe(true);
    await w.user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Reload' }));
    await waitFor(() => expect(screen.queryByText(PERSON_BANNER)).toBeNull());
  });

  it('a_write_during_the_reload_leaves_it_stale', async () => {
    const w = await world();
    const dlg = await openOwnEditor(w);
    w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.people[0].id, note: 'First' }] });
    const reload = await within(dlg).findByRole('button', { name: 'Reload' });
    await settle();
    const held = holdNextLoad(w.raw);
    await w.user.click(reload);
    w.platform.simulateAgentEvent([w.people[0].id]);
    held.release();
    await settle();
    expect(screen.getByText(PERSON_BANNER)).toBeInTheDocument();
  });

  it('an_editor_opened_during_a_load_is_stale_from_the_start', async () => {
    const w = await world();
    render(PersonEditor, { props: { id: w.people[0].id }, context: apiContext(w.api, w.platform) });
    const add = await screen.findByRole('button', { name: 'Add name' });
    await settle();
    const held = holdNextLoad(w.raw);
    w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.people[0].id, note: 'Underneath you' }] });
    // The page still shows what it read before the write.
    await w.user.click(add);
    expect(await screen.findByText(PERSON_BANNER)).toBeInTheDocument();
    held.release();
    await settle();
    // The load landed, but this form was built from the old data.
    expect(screen.getByText(PERSON_BANNER)).toBeInTheDocument();
  });

  it('an_editor_opened_after_the_load_landed_is_not_stale', async () => {
    const w = await world();
    render(PersonEditor, { props: { id: w.people[0].id }, context: apiContext(w.api, w.platform) });
    const add = await screen.findByRole('button', { name: 'Add name' });
    w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.people[0].id, note: 'Underneath you' }] });
    await settle();
    await w.user.click(add);
    await screen.findByRole('dialog', { name: 'Add name' });
    expect(screen.queryByText(PERSON_BANNER)).toBeNull();
  });
});

describe('editors outside EditorDialog', () => {
  it('citation_editor', async () => {
    const w = await world();
    const onCancel = vi.fn();
    const id = w.people[0].id;
    renderWithApi(w.raw, CitationEditor, { target: { kind: 'person', id }, onDone: () => {}, onCancel });
    const page = await screen.findByRole('textbox', { name: 'Page / entry' });
    await w.user.type(page, '12');
    const save = () => within(screen.getByRole('dialog')).getByRole('button', { name: 'Add citation' });
    w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.people[1].id, note: 'Elsewhere' }] });
    await settle();
    expect(save()).toBeEnabled();
    expect(screen.queryByText(BANNER)).toBeNull();

    w.platform.simulateAgentEvent([id]);
    expect(await screen.findByText(BANNER)).toBeInTheDocument();
    expect(save()).toBeDisabled();
    await w.user.click(screen.getByRole('button', { name: 'Reload' }));
    expect(onCancel).not.toHaveBeenCalled();
    expect(page).toHaveValue('12');
    await w.user.click(screen.getByRole('button', { name: 'Discard changes' }));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it('couple_end_dialog', async () => {
    const w = await world();
    const onclose = vi.fn();
    render(CoupleEndDialog, {
      props: { family: 'f1', partners: [{ id: 'p1', name: 'Ivan Petrovych Koval' }, { id: 'p2', name: 'Olena Koval' }] as never, end: null, onclose },
      context: apiContext(w.raw, w.platform),
    });
    const save = () => within(screen.getByRole('dialog')).getByRole('button', { name: 'Save' });
    expect(save()).toBeEnabled();
    w.platform.simulateAgentEvent(['elsewhere']);
    await settle();
    expect(save()).toBeEnabled();
    const apply = vi.spyOn(w.raw, 'apply');
    w.platform.simulateAgentEvent(['f1']);
    expect(await screen.findByText(BANNER)).toBeInTheDocument();
    expect(save()).toBeDisabled();
    await w.user.click(save());
    expect(apply).not.toHaveBeenCalled();
    // Nothing was touched, so it closes straight away; the tree behind it holds the new data.
    await w.user.click(screen.getByRole('button', { name: 'Reload' }));
    expect(onclose).toHaveBeenCalledOnce();
  });

  it('couple_end_dialog_asks_when_something_was_changed', async () => {
    const w = await world();
    const onclose = vi.fn();
    render(CoupleEndDialog, {
      props: { family: 'f1', partners: [{ id: 'p1', name: 'Ivan Petrovych Koval' }] as never, end: null, onclose },
      context: apiContext(w.raw, w.platform),
    });
    await w.user.click(screen.getByRole('radio', { name: 'Divorce' }));
    w.platform.simulateAgentEvent(['f1']);
    await w.user.click(await screen.findByRole('button', { name: 'Reload' }));
    expect(onclose).not.toHaveBeenCalled();
    await w.user.click(screen.getByRole('button', { name: 'Keep editing' }));
    expect(screen.getByRole('radio', { name: 'Divorce' })).toBeChecked();
    expect(onclose).not.toHaveBeenCalled();
  });

  async function sourcesPage() {
    const w = await world();
    const r = await w.raw.apply({ type: 'saveSource', source: { title: 'Register', author: 'Old', publication: '', abbreviation: '', text: '' } });
    const sid = r.touched[0];
    render(Sources, { props: { selectedId: sid }, context: apiContext(w.api, w.platform) });
    const author = (await screen.findByRole('textbox', { name: 'Author' })) as HTMLInputElement;
    await waitFor(() => expect(author).toHaveValue('Old'));
    const agentWrites = async () => {
      await w.raw.apply({ type: 'saveSource', source: { id: sid, title: 'Register', author: 'From the agent', publication: '', abbreviation: '', text: '' } });
      w.platform.simulateAgentEvent([sid]);
    };
    return { ...w, sid, author, agentWrites };
  }
  const saveSource = () => screen.getByRole('button', { name: 'Save source' });

  it('source_form_blocks_a_save_that_would_blank_what_an_agent_filled', async () => {
    const { raw, user, author, agentWrites } = await sourcesPage();
    await user.type(author, ' typed');
    await agentWrites();
    expect(await screen.findByText(BANNER)).toBeInTheDocument();
    expect(saveSource()).toBeDisabled();
    const apply = vi.spyOn(raw, 'apply');
    await user.type(author, '{Enter}');
    expect(apply).not.toHaveBeenCalled();

    await user.click(screen.getByRole('button', { name: 'Reload' }));
    expect(author).toHaveValue('Old typed');
    await user.click(screen.getByRole('button', { name: 'Keep editing' }));
    expect(author).toHaveValue('Old typed');
    await user.click(screen.getByRole('button', { name: 'Reload' }));
    await user.click(screen.getByRole('button', { name: 'Discard changes' }));
    await waitFor(() => expect(screen.queryByText(BANNER)).toBeNull());
    expect(screen.getByRole('textbox', { name: 'Author' })).toHaveValue('From the agent');
    expect(saveSource()).toBeEnabled();
  });

  it('source_form_reloads_at_once_when_nothing_was_typed_even_after_a_save', async () => {
    const { raw, user, author, agentWrites } = await sourcesPage();
    await user.type(author, ' saved');
    const apply = vi.spyOn(raw, 'apply');
    await user.click(saveSource());
    await waitFor(() => expect(apply).toHaveBeenCalledOnce());
    await settle();
    await agentWrites();
    await user.click(await screen.findByRole('button', { name: 'Reload' }));
    await waitFor(() => expect(screen.queryByText(BANNER)).toBeNull());
    expect(screen.getByRole('textbox', { name: 'Author' })).toHaveValue('From the agent');
  });

  it('source_form_stays_stale_when_the_reload_fails', async () => {
    const { raw, user, agentWrites } = await sourcesPage();
    await agentWrites();
    const reload = await screen.findByRole('button', { name: 'Reload' });
    await settle();
    vi.spyOn(raw, 'suggestSources').mockRejectedValueOnce({ code: 'io', message: 'The disk is gone.' });
    await user.click(reload);
    expect(await screen.findByText('The disk is gone.')).toBeInTheDocument();
    expect(screen.getByText(BANNER)).toBeInTheDocument();
    expect(saveSource()).toBeDisabled();
  });

  it('source_form_for_a_new_source_is_never_stale', async () => {
    const { user, sid, platform } = await sourcesPage();
    await user.click(screen.getByRole('button', { name: 'New source' }));
    const dlg = await screen.findByRole('dialog');
    platform.simulateAgentEvent([sid]);
    await settle();
    expect(within(dlg).queryByText(BANNER)).toBeNull();
    expect(within(dlg).getByRole('button', { name: 'Add source' })).toBeEnabled();
  });
});

describe('while an editor is stale', () => {
  const stale = async (w: Awaited<ReturnType<typeof world>>, id: string) => {
    w.platform.simulateAgentEvent([id]);
    return screen.findByText(BANNER);
  };

  it('event_delete_is_disabled_and_the_banner_does_not_say_person', async () => {
    const w = await world();
    const d: PersonDetail = await findPerson(w.raw, (p) => p.events.length > 0);
    const event = d.events[0];
    renderWithApi(w.raw, EventEditor, { owner: { kind: 'person', id: d.ref.id }, event, onCancel: () => {}, onSaved: () => {} });
    const del = await screen.findByRole('button', { name: 'Delete event' });
    expect(del).toBeEnabled();
    await stale(w, event.id);
    expect(screen.queryByText(PERSON_BANNER)).toBeNull();
    expect(screen.getByRole('button', { name: 'Delete event' })).toBeDisabled();
  });

  it('family_delete_is_disabled_and_the_banner_does_not_say_person', async () => {
    const w = await world();
    const d = await findPerson(w.raw, (p) => p.partnerships.length > 0);
    const family = d.partnerships[0].familyId;
    renderWithApi(w.raw, FamilyEditor, { family, anchor: d.ref.id, onCancel: () => {}, onChanged: () => {}, onDone: () => {} });
    const del = await screen.findByRole('button', { name: 'Delete family' });
    expect(del).toBeEnabled();
    await stale(w, family);
    expect(screen.queryByText(PERSON_BANNER)).toBeNull();
    expect(screen.getByRole('button', { name: 'Delete family' })).toBeDisabled();
  });

  it('note_banner_does_not_say_person', async () => {
    const w = await world();
    const id = w.people[0].id;
    renderWithApi(w.raw, NoteEditor, { target: { kind: 'person', id }, onCancel: () => {}, onSaved: () => {} });
    await stale(w, id);
    expect(screen.queryByText(PERSON_BANNER)).toBeNull();
  });
});
