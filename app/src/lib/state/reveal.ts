/** The class that marks the record a person was sent to; styled in `ui/controls.css`. */
export const REVEAL_CLASS = 'revealed';

/** Kinds of record a page can bring into view: pages put `data-<kind>-id` on each such record. */
export type RevealKind = 'person' | 'family' | 'event' | 'name' | 'note' | 'media' | 'citation' | 'source';

const KINDS: readonly string[] = ['person', 'family', 'event', 'name', 'note', 'media', 'citation', 'source'];
/** How long the page is waited for, and how often it is looked at. */
const WAIT_MS = 4000;
const STEP_MS = 100;
const MARK_MS = 2500;

/** Opens every closed `<details>` the element sits in, so that it can be seen. */
function openSections(el: HTMLElement): void {
  for (let up = el.parentElement; up; up = up.parentElement) {
    if (up instanceof HTMLDetailsElement && !up.open) up.open = true;
  }
}

/** Moves keyboard focus to the element (or to its heading), never into a field. */
function focusOn(el: HTMLElement): void {
  const target = el.matches('input, textarea, select') ? (el.closest('section')?.querySelector<HTMLElement>('h1, h2') ?? null) : el;
  if (!target) return;
  if (!target.hasAttribute('tabindex') && !target.matches('a[href], button')) {
    target.setAttribute('tabindex', '-1');
    target.addEventListener('blur', () => target.removeAttribute('tabindex'), { once: true });
  }
  target.focus({ preventScroll: true });
}

function reduceMotion(): boolean {
  return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/**
 * Brings the record `kind`/`id` into view, marks it for a moment and puts keyboard focus on it, once the page that
 * holds it has drawn it. A closed section around it is opened first. A record that never appears (the page is
 * still loading, or does not draw it) is given up on quietly after a few seconds.
 */
export function revealRecord(kind: string, id: string): void {
  if (!KINDS.includes(kind)) return;
  const selector = `[data-${kind}-id="${id}"]`;
  const started = Date.now();
  const look = () => {
    const el = document.querySelector<HTMLElement>(selector);
    if (el) {
      openSections(el);
      el.scrollIntoView?.({ block: 'center', behavior: reduceMotion() ? 'auto' : 'smooth' });
      el.classList.add(REVEAL_CLASS);
      const clear = () => el.classList.remove(REVEAL_CLASS);
      // The glow ends the mark; the timer is for steady marking under reduced motion.
      el.addEventListener('animationend', (e) => e.target === el && clear());
      setTimeout(clear, MARK_MS);
      focusOn(el);
      return;
    }
    if (Date.now() - started < WAIT_MS) setTimeout(look, STEP_MS);
  };
  setTimeout(look, 50);
}

/** The same for a note, kept for callers that have only a note id. */
export const revealNote = (id: string): void => revealRecord('note', id);
