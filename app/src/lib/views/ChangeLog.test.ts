import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import ChangeLog from './ChangeLog.svelte';
import AgentChangesPanel from './AgentChangesPanel.svelte';
import People from './People.svelte';
import PersonEditor from './PersonEditor.svelte';
import FamilyTree from './FamilyTree.svelte';
import PersonCard from '../ui/PersonCard.svelte';
import PersonRow from '../ui/PersonRow.svelte';
import { apiContext } from '../api/context';
import type { Id, PersonQuery, PersonRow as Row } from '../api/types';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { makeFakeContext } from '../../test-setup';
import { draw, type Theme } from '../chart/renderer';
import { drawWhole, type WholeTheme } from '../chart/wholeRenderer';
import { renderWithApi } from '../editors/testing/helpers';
import NameEditor from '../editors/NameEditor.svelte';
import { agentChanges } from '../state/agentChanges.svelte';
import { nav } from '../state/nav.svelte';
import { observeApi } from '../state/observe';
import { project } from '../state/project.svelte';
import { settings } from '../state/settings.svelte';
import { toasts } from '../state/toast.svelte';
import { setLanguage } from '../i18n';
import { en } from '../i18n/en';
import { uk } from '../i18n/uk';

const ALL: PersonQuery = { sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false };
const fixture = fixtureTree(60);

let stop: (() => void) | null = null;

async function world() {
  const raw = new MockApi(fixture);
  const api = observeApi(raw);
  const platform = new MockPlatform();
  project.open(await api.openProject('C:/Genealogy/Test'));
  nav.reset();
  stop = agentChanges.start(api, platform);
  const people = (await raw.listPersons(ALL, 0, 3)).items;
  return { raw, api, platform, people, user: userEvent.setup() };
}

async function logScreen() {
  const w = await world();
  nav.go({ name: 'log' });
  render(ChangeLog, { context: apiContext(w.api, w.platform) });
  await screen.findByRole('heading', { level: 1, name: 'Change log' });
  return w;
}

const rowsOnScreen = () => within(screen.getByRole('group', { name: 'Changes' })).queryAllByRole('listitem');

beforeEach(() => {
  toasts.clear();
  project.close();
  agentChanges.reset();
});

afterEach(() => {
  stop?.();
  stop = null;
});

