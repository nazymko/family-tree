import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import PersonEditor from './PersonEditor.svelte';
import { apiContext } from '../api/context';
import type { FieldChangeDto, PersonDetail } from '../api/types';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { findPerson } from '../editors/testing/helpers';
import { agentChanges } from '../state/agentChanges.svelte';
import { nav } from '../state/nav.svelte';
import { observeApi } from '../state/observe';
import { project } from '../state/project.svelte';
import { settings } from '../state/settings.svelte';
import { toasts } from '../state/toast.svelte';
import { setLanguage } from '../i18n';

let stop: (() => void) | null = null;

async function world() {
  const raw = new MockApi(fixtureTree(60));
  const api = observeApi(raw);
  const platform = new MockPlatform();
  project.open(await api.openProject('C:/Genealogy/Test'));
  nav.reset();
  stop = agentChanges.start(api, platform);
  await settings.load(api);
  const d = await findPerson(raw, (x) => x.events.length > 0 && x.partnerships.length > 0 && x.names.length > 0);
  return { raw, api, platform, d, user: userEvent.setup() };
}
type World = Awaited<ReturnType<typeof world>>;

async function open(w: World, id = w.d.ref.id) {
  render(PersonEditor, { props: { id }, context: apiContext(w.api, w.platform) });
  await screen.findByRole('heading', { level: 2, name: 'Names' });
}

const panel = () => screen.findByRole('region', { name: 'What an agent changed' });
const tableRows = (el: HTMLElement) => [...el.querySelectorAll('tbody tr')].map((r) => [...r.children].map((c) => c.textContent?.replace(/\s+/g, ' ').trim()).join(' | '));

const field = (f: string, before: string | null, after: string | null, extra: Partial<FieldChangeDto> = {}): FieldChangeDto => ({ field: f, before, after, ...extra });

beforeEach(() => {
  toasts.clear();
  project.close();
  agentChanges.reset();
});

afterEach(() => {
  stop?.();
  stop = null;
  setLanguage('en');
});

describe('the agent panel shows a table for each record', () => {
  it('a_note_in_full_with_its_kind_in_words', async () => {
    const w = await world();
    const text = 'A long remembered story. '.repeat(30).trim();
    w.platform.simulateAgentBatch(w.raw, {
      changes: [{ person: w.d.ref.id, note: text, fields: [field('noteKind', null, 'story'), field('text', null, text)] }],
    });
    await open(w);
    const p = await panel();
    expect(within(p).getByText('Added · Note')).toBeInTheDocument();
    expect(tableRows(p)).toEqual(['Kind of note | empty | Story', `Text | empty | ${text}`]);
    expect(within(p).getAllByRole('columnheader').map((h) => h.textContent)).toEqual(['Field', 'Before', 'After']);
  });

  it('an_event_with_a_place_a_date_and_a_participant', async () => {
    const w = await world();
    const ev = w.d.events[0];
    w.platform.simulateAgentBatch(w.raw, {
      changes: [
        {
          person: w.d.ref.id,
          kind: 'event',
          record: ev.id,
          fields: [
            field('eventKind', null, 'birth'),
            field('when', null, 'about 1850'),
            field('place', null, 'Lviv'),
            field('participant', null, 'principal', { of: 'Olena Koval' }),
          ],
          fact: { kind: 'event', event: 'birth', label: 'Birth', date: 'about 1850', place: 'Lviv' },
        },
      ],
    });
    await open(w);
    const p = await panel();
    expect(within(p).getByText('Added · Event')).toBeInTheDocument();
    expect(tableRows(p)).toEqual(['Kind of event | empty | Birth', 'Date | empty | about 1850', 'Place | empty | Lviv', 'Participant · Olena Koval | empty | principal']);
  });

  it('a_family_with_partners_and_a_child_link', async () => {
    const w = await world();
    const family = w.d.partnerships[0].familyId;
    w.platform.simulateAgentBatch(w.raw, {
      changes: [
        {
          person: w.d.ref.id,
          kind: 'family',
          record: family,
          fields: [
            field('partner1', null, 'Ivan Koval'),
            field('unionKind', null, 'civil_union'),
            field('child', null, 'Olena Koval'),
            field('childRel1', null, 'adopted', { of: 'Olena Koval' }),
          ],
          fact: { kind: 'family', partners: ['Ivan Koval', 'Maria Koval'] },
        },
      ],
    });
    await open(w);
    const p = await panel();
    expect(within(p).getByText('Added · Family')).toBeInTheDocument();
    expect(tableRows(p)).toEqual([
      'First partner | empty | Ivan Koval',
      'Kind of union | empty | Civil union',
      'Child | empty | Olena Koval',
      'Relation to the first parent · Olena Koval | empty | Adopted',
    ]);
  });

  it('a_change_without_fields_keeps_its_fact_line_and_has_no_empty_table', async () => {
    const w = await world();
    w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.d.ref.id, note: 'Only the fact' }] });
    await open(w);
    const p = await panel();
    expect(within(p).getByText('Only the fact')).toBeInTheDocument();
    expect(p.querySelector('table')).toBeNull();
  });

  it('in_ukrainian_with_the_words_of_the_language', async () => {
    const w = await world();
    w.platform.simulateAgentBatch(w.raw, {
      changes: [{ person: w.d.ref.id, note: 'Нотатка', fields: [field('noteKind', null, 'research'), field('text', null, 'Нотатка')] }],
    });
    setLanguage('uk');
    render(PersonEditor, { props: { id: w.d.ref.id }, context: apiContext(w.api, w.platform) });
    const p = await screen.findByRole('region', { name: 'Що змінив агент' });
    expect(within(p).getAllByRole('columnheader').map((h) => h.textContent)).toEqual(['Поле', 'Було', 'Стало']);
    expect(within(p).getByText('Додано · Нотатка')).toBeInTheDocument();
    expect(tableRows(p)).toEqual(['Вид нотатки | порожньо | Дослідження', 'Текст | порожньо | Нотатка']);
    expect(within(p).getByRole('button', { name: /^Скасувати/ })).toBeInTheDocument();
    expect(within(p).getByRole('button', { name: /^Редагувати/ })).toBeInTheDocument();
  });
});

