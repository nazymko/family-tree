import { plural, t, type MessageKey } from '.';
import type { LogFactDto, WholeBlockDto } from '../api/types';
import { en } from './en';

/**
 * Names of the changes the backend reports ("Add person", "Undo: Edit name"). The set is closed, so the
 * frontend translates them by lookup; an unknown label is shown as the backend wrote it.
 * The plan for the rest of the backend text is in .superpowers/sdd/2026-11-03-wave3/i18n-report.md.
 */
const EDIT_KEYS: Record<string, [label: MessageKey, done: MessageKey]> = {
  'Add person': ['edit.addPerson', 'edit.donePersonAdded'],
  'Edit person': ['edit.editPerson', 'edit.donePersonSaved'],
  'Delete person': ['edit.deletePerson', 'edit.donePersonDeleted'],
  'Edit name': ['edit.editName', 'edit.doneNameSaved'],
  'Delete name': ['edit.deleteName', 'edit.doneNameDeleted'],
  'Save event': ['edit.saveEvent', 'edit.doneEventSaved'],
  'Delete event': ['edit.deleteEvent', 'edit.doneEventDeleted'],
  'Save family': ['edit.saveFamily', 'edit.doneFamilySaved'],
  'Delete family': ['edit.deleteFamily', 'edit.doneFamilyDeleted'],
  'Add child to family': ['edit.addChildToFamily', 'edit.doneChildAddedToFamily'],
  'Edit child link': ['edit.editChildLink', 'edit.doneChildLinkSaved'],
  'Remove child from family': ['edit.removeChildFromFamily', 'edit.doneChildRemovedFromFamily'],
  'Reorder children': ['edit.reorderChildren', 'edit.doneChildrenReordered'],
  'Add unnamed brothers and sisters': ['edit.addSiblingMarker', 'edit.doneSiblingMarkerAdded'],
  'Save note': ['edit.saveNote', 'edit.doneNoteSaved'],
  'Delete note': ['edit.deleteNote', 'edit.doneNoteDeleted'],
  'Save source': ['edit.saveSource', 'edit.doneSourceSaved'],
  'Add citation': ['edit.addCitation', 'edit.doneCitationAdded'],
  'Edit citation': ['edit.editCitation', 'edit.doneCitationSaved'],
  'Remove citation': ['edit.removeCitation', 'edit.doneCitationRemoved'],
  'Attach media': ['edit.attachMedia', 'edit.doneMediaAttached'],
  'Detach media': ['edit.detachMedia', 'edit.doneMediaDetached'],
  'Delete media': ['edit.deleteMedia', 'edit.doneMediaDeleted'],
  'Set primary media': ['edit.setPrimaryMedia', 'edit.donePrimaryMediaSet'],
  'Edit media': ['edit.editMedia', 'edit.doneMediaSaved'],
  'Record number': ['edit.recordNumber', 'edit.doneNumberRecorded'],
  'Link media files': ['edit.linkMediaFiles', 'edit.doneMediaFilesLinked'],
  'Add father': ['edit.addFather', 'edit.doneFatherAdded'],
  'Add mother': ['edit.addMother', 'edit.doneMotherAdded'],
  'Add stepfather': ['family.addStepFather', 'edit.doneStepFatherAdded'],
  'Add stepmother': ['family.addStepMother', 'edit.doneStepMotherAdded'],
  'Add partner': ['edit.addPartner', 'edit.donePartnerAdded'],
  'Add child': ['edit.addChild', 'edit.doneChildAdded'],
  'Add sibling': ['edit.addSibling', 'edit.doneSiblingAdded'],
  'Add traced people': ['edit.addTracedPeople', 'edit.doneTracedPeopleAdded'],
};

/** A backend change name in the active language ("Undo: Add person" keeps its prefix, translated). */
export function editLabel(label: string): string {
  const m = /^(Undo|Redo): (.+)$/.exec(label);
  if (m) return t(m[1] === 'Undo' ? 'edit.undoPrefix' : 'edit.redoPrefix', { label: editLabel(m[2]) });
  const merged = /^Merged copy: (.+)$/.exec(label);
  if (merged) return t('merge.mergedLabel', { name: merged[1] });
  const keys = EDIT_KEYS[label];
  return keys ? t(keys[0]) : label;
}

/** The toast sentence after a change ("Person added"). */
export function editDoneText(label: string): string {
  const keys = EDIT_KEYS[label];
  return keys ? t(keys[1]) : t('edit.doneFallback', { label: editLabel(label) });
}

/** What agent access answers. The English catalogue holds the backend's words, so it is also the pattern. */
const AGENT_ERRORS = (Object.keys(en.agent) as (keyof typeof en.agent)[])
  .filter((name) => name.startsWith('err'))
  .map((name) => {
    const pattern = en.agent[name]
      .replace(/[.*+?^$()[\]\\|]/g, '\\$&')
      .replace(/\{(\w+)\}/g, '(?<$1>[\\s\\S]*)');
    return { key: `agent.${name}` as MessageKey, pattern: new RegExp(`^${pattern}$`) };
  });

