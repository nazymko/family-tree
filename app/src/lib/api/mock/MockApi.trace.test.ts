import { describe, expect, it } from 'vitest';
import type { DraftPerson } from '../types';
import { MockApi } from './MockApi';
import { fixtureTree } from './fixtures';

const person = (key: string, given: string, extra: Partial<DraftPerson> = {}): DraftPerson => ({
  key,
  given,
  surname: 'Koval',
  sex: 'unknown',
  ...extra,
});

describe('MockApi importDraft and trace draft', () => {
  const fresh = () => new MockApi(fixtureTree(200));
  const count = async (api: MockApi) => (await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 1)).total;

  it('adds people and a family in one undo step', async () => {
    const api = fresh();
    const before = await count(api);
    const res = await api.apply({
      type: 'importDraft',
      persons: [person('a', 'Ivan', { birthDateRaw: '1901' }), person('b', 'Olena'), person('c', 'Petro')],
      families: [{ partner1: 'a', partner2: 'b', children: ['c'] }],
      attachPicture: false,
    });
    expect(res.label).toBe('Add traced people');
    expect(await count(api)).toBe(before + 3);
    await api.undo();
    expect(await count(api)).toBe(before);
    await api.redo();
    expect(await count(api)).toBe(before + 3);
  });

  it('refuses a bad draft and leaves the tree alone', async () => {
    const api = fresh();
    const before = await count(api);
    await expect(api.apply({ type: 'importDraft', persons: [person('a', '', { surname: '' })], families: [], attachPicture: false })).rejects.toMatchObject({ code: 'invalid' });
    await expect(
      api.apply({
        type: 'importDraft',
        persons: [person('a', 'A'), person('b', 'B')],
        families: [
          { partner1: 'a', children: ['b'] },
          { partner1: 'b', children: ['a'] },
        ],
        attachPicture: false,
      }),
    ).rejects.toMatchObject({ code: 'invalid' });
    expect(await count(api)).toBe(before);
    expect((await api.undoState()).canUndo).toBe(false);
  });

  it('keeps and forgets the draft and the picture', async () => {
    const api = fresh();
    expect(await api.loadTraceDraft()).toEqual({ json: null, image: null });
    await expect(api.stageTraceImage('C:/x/notes.txt')).rejects.toMatchObject({ code: 'invalid' });
    const info = await api.stageTraceImage('C:/x/wall chart.png');
    expect(info.name).toBe('wall chart.png');
    await api.saveTraceDraft('{"a":1}');
    expect((await api.loadTraceDraft()).json).toBe('{"a":1}');
    await api.discardTraceDraft();
    expect(await api.loadTraceDraft()).toEqual({ json: null, image: null });
  });
});
