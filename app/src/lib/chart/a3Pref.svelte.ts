import type { A3DirectionMode } from './a3';

/** Where the A3 direction choice is kept on this computer (like the other view choices of the chart screens). */
export const A3_DIRECTION_KEY = 'gt.a3Direction';

function stored(): A3DirectionMode {
  try {
    return localStorage.getItem(A3_DIRECTION_KEY) === 'best' ? 'best' : 'screen';
  } catch {
    // Storage can be unavailable; the default applies.
    return 'screen';
  }
}

/** The direction an A3 print uses: as on the screen (the default) or the best fit. Shared by every chart. */
class A3DirectionPref {
  value = $state<A3DirectionMode>(stored());

  set(mode: A3DirectionMode): void {
    this.value = mode;
    try {
      localStorage.setItem(A3_DIRECTION_KEY, mode);
    } catch {
      // Not remembered after a restart; this session still uses it.
    }
  }

  /** Reads the stored choice again (tests start each case from storage). */
  reload(): void {
    this.value = stored();
  }
}

export const a3Direction = new A3DirectionPref();
