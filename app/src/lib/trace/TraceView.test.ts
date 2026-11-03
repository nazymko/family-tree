import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import TraceView from './TraceView.svelte';
import { apiContext } from '../api/context';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { nav } from '../state/nav.svelte';
import { project } from '../state/project.svelte';
import { notices } from '../ui/notices.svelte';
import type { EditCommand } from '../api/types';

async function setup(opts: { picture?: boolean } = {}) {
  const api = new MockApi(fixtureTree(80));
  const platform = new MockPlatform();
  project.open(await api.openProject('C:/Genealogy/Test'));
  nav.reset();
  notices.dismiss();
  if (opts.picture !== false) await api.stageTraceImage('C:/Mock/wall chart.png');
  const user = userEvent.setup();
  const view = render(TraceView, { context: apiContext(api, platform) });
  return { api, platform, user, view };
}

/** Drags a box on the picture (the test viewport has no size: 1 screen pixel is 1 picture pixel). */
async function drag(x0: number, y0: number, x1: number, y1: number) {
  const vp = await screen.findByRole('application');
  await fireEvent.mouseDown(vp, { clientX: x0, clientY: y0, button: 0 });
  await fireEvent.mouseMove(window, { clientX: x1, clientY: y1 });
  await fireEvent.mouseUp(window, { clientX: x1, clientY: y1 });
  return vp;
}

async function markPerson(user: ReturnType<typeof userEvent.setup>, at: [number, number], given: string, surname: string) {
  await drag(at[0], at[1], at[0] + 150, at[1] + 40);
  const form = await screen.findByRole('group', { name: /Person on the picture/ });
  await user.type(within(form).getByLabelText('Given name'), given);
  await user.type(within(form).getByLabelText('Surname'), surname);
  return form;
}

const cardIn = (vp: HTMLElement, name: RegExp | string) => within(vp).getByRole('button', { name });

beforeEach(() => {
  nav.reset();
  project.close();
});

