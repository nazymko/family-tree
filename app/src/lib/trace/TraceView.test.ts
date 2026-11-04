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
import type { AiReadResult, EditCommand } from '../api/types';

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

  it('a_click_marker_is_sized_from_the_picture_and_the_review_shows_a_linking_tip', async () => {
    const { user } = await setup();
    expect(screen.queryByText(/Select a card, then use Parent of/)).not.toBeInTheDocument();
    await drag(300, 300, 301, 301);
    await screen.findByRole('group', { name: /Person on the picture/ });
    await user.type(screen.getByLabelText('Given name'), 'Ivan');
    expect(screen.getByText('Select a card, then use Parent of…, Child of… or Partner of… to link people.')).toBeInTheDocument();
    const card = screen.getByRole('application').querySelector<HTMLElement>('[data-card]');
    const w = parseFloat(card?.style.width ?? '0');
    expect(w).toBeGreaterThanOrEqual(60);
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

  it('deleting_the_selected_card_moves_focus_to_the_next_card_then_the_picture', async () => {
    const { user } = await setup();
    await markPerson(user, [100, 100], 'Ivan', 'Koval');
    await markPerson(user, [400, 100], 'Olena', 'Koval');
    const vp = screen.getByRole('application');
    await user.click(cardIn(vp, 'Ivan Koval'));
    await user.click(await screen.findByRole('button', { name: 'Delete card' }));
    await waitFor(() => expect(cardIn(vp, 'Olena Koval')).toHaveFocus());
    await user.click(cardIn(vp, 'Olena Koval'));
    await user.click(await screen.findByRole('button', { name: 'Delete card' }));
    await waitFor(() => expect(vp).toHaveFocus());
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

    await waitFor(() => expect(notices.current?.message).toBe('Added 2 people and one family (new or updated).'));
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

  describe('keyboard', () => {
    it('focus_moves_to_the_person_form_when_a_card_is_added_and_back_to_the_card_when_done', async () => {
      const { user } = await setup();
      await user.click(await screen.findByRole('button', { name: 'Add a person' }));
      const form = await screen.findByRole('group', { name: /Person on the picture/ });
      await waitFor(() => expect(within(form).getByLabelText('Given name')).toHaveFocus());
      await user.type(within(form).getByLabelText('Given name'), 'Ivan');
      await user.click(within(form).getByRole('button', { name: 'Done' }));
      const vp = screen.getByRole('application');
      await waitFor(() => expect(within(vp).getByRole('button', { name: 'Ivan' })).toHaveFocus());
    });

    it('a_skip_button_before_the_cards_jumps_to_the_review_panel', async () => {
      const { user } = await setup();
      await screen.findByRole('application');
      const skip = screen.getByRole('button', { name: /Skip the picture/ });
      // It comes before the picture in the tab order.
      const vp = screen.getByRole('application');
      expect(skip.compareDocumentPosition(vp) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
      await user.click(skip);
      expect(screen.getByRole('complementary', { name: 'Review' }).contains(document.activeElement)).toBe(true);
    });
  });

  describe('Read names with AI', () => {
    async function openDialog() {
      const ctx = await setup();
      await ctx.user.click(await screen.findByRole('button', { name: 'Read names with AI…' }));
      const dlg = await screen.findByRole('dialog');
      return { ...ctx, dlg };
    }

    it('says_where_the_picture_goes_and_cannot_send_without_the_tick', async () => {
      const { api, user, dlg } = await openDialog();
      expect(dlg).toHaveTextContent(/sent over the internet to Anthropic/);
      expect(dlg).toHaveTextContent(/Living relatives/);
      const send = within(dlg).getByRole('button', { name: 'Send picture' });
      expect(send).toBeDisabled();
      await user.type(within(dlg).getByLabelText('Anthropic API key'), 'invented-key-for-tests');
      // A key alone is not enough.
      expect(send).toBeDisabled();
      await user.click(send);
      expect(api.aiSends).toBe(0);
      await user.click(within(dlg).getByRole('checkbox', { name: /I understand/ }));
      expect(send).toBeEnabled();
    });

    it('asks_again_every_time_and_forgets_the_typed_key', async () => {
      const { api, user, dlg } = await openDialog();
      await user.type(within(dlg).getByLabelText('Anthropic API key'), 'invented-key-for-tests');
      await user.click(within(dlg).getByRole('checkbox', { name: /I understand/ }));
      await user.click(within(dlg).getByRole('button', { name: 'Send picture' }));
      await waitFor(() => expect(api.aiSends).toBe(1));
      await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
      await user.click(screen.getByRole('button', { name: 'Read names with AI…' }));
      const again = await screen.findByRole('dialog');
      expect(within(again).getByRole('checkbox', { name: /I understand/ })).not.toBeChecked();
      expect(within(again).getByRole('button', { name: 'Send picture' })).toBeDisabled();
      // The key is now stored, so there is no key field, and it can be removed.
      await within(again).findByRole('button', { name: 'Remove key' });
      expect(within(again).queryByLabelText('Anthropic API key')).not.toBeInTheDocument();
      await user.click(within(again).getByRole('button', { name: 'Remove key' }));
      expect(await within(again).findByLabelText('Anthropic API key')).toBeInTheDocument();
      expect(await api.aiKeyStatus()).toBe(false);
    });

    it('results_are_suggestions_that_need_accepting_before_they_count', async () => {
      const { api, user, dlg } = await openDialog();
      await user.type(within(dlg).getByLabelText('Anthropic API key'), 'invented-key-for-tests');
      await user.click(within(dlg).getByRole('checkbox', { name: /I understand/ }));
      await user.click(within(dlg).getByRole('button', { name: 'Send picture' }));
      await waitFor(() => expect(api.aiSends).toBe(1));
      const vp = await screen.findByRole('application');
      expect(await within(vp).findByRole('button', { name: /Ivan Petrovych Koval/ })).toBeInTheDocument();
      // Nothing is added yet.
      expect(screen.getByText(/0 new people/)).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Add to tree' })).toBeDisabled();
      await user.click(screen.getByRole('button', { name: /Accept all/ }));
      expect(await screen.findByText(/3 new people, 1 family, 3 links/)).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Add to tree' })).toBeEnabled();
    });

    it('cancel_while_reading_aborts_and_closes_and_the_late_answer_is_ignored', async () => {
      const { api, user, dlg } = await openDialog();
      let release: (r: AiReadResult) => void = () => {};
      const real = api.aiReadPicture.bind(api);
      api.aiReadPicture = (c, k) => new Promise<AiReadResult>((res) => (release = res)).then(async () => real(c, k));
      await user.type(within(dlg).getByLabelText('Anthropic API key'), 'invented-key-for-tests');
      await user.click(within(dlg).getByRole('checkbox', { name: /I understand/ }));
      await user.click(within(dlg).getByRole('button', { name: 'Send picture' }));
      expect(await within(dlg).findByText(/can take a few minutes/)).toBeInTheDocument();
      await user.click(within(dlg).getByRole('button', { name: 'Cancel' }));
      expect(api.aiCancels).toBe(1);
      await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
      release({} as AiReadResult);
      await new Promise((r) => setTimeout(r, 20));
      expect(screen.getByText(/0 new people/)).toBeInTheDocument();
    });

    it('a_refused_send_shows_what_to_do', async () => {
      const { api, user, dlg } = await openDialog();
      api.aiReadPicture = async () => {
        throw new Error('Anthropic did not accept the key. Remove the key and enter it again.');
      };
      await user.type(within(dlg).getByLabelText('Anthropic API key'), 'invented-key-for-tests');
      await user.click(within(dlg).getByRole('checkbox', { name: /I understand/ }));
      await user.click(within(dlg).getByRole('button', { name: 'Send picture' }));
      expect(await within(dlg).findByRole('alert')).toHaveTextContent('Remove the key');
      expect(screen.getByRole('dialog')).toBeInTheDocument();
      // The dialog is usable again: not busy, and the tick has to be given again.
      expect(within(dlg).getByRole('button', { name: 'Cancel' })).toBeEnabled();
      expect(within(dlg).getByRole('checkbox', { name: /I understand/ })).not.toBeChecked();
      expect(within(dlg).getByRole('button', { name: 'Send picture' })).toBeDisabled();
    });

    it('says_when_the_answer_was_cut_short_and_replaces_older_suggestions', async () => {
      const { api, user, dlg } = await openDialog();
      const real = api.aiReadPicture.bind(api);
      api.aiReadPicture = async (c, k) => ({ ...(await real(c, k)), truncated: true, skipped: 2 });
      await user.type(within(dlg).getByLabelText('Anthropic API key'), 'invented-key-for-tests');
      await user.click(within(dlg).getByRole('checkbox', { name: /I understand/ }));
      await user.click(within(dlg).getByRole('button', { name: 'Send picture' }));
      await waitFor(() => expect(notices.current?.message).toMatch(/cut short/));
      expect(notices.current?.message).toMatch(/skipped 2 unclear entries/);
      // Reading again swaps the suggestions instead of doubling them.
      await user.click(screen.getByRole('button', { name: 'Read names with AI…' }));
      const again = await screen.findByRole('dialog');
      await user.click(within(again).getByRole('checkbox', { name: /I understand/ }));
      await user.click(within(again).getByRole('button', { name: 'Send picture' }));
      await waitFor(() => expect(api.aiSends).toBe(2));
      const vp = screen.getByRole('application');
      await waitFor(() => expect(within(vp).getAllByRole('button', { name: /Ivan Petrovych Koval/ })).toHaveLength(1));
    });
  });
});
