import { t, type MessageKey } from '.';
import type { LogFactDto } from '../api/types';

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
  const keys = EDIT_KEYS[label];
  return keys ? t(keys[0]) : label;
}

/** The toast sentence after a change ("Person added"). */
export function editDoneText(label: string): string {
  const keys = EDIT_KEYS[label];
  return keys ? t(keys[1]) : t('edit.doneFallback', { label: editLabel(label) });
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
  'Something this depends on was removed from the change log, so it cannot be undone. The step has been dropped from the undo list.': 'changeLog.errors.undoDropped',
  'Something this depends on was removed from the change log, so it cannot be redone. The step has been dropped from the redo list.': 'changeLog.errors.redoDropped',
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

function simpleDate(s: string): string | null {
  const m = /^(?:(\d{1,2}) )?(?:(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) )?(\d{1,4}(?:\/\d{2})?)$/.exec(s);
  if (!m) return null;
  const month = m[2] ? t(`changeLog.month.${m[2]}` as MessageKey) : '';
  return [m[1], month, m[3]].filter(Boolean).join(' ');
}

/**
 * The backend's date text ("about 3 Apr 1901") in the active language. A date it cannot read as that
 * grammar, such as a phrase the owner typed, stays exactly as written.
 */
export function dateText(display: string): string {
  const phrase = /^(.*\S) (\(.*\))$/.exec(display);
  if (phrase) {
    const head = dateText(phrase[1]);
    return head === phrase[1] && !simpleDate(phrase[1]) ? display : `${head} ${phrase[2]}`;
  }
  const between = /^between (.+) and (.+)$/.exec(display);
  const fromTo = /^from (.+) to (.+)$/.exec(display);
  const pair = between ?? fromTo;
  if (pair) {
    const a = simpleDate(pair[1]);
    const b = simpleDate(pair[2]);
    if (a && b) return t(between ? 'changeLog.date.between' : 'changeLog.date.fromTo', { a, b });
    return display;
  }
  const word = /^(about|calculated|estimated|before|after|from|to) (.+)$/.exec(display);
  if (word) {
    const date = simpleDate(word[2]);
    return date ? t(DATE_WORDS[word[1]], { date }) : display;
  }
  return simpleDate(display) ?? display;
}

/** One fact of the change log. Fixed words are translated; what the owner or an agent wrote stays as written. */
export function factText(fact: LogFactDto): string {
  const name = (s: string) => (s === 'Unknown' ? t('changeLog.unknownPerson') : s);
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
