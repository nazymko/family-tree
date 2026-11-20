import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import NotesHarness from './NotesHarness.svelte';
import { NOTES_VIEW_KEY, notesView, shownNote } from './notesView.svelte';
import { setLanguage } from '../i18n';

// Invented notes only.
const notes = [
  { id: 'n1', text: 'Born 1890 (ДАХеО 1-2-3, арк. 5), maybe in Lisky? H7', story: null },
  { id: 'n2', text: 'Second note, with a retelling (PDF с. 29).', story: 'A written retelling.' },
];

beforeEach(() => {
  localStorage.removeItem(NOTES_VIEW_KEY);
  notesView.reload();
});
afterEach(() => {
  localStorage.removeItem(NOTES_VIEW_KEY);
  notesView.reload();
  setLanguage('en');
});

describe('the notes view choice', () => {
  it('is_research_by_default', () => {
    expect(notesView.value).toBe('research');
    expect(shownNote(notes[0])).toEqual({ text: notes[0].text, cleaned: false });
  });

  it('is_remembered_and_read_again_on_the_next_run', () => {
    notesView.set('story');
    expect(localStorage.getItem(NOTES_VIEW_KEY)).toBe('story');
    notesView.value = 'research';
    notesView.reload();
    expect(notesView.value).toBe('story');
    notesView.set('research');
    expect(localStorage.getItem(NOTES_VIEW_KEY)).toBe('research');
  });

  it('reads_an_unknown_stored_value_as_research', () => {
    localStorage.setItem(NOTES_VIEW_KEY, 'nonsense');
    notesView.reload();
    expect(notesView.value).toBe('research');
  });
});

describe('the toggle next to the notes', () => {
  it('switches_every_list_of_notes_at_once_and_keeps_the_choice', async () => {
    const user = userEvent.setup();
    render(NotesHarness, { notes });
    const lists = screen.getAllByTestId('notes-list');
    expect(lists).toHaveLength(2);
    const toggles = screen.getAllByRole('group', { name: 'Show notes as' });
    expect(toggles).toHaveLength(2);
    for (const l of lists) expect(within(l).getAllByText(/ДАХеО 1-2-3|PDF с\. 29/)).toHaveLength(2);

    await user.click(within(toggles[0]).getByRole('button', { name: 'Story' }));

    // Both lists changed, not only the one next to the clicked toggle.
    for (const l of lists) {
      expect(within(l).queryByText(/ДАХеО/)).toBeNull();
      expect(within(l).getByText('Born 1890, maybe in Lisky?')).toBeInTheDocument();
      expect(within(l).getByText('A written retelling.')).toBeInTheDocument();
      // Only the note that has no written story says it was shortened.
      expect(within(l).getAllByTestId('note-shortened')).toHaveLength(1);
    }
    for (const g of toggles) {
      expect(within(g).getByRole('button', { name: 'Story' })).toHaveAttribute('aria-pressed', 'true');
      expect(within(g).getByRole('button', { name: 'Research' })).toHaveAttribute('aria-pressed', 'false');
    }
    expect(localStorage.getItem(NOTES_VIEW_KEY)).toBe('story');

    // A new run starts from the stored choice; a click goes back.
    notesView.value = 'research';
    notesView.reload();
    expect(notesView.value).toBe('story');
    await user.click(within(toggles[1]).getByRole('button', { name: 'Research' }));
    expect(within(lists[0]).getByText(/ДАХеО 1-2-3/)).toBeInTheDocument();
    expect(localStorage.getItem(NOTES_VIEW_KEY)).toBe('research');
  });

  it('is_keyboard_operable_and_speaks_ukrainian', async () => {
    setLanguage('uk');
    const user = userEvent.setup();
    render(NotesHarness, { notes });
    const g = screen.getAllByRole('group', { name: 'Показувати примітки як' })[0];
    const story = within(g).getByRole('button', { name: 'Розповідь' });
    story.focus();
    await user.keyboard('{Enter}');
    expect(story).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getAllByText('скорочено з дослідницької примітки').length).toBeGreaterThan(0);
  });
});
