export type ThemePref = 'light' | 'dark' | 'system';

/** Sets `data-theme` on <html> for an explicit choice; removes it so the system preference applies. */
export function applyTheme(pref: ThemePref): void {
  const root = document.documentElement;
  if (pref === 'system') root.removeAttribute('data-theme');
  else root.setAttribute('data-theme', pref);
  syncSystemBars();
}

interface AndroidBars {
  set(color: string, darkIcons: boolean): void;
}

let watching = false;

/**
 * On Android the strips behind the status and navigation bars show the window's own background, not the page.
 * Tell the shell the page colour (and whether the bar icons should be dark) so they match the theme. Nothing
 * happens anywhere else: only the Android shell provides `GeneTreeBars`.
 */
export function syncSystemBars(): void {
  const bars = (window as unknown as { GeneTreeBars?: AndroidBars }).GeneTreeBars;
  if (!bars) return;
  const rgb = getComputedStyle(document.body).backgroundColor.match(/\d+(\.\d+)?/g)?.map(Number);
  if (!rgb || rgb.length < 3) return;
  const [r, g, b] = rgb;
  const hex = '#' + [r, g, b].map((v) => Math.round(v).toString(16).padStart(2, '0')).join('');
  const luma = 0.299 * r + 0.587 * g + 0.114 * b;
  try {
    bars.set(hex, luma > 140);
  } catch {
    // The bridge is gone (a page reload while the shell restarts): the bars keep their last colour.
  }
  if (!watching && window.matchMedia) {
    watching = true;
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener?.('change', () => queueMicrotask(syncSystemBars));
  }
}