describe('undo from the person page', () => {
  it('confirms_reports_refreshes_and_moves_focus_to_the_page', async () => {
    const w = await world();
    w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.d.ref.id, note: 'Goes away' }] });
    await open(w);
    const p = await panel();
    await w.user.click(within(p).getByRole('button', { name: /^Undo/ }));
    const ask = await screen.findByRole('dialog', { name: 'Undo this change?' });
    await w.user.click(within(ask).getByRole('button', { name: 'Undo' }));
    const done = await screen.findByRole('dialog', { name: 'Undo finished' });
    expect(within(done).getByText('1 change undone')).toBeInTheDocument();
    await w.user.click(within(done).getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(screen.queryByRole('region', { name: 'What an agent changed' })).toBeNull());
    expect((await w.raw.getPerson(w.d.ref.id)).notes.some((n) => n.text === 'Goes away')).toBe(false);
    // The row, and the button that had focus, are gone: focus is on the page, not nowhere.
    expect(screen.getByRole('heading', { level: 1 })).toHaveFocus();
    expect(agentChanges.unreviewed.size).toBe(0);
  });

  it('keeping_changes_nothing', async () => {
    const w = await world();
    w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.d.ref.id, note: 'Stays' }] });
    await open(w);
    const spy = vi.spyOn(w.raw, 'undoChangeGroup');
    const p = await panel();
    const undo = within(p).getByRole('button', { name: /^Undo/ });
    await w.user.click(undo);
    await w.user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Keep' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(spy).not.toHaveBeenCalled();
    expect(undo).toHaveFocus();
  });

  it('a_change_that_could_not_be_taken_back_in_full_says_what_stayed', async () => {
    const w = await world();
    w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.d.ref.id, note: 'Owner edited this one', conflict: true }] });
    await open(w);
    const p = await panel();
    await w.user.click(within(p).getByRole('button', { name: /^Undo/ }));
    await w.user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Undo' }));
    const done = await screen.findByRole('dialog', { name: 'Undo finished' });
    expect(within(done).getByText('These stayed, because they were changed since:')).toBeInTheDocument();
    expect(done).toHaveTextContent('Owner edited this one');
    await w.user.click(within(done).getByRole('button', { name: 'Done' }));
    // Something of it still stands: the row stays, and focus is back on its button.
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(await panel()).toBeInTheDocument();
  });

  it('a_dropped_step_shows_its_message', async () => {
    const w = await world();
    w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.d.ref.id, note: 'Needs a record' }] });
    await open(w);
    vi.spyOn(w.raw, 'undoChangeGroup').mockRejectedValue({ code: 'invalid', message: 'This step was dropped: a record it needs no longer exists.' });
    await w.user.click(within(await panel()).getByRole('button', { name: /^Undo/ }));
    const ask = await screen.findByRole('dialog', { name: 'Undo this change?' });
    await w.user.click(within(ask).getByRole('button', { name: 'Undo' }));
    expect(await within(ask).findByText('This step was dropped: a record it needs no longer exists.')).toBeInTheDocument();
  });

  it('the_person_the_undo_removes_leaves_the_page_without_an_error_screen', async () => {
    const w = await world();
    w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.d.ref.id, note: 'Made them' }] });
    await open(w);
    const real = w.raw.undoChangeGroup.bind(w.raw);
    vi.spyOn(w.raw, 'undoChangeGroup').mockImplementation(async (g) => {
      const r = await real(g);
      await w.raw.apply({ type: 'deletePerson', id: w.d.ref.id });
      return r;
    });
    await w.user.click(within(await panel()).getByRole('button', { name: /^Undo/ }));
    await w.user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Undo' }));
    await w.user.click(within(await screen.findByRole('dialog', { name: 'Undo finished' })).getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(nav.route.name).toBe('people'));
    expect(screen.queryByRole('alert')).toBeNull();
  });
});

