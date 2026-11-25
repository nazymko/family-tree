import { storyOf, type StoryView } from './storyText';

/** Where the "notes: research / story" choice is kept on this computer. */
export const NOTES_VIEW_KEY = 'gt.notesView';

/** `research` shows a note as written, with every reference; `story` shows the readable retelling. */
export type NotesView = 'research' | 'story';

function stored(): NotesView {
  try {
    return localStorage.getItem(NOTES_VIEW_KEY) === 'story' ? 'story' : 'research';
  } catch {
    // Storage can be unavailable; the default applies.
    return 'research';
  }
}

/** Which view every list of notes shows. One choice for all notes everywhere; it survives a restart. */
class NotesViewPref {
  value = $state<NotesView>(stored());

  set(view: NotesView): void {
    this.value = view;
    try {
      localStorage.setItem(NOTES_VIEW_KEY, view);
    } catch {
      // Not remembered after a restart; this session still uses it.
    }
  }

  /** Reads the stored choice again (tests start each case from storage). */
  reload(): void {
    this.value = stored();
  }
}

export const notesView = new NotesViewPref();

/** What a note shows in the view the reader chose: its text as written (research) or the readable story. */
export function shownNote(note: { text: string; story?: string | null }): StoryView {
  return notesView.value === 'story' ? storyOf(note) : { text: note.text, cleaned: false };
}
