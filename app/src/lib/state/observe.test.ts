import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import { observeApi } from './observe';
import { project } from './project.svelte';
import { toasts } from './toast.svelte';
import { undoState } from './undo.svelte';
import { setLanguage } from '../i18n';

const draft = (given: string) => ({ type: 'createPerson' as const, draft: { given, surnames: ['T'], sex: 'unknown' as const } });

function make() {
  const raw = new MockApi(fixtureTree(20));
  return { raw, api: observeApi(raw) };
}

beforeEach(() => {
  toasts.clear();
  undoState.reset();
  project.close();
});

describe('observeApi', () => {
  it('apply_toasts_with_undo_unless_silent', async () => {
    const { api } = make();
    await api.apply(draft('A'), { silent: true });
    expect(toasts.items).toHaveLength(0);
    await api.apply(draft('B'));
    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0].action?.label).toBe('Undo');
  });

  it('older_toast_loses_undo_when_newer_change_arrives', async () => {
    const { api } = make();
    await api.apply(draft('A'));
    await api.apply(draft('B'));
    expect(toasts.items.filter((t) => t.action)).toHaveLength(1);
  });

  it('undo_click_is_rechecked_at_click_time', async () => {
    const { api, raw } = make();
    await api.apply(draft('A'));
    const action = toasts.items[0].action!;
    const spy = vi.spyOn(raw, 'undo');
    await api.apply(draft('B'), { silent: true });
    action.run();
    expect(spy).not.toHaveBeenCalled();
    expect(toasts.items.some((t) => t.kind === 'warn')).toBe(true);
  });

  it('undo_click_undoes_latest_change', async () => {
    const { api, raw } = make();
    await api.apply(draft('A'));
    const spy = vi.spyOn(raw, 'undo');
    toasts.items[0].action!.run();
    expect(spy).toHaveBeenCalledTimes(1);
  });

  const REFUSED = { code: 'conflict', message: 'An agent changed this since. Undo it from the change log first.' };

  it('refused_undo_shows_the_error_and_refreshes_the_buttons', async () => {
    const { api, raw } = make();
    await api.apply(draft('A'), { silent: true });
    expect(undoState.canUndo).toBe(true);
    vi.spyOn(raw, 'undo').mockRejectedValue(REFUSED);
    vi.spyOn(raw, 'undoState').mockResolvedValue({ canUndo: false, canRedo: false, undoLabel: null, redoLabel: null });
    await expect(api.undo()).resolves.toBeNull();
    expect(toasts.items.map((t) => [t.text, t.kind])).toEqual([[REFUSED.message, 'error']]);
    expect(undoState.canUndo).toBe(false);
    // Nothing failed to save: the undo was refused and the tree is as it was.
    expect(project.save).toBe('saved');
  });

  it('refused_redo_shows_the_error_and_refreshes_the_buttons', async () => {
    const { api, raw } = make();
    await api.apply(draft('A'), { silent: true });
    await api.undo();
    toasts.clear();
    expect(undoState.canRedo).toBe(true);
    vi.spyOn(raw, 'redo').mockRejectedValue(REFUSED);
    vi.spyOn(raw, 'undoState').mockResolvedValue({ canUndo: false, canRedo: false, undoLabel: null, redoLabel: null });
    await expect(api.redo()).resolves.toBeNull();
    expect(toasts.items.map((t) => t.text)).toEqual([REFUSED.message]);
    expect(undoState.canRedo).toBe(false);
  });

  it('refused_undo_reads_in_ukrainian', async () => {
    const { api, raw } = make();
    setLanguage('uk');
    vi.spyOn(raw, 'undo').mockRejectedValue(REFUSED);
    await api.undo();
    expect(toasts.items[0].text).toBe('Агент змінив це відтоді. Спершу скасуйте його зміну в журналі змін.');
  });

  it('a_real_undo_failure_still_shows_as_not_saved', async () => {
    const { api, raw } = make();
    vi.spyOn(raw, 'undo').mockRejectedValue({ code: 'io', message: 'The disk is full.' });
    await api.undo();
    expect(toasts.items[0].text).toBe('The disk is full.');
    expect(project.save).toBe('failed');
  });

  it('refused_undo_survives_a_failing_refresh', async () => {
    const { api, raw } = make();
    vi.spyOn(raw, 'undo').mockRejectedValue(REFUSED);
    vi.spyOn(raw, 'undoState').mockRejectedValue({ code: 'internal', message: 'x' });
    await expect(api.undo()).resolves.toBeNull();
    expect(toasts.items).toHaveLength(1);
  });

  it('undo_from_the_log_bumps_rev_and_refreshes_the_buttons', async () => {
    const { api, raw } = make();
    vi.spyOn(raw, 'undoChangeGroup').mockResolvedValue({ undone: 1, skipped: [] });
    vi.spyOn(raw, 'undoChangeBatch').mockResolvedValue({ undone: 1, skipped: [] });
    const state = vi.spyOn(raw, 'undoState');
    const r0 = project.rev;
    await api.undoChangeGroup('g');
    await api.undoChangeBatch('b');
    expect(project.rev).toBe(r0 + 2);
    expect(state).toHaveBeenCalledTimes(2);
  });

  it('other_mutations_bump_rev', async () => {
    const { api, raw } = make();
    vi.spyOn(raw, 'relinkMedia').mockResolvedValue({ relinked: 0, stillMissing: [], searchIncomplete: false });
    vi.spyOn(raw, 'discardImport').mockResolvedValue(undefined as never);
    vi.spyOn(raw, 'setSettings').mockResolvedValue(undefined as never);
    const r0 = project.rev;
    await api.relinkMedia('C:/x');
    await api.discardImport();
    await api.setSettings({});
    expect(project.rev).toBe(r0 + 3);
    expect(project.save).toBe('saved');
  });

  it('attach_bumps_rev_and_toasts_with_undo', async () => {
    const { api, raw } = make();
    vi.spyOn(raw, 'attachFiles').mockResolvedValue({ attached: [{ id: 'm1' }], skipped: [] } as never);
    const r0 = project.rev;
    await api.attachFiles({ kind: 'person', id: 'p1' } as never, ['a.jpg']);
    expect(project.rev).toBe(r0 + 1);
    expect(toasts.items[0].text).toBe('1 file attached');
    expect(toasts.items[0].action?.label).toBe('Undo');
  });
});
