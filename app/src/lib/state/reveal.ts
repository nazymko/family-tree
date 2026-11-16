/** The class that marks the note a person was sent to; styled in `ui/controls.css`. */
export const REVEAL_CLASS = 'revealed';

/**
 * Brings the note `id` into view and marks it for a moment, once the page that holds it has drawn it.
 * Pages put `data-note-id` on each note. A note the page does not show (one on an event) is left alone.
 */
export function revealNote(id: string): void {
  let tries = 0;
  const look = () => {
    const el = document.querySelector<HTMLElement>(`[data-note-id="${id}"]`);
    if (el) {
      el.scrollIntoView?.({ block: 'center' });
      el.classList.add(REVEAL_CLASS);
      setTimeout(() => el.classList.remove(REVEAL_CLASS), 2500);
      return;
    }
    if (++tries < 20) setTimeout(look, 100);
  };
  setTimeout(look, 50);
}