describe('edit from the person page', () => {
  async function withNoteEvent(w: World) {
    const detail = await w.raw.getPerson(w.d.ref.id);
    w.platform.simulateAgentBatch(w.raw, {
      changes: [{ person: w.d.ref.id, note: 'Told at the kitchen table', fields: [field('text', null, 'Told at the kitchen table')] }],
    });
    return detail;
  }

  it('a_note_opens_the_note_editor_and_saving_refreshes_the_panel', async () => {
    const w = await world();
    await withNoteEvent(w);
    await open(w);
    const spy = vi.spyOn(w.raw, 'apply');
    const p = await panel();
    await w.user.click(within(p).getByRole('button', { name: /^Edit Note: Told at the kitchen table/ }));
    const dialog = await screen.findByRole('dialog', { name: 'Edit note' });
    const box = within(dialog).getByRole('textbox', { name: 'Note' });
    expect(box).toHaveValue('Told at the kitchen table');
    await w.user.clear(box);
    await w.user.type(box, 'Told on the porch');
    await w.user.click(within(dialog).getByRole('button', { name: 'Save note' }));
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(spy.mock.calls[0][0]).toMatchObject({ type: 'saveNote', note: { text: 'Told on the porch' } });
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    // Editing does not review it.
    expect(agentChanges.unreviewed.has(w.d.ref.id)).toBe(true);
    expect(await screen.findByRole('button', { name: 'Reviewed' })).toBeInTheDocument();
  });

  it('an_event_opens_the_event_editor', async () => {
    const w = await world();
    const ev = w.d.events[0];
    w.platform.simulateAgentBatch(w.raw, {
      changes: [{ person: w.d.ref.id, kind: 'event', record: ev.id, fields: [field('eventKind', null, ev.kind)], fact: { kind: 'event', event: ev.kind, label: ev.label, date: null, place: null } }],
    });
    await open(w);
    await w.user.click(within(await panel()).getByRole('button', { name: /^Edit Event/ }));
    expect(await screen.findByRole('dialog', { name: /^Edit / })).toBeInTheDocument();
    expect(screen.getByRole('dialog')).toHaveAccessibleName(/event|birth|death|marriage|baptism|burial|residence|occupation/i);
  });

  it('a_name_opens_the_name_editor_one_button_per_name', async () => {
    const w = await world();
    const name = w.d.names[0];
    w.platform.simulateAgentBatch(w.raw, {
      changes: [{ person: w.d.ref.id, kind: 'person', record: w.d.ref.id, fields: [field('given', 'Old', 'New', { nameId: name.id })], fact: { kind: 'text', text: w.d.ref.name } }],
    });
    await open(w);
    const p = await panel();
    expect(within(p).getAllByRole('button', { name: /^Edit Name/ })).toHaveLength(1);
    await w.user.click(within(p).getByRole('button', { name: /^Edit Name/ }));
    expect(await screen.findByRole('dialog', { name: 'Edit name' })).toBeInTheDocument();
  });

  it('a_family_opens_the_family_editor', async () => {
    const w = await world();
    w.platform.simulateAgentBatch(w.raw, {
      changes: [{ person: w.d.ref.id, kind: 'family', record: w.d.partnerships[0].familyId, fields: [field('unionKind', 'married', 'unmarried')], fact: { kind: 'family', partners: [] } }],
    });
    await open(w);
    await w.user.click(within(await panel()).getByRole('button', { name: /^Edit Family/ }));
    expect(await screen.findByRole('dialog', { name: 'Edit family' })).toBeInTheDocument();
  });

  it('a_record_that_is_gone_has_no_edit_button', async () => {
    const w = await world();
    w.platform.simulateAgentBatch(w.raw, {
      changes: [{ person: w.d.ref.id, kind: 'note', record: 'no-such-note', op: 'delete', fields: [field('text', 'Was here', null)] }],
    });
    await open(w);
    const p = await panel();
    expect(within(p).queryByRole('button', { name: /^Edit/ })).toBeNull();
    expect(within(p).getByRole('button', { name: /^Undo/ })).toBeInTheDocument();
  });

  it('the_unnamed_siblings_marker_can_only_be_undone', async () => {
    const w = await world();
    const other = (await w.raw.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 5)).items.find((r) => r.id !== w.d.ref.id)!;
    w.platform.simulateAgentBatch(w.raw, {
      changes: [{ person: w.d.ref.id, kind: 'person', record: other.id, fields: [field('given', null, 'x', { nameId: 'n1' })], fact: { kind: 'text', text: '' } }],
    });
    await open(w);
    const p = await panel();
    expect(within(p).queryByRole('button', { name: /^Edit/ })).toBeNull();
    expect(within(p).getByRole('button', { name: /^Undo/ })).toBeInTheDocument();
  });
});