describe('ChangeLog', () => {
  it('rows_show_author_and_person_link', async () => {
    const { raw, platform, people, user, api } = await logScreen();
    await api.apply({ type: 'createPerson', draft: { given: 'Olena', surnames: ['Koval'], sex: 'female' } });
    platform.simulateAgentBatch(raw, { agent: 'Transcript reader', changes: [{ person: people[0].id, note: 'Told at the kitchen table' }] });
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(2));
    const [agentRow, ownerRow] = rowsOnScreen();
    expect(within(ownerRow).getByText('You')).toBeInTheDocument();
    expect(within(ownerRow).getByText('Add person')).toBeInTheDocument();
    expect(within(ownerRow).queryByRole('button', { name: 'Undo' })).toBeNull();
    expect(within(agentRow).getByText('Transcript reader')).toBeInTheDocument();
    expect(agentRow).toHaveTextContent('Told at the kitchen table');
    await user.click(within(agentRow).getByRole('button', { name: people[0].name }));
    expect(nav.route).toEqual({ name: 'person', id: people[0].id });
  });

  it('before_tracking_label', async () => {
    const { raw, people } = await logScreen();
    raw.recordBeforeTracking('Edit person', people[0].id);
    project.rev++;
    expect(await screen.findByText('Before tracking')).toBeInTheDocument();
    expect(rowsOnScreen()).toHaveLength(1);
    expect(within(rowsOnScreen()[0]).queryByRole('button', { name: 'Undo' })).toBeNull();
  });

  it('filters_apply', async () => {
    const { raw, platform, people, user, api } = await logScreen();
    await api.apply({ type: 'createPerson', draft: { given: 'Olena', surnames: ['Koval'], sex: 'female' } });
    platform.simulateAgentBatch(raw, { agent: 'Reader A', title: 'Recording 1', changes: [{ person: people[0].id, note: 'First' }] });
    platform.simulateAgentBatch(raw, { agent: 'Reader B', title: 'Recording 2', changes: [{ person: people[1].id, note: 'Second' }] });
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(3));

    await user.selectOptions(screen.getByRole('combobox', { name: 'Author' }), 'You');
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(1));
    expect(rowsOnScreen()[0]).toHaveTextContent('Add person');

    await user.selectOptions(screen.getByRole('combobox', { name: 'Author' }), 'Reader B');
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(1));
    expect(rowsOnScreen()[0]).toHaveTextContent('Second');

    await user.selectOptions(screen.getByRole('combobox', { name: 'Author' }), 'Everyone');
    await user.selectOptions(screen.getByRole('combobox', { name: 'Batch' }), 'Recording 1');
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(1));
    expect(rowsOnScreen()[0]).toHaveTextContent('First');

    await user.selectOptions(screen.getByRole('combobox', { name: 'Batch' }), 'All changes');
    await raw.markReviewed({ kind: 'person', id: people[0].id });
    await user.click(screen.getByRole('checkbox', { name: 'Not yet reviewed' }));
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(1));
    expect(rowsOnScreen()[0]).toHaveTextContent('Second');
  });

  it('batch_heading_shows_title_agent_counts_report_and_unfinished', async () => {
    const { raw, platform, people } = await logScreen();
    platform.simulateAgentBatch(raw, {
      agent: 'Transcript reader',
      title: 'Recording 2 summary',
      report: 'Could not settle: two Ivans in 1901.',
      open: true,
      changes: [
        { person: people[0].id, note: 'One' },
        { person: people[1].id, note: 'Two' },
      ],
    });
    const heading = await screen.findByRole('region', { name: 'Recording 2 summary' });
    const box = within(heading);
    expect(box.getByText(/By Transcript reader/)).toBeInTheDocument();
    expect(box.getByText(/2 changes/)).toBeInTheDocument();
    expect(box.getByText(/2 people/)).toBeInTheDocument();
    expect(box.getByText('Could not settle: two Ivans in 1901.')).toBeInTheDocument();
    expect(box.getByText('Unfinished')).toBeInTheDocument();
    expect(box.getByRole('button', { name: 'Mark reviewed' })).toBeInTheDocument();
    expect(box.getByRole('button', { name: 'Undo batch' })).toBeInTheDocument();
  });

  it('closed_batch_is_not_unfinished', async () => {
    const { raw, platform, people } = await logScreen();
    platform.simulateAgentBatch(raw, { title: 'Done', changes: [{ person: people[0].id, note: 'One' }] });
    const heading = await screen.findByRole('region', { name: 'Done' });
    expect(within(heading).queryByText('Unfinished')).toBeNull();
  });

  it('batch_review_clears_badges', async () => {
    const { raw, platform, people, user } = await logScreen();
    platform.simulateAgentBatch(raw, {
      title: 'Recording 2',
      changes: [
        { person: people[0].id, note: 'One' },
        { person: people[1].id, note: 'Two' },
      ],
    });
    const heading = await screen.findByRole('region', { name: 'Recording 2' });
    await waitFor(() => expect(agentChanges.unreviewed.size).toBe(2));
    await user.click(within(heading).getByRole('button', { name: 'Mark reviewed' }));
    await waitFor(() => expect(agentChanges.unreviewed.size).toBe(0));
    // The entries stay forever.
    expect(rowsOnScreen()).toHaveLength(2);
    await waitFor(() => expect(within(screen.getByRole('region', { name: 'Recording 2' })).queryByRole('button', { name: 'Mark reviewed' })).toBeNull());
  });

  it('person_review_clears_only_that_person', async () => {
    const { raw, api, platform, people, user } = await world();
    platform.simulateAgentBatch(raw, {
      title: 'Recording 2',
      changes: [
        { person: people[0].id, note: 'About the first', sources: ['Recording 2 transcript'] },
        { person: people[1].id, note: 'About the second' },
      ],
    });
    await waitFor(() => expect(agentChanges.unreviewed.size).toBe(2));
    render(AgentChangesPanel, { props: { id: people[0].id }, context: apiContext(api, platform) });
    expect(await screen.findByText(/About the first/)).toBeInTheDocument();
    expect(screen.getByText(/Recording 2 transcript/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Reviewed' }));
    await waitFor(() => expect(agentChanges.unreviewed.has(people[0].id)).toBe(false));
    expect(agentChanges.unreviewed.has(people[1].id)).toBe(true);
    // Reviewing removes the badge; what was added stays listed.
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Reviewed' })).toBeNull());
    expect(screen.getByText(/About the first/)).toBeInTheDocument();
  });

  it('undo_batch_shows_skipped', async () => {
    const { raw, platform, people, user } = await logScreen();
    platform.simulateAgentBatch(raw, {
      title: 'Recording 2',
      changes: [
        { person: people[0].id, note: 'Goes away' },
        { person: people[1].id, note: 'Owner edited this one', conflict: true },
      ],
    });
    const heading = await screen.findByRole('region', { name: 'Recording 2' });
    await user.click(within(heading).getByRole('button', { name: 'Undo batch' }));
    const ask = await screen.findByRole('dialog', { name: 'Undo this batch?' });
    await user.click(within(ask).getByRole('button', { name: 'Undo' }));
    const done = await screen.findByRole('dialog', { name: 'Undo finished' });
    expect(within(done).getByText('1 change undone')).toBeInTheDocument();
    const skipped = within(done).getByRole('list');
    expect(skipped).toHaveTextContent('Owner edited this one');
    expect(skipped).toHaveTextContent(people[1].name);
    expect(skipped).toHaveTextContent('Note');
    expect((await raw.getPerson(people[0].id)).notes.some((n) => n.text === 'Goes away')).toBe(false);
    expect((await raw.getPerson(people[1].id)).notes.some((n) => n.text === 'Owner edited this one')).toBe(true);
  });

  it('undo_row_confirms_and_then_is_disabled', async () => {
    const { raw, platform, people, user } = await logScreen();
    platform.simulateAgentBatch(raw, { changes: [{ person: people[0].id, note: 'Goes away' }] });
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(1));
    const row = () => rowsOnScreen().find((r) => within(r).queryByRole('button', { name: 'Undo' }))!;
    await user.click(within(row()).getByRole('button', { name: 'Undo' }));
    await user.click(within(await screen.findByRole('dialog', { name: 'Undo this change?' })).getByRole('button', { name: 'Undo' }));
    const done = await screen.findByRole('dialog', { name: 'Undo finished' });
    expect(within(done).getByText('1 change undone')).toBeInTheDocument();
    await user.click(within(done).getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(within(row()).getByRole('button', { name: 'Undo' })).toBeDisabled());
    expect(within(row()).getByText('Undone')).toBeInTheDocument();
    // The undo is itself logged under "You".
    expect(rowsOnScreen()[0]).toHaveTextContent('You');
    expect(agentChanges.unreviewed.size).toBe(0);
  });

  it('declining_the_undo_changes_nothing', async () => {
    const { raw, platform, people, user } = await logScreen();
    platform.simulateAgentBatch(raw, { changes: [{ person: people[0].id, note: 'Stays' }] });
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(1));
    const spy = vi.spyOn(raw, 'undoChangeGroup');
    await user.click(within(rowsOnScreen()[0]).getByRole('button', { name: 'Undo' }));
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Keep' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(spy).not.toHaveBeenCalled();
  });

  it('focus_stays_in_the_dialog_when_the_confirm_button_goes', async () => {
    const { raw, platform, people, user } = await logScreen();
    platform.simulateAgentBatch(raw, { changes: [{ person: people[0].id, note: 'Goes away' }] });
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(1));
    await user.click(within(rowsOnScreen()[0]).getByRole('button', { name: 'Undo' }));
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Undo' }));
    const done = await screen.findByRole('dialog', { name: 'Undo finished' });
    await waitFor(() => expect(done.contains(document.activeElement)).toBe(true));
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    // The row's Undo is disabled now, so focus lands on the screen's heading, not on nothing.
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole('heading', { level: 1, name: 'Change log' })));
  });

  it('undo_failure_shows_in_the_dialog_and_keeps_focus', async () => {
    const { raw, platform, people, user } = await logScreen();
    platform.simulateAgentBatch(raw, { changes: [{ person: people[0].id, note: 'Stays' }] });
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(1));
    vi.spyOn(raw, 'undoChangeGroup').mockRejectedValue({ code: 'io', message: 'The disk is full.' });
    await user.click(within(rowsOnScreen()[0]).getByRole('button', { name: 'Undo' }));
    const ask = await screen.findByRole('dialog');
    await user.click(within(ask).getByRole('button', { name: 'Undo' }));
    expect(await within(ask).findByRole('alert')).toHaveTextContent('The disk is full.');
    expect(ask.contains(document.activeElement)).toBe(true);
  });

  it('refreshes_on_agent_event', async () => {
    const { raw, platform, people } = await logScreen();
    expect(await screen.findByText('No changes in this tree yet.')).toBeInTheDocument();
    const rev = project.rev;
    const fired = platform.simulateAgentBatch(raw, { agent: 'Transcript reader', changes: [{ person: people[0].id, note: 'While you watch' }] });
    expect(project.rev).toBe(rev + 1);
    expect(agentChanges.lastTouched).toEqual(fired.touched);
    expect(fired.touched).toContain(people[0].id);
    expect(fired.agent).toBe('Transcript reader');
    expect(await screen.findByText(/While you watch/)).toBeInTheDocument();
    await waitFor(() => expect(agentChanges.unreviewed.has(people[0].id)).toBe(true));
  });

  it('person_gone_from_the_tree_is_named_without_a_link', async () => {
    const { raw, platform, people, api } = await logScreen();
    platform.simulateAgentBatch(raw, { changes: [{ person: people[2].id, note: 'Left behind' }] });
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(1));
    const name = people[2].name;
    await api.apply({ type: 'deletePerson', id: people[2].id });
    await waitFor(() => expect(rowsOnScreen().length).toBeGreaterThan(1));
    const row = rowsOnScreen().find((r) => r.textContent?.includes('Left behind'))!;
    expect(within(row).queryByRole('button', { name })).toBeNull();
    expect(row).toHaveTextContent(`${name} (no longer in the tree)`);
  });

  const manyNotes = (n: number, person: Id) => Array.from({ length: n }, (_, i) => ({ person, note: `Note ${i}` }));

  it('author_filter_names_an_agent_whose_rows_are_not_loaded', async () => {
    const { raw, api, platform, people } = await world();
    platform.simulateAgentBatch(raw, { agent: 'Old reader', changes: [{ person: people[0].id, note: 'Long ago' }] });
    platform.simulateAgentBatch(raw, { agent: 'New reader', changes: manyNotes(60, people[1].id) });
    render(ChangeLog, { context: apiContext(api, platform) });
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(50));
    expect(rowsOnScreen().some((r) => r.textContent?.includes('Old reader'))).toBe(false);
    const box = screen.getByRole('combobox', { name: 'Author' });
    await waitFor(() => expect(within(box).getByRole('option', { name: 'Old reader' })).toBeInTheDocument());
    expect(within(box).getByRole('option', { name: 'New reader' })).toBeInTheDocument();
  });

  it('a_live_refresh_during_show_more_keeps_the_longer_list', async () => {
    const { raw, platform, people, user } = await logScreen();
    platform.simulateAgentBatch(raw, { changes: manyNotes(120, people[0].id) });
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(50));
    // The agent writes again while "Show more" is still fetching its first page.
    const real = raw.changeLog.bind(raw);
    let release: () => void = () => {};
    const held = new Promise<void>((res) => (release = res));
    vi.spyOn(raw, 'changeLog').mockImplementationOnce(async (f, offset, limit) => {
      await held;
      return real(f, offset, limit);
    });
    await user.click(screen.getByRole('button', { name: 'Show more' }));
    project.rev++;
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(100));
    release();
  });

  it('a_refresh_after_show_more_keeps_the_longer_list', async () => {
    const { raw, platform, people, user } = await logScreen();
    platform.simulateAgentBatch(raw, { changes: manyNotes(120, people[0].id) });
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(50));
    await user.click(screen.getByRole('button', { name: 'Show more' }));
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(100));
    project.rev++;
    await new Promise((r) => setTimeout(r, 20));
    expect(rowsOnScreen()).toHaveLength(100);
  });

  it('a_new_filter_starts_from_one_page_again', async () => {
    const { raw, platform, people, user } = await logScreen();
    platform.simulateAgentBatch(raw, { changes: manyNotes(120, people[0].id) });
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(50));
    await user.click(screen.getByRole('button', { name: 'Show more' }));
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(100));
    await user.click(screen.getByRole('checkbox', { name: 'Not yet reviewed' }));
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(50));
  });

  it('a_write_between_pages_does_not_show_a_row_twice', async () => {
    const { raw, platform, people, user } = await logScreen();
    platform.simulateAgentBatch(raw, { changes: manyNotes(120, people[0].id) });
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(50));
    // A write landed after the first page: everything slid down by one, so page two repeats a row.
    const real = raw.changeLog.bind(raw);
    vi.spyOn(raw, 'changeLog').mockImplementation((f, offset, limit) => real(f, offset === 50 ? 49 : offset, limit));
    await user.click(screen.getByRole('button', { name: 'Show more' }));
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(100));
    const texts = rowsOnScreen().map((r) => /Note \d+/.exec(r.textContent ?? '')?.[0]);
    expect(new Set(texts).size).toBe(100);
  });

  it('closing_the_dialog_while_the_undo_runs_keeps_the_report', async () => {
    const { raw, platform, people, user } = await logScreen();
    platform.simulateAgentBatch(raw, { changes: [{ person: people[0].id, note: 'Goes away' }] });
    await waitFor(() => expect(rowsOnScreen()).toHaveLength(1));
    let finish: (r: { undone: number; skipped: never[] }) => void = () => {};
    vi.spyOn(raw, 'undoChangeGroup').mockImplementation(() => new Promise((res) => (finish = res)));
    await user.click(within(rowsOnScreen()[0]).getByRole('button', { name: 'Undo' }));
    const ask = await screen.findByRole('dialog');
    await user.click(within(ask).getByRole('button', { name: 'Undo' }));
    await user.keyboard('{Escape}');
    expect(screen.getByRole('dialog', { name: 'Undo this change?' })).toBeInTheDocument();
    expect(within(ask).getByRole('button', { name: 'Keep' })).toBeDisabled();
    finish({ undone: 1, skipped: [] });
    const done = await screen.findByRole('dialog', { name: 'Undo finished' });
    expect(within(done).getByText('1 change undone')).toBeInTheDocument();
  });

  it('undo_batch_goes_disabled_once_all_of_it_is_undone', async () => {
    const { raw, platform, people, user } = await logScreen();
    platform.simulateAgentBatch(raw, {
      title: 'Recording 2',
      changes: [
        { person: people[0].id, note: 'One' },
        { person: people[1].id, note: 'Two' },
      ],
    });
    const heading = await screen.findByRole('region', { name: 'Recording 2' });
    const button = () => within(screen.getByRole('region', { name: 'Recording 2' })).getByRole('button', { name: 'Undo batch' });
    expect(within(heading).getByRole('button', { name: 'Undo batch' })).toBeEnabled();

    // One change back: the rest of the batch can still go.
    const row = within(heading).getAllByRole('listitem')[0];
    await user.click(within(row).getByRole('button', { name: 'Undo' }));
    await user.click(within(await screen.findByRole('dialog', { name: 'Undo this change?' })).getByRole('button', { name: 'Undo' }));
    await user.click(within(await screen.findByRole('dialog', { name: 'Undo finished' })).getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(button()).toBeEnabled();

    await user.click(button());
    await user.click(within(await screen.findByRole('dialog', { name: 'Undo this batch?' })).getByRole('button', { name: 'Undo' }));
    await user.click(within(await screen.findByRole('dialog', { name: 'Undo finished' })).getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(button()).toBeDisabled());
  });

  it('agent_changes_stay_off_the_ctrl_z_stack', async () => {
    const { raw, platform, people } = await world();
    platform.simulateAgentBatch(raw, { changes: [{ person: people[0].id, note: 'Not mine' }] });
    expect((await raw.undoState()).canUndo).toBe(false);
  });
});

