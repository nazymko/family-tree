import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import FamilyPage from './FamilyPage.svelte';
import { apiContext } from '../api/context';
import type { Api } from '../api/api';
import type { FamilyDto, PersonRef } from '../api/types';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { setLanguage } from '../i18n';
import { nav } from '../state/nav.svelte';
import { toasts } from '../state/toast.svelte';

const base = new MockApi(fixtureTree(20));

const person = (id: string, name: string): PersonRef => ({ id, name, lifespan: '1850 – 1921', sex: 'unknown', living: false, private: false, siblingMarker: false });

const family = (over: Partial<FamilyDto> = {}): FamilyDto => ({
  id: 'f1',
  partners: [person('p1', 'Ivan Petrovych Koval'), person('p2', 'Olena Koval')],
  kind: 'married',
  summary: null,
  events: [],
  children: [{ person: person('p3', 'Taras Koval'), rel: ['birth', 'birth'], preferred: false }],
  notes: [{ id: 'n1', kind: 'story', text: 'They moved to Lviv together.', media: [] }],
  ...over,
} as FamilyDto);

function setup(getFamily: Api['getFamily']) {
  const api = Object.assign(Object.create(base), { getFamily }) as Api;
  const user = userEvent.setup();
  render(FamilyPage, { props: { id: 'f1' }, context: apiContext(api, new MockPlatform()) });
  return { user };
}

beforeEach(() => {
  nav.reset();
  toasts.clear();
  nav.go({ name: 'family' });
  nav.go({ name: 'familyPage', id: 'f1' });
});
afterEach(() => setLanguage('en'));

describe('the family page', () => {
  it('shows_partners_children_and_notes_to_read_in_ukrainian_with_no_note_editing', async () => {
    setLanguage('uk');
    setup(vi.fn(async () => family()));
    expect(await screen.findByRole('heading', { level: 1, name: 'Родина: Ivan Petrovych Koval і Olena Koval' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Taras Koval' })).toBeInTheDocument();
    const notes = screen.getByRole('region', { name: 'Спільні нотатки родини' });
    expect(within(notes).getByText('They moved to Lviv together.')).toBeInTheDocument();
    expect(within(notes).queryByRole('button')).toBeNull();
  });

  it('back_returns_to_where_it_was_opened', async () => {
    const { user } = setup(vi.fn(async () => family()));
    await user.click(await screen.findByRole('button', { name: 'Back' }));
    expect(nav.route).toEqual({ name: 'family' });
  });

  it('a_person_opens_on_the_family_screen', async () => {
    const { user } = setup(vi.fn(async () => family()));
    await user.click(await screen.findByRole('button', { name: 'Taras Koval' }));
    expect(nav.route).toEqual({ name: 'family' });
    expect(nav.focusPerson).toBe('p3');
  });

  it('says_when_there_are_no_notes', async () => {
    setup(vi.fn(async () => family({ notes: [] })));
    expect(await screen.findByText('No shared notes.')).toBeInTheDocument();
  });

  it('a_deleted_family_steps_back_with_a_message', async () => {
    setup(vi.fn(async () => Promise.reject({ code: 'not_found', message: 'gone' })));
    await waitFor(() => expect(nav.route).toEqual({ name: 'family' }));
  });
});
