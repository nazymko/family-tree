import type { A3Background, A3DirectionMode } from './a3';

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

/** Where the A3 title block choice is kept ('0' = off; on by default). */
export const A3_TITLE_KEY = 'gt.a3Title';
/** Where the picture background choice is kept. */
export const A3_BACKGROUND_KEY = 'gt.a3Background';

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    // Storage can be unavailable; the default applies.
    return null;
  }
}

function write(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Not remembered after a restart; this session still uses it.
  }
}

/** Whether an A3 print (and a saved picture's caption strip) has its title block. On by default. */
class A3TitlePref {
  value = $state<boolean>(read(A3_TITLE_KEY) !== '0');

  set(on: boolean): void {
    this.value = on;
    write(A3_TITLE_KEY, on ? '1' : '0');
  }

  /** Reads the stored choice again (tests start each case from storage). */
  reload(): void {
    this.value = read(A3_TITLE_KEY) !== '0';
  }
}

function storedBackground(): A3Background {
  const v = read(A3_BACKGROUND_KEY);
  return v === 'white' || v === 'transparent' ? v : 'paper';
}

/** The background of A3 prints and saved pictures: the paper tint (the default), white, or transparent. */
class A3BackgroundPref {
  value = $state<A3Background>(storedBackground());

  set(mode: A3Background): void {
    this.value = mode;
    write(A3_BACKGROUND_KEY, mode);
  }

  /** Reads the stored choice again (tests start each case from storage). */
  reload(): void {
    this.value = storedBackground();
  }
}

export const a3Title = new A3TitlePref();
export const a3Background = new A3BackgroundPref();