/** The built-in agent that works when no key is required. Mirrors `KEYLESS_AGENT_ID` and `KEYLESS_AGENT_NAME` in gt-mcp. */
export const KEYLESS_AGENT_ID = '01927a00-0000-7000-8000-6e6f6b657900';
export const KEYLESS_AGENT_NAME = 'Agent without a key';

/**
 * An agent's name for the screen. The built-in agent is known by its fixed id and gets a translated label.
 * Never by its name: a connection that already had that name before it was reserved shows as itself.
 */
export function agentLabel(id: string, name: string): string {
  return id === KEYLESS_AGENT_ID ? t('agent.keylessAgent') : name;
}

/** An error of agent access in the active language; other text is shown as the backend wrote it. */
export function agentErrorText(text: string): string {
  for (const { key, pattern } of AGENT_ERRORS) {
    const m = pattern.exec(text);
    if (m) return t(key, m.groups);
  }
  return text;
}

const EVENT_KINDS = new Set([
  'birth',
  'christening',
  'baptism',
  'death',
  'burial',
  'cremation',
  'adoption',
  'marriage',
  'marriage_banns',
  'marriage_contract',
  'marriage_license',
  'marriage_settlement',
  'engagement',
  'divorce',
  'divorce_filed',
  'annulment',
  'separation',
  'census',
  'residence',
  'occupation',
  'education',
  'religion',
  'nationality',
  'title',
  'emigration',
  'immigration',
  'naturalization',
  'probate',
  'will',
  'graduation',
  'retirement',
  'military',
  'physical_description',
]);

/** The display name of an event kind ("birth", "custom:Pilgrimage"); `fallback` is the label the backend sent. */
export function eventKindLabel(kind: string, fallback?: string): string {
  if (EVENT_KINDS.has(kind)) return t(`eventKind.${kind}` as MessageKey);
  if (kind.startsWith('custom:')) return kind.slice(7);
  return fallback || kind;
}

/** Whole messages the undo and change-log commands send; the set is closed, like the edit labels. */
const ERROR_KEYS: Record<string, MessageKey> = {
  'An agent changed this since. Undo it from the change log first.': 'changeLog.errors.agentChanged',
  'This step was dropped: a record it needs no longer exists.': 'changeLog.errors.stepDropped',
  'This step was dropped: a record it needs has changed since, and the change log cannot undo that.': 'changeLog.errors.stepOutdated',
  "Only an agent's change can be undone from the change log.": 'changeLog.errors.onlyAgent',
};

/** A backend error message in the active language; anything not known is shown as the backend wrote it. */
export function backendError(message: string): string {
  const key = ERROR_KEYS[message];
  if (key) return t(key);
  return /^No (change|batch) with id /.test(message) ? t('changeLog.errors.notFound') : message;
}

const DATE_WORDS: Record<string, MessageKey> = {
  about: 'changeLog.date.about',
  calculated: 'changeLog.date.calculated',
  estimated: 'changeLog.date.estimated',
  before: 'changeLog.date.before',
  after: 'changeLog.date.after',
  from: 'changeLog.date.from',
  to: 'changeLog.date.to',
};

/** What `display_date` in gt-core's format.rs writes. The longer of two names that start alike comes first. */
const MONTH_NAMES = [
  'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec',
  'Tishrei', 'Cheshvan', 'Kislev', 'Tevet', 'Shevat', 'Adar II', 'Adar I', 'Adar', 'Nisan', 'Iyar', 'Sivan', 'Tamuz', 'Av', 'Elul',
  'Vendemiaire', 'Brumaire', 'Frimaire', 'Nivose', 'Pluviose', 'Ventose', 'Germinal', 'Floreal', 'Prairial', 'Messidor', 'Thermidor', 'Fructidor', 'Sansculottides',
];
const CALENDARS: Record<string, MessageKey> = {
  Julian: 'changeLog.calendar.julian',
  Hebrew: 'changeLog.calendar.hebrew',
  'French Republican': 'changeLog.calendar.french',
};
const ONE_DATE = new RegExp(`^(?:(\\d{1,2}) )?(?:(${MONTH_NAMES.join('|')}) )?(\\d{1,4}(?:/\\d{2})?)( BCE)?(?: \\((${Object.keys(CALENDARS).join('|')})\\))?$`);

function simpleDate(s: string): string | null {
  const m = ONE_DATE.exec(s);
  if (!m) return null;
  const month = m[2] ? t(`changeLog.month.${m[2].replace(' ', '')}` as MessageKey) : '';
  return [m[1], month, m[3], m[4] && t('changeLog.bce'), m[5] && t(CALENDARS[m[5]])].filter(Boolean).join(' ');
}

/**
 * The backend's date text ("about 3 Apr 1901") in the active language. A date it cannot read as that
 * grammar, such as a phrase the owner typed, stays exactly as written; so does the owner's phrase after a date.
 */
