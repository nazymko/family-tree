import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import FamilyEditor from './FamilyEditor.svelte';
import { findPerson, newApi, renderWithApi } from './testing/helpers';
import type { PartnershipDto, PersonDetail } from '../api/types';

let api: ReturnType<typeof newApi>;
let owner: PersonDetail;
let ps: PartnershipDto;

beforeEach(async () => {
  api = newApi(500);
  owner = await findPerson(api, (d) => d.partnerships.some((p) => p.children.length >= 3 && p.partner !== null));
  ps = owner.partnerships.find((p) => p.children.length >= 3 && p.partner)!;
});

function show(props: Record<string, unknown> = {}) {
  const onDone = vi.fn();
  const onCancel = vi.fn();
  renderWithApi(api, FamilyEditor, { family: ps.familyId, anchor: owner.ref.id, onDone, onCancel, ...props });
  return { onDone, onCancel };
}

async function pick(user: ReturnType<typeof userEvent.setup>, label: string, text: string) {
  await user.type(screen.getByRole('combobox', { name: label }), text);
  const list = await screen.findByRole('listbox', { name: label });
  await user.click(within(list).getAllByRole('option')[0]);
}

async function mkPerson(given: string, surname: string, sex: 'male' | 'female') {
  await api.apply({ type: 'createPerson', draft: { given, surnames: [surname], sex } });
}

async function loaded() {
  await screen.findByRole('group', { name: new RegExp(`^${ps.children[0].person.name}, child 1`) });
}

describe('FamilyEditor', () => {
  it('family_notes_are_shown_added_and_deleted', async () => {
    const user = userEvent.setup();
    await api.apply({ type: 'saveNote', target: { kind: 'family', id: ps.familyId }, note: { kind: 'general', text: 'They moved in spring.' } });
    show();
    const text = await screen.findByText('They moved in spring.');
    await user.click(within(text.closest('article') as HTMLElement).getByRole('button', { name: /Delete/ }));
    await waitFor(() => expect(screen.queryByText('They moved in spring.')).toBeNull());
    expect((await api.getFamily(ps.familyId)).notes).toEqual([]);
  });

  it('same_sex_couple_allowed', async () => {
    const user = userEvent.setup();
    await mkPerson('Alpha', 'Zzmaleone', 'male');
    await mkPerson('Beta', 'Zzmaletwo', 'male');
    const spy = vi.spyOn(api, 'apply');
    const { onDone } = show({ family: undefined, anchor: undefined });
    await pick(user, 'Partner 1', 'Zzmaleone');
    await pick(user, 'Partner 2', 'Zzmaletwo');
    await user.click(screen.getByRole('button', { name: 'Save family' }));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    const cmd = spy.mock.calls[0][0] as { type: string; partner1: string; partner2: string };
    expect(cmd.type).toBe('saveFamily');
    expect(cmd.partner1).toBeTruthy();
    expect(cmd.partner2).toBeTruthy();
  });

  it('empty_slot_allowed_one_partner_only', async () => {
    const user = userEvent.setup();
    await mkPerson('Solo', 'Zzsolo', 'female');
    const spy = vi.spyOn(api, 'apply');
    const { onDone } = show({ family: undefined, anchor: undefined });
    await pick(user, 'Partner 1', 'Zzsolo');
    await user.click(screen.getByRole('button', { name: 'Save family' }));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(spy.mock.calls[0][0]).toMatchObject({ type: 'saveFamily', partner2: null });
    expect(screen.getByText('Leave a slot empty when the partner is unknown')).toBeInTheDocument();
  });

  it('cannot_pick_same_person_twice', async () => {
    const user = userEvent.setup();
    await mkPerson('Twin', 'Zztwin', 'female');
    show({ family: undefined, anchor: undefined });
    await pick(user, 'Partner 1', 'Zztwin');
    await user.type(screen.getByRole('combobox', { name: 'Partner 2' }), 'Zztwin');
    expect(await screen.findByText('Choose a different person')).toBeInTheDocument();
    expect(screen.queryByRole('listbox', { name: 'Partner 2' })).toBeNull();
  });

  it('per_parent_relationship_payload', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    show();
    await loaded();
    const first = ps.children[0].person;
    const row = screen.getByRole('group', { name: new RegExp(`^${first.name}, child 1`) });
    const selects = within(row).getAllByRole('combobox', { name: /^Relationship to / });
    expect(selects).toHaveLength(2);
    await user.selectOptions(selects[1], 'adopted');
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(spy.mock.calls[0][0]).toMatchObject({
      type: 'updateChildLink',
      family: ps.familyId,
      child: first.id,
      rel: [ps.children[0].rel[0], 'adopted'],
    });
  });

  it('reorder_with_keyboard', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    show();
    await loaded();
    const [c1, c2, c3] = ps.children.map((c) => c.person);
    const row = screen.getByRole('group', { name: new RegExp(`^${c1.name}, child 1`) });
    row.focus();
    await user.keyboard('{Alt>}{ArrowDown}{/Alt}');
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(spy.mock.calls[0][0]).toEqual({
      type: 'reorderChildren',
      family: ps.familyId,
      order: [c2.id, c1.id, ...ps.children.slice(2).map((c) => c.person.id)],
    });
    expect(c3).toBeTruthy();
    await waitFor(() =>
      expect(screen.getByRole('group', { name: new RegExp(`^${c1.name}, child 2`) })).toHaveFocus(),
    );
  });

  it('preferred_parents_checkbox_payload', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    show();
    await loaded();
    const first = ps.children[0].person;
    const row = screen.getByRole('group', { name: new RegExp(`^${first.name}, child 1`) });
    const cb = within(row).getByRole('checkbox', { name: 'Preferred parents for this child' });
    const was = (cb as HTMLInputElement).checked;
    await user.click(cb);
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(spy.mock.calls[0][0]).toMatchObject({ type: 'updateChildLink', child: first.id, preferred: !was });
  });

  it('divorce_is_an_event_not_a_flag', async () => {
    const user = userEvent.setup();
    show();
    await loaded();
    expect(screen.queryByRole('checkbox', { name: /divorc/i })).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Add event' }));
    const dlg = screen.getByRole('dialog', { name: 'Add event' });
    expect(within(dlg).getByRole('option', { name: 'Divorce' })).toBeInTheDocument();
  });

  it('delete_family_keeps_children_message', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    const { onDone } = show();
    await loaded();
    await user.click(screen.getByRole('button', { name: 'Delete family' }));
    expect(screen.getByText(/Children are kept/)).toBeInTheDocument();
    expect(spy).not.toHaveBeenCalled();
    const dlg = screen.getByRole('dialog', { name: 'Delete this family?' });
    await user.click(within(dlg).getByRole('button', { name: 'Delete family' }));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(spy).toHaveBeenCalledWith({ type: 'deleteFamily', id: ps.familyId });
  });

  it('cannot_make_person_their_own_parent', async () => {
    const user = userEvent.setup();
    show();
    await loaded();
    const child = ps.children[0].person;
    const p2 = screen.getByRole('combobox', { name: 'Partner 2' });
    await user.clear(p2);
    await user.type(p2, child.name);
    const list = await screen.findByRole('listbox', { name: 'Partner 2' });
    const opt = within(list)
      .getAllByRole('option')
      .find((o) => o.textContent?.includes(child.name));
    if (!opt) throw new Error('child not offered by search');
    await user.click(opt);
    expect(await screen.findByText('A person cannot be their own parent.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save family' })).toBeDisabled();
  });
});