describe('agent badges', () => {
  const row: Row = {
    id: 'p1',
    name: 'Olena Koval',
    lifespan: '',
    living: false,
    private: false,
    sourceCount: 0,
    sex: 'female',
  } as unknown as Row;
  const theme: Theme = { paper: '#fff', rule: '#ccc', ink: '#000', ink2: '#666', ribbon: '#f00', wash: '#fee', font: 'sans-serif', serif: 'serif' };
  const node = (id: number, person: Id) => ({
    id,
    person: { id: person, name: person, lifespan: '', sex: 'female' as const, living: false, private: false },
    x: id * 200,
    y: 0,
    w: 176,
    h: 56,
    gen: 0,
    ahnen: null,
    dupOf: null,
    moreUp: false,
    moreDown: false,
    nav: { parent: null, child: null, prev: null, next: null },
    label: '',
  });
  const win = { nodes: [node(0, 'p1'), node(1, 'p2')], edges: [] };
  const arcs = (ctx: ReturnType<typeof makeFakeContext>) => ctx.calls.filter((c) => c.op === 'arc').map((c) => c.args[0]);

  it('badge_on_card_row_and_chart', async () => {
    const card = render(PersonCard, { props: { name: 'Olena Koval', relationship: 'Mother', agentChanged: true, onclick: () => {} } });
    expect(card.getByRole('button', { name: /Changed by an agent/ })).toBeInTheDocument();
    card.unmount();

    const plainCard = render(PersonCard, { props: { name: 'Olena Koval', onclick: () => {} } });
    expect(plainCard.queryByText('Agent')).toBeNull();
    plainCard.unmount();

    const listRow = render(PersonRow, { props: { row, agentChanged: true } });
    expect(listRow.getByText('Agent')).toBeInTheDocument();
    listRow.unmount();

    const ctx = makeFakeContext();
    draw(ctx, win, { x: 0, y: 0, zoom: 1 }, { w: 800, h: 600, dpr: 1 }, { selected: null, focus: null, theme, handles: false, badged: new Set(['p2']) });
    expect(arcs(ctx)).toEqual([200 + 176]);

    const dots = makeFakeContext();
    draw(dots, win, { x: 0, y: 0, zoom: 0.1 }, { w: 800, h: 600, dpr: 1 }, { selected: null, focus: null, theme, handles: false, badged: new Set(['p1']) });
    expect(arcs(dots)).toEqual([176]);

    const none = makeFakeContext();
    draw(none, win, { x: 0, y: 0, zoom: 1 }, { w: 800, h: 600, dpr: 1 }, { selected: null, focus: null, theme, handles: false });
    expect(arcs(none)).toEqual([]);

    const whole = makeFakeContext();
    const wholeTheme = { ...theme, male: '#00f', female: '#f0f', other: '#999' } as unknown as WholeTheme;
    drawWhole(whole, win, [], { x: 0, y: 0, zoom: 1 }, { w: 800, h: 600, dpr: 1 }, { selected: null, home: null, lineage: null, theme: wholeTheme, badged: new Set(['p1']) });
    expect(arcs(whole)).toEqual([176]);
  });

  it('people_list_family_tree_and_person_page_badge_follow_the_store', async () => {
    const { raw, api, platform, people, user } = await world();
    await settings.load(api);
    platform.simulateAgentBatch(raw, { changes: [{ person: people[0].id, note: 'One' }] });
    await waitFor(() => expect(agentChanges.unreviewed.has(people[0].id)).toBe(true));

    const list = render(People, { context: apiContext(api, platform) });
    await waitFor(() => expect(screen.getAllByText('Agent')).toHaveLength(1));
    list.unmount();

    const view = await raw.getFamilyView(people[0].id);
    const tree = render(FamilyTree, {
      props: { view, homeId: null, onopen: () => {}, onadd: () => {} },
      context: apiContext(api, platform),
    });
    await waitFor(() => expect(screen.getAllByRole('button', { name: /Changed by an agent/ }).length).toBeGreaterThan(0));
    tree.unmount();

    render(PersonEditor, { props: { id: people[0].id }, context: apiContext(api, platform) });
    const panel = await screen.findByRole('region', { name: 'What an agent changed' });
    expect(within(panel).getByText(/One/)).toBeInTheDocument();
    expect(within(screen.getByRole('heading', { level: 1 })).getByTitle('An agent changed this person. Not yet reviewed.')).toBeInTheDocument();
    await user.click(within(panel).getByRole('button', { name: 'Reviewed' }));
    await waitFor(() => expect(screen.queryAllByTitle('An agent changed this person. Not yet reviewed.')).toEqual([]));
  });
});

