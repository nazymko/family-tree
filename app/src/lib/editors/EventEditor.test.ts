import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import EventEditor from './EventEditor.svelte';
import { ALL, findPerson, newApi, renderWithApi } from './testing/helpers';
import type { EventDto } from '../api/types';

let api: ReturnType<typeof newApi>;
let pid: string;

beforeEach(async () => {
  api = newApi(80);
  pid = (await findPerson(api, (d) => d.events.length > 0)).ref.id;
});

function show(extra: Record<string, unknown> = {}) {
  const onSaved = vi.fn();
  const onCancel = vi.fn();
  renderWithApi(api, EventEditor, { owner: { kind: 'person', id: pid }, onSaved, onCancel, ...extra });
  return { onSaved, onCancel };
}

function lastEvent(spy: { mock: { calls: unknown[][] } }): Record<string, unknown> {
  return (spy.mock.calls[0][0] as { event: Record<string, unknown> }).event;
}

describe('EventEditor', () => {
  it('a_birth_takes_its_date_in_parts_and_may_have_none', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    const { onSaved } = show();
    await user.selectOptions(screen.getByRole('combobox', { name: 'Kind of event' }), 'birth');
    await user.type(screen.getByRole('textbox', { name: 'Year' }), '1850');
    await user.selectOptions(screen.getByRole('combobox', { name: 'Month' }), '3');
    await user.clear(screen.getByRole('textbox', { name: 'Year' }));
    await user.type(screen.getByRole('combobox', { name: 'Place' }), 'Berlin, Brandenburg, Germany');
    await user.click(screen.getByRole('button', { name: 'Save event' }));
    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect(lastEvent(spy).dateRaw).toBeUndefined();
  });

  it('other_events_keep_the_text_field', async () => {
    show();
    await userEvent.setup().selectOptions(screen.getByRole('combobox', { name: 'Kind of event' }), 'death');
    expect(screen.getByRole('textbox', { name: 'Date' })).toBeTruthy();
    expect(screen.queryByRole('textbox', { name: 'Year' })).toBeNull();
  });

  it('birth_event_saves_expected_command', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    const { onSaved } = show();
    await user.selectOptions(screen.getByRole('combobox', { name: 'Kind of event' }), 'birth');
    await user.click(screen.getByRole('button', { name: /Type it another way/ }));
    await user.type(screen.getByRole('textbox', { name: 'Date' }), 'abt 1850');
    await user.type(screen.getByRole('combobox', { name: 'Place' }), 'Berlin, Brandenburg, Germany');
    await user.click(screen.getByRole('button', { name: 'Save event' }));
    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect(spy.mock.calls[0][0]).toMatchObject({ type: 'saveEvent' });
    expect(lastEvent(spy)).toMatchObject({
      kind: 'birth',
      dateRaw: 'abt 1850',
      placeText: 'Berlin, Brandenburg, Germany',
      negative: false,
    });
  });

  it('cause_hidden_for_birth', async () => {
    const user = userEvent.setup();
    show();
    expect(screen.queryByRole('textbox', { name: 'Cause' })).toBeNull();
    await user.selectOptions(screen.getByRole('combobox', { name: 'Kind of event' }), 'death');
    expect(screen.getByRole('textbox', { name: 'Cause' })).toBeInTheDocument();
  });

  it('custom_kind_prefix', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    show();
    await user.selectOptions(screen.getByRole('combobox', { name: 'Kind of event' }), 'custom');
    await user.type(screen.getByRole('textbox', { name: 'Name of the event or fact' }), 'Baptism of adult');
    await user.click(screen.getByRole('button', { name: 'Save event' }));
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(lastEvent(spy).kind).toBe('custom:Baptism of adult');
  });

  it('negative_checkbox_helper_text', () => {
    show();
    const cb = screen.getByRole('checkbox', { name: 'This event did not happen' });
    expect(cb).toHaveAccessibleDescription('Records that the person never married, had no children, and so on');
  });

  it('participant_role_defaults_to_witness', async () => {
    const user = userEvent.setup();
    show();
    const other = (await api.listPersons(ALL, 5, 1)).items[0];
    await user.type(screen.getByRole('combobox', { name: 'Add participant' }), other.surname.slice(0, 4));
    const list = await screen.findByRole('listbox', { name: 'Add participant' });
    await user.click(within(list).getAllByRole('option')[0]);
    const sel = await screen.findByRole('combobox', { name: /^Role of / });
    expect(sel).toHaveValue('witness');
  });

  it('enter_saves', async () => {
    const user = userEvent.setup();
    const a = show();
    await user.type(screen.getByRole('textbox', { name: 'Description' }), 'x{Enter}');
    await waitFor(() => expect(a.onSaved).toHaveBeenCalledTimes(1));
  });

  it('esc_cancels_when_clean', async () => {
    const user = userEvent.setup();
    const a = show();
    await user.keyboard('{Escape}');
    expect(a.onCancel).toHaveBeenCalled();
  });

  it('editing_existing_event_keeps_id', async () => {
    const user = userEvent.setup();
    const d = await api.getPerson(pid);
    const ev: EventDto = d.events[0];
    const spy = vi.spyOn(api, 'apply');
    show({ event: ev });
    const dlg = screen.getByRole('dialog');
    await user.type(within(dlg).getByRole('textbox', { name: 'Description' }), ' more');
    await user.click(within(dlg).getByRole('button', { name: 'Save event' }));
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(lastEvent(spy).id).toBe(ev.id);
  });

  it('clearing_the_place_sends_empty_place_text_and_the_mock_clears_it', async () => {
    const user = userEvent.setup();
    const d = await findPerson(api, (x) => x.events.some((e) => e.place !== null));
    const ev = d.events.find((e) => e.place !== null)!;
    const spy = vi.spyOn(api, 'apply');
    const onSaved = vi.fn();
    renderWithApi(api, EventEditor, { owner: { kind: 'person', id: d.ref.id }, event: ev, onSaved, onCancel: vi.fn() });
    await user.clear(screen.getByRole('combobox', { name: 'Place' }));
    await user.click(screen.getByRole('button', { name: 'Save event' }));
    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect(lastEvent(spy).placeText).toBe('');
    const after = (await api.getPerson(d.ref.id)).events.find((e) => e.id === ev.id)!;
    expect(after.place).toBeNull();
  });
});
