import type { EditCommand } from '../types';

const ID = '018f3a6e-0000-7000-8000-00000000000';

/**
 * One object per EditCommand variant. The compiler checks every variant is present and well-formed;
 * `commands.json` is the same data, which the Rust crate gt-session deserialises
 * (test `edit_command_json_matches_ts_fixtures`). MockApi.test.ts fails if the two drift.
 */
export const commandFixtureMap = {
  createPerson: { type: 'createPerson', draft: { given: 'Anna', surnames: ['Schmidt'], patronymic: 'Ivanivna', sex: 'female', birthDateRaw: '3 Apr 1850', birthPlaceText: 'Hamburg, Germany' } },
  updatePerson: { type: 'updatePerson', id: `${ID}1`, sex: 'other', sexText: 'non-binary', livingOverride: null, private: true },
  deletePerson: { type: 'deletePerson', id: `${ID}1` },
  saveName: {
    type: 'saveName',
    person: `${ID}1`,
    name: { kind: 'married', preferred: true, private: false, given: 'Anna', surnames: ['Meyer'], dateRaw: '1875' },
  },
  deleteName: { type: 'deleteName', person: `${ID}1`, name: `${ID}2` },
  saveEvent: {
    type: 'saveEvent',
    owner: { kind: 'person', id: `${ID}1` },
    event: { kind: 'custom:apprenticeship', dateRaw: 'between 1865 and 1868', placeText: 'Berlin, Brandenburg, Germany', negative: false, private: false, preferred: false, participants: [{ person: `${ID}1`, role: 'principal' }] },
  },
  deleteEvent: { type: 'deleteEvent', owner: { kind: 'family', id: `${ID}3` }, event: `${ID}4` },
  quickAdd: { type: 'quickAdd', anchor: `${ID}1`, relation: 'father', draft: { surnames: ['Schmidt'], sex: 'male' }, family: `${ID}3`, newParentSet: true },
  saveFamily: { type: 'saveFamily', id: `${ID}3`, partner1: `${ID}1`, partner2: null, kind: 'civil_union' },
  deleteFamily: { type: 'deleteFamily', id: `${ID}3` },
  linkChild: { type: 'linkChild', family: `${ID}3`, child: `${ID}5`, rel: ['birth', 'adopted'], preferred: true },
  updateChildLink: { type: 'updateChildLink', family: `${ID}3`, child: `${ID}5`, rel: ['foster', 'foster'], preferred: false },
  unlinkChild: { type: 'unlinkChild', family: `${ID}3`, child: `${ID}5` },
  reorderChildren: { type: 'reorderChildren', family: `${ID}3`, order: [`${ID}6`, `${ID}5`] },
  saveNote: { type: 'saveNote', target: { kind: 'person', id: `${ID}1` }, note: { kind: 'story', text: 'Moved to Hamburg in spring.' } },
  deleteNote: { type: 'deleteNote', target: { kind: 'event', id: `${ID}4` }, note: `${ID}7` },
  saveSource: { type: 'saveSource', source: { title: 'Parish register of St Michael', abbreviation: 'PRSM' } },
  addCitation: {
    type: 'addCitation',
    target: { kind: 'event', id: `${ID}4` },
    citation: { source: { title: 'Census 1880' }, page: 'p. 12', quality: 2, evidence: { original: true, primaryInfo: true, direct: false } },
  },
  updateCitation: { type: 'updateCitation', id: `${ID}8`, citation: { page: 'p. 13', quality: 3 } },
  removeCitation: { type: 'removeCitation', target: { kind: 'name', id: `${ID}2` }, citation: `${ID}8` },
  attachMedia: { type: 'attachMedia', target: { kind: 'person', id: `${ID}1` }, media: [`${ID}9`] },
  detachMedia: { type: 'detachMedia', target: { kind: 'person', id: `${ID}1` }, media: `${ID}9` },
  deleteMedia: { type: 'deleteMedia', id: `${ID}9` },
  setPrimaryMedia: { type: 'setPrimaryMedia', person: `${ID}1`, media: `${ID}9` },
  updateMedia: { type: 'updateMedia', id: `${ID}9`, title: 'Wedding photograph', description: 'Outside the church', dateRaw: 'abt 1878' },
  linkExisting: { type: 'linkExisting', anchor: `${ID}1`, relation: 'mother', person: `${ID}3`, family: `${ID}4`, newParentSet: false },
  importDraft: {
    type: 'importDraft',
    persons: [
      { key: 'a', given: 'Ivan', surname: 'Koval', patronymic: 'Petrovych', sex: 'male', birthDateRaw: '1901', deathDateRaw: '1975', notes: 'Top row' },
      { key: 'b', existing: `${ID}1`, sex: 'unknown' },
    ],
    families: [{ partner1: 'a', partner2: 'b', children: [] }],
    attachPicture: true,
  },
} satisfies { [K in EditCommand['type']]: Extract<EditCommand, { type: K }> };

export const commandFixtures: EditCommand[] = Object.values(commandFixtureMap) as EditCommand[];
