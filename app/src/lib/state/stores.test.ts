import { beforeEach, describe, expect, it } from 'vitest';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import { settings } from './settings.svelte';
import { undoState } from './undo.svelte';

describe('settings and undo stores', () => {
  beforeEach(() => {
    undoState.reset();
    document.documentElement.removeAttribute('data-theme');
  });

  it('settings_update_applies_theme', async () => {
    const api = new MockApi(fixtureTree(100));
    await settings.update(api, { theme: 'dark', dayOrder: 'mdy' });
    expect(settings.value.dayOrder).toBe('mdy');
    expect(document.documentElement.dataset.theme).toBe('dark');
    await settings.update(api, { theme: 'system' });
    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
  });

  it('undo_state_follows_every_edit_result', async () => {
    const api = new MockApi(fixtureTree(100));
    await undoState.refresh(api);
    expect(undoState.canUndo).toBe(false);
    undoState.ingest(await api.apply({ type: 'createPerson', draft: { given: 'A', surnames: ['B'], sex: 'unknown' } }));
    expect(undoState.canUndo).toBe(true);
    expect(undoState.undoLabel).toBe('Add person');
    undoState.ingest(await api.undo());
    expect(undoState.canRedo).toBe(true);
    expect(undoState.redoLabel).toBe('Add person');
  });
});
