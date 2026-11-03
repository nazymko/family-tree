export type ThemePref = 'light' | 'dark' | 'system';

/** Sets `data-theme` on <html> for an explicit choice; removes it so the system preference applies. */
export function applyTheme(pref: ThemePref): void {
  const root = document.documentElement;
  if (pref === 'system') root.removeAttribute('data-theme');
  else root.setAttribute('data-theme', pref);
}