describe('an editor open on a changed person', () => {
  it('stale_editor_blocks_save', async () => {
    const { raw, api, platform, people, user } = await world();
    await settings.load(api);
    render(PersonEditor, { props: { id: people[0].id }, context: apiContext(api, platform) });
    await user.click(await screen.findByRole('button', { name: 'Add name' }));
    const dlg = await screen.findByRole('dialog', { name: 'Add name' });
    await user.type(within(dlg).getByRole('textbox', { name: 'Given name' }), 'Typed');
    const save = () => within(screen.getByRole('dialog')).getByRole('button', { name: 'Save name' });
    expect(save()).toBeEnabled();

    // Someone else's change does not get in the way.
    platform.simulateAgentBatch(raw, { changes: [{ person: people[1].id, note: 'Elsewhere' }] });
    await waitFor(() => expect(agentChanges.seq).toBe(1));
    expect(save()).toBeEnabled();
    expect(screen.queryByText('An agent changed this person. Reload to continue.')).toBeNull();

    // This person's does.
    save().focus();
    platform.simulateAgentBatch(raw, { changes: [{ person: people[0].id, note: 'Underneath you' }] });
    expect(await screen.findByText('An agent changed this person. Reload to continue.')).toBeInTheDocument();
    expect(save()).toBeDisabled();
    const reload = within(screen.getByRole('dialog')).getByRole('button', { name: 'Reload' });
    // The focused Save went disabled; focus must not be stranded on it.
    await waitFor(() => expect(document.activeElement).toBe(reload));
    const spy = vi.spyOn(raw, 'apply');
    await user.keyboard('{Control>}s{/Control}');
    expect(spy).not.toHaveBeenCalled();

    await user.click(reload);
    // What was typed stays until the owner agrees to lose it.
    expect(within(screen.getByRole('dialog')).getByRole('textbox', { name: 'Given name' })).toHaveValue('Typed');
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Discard changes' }));
    await waitFor(() => expect(screen.queryByText('An agent changed this person. Reload to continue.')).toBeNull());
    expect(save()).toBeEnabled();
    // The dialog came back with what the tree says now, and focus inside it.
    expect(within(screen.getByRole('dialog')).getByRole('textbox', { name: 'Given name' })).toHaveValue('');
    expect(screen.getByRole('dialog').contains(document.activeElement)).toBe(true);
  });

  it('enter_key_does_not_save_while_stale', async () => {
    const { raw, platform, people, user } = await world();
    stop?.();
    const spy = vi.spyOn(raw, 'apply');
    renderWithApi(raw, NameEditor, { person: people[0].id, onCancel: () => {}, onSaved: () => {} });
    stop = agentChanges.start(raw, platform);
    const given = await screen.findByRole('textbox', { name: 'Given name' });
    await user.type(given, 'Typed');
    platform.simulateAgentBatch(raw, { changes: [{ person: people[0].id, note: 'Underneath you' }] });
    await screen.findByText('An agent changed this person. Reload to continue.');
    await user.type(given, '{Enter}');
    expect(spy).not.toHaveBeenCalled();
  });
});