describe('the page order and the history', () => {
  const after = (a: Element, b: Element) => Boolean(a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING);

  it('the_agent_panel_comes_after_every_other_section_including_the_history', async () => {
    const w = await world();
    w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.d.ref.id, note: 'Last on the page' }] });
    await open(w);
    const p = await panel();
    for (const h of ['h-tree', 'h-names', 'h-basics', 'h-events', 'h-rel', 'h-notes', 'h-media', 'h-sources', 'h-history']) {
      const el = document.getElementById(h);
      if (el) expect(after(el, p), h).toBe(true);
    }
    expect(document.getElementById('h-history')).not.toBeNull();
  });

  it('a_person_nobody_changed_says_nothing_was_recorded', async () => {
    const w = await world();
    await open(w);
    expect(screen.getByText('No changes recorded yet.')).toBeInTheDocument();
  });

  it('a_person_with_only_an_agent_change_has_history_read_only', async () => {
    const w = await world();
    w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.d.ref.id, note: 'Only this', fields: [field('text', null, 'Only this')] }] });
    await open(w);
    const history = document.querySelector('section[aria-labelledby="h-history"]') as HTMLElement;
    await waitFor(() => expect(history.querySelectorAll('ul.rows li')).toHaveLength(1));
    expect(screen.queryByText('No changes recorded yet.')).toBeNull();
    expect(history).toHaveTextContent('Added · Note');
    expect(within(history).queryByRole('button', { name: /^(Edit|Undo)/ })).toBeNull();
  });

  it('history_pages_twenty_at_a_time', async () => {
    const w = await world();
    w.platform.simulateAgentBatch(w.raw, { changes: Array.from({ length: 25 }, (_, i) => ({ person: w.d.ref.id, note: `Note ${i}` })) });
    await open(w);
    const history = document.querySelector('section[aria-labelledby="h-history"]') as HTMLElement;
    await waitFor(() => expect(history.querySelectorAll('ul.rows li')).toHaveLength(20));
    await w.user.click(within(history).getByRole('button', { name: 'Show more' }));
    await waitFor(() => expect(history.querySelectorAll('ul.rows li')).toHaveLength(25));
    expect(within(history).queryByRole('button', { name: 'Show more' })).toBeNull();
  });

  it('an_undone_change_shows_undone_in_the_history', async () => {
    const w = await world();
    const { groups } = w.platform.simulateAgentBatch(w.raw, { changes: [{ person: w.d.ref.id, note: 'Taken back' }] });
    await w.api.undoChangeGroup(groups[0]);
    await open(w);
    const history = document.querySelector('section[aria-labelledby="h-history"]') as HTMLElement;
    await waitFor(() => expect(within(history).getByText('Undone')).toBeInTheDocument());
  });
});

export type { PersonDetail };