export function dateText(display: string): string {
  const whole = dateGrammar(display);
  if (whole !== null) return whole;
  if (display.endsWith(')')) {
    for (let i = display.lastIndexOf(' ('); i > 0; i = display.lastIndexOf(' (', i - 1)) {
      const head = dateGrammar(display.slice(0, i));
      if (head !== null) return head + display.slice(i);
    }
  }
  return display;
}

/** The range under a date field ("1704 to 1712", "3 Apr 1901 to 9 Apr 1901") in the active language. */
export function dateRangeText(range: string): string {
  const [a, b, ...rest] = range.split(' to ');
  return b !== undefined && rest.length === 0 ? t('fields.understoodRange', { a: dateText(a), b: dateText(b) }) : range;
}

const DATE_WARNINGS: Record<string, MessageKey> = {
  'Day and month are ambiguous; read as DMY': 'fields.dateAmbiguousDmy',
  'Day and month are ambiguous; read as MDY': 'fields.dateAmbiguousMdy',
  'Double year: the first year is Old Style, the second New Style.': 'fields.dateDualYear',
};

/** A warning from the date parser in the active language. */
export function dateWarningText(warning: string): string {
  const key = DATE_WARNINGS[warning];
  return key ? t(key) : warning;
}

function dateGrammar(display: string): string | null {
  const between = /^between (.+) and (.+)$/.exec(display);
  const fromTo = /^from (.+) to (.+)$/.exec(display);
  const pair = between ?? fromTo;
  if (pair) {
    const a = simpleDate(pair[1]);
    const b = simpleDate(pair[2]);
    return a && b ? t(between ? 'changeLog.date.between' : 'changeLog.date.fromTo', { a, b }) : null;
  }
  const word = /^(about|calculated|estimated|before|after|from|to) (.+)$/.exec(display);
  if (word) {
    const date = simpleDate(word[2]);
    return date ? t(DATE_WORDS[word[1]], { date }) : null;
  }
  return simpleDate(display);
}

/** One fact of the change log. Fixed words are translated; what the owner or an agent wrote stays as written. */
export function factText(fact: LogFactDto): string {
  const name = (s: string) => (s === 'Unknown' ? t('changeLog.unknownPerson') : s === '\u0001siblings' ? t('person.siblingMarker') : s);
  if (fact.kind === 'text') return name(fact.text);
  if (fact.kind === 'family') {
    const [a, b] = fact.partners.map(name);
    return b ? t('changeLog.couple', { a, b }) : (a ?? '');
  }
  const date = fact.date ? dateText(fact.date) : '';
  return [eventKindLabel(fact.event, fact.label), date, fact.place ?? ''].filter(Boolean).join(', ');
}

const LOG_KINDS = new Set(['person', 'family', 'event', 'place', 'source', 'repository', 'citation', 'media', 'note']);

/** The word for a record kind in the change log. */
export function recordKindLabel(kind: string): string {
  return LOG_KINDS.has(kind) ? t(`changeLog.kind.${kind}` as MessageKey) : kind;
}

/** The heading of a group on the whole-tree chart, in the app's language. */
export function wholeBlockTitle(b: Pick<WholeBlockDto, 'surname' | 'people' | 'unconnected'>): string {
  if (b.unconnected) return t('whole.blockUnconnected', { n: b.people });
  const people = plural('common.people', b.people);
  return b.surname ? t('whole.blockFamily', { surname: b.surname, people }) : t('whole.blockNoSurname', { people });
}

const LIFESPAN_WORDS: Record<string, MessageKey> = {
  c: 'lifespan.about',
  bef: 'lifespan.before',
  aft: 'lifespan.after',
  b: 'lifespan.born',
  d: 'lifespan.died',
  m: 'lifespan.married',
};

/** The backend's lifespan ("c. 1968 – 2021", "b. 1970", "bef. 44 BCE – ?") in the active language. */
export function lifespanText(s: string): string {
  if (!s) return s;
  return s
    .replace(/(^|– )(c|bef|aft|b|d)\. /g, (_, lead: string, word: string) => `${lead}${t(LIFESPAN_WORDS[word])} `)
    .replace(/ BCE\b/g, ` ${t('changeLog.bce')}`);
}

/** The backend's one-line summary of events ("b. 1 Jan 1950, Lviv; d. 2001", "m. 1 Jun 1878, Hamburg") in the active language. */
export function eventSummaryText(s: string): string {
  return s
    .split('; ')
    .map((part) => {
      const m = /^(m|b|d)\. (.*)$/s.exec(part);
      if (!m) return part;
      const rest = m[2];
      const comma = rest.indexOf(', ');
      const head = comma < 0 ? rest : rest.slice(0, comma);
      const shown = dateText(head);
      // A date alone, or a date then the place; a place alone stays as written.
      const text = comma < 0 ? shown : shown + rest.slice(comma);
      return `${t(LIFESPAN_WORDS[m[1]])} ${text}`;
    })
    .join('; ');
}