describe('an editor no page can reopen', () => {
  it('reload_closes_it_when_nothing_was_typed', async () => {
    const { raw, platform, people, user } = await world();
    stop?.();
    const onCancel = vi.fn();
    renderWithApi(raw, NameEditor, { person: people[0].id, onCancel, onSaved: () => {} });
    stop = agentChanges.start(raw, platform);
    platform.simulateAgentBatch(raw, { changes: [{ person: people[0].id, note: 'Underneath you' }] });
    await user.click(await screen.findByRole('button', { name: 'Reload' }));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it('reload_asks_before_it_drops_typed_text', async () => {
    const { raw, platform, people, user } = await world();
    stop?.();
    const onCancel = vi.fn();
    renderWithApi(raw, NameEditor, { person: people[0].id, onCancel, onSaved: () => {} });
    stop = agentChanges.start(raw, platform);
    const given = await screen.findByRole('textbox', { name: 'Given name' });
    await user.type(given, 'Typed');
    platform.simulateAgentBatch(raw, { changes: [{ person: people[0].id, note: 'Underneath you' }] });
    await user.click(await screen.findByRole('button', { name: 'Reload' }));
    expect(onCancel).not.toHaveBeenCalled();
    expect(screen.getByText('Discard changes?')).toBeInTheDocument();
    expect(given).toHaveValue('Typed');

    // No: everything stays, including the way back to Reload.
    await user.click(screen.getByRole('button', { name: 'Keep editing' }));
    expect(onCancel).not.toHaveBeenCalled();
    expect(given).toHaveValue('Typed');
    const again = screen.getByRole('button', { name: 'Reload' });
    await waitFor(() => expect(document.activeElement).toBe(again));

    // Yes.
    await user.click(again);
    expect(onCancel).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Discard changes' }));
    expect(onCancel).toHaveBeenCalledOnce();
  });
});

describe('in Ukrainian', () => {
  type Tree = { [k: string]: string | Tree };
  const leaves = (n: Tree): string[] => Object.values(n).flatMap((v) => (typeof v === 'string' ? [v] : leaves(v)));

  it('ukrainian_strings_use_rodovid', () => {
    const texts = leaves(uk.changeLog as unknown as Tree);
    expect(texts.length).toBeGreaterThan(20);
    expect(texts.filter((s) => /дерев/i.test(s))).toEqual([]);
    expect(texts.some((s) => /родовод/i.test(s))).toBe(true);
    expect(uk.nav.log).toBe('Журнал змін');
    expect(en.nav.log).toBe('Change log');
    expect(uk.changeLog.beforeTracking).toBe('До початку обліку');
    expect(texts.filter((s) => /[A-Za-z]{2}/.test(s.replace(/\{\w+\}/g, '')))).toEqual([]);
  });

  it('fixed_text_of_a_row_is_translated_and_user_data_stays', async () => {
    const { raw, platform, people, api } = await world();
    setLanguage('uk');
    platform.simulateAgentBatch(raw, {
      agent: 'Transcript reader',
      title: 'Recording 2',
      report: 'Left as written',
      open: true,
      changes: [
        {
          person: people[0].id,
          note: 'Kept as written',
          label: 'Save event',
          op: 'create',
          kind: 'event',
          fact: { kind: 'event', event: 'birth', label: 'Birth', date: 'about 3 Apr 1901', place: 'Lviv' },
          sources: ['Register of Lviv'],
        },
        { person: people[1].id, note: 'x', label: 'Save family', kind: 'family', op: 'update', fact: { kind: 'family', partners: ['Ivan Koval', 'Unknown'] } },
        { person: people[1].id, note: 'y', label: 'Save family', kind: 'family', op: 'delete', fact: { kind: 'family', partners: [] } },
        { person: people[1].id, note: 'z', label: 'Undo: Save note', kind: 'note', fact: { kind: 'text', text: 'between 1 Apr 1901 and 2 May 1902' } },
      ],
    });
    render(ChangeLog, { context: apiContext(api, platform) });
    await screen.findByRole('heading', { level: 1, name: 'Журнал змін' });
    await waitFor(() => expect(within(screen.getByRole('group', { name: 'Зміни' })).queryAllByRole('listitem')).toHaveLength(4));
    const all = document.body.textContent ?? '';
    expect(all).toContain('Народження');
    expect(all).not.toContain('Birth');
    expect(all).not.toContain('about');
    expect(all).toContain('близько 3 квіт');
    expect(all).toContain('Lviv');
    expect(all).toContain('Ivan Koval і Невідомо');
    expect(all).toContain('Подія');
    expect(all).toContain('Додано');
    expect(all).toContain('Вилучено');
    expect(all).toContain('Не завершено');
    expect(all).toContain('Register of Lviv');
    expect(all).toContain('Left as written');
    // A note's own words are data, even when they read like a date.
    expect(all).toContain('between 1 Apr 1901 and 2 May 1902');
    expect(all).not.toMatch(/\b(Save|Undo|Family|Event|Note|Unfinished|Unknown)\b/);
  });
});
