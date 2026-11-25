/**
 * True on a phone (and a tablet held short): a finger as the main input and a small window, in either direction.
 * Wide chart toolbars, the minimap and the like give way to icons and an overflow menu there. A desktop window has a
 * mouse and a minimum size of 960 x 600, so it never matches and the desktop layout does not change.
 */
export const COMPACT_QUERY = '(pointer: coarse) and ((max-width: 940px) or (max-height: 500px))';

class Compact {
  value = $state(false);

  constructor() {
    if (typeof window === 'undefined' || !window.matchMedia) return;
    const q = window.matchMedia(COMPACT_QUERY);
    this.value = q.matches;
    q.addEventListener?.('change', () => (this.value = q.matches));
  }
}

export const compact = new Compact();
