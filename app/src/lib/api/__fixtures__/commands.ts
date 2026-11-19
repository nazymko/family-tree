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
  setMediaTranscription: {
    type: 'setMediaTranscription',
    id: `${ID}9`,
    regions: [{ id: `${ID}a`, area: { x: 0.1, y: 0.2, w: 0.5, h: 0.1 }, rotation: 90, text: 'Dear Olha,' }, { text: 'Signed below' }],
  },
  setMediaRotation: { type: 'setMediaRotation', id: `${ID}9`, rotation: 270 },
  groupMediaVersions: { type: 'groupMediaVersions', primary: `${ID}9`, others: [`${ID}8`] },
  ungroupMediaVersion: { type: 'ungroupMediaVersion', media: `${ID}8`, newPrimary: `${ID}7` },
  addAudioClip: { type: 'addAudioClip', target: { kind: 'note', id: `${ID}5` }, media: `${ID}9`, startMs: 65000, endMs: 102500, label: 'How the house was built' },
  updateAudioClip: {
    type: 'updateAudioClip',
    target: { kind: 'note', id: `${ID}5` },
    clip: `${ID}8`,
    startMs: 64000,
    endMs: 103000,
    label: 'The house',
    moveTo: { kind: 'event', id: `${ID}6` },
  },
  deleteAudioClip: { type: 'deleteAudioClip', target: { kind: 'person', id: `${ID}1` }, clip: `${ID}8` },
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
  addSiblingMarker: { type: 'addSiblingMarker', family: `${ID}3` },
  setPlaceCoordinates: { type: 'setPlaceCoordinates', place: `${ID}7`, latE6: 50123456, lonE6: 30654321 },
  setMapLayer: {
    type: 'setMapLayer',
    media: `${ID}9`,
    title: 'Sheet 30-13',
    layer: {
      series: 'Three-verst map',
      year: '1865',
      source: 'https://example.org/sheet-30-13',
      corners: [
        { latE6: 50600000, lonE6: 30000000 },
        { latE6: 50600000, lonE6: 31200000 },
        { latE6: 49900000, lonE6: 31200000 },
        { latE6: 49900000, lonE6: 30000000 },
      ],
      opacity: 60,
      visible: true,
    },
  },
  reorderMapLayers: { type: 'reorderMapLayers', order: [`${ID}9`, `${ID}8`] },
  setMapLayersVisible: { type: 'setMapLayersVisible', shown: [`${ID}9`], hidden: [`${ID}8`] },
  addNumber: { type: 'addNumber', person: `${ID}1`, sourceFile: 'recording2.txt', number: '12' },
  addGodparent: { type: 'addGodparent', event: `${ID}4`, godparent: { name: 'Ivan Petrovych Koval, a peasant', role: 'godfather', note: 'of Ivanivka' } },
  removeGodparent: { type: 'removeGodparent', event: `${ID}4`, godparent: `${ID}8` },
} satisfies { [K in EditCommand['type']]: Extract<EditCommand, { type: K }> };

export const commandFixtures: EditCommand[] = Object.values(commandFixtureMap) as EditCommand[];
