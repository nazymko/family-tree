import { describe, expect, it } from 'vitest';
import { MockApi } from './MockApi';
import { fixtureTree } from './fixtures';

/** The record each Open scrolls to: the one the page draws, found up the chain of holders. */
describe('MockApi.locateRecords targets', () => {
  async function world() {
    const api = new MockApi(fixtureTree(10));
    const id = (await api.apply({ type: 'createPerson', draft: { given: 'Ada', surnames: ['Koval'], sex: 'female' } })).focus!;
    return { api, id };
  }

  it('a_note_on_the_person_is_the_target_and_a_note_on_an_event_is_the_event', async () => {
    const { api, id } = await world();
    const own = (await api.apply({ type: 'saveNote', target: { kind: 'person', id }, note: { kind: 'general', text: 'On Ada' } })).touched[1];
    const ev = (
      await api.apply({
        type: 'saveEvent',
        owner: { kind: 'person', id },
        event: { kind: 'birth', dateRaw: '1900', negative: false, private: false, preferred: true },
      })
    ).touched[1];
    const onEvent = (await api.apply({ type: 'saveNote', target: { kind: 'event', id: ev }, note: { kind: 'general', text: 'On the birth' } })).touched[1];
    const got = await api.locateRecords([
      { kind: 'note', id: own },
      { kind: 'event', id: ev },
      { kind: 'note', id: onEvent },
      { kind: 'person', id },
    ]);
    expect(got.map((g) => g?.target)).toEqual([
      { kind: 'note', id: own },
      { kind: 'event', id: ev },
      { kind: 'event', id: ev },
      { kind: 'person', id },
    ]);
    expect(got.every((g) => g?.page === 'person' && g.id === id)).toBe(true);
  });
});
