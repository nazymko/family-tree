import { describe, expect, it } from 'vitest';
import { MockApi } from './MockApi';
import { fixtureTree } from './fixtures';

const q = { sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false } as const;

describe('MockApi read isolation', () => {
  it('mutating_returned_objects_does_not_change_db_or_undo', async () => {
    const api = new MockApi(fixtureTree(200));
    const page = await api.listPersons(q, 0, 5);
    const row = page.items[0];
    const name = row.name;
    row.name = 'Changed';
    page.items.length = 0;

    const detail = await api.getPerson(row.id);
    const display = detail.names[0].display;
    detail.names[0].display = 'Changed';
    detail.events.length = 0;
    const view = await api.getFamilyView(row.id);
    view.focus.name = 'Changed';

    expect((await api.listPersons(q, 0, 5)).items[0].name).toBe(name);
    const again = await api.getPerson(row.id);
    expect(again.names[0].display).toBe(display);
    expect(again.events.length).toBeGreaterThan(0);
    expect((await api.getFamilyView(row.id)).focus.name).toBe(name);
    expect(await api.undoState()).toEqual({ canUndo: false, canRedo: false, undoLabel: null, redoLabel: null });
  });

  it('mutating_after_an_edit_leaves_undo_snapshot_intact', async () => {
    const api = new MockApi(fixtureTree(200));
    const row = (await api.listPersons(q, 0, 1)).items[0];
    await api.apply({ type: 'updatePerson', id: row.id, private: true });
    const d = await api.getPerson(row.id);
    d.names[0].display = 'Changed';
    d.private = false;
    await api.undo();
    const back = await api.getPerson(row.id);
    expect(back.names[0].display).not.toBe('Changed');
    expect(back.private).toBe(row.private);
  });
});
