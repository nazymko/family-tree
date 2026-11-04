import { t, type MessageKey } from '.';

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
  'Set primary media': ['edit.setPrimaryMedia', 'edit.donePrimaryMediaSet'],
  'Edit media': ['edit.editMedia', 'edit.doneMediaSaved'],
  'Link media files': ['edit.linkMediaFiles', 'edit.doneMediaFilesLinked'],
  'Add father': ['edit.addFather', 'edit.doneFatherAdded'],
  'Add mother': ['edit.addMother', 'edit.doneMotherAdded'],
  'Add partner': ['edit.addPartner', 'edit.donePartnerAdded'],
  'Add child': ['edit.addChild', 'edit.doneChildAdded'],
  'Add sibling': ['edit.addSibling', 'edit.doneSiblingAdded'],
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
