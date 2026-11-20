/** Where the "names: modern / as recorded" choice is kept on this computer ('1' = as recorded; modern by default). */
export const NAMES_AS_RECORDED_KEY = 'gt.namesAsRecorded';

function stored(): boolean {
  try {
    return localStorage.getItem(NAMES_AS_RECORDED_KEY) === '1';
  } catch {
    // Storage can be unavailable; the default applies.
    return false;
  }
}

/** Whether charts, pictures, prints and the people list show a person's name as a record wrote it, when there is one. */
class NamesAsRecordedPref {
  value = $state<boolean>(stored());

  set(on: boolean): void {
    this.value = on;
    try {
      localStorage.setItem(NAMES_AS_RECORDED_KEY, on ? '1' : '0');
    } catch {
      // Not remembered after a restart; this session still uses it.
    }
  }

  /** Reads the stored choice again (tests start each case from storage). */
  reload(): void {
    this.value = stored();
  }
}

export const namesAsRecorded = new NamesAsRecordedPref();

/** The part of a person the name display needs. */
export interface NamedPerson {
  name: string;
  asRecorded?: string | null;
}

/** What to show for a person: the as-recorded name when the choice is on and the person has one, else the main name. */
export interface ShownName {
  text: string;
  /** True when `text` is the source spelling; `modern` then holds the main name (for a tooltip). */
  recorded: boolean;
  modern: string;
}

export function shownName(p: NamedPerson | null | undefined, on: boolean = namesAsRecorded.value): ShownName {
  const modern = p?.name ?? '';
  const rec = p?.asRecorded?.trim();
  if (on && rec) return { text: rec, recorded: true, modern };
  return { text: modern, recorded: false, modern };
}

/** The mark that tells a reader a name is the source spelling: the name in guillemets. */
export function markedName(s: ShownName): string {
  return s.recorded ? `«${s.text}»` : s.text;
}

/** The name a chart card draws: marked when it is the source spelling. */
export function cardName(p: NamedPerson | null | undefined): string {
  return markedName(shownName(p));
}