describe('TraceView', () => {
  it('asks_for_a_picture_first', async () => {
    const { platform, user } = await setup({ picture: false });
    platform.next.file = 'C:/Mock/wall chart.png';
    await user.click(await screen.findByRole('button', { name: 'Choose a picture…' }));
    expect(await screen.findByRole('application')).toBeInTheDocument();
    expect(platform.calls[0].args[0]).toMatchObject({ extensions: expect.arrayContaining(['jpg', 'png', 'webp', 'tiff', 'gif']) });
  });

  it('says_so_when_the_file_is_not_a_picture', async () => {
    const { platform, user } = await setup({ picture: false });
    platform.next.file = 'C:/Mock/notes.txt';
    await user.click(await screen.findByRole('button', { name: 'Choose a picture…' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('not a picture');
  });

  it('marking_a_person_makes_a_card_with_a_form_that_needs_only_a_name', async () => {
    const { user } = await setup();
    const form = await markPerson(user, [200, 100], 'Ivan', 'Koval');
    expect(within(form).getByLabelText('Patronymic')).toBeInTheDocument();
    expect(within(form).getByLabelText('Birth year')).toBeInTheDocument();
    expect(within(form).getByLabelText('Death year')).toBeInTheDocument();
    expect(within(form).getByLabelText('Notes')).toBeInTheDocument();
    expect(within(form).getByLabelText('Sex')).toBeInTheDocument();
    const vp = screen.getByRole('application');
    expect(cardIn(vp, 'Ivan Koval')).toBeInTheDocument();
    expect(screen.getByRole('status', { name: '' })).toBeTruthy();
    expect(screen.getByText(/1 new person/)).toBeInTheDocument();
  });

  it('a_click_places_a_marker_and_an_unnamed_card_blocks_adding', async () => {
    const { user } = await setup();
    await drag(300, 300, 301, 301);
    expect(await screen.findByText('Enter a given name or a surname.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add to tree' })).toBeDisabled();
    await user.type(screen.getByLabelText('Given name'), 'Ivan');
    expect(screen.getByRole('button', { name: 'Add to tree' })).toBeEnabled();
  });

  it('the_add_a_person_button_works_without_a_mouse', async () => {
    const { user } = await setup();
    await user.click(await screen.findByRole('button', { name: /Add a person/ }));
    expect(await screen.findByRole('group', { name: /Person on the picture/ })).toBeInTheDocument();
  });

  it('links_parent_and_partner_and_groups_the_family', async () => {
    const { user } = await setup();
    await markPerson(user, [100, 100], 'Ivan', 'Koval');
    await markPerson(user, [400, 100], 'Olena', 'Koval');
    await markPerson(user, [250, 400], 'Petro', 'Koval');
    const vp = screen.getByRole('application');

    // Petro is a child of Ivan.
    await user.click(cardIn(vp, 'Ivan Koval'));
    await user.click(await screen.findByRole('button', { name: 'Parent of…' }));
    expect(screen.getByText(/Click the card of the person who is the child of Ivan Koval/)).toBeInTheDocument();
    await user.click(cardIn(vp, 'Petro Koval'));
    // Olena is a partner of Ivan.
    await user.click(cardIn(vp, 'Ivan Koval'));
    await user.click(await screen.findByRole('button', { name: 'Partner of…' }));
    await user.click(cardIn(vp, 'Olena Koval'));
    // Petro is a child of Olena: now the two partners share one family.
    await user.click(cardIn(vp, 'Petro Koval'));
    await user.click(await screen.findByRole('button', { name: 'Child of…' }));
    await user.click(cardIn(vp, 'Olena Koval'));

    expect(screen.getByText(/3 new people, 1 family, 3 links/)).toBeInTheDocument();
    expect(screen.getByText('Ivan Koval is a parent of Petro Koval')).toBeInTheDocument();
    expect(screen.getByText('Ivan Koval is a partner of Olena Koval')).toBeInTheDocument();
    expect(screen.getByText('Olena Koval is a parent of Petro Koval')).toBeInTheDocument();
  });

  it('escape_cancels_a_link_and_a_card_cannot_be_linked_to_itself', async () => {
    const { user } = await setup();
    await markPerson(user, [100, 100], 'Ivan', 'Koval');
    const vp = screen.getByRole('application');
    await user.click(await screen.findByRole('button', { name: 'Partner of…' }));
    await user.click(cardIn(vp, 'Ivan Koval'));
    expect(await screen.findByRole('alert')).toHaveTextContent('different card');
    await user.keyboard('{Escape}');
    expect(screen.queryByText(/Click the card of the person/)).not.toBeInTheDocument();
  });

  it('a_link_can_be_removed_and_a_card_deleted', async () => {
    const { user } = await setup();
    await markPerson(user, [100, 100], 'Ivan', 'Koval');
    await markPerson(user, [400, 100], 'Olena', 'Koval');
    const vp = screen.getByRole('application');
    await user.click(cardIn(vp, 'Ivan Koval'));
    await user.click(await screen.findByRole('button', { name: 'Partner of…' }));
    await user.click(cardIn(vp, 'Olena Koval'));
    await user.click(screen.getByRole('button', { name: /Remove link: Ivan Koval partner of Olena Koval/ }));
    expect(screen.queryByText(/is a partner of/)).not.toBeInTheDocument();
    await user.click(cardIn(vp, 'Olena Koval'));
    await user.click(await screen.findByRole('button', { name: 'Delete card' }));
    expect(within(vp).queryByRole('button', { name: 'Olena Koval' })).not.toBeInTheDocument();
  });

  it('offers_same_as_for_a_person_already_in_the_tree_and_links_to_them', async () => {
    const { api, user } = await setup();
    const first = (await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 1)).items[0];
    const [given, ...rest] = first.name.split(' ');
    const surname = rest.join(' ');
    await drag(200, 100, 350, 140);
    const form = await screen.findByRole('group', { name: /Person on the picture/ });
    await user.type(within(form).getByLabelText('Given name'), given);
    await user.type(within(form).getByLabelText('Surname'), surname);
    const same = (await within(form).findAllByRole("button", { name: new RegExp(`Same as ${first.name}`) }))[0];
    await user.click(same);
    expect(await screen.findByText(first.name, { selector: 'strong' })).toBeInTheDocument();
    expect(screen.getByText(/1 person already in the tree/)).toBeInTheDocument();
    expect(screen.getByText(/0 new people/)).toBeInTheDocument();
    // Taking it back turns the card into a new person again.
    await user.click(screen.getByRole('button', { name: 'Not the same person' }));
    expect(screen.getByText(/1 new person/)).toBeInTheDocument();
  });

  it('add_to_tree_calls_import_draft_once_and_clears_the_draft', async () => {
    const { api, user } = await setup();
    const apply = vi.spyOn(api, 'apply');
    await markPerson(user, [100, 100], 'Ivan', 'Koval');
    await markPerson(user, [400, 100], 'Olena', 'Koval');
    const vp = screen.getByRole('application');
    await user.click(cardIn(vp, 'Ivan Koval'));
    await user.click(await screen.findByRole('button', { name: 'Partner of…' }));
    await user.click(cardIn(vp, 'Olena Koval'));
    const before = (await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 1)).total;

    await user.click(screen.getByRole('button', { name: 'Add to tree' }));
    await waitFor(() => expect(apply).toHaveBeenCalledTimes(1));
    const [cmd, opts] = apply.mock.calls[0] as unknown as [EditCommand, { silent?: boolean }];
    expect(cmd).toMatchObject({ type: 'importDraft', attachPicture: true });
    expect(cmd.type === 'importDraft' && cmd.persons.map((p) => p.given)).toEqual(['Ivan', 'Olena']);
    expect(cmd.type === 'importDraft' && cmd.families).toHaveLength(1);
    expect(opts).toEqual({ silent: true });

    await waitFor(() => expect(notices.current?.message).toBe('Added 2 people and 1 family.'));
    expect(notices.current?.actionLabel).toBe('Undo');
    expect((await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 1)).total).toBe(before + 2);
    expect(await api.loadTraceDraft()).toEqual({ json: null, image: null });
    expect(nav.route.name).toBe('family');
  });

  it('the_keep_picture_box_is_ticked_and_turns_the_flag_off', async () => {
    const { api, user } = await setup();
    const apply = vi.spyOn(api, 'apply');
    await markPerson(user, [100, 100], 'Ivan', 'Koval');
    const box = screen.getByLabelText('Keep the picture with the new people');
    expect(box).toBeChecked();
    await user.click(box);
    await user.click(screen.getByRole('button', { name: 'Add to tree' }));
    await waitFor(() => expect(apply).toHaveBeenCalled());
    expect(apply.mock.calls[0][0]).toMatchObject({ attachPicture: false });
  });

  it('an_error_from_the_backend_is_shown_and_the_draft_stays', async () => {
    const { api, user } = await setup();
    vi.spyOn(api, 'apply').mockRejectedValue(new Error('These links make someone their own ancestor. Check the parent and child links.'));
    await markPerson(user, [100, 100], 'Ivan', 'Koval');
    await user.click(screen.getByRole('button', { name: 'Add to tree' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('own ancestor');
    expect(screen.getByText(/1 new person/)).toBeInTheDocument();
  });

  it('the_draft_is_saved_and_comes_back', async () => {
    const { api, user, view } = await setup();
    await markPerson(user, [100, 100], 'Ivan', 'Koval');
    await waitFor(async () => expect((await api.loadTraceDraft()).json).toContain('Ivan'), { timeout: 2000 });
    view.unmount();
    render(TraceView, { context: apiContext(api, new MockPlatform()) });
    const vp = await screen.findByRole('application');
    expect(cardIn(vp, 'Ivan Koval')).toBeInTheDocument();
  });

  it('discarding_asks_first_and_then_forgets_everything', async () => {
    const { api, user } = await setup();
    await markPerson(user, [100, 100], 'Ivan', 'Koval');
    await user.click(screen.getByRole('button', { name: 'Discard trace' }));
    const dlg = await screen.findByRole('dialog');
    await user.click(within(dlg).getByRole('button', { name: 'Keep working' }));
    expect(screen.getByText(/1 new person/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Discard trace' }));
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Discard trace' }));
    expect(await screen.findByRole('button', { name: 'Choose a picture…' })).toBeInTheDocument();
    expect((await api.loadTraceDraft()).json).toBeNull();
  });

  it('zoom_buttons_and_fit_work', async () => {
    const { user } = await setup();
    await screen.findByRole('application');
    expect(await screen.findByText('67%')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Zoom in' }));
    expect(screen.getByText('83%')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Zoom out' }));
    await user.click(screen.getByRole('button', { name: 'Zoom out' }));
    expect(screen.getByText('53%')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Fit' }));
    expect(screen.getByText('67%')).toBeInTheDocument();
  });
});
