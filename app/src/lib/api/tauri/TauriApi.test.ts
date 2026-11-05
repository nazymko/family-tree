import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { clearMocks, mockConvertFileSrc, mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { emit } from '@tauri-apps/api/event';
import { waitFor } from '@testing-library/dom';
import apiSource from '../api.ts?raw';
import { MockApi } from '../mock/MockApi';
import { defaultApi } from '../context';
import type { ImportProgress, ImportRequest, PersonQuery } from '../types';
import { TauriApi } from './TauriApi';
import { TauriPlatform } from './TauriPlatform';
import { toApiError } from './errors';

type Args = Record<string, unknown> | undefined;
type Call = { cmd: string; args: Args };
let calls: Call[];
let reply: (cmd: string, args: Args) => unknown;

beforeEach(() => {
  calls = [];
  reply = () => null;
  mockWindows('main');
  mockConvertFileSrc('windows');
  mockIPC(
    (cmd, args) => {
      if (cmd.startsWith('plugin:')) return null;
      calls.push({ cmd, args: args as Args });
      return reply(cmd, args as Args);
    },
    { shouldMockEvents: true },
  );
});

afterEach(() => {
  clearMocks();
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
});

const query: PersonQuery = { sort: 'surname', desc: false, text: 'ann', living: 'any', noSource: false, noBirthDate: false, privateOnly: false };
const request: ImportRequest = { file: 'a.ged', projectDir: 'd', projectName: 'n', mediaRoots: [], dayOrder: 'dmy' };

describe('TauriApi', () => {
  it('list_persons_args_shape', async () => {
    reply = () => ({ total: 0, offset: 5, items: [] });
    const page = await new TauriApi().listPersons(query, 5, 50);
    expect(calls).toEqual([{ cmd: 'list_persons', args: { q: query, offset: 5, limit: 50 } }]);
    expect(page.total).toBe(0);
  });

  it('save_picture_sends_the_bytes_as_the_raw_body', async () => {
    reply = () => ({ bytes: 4 });
    const bytes = new Uint8Array([0x89, 0x50, 0x4e, 0x47]);
    await expect(new TauriApi().savePicture('C:/Pictures/Pedigree - Ivan Koval.png', bytes)).resolves.toEqual({ bytes: 4 });
    expect(calls).toHaveLength(1);
    expect(calls[0].cmd).toBe('save_picture');
    expect(calls[0].args).toBe(bytes);
  });

  it('save_picture_failure_is_a_plain_api_error', async () => {
    reply = () => {
      throw { code: 'io', message: 'Could not save the picture: the disk is full. Choose another place and try again.' };
    };
    await expect(new TauriApi().savePicture('C:/x.png', new Uint8Array([1]))).rejects.toMatchObject({ code: 'io' });
  });

  it('change_log_commands_args_shape', async () => {
    const api = new TauriApi();
    const filter = { unreviewedOnly: true, actorKind: 'agent' };
    await api.changeLog(filter, 5, 50);
    await api.changeBatches();
    await api.markReviewed({ kind: 'batch', id: 'b1' });
    await api.unreviewedPeople();
    await api.personAgentChanges('p1', 0, 20);
    await api.hasUntrackedChanges();
    await api.recordHome('note', 'n1');
    await api.personHistory('p1', 20, 20);
    await api.undoChangeGroup('g1');
    await api.undoChangeBatch('b1');
    expect(calls).toEqual([
      { cmd: 'change_log', args: { filter, offset: 5, limit: 50 } },
      { cmd: 'change_batches', args: {} },
      { cmd: 'mark_reviewed', args: { scope: { kind: 'batch', id: 'b1' } } },
      { cmd: 'unreviewed_people', args: {} },
      { cmd: 'person_agent_changes', args: { id: 'p1', offset: 0, limit: 20 } },
      { cmd: 'has_untracked_changes', args: {} },
      { cmd: 'record_home', args: { kind: 'note', id: 'n1' } },
      { cmd: 'person_history', args: { id: 'p1', offset: 20, limit: 20 } },
      { cmd: 'undo_change_group', args: { group: 'g1' } },
      { cmd: 'undo_change_batch', args: { batch: 'b1' } },
    ]);
  });

  it('apply_passes_command_object_unchanged', async () => {
    const cmd = { type: 'deletePerson', id: 'p1' } as const;
    reply = () => ({ label: 'x', focus: null, touched: [], undo: {} });
    await new TauriApi().apply(cmd);
    expect(calls).toHaveLength(1);
    expect(calls[0].cmd).toBe('apply');
    expect(calls[0].args).toEqual({ cmd });
    expect(calls[0].args!.cmd).toBe(cmd);
  });

  it('string_error_becomes_internal', async () => {
    reply = () => {
      throw 'command nope not found';
    };
    await expect(new TauriApi().getSettings()).rejects.toEqual({ code: 'internal', message: 'command nope not found' });
    expect(toApiError({ weird: true })).toEqual({ code: 'internal', message: 'Something went wrong. Try again.' });
    expect(toApiError({ code: 'bogus', message: 'm' }).code).toBe('internal');
    expect(toApiError(null).code).toBe('internal');
  });

  it('structured_error_preserved', async () => {
    reply = () => {
      throw { code: 'no_project', message: 'Open a project first.' };
    };
    await expect(new TauriApi().undoState()).rejects.toEqual({ code: 'no_project', message: 'Open a project first.' });
  });

  it('import_progress_listener_cleaned_up_on_failure', async () => {
    const seen: ImportProgress[] = [];
    reply = () => {
      throw { code: 'io', message: 'disk gone' };
    };
    await expect(new TauriApi().importGedcom(request, (x) => seen.push(x))).rejects.toMatchObject({ code: 'io' });
    expect(calls.map((c) => c.cmd)).toEqual(['import_gedcom']);
    // Unsubscribed: a late event reaches nobody.
    await emit('import-progress', { phase: 'reading', done: 1, total: 2 });
    expect(seen).toEqual([]);
  });

  it('import_progress_is_forwarded_during_the_call', async () => {
    const seen: ImportProgress[] = [];
    reply = (cmd) => (cmd === 'import_gedcom' ? new Promise(() => {}) : null);
    void new TauriApi().importGedcom(request, (x) => seen.push(x));
    await waitFor(() => expect(calls.some((c) => c.cmd === 'import_gedcom')).toBe(true));
    await emit('import-progress', { phase: 'parsing', done: 1, total: 2 });
    expect(seen).toEqual([{ phase: 'parsing', done: 1, total: 2 }]);
  });

  it('media_url_uses_gtmedia_scheme', () => {
    const id = '0190a5c0-0000-7000-8000-000000000001';
    // beforeEach mocked the Windows platform.
    expect(new TauriApi().mediaUrl(id, 'thumb')).toBe(`http://gtmedia.localhost/${id}/thumb`);
    mockConvertFileSrc('linux');
    expect(new TauriApi().mediaUrl(id, 'original')).toBe(`gtmedia://localhost/${id}/original`);
    expect(new TauriApi().mediaUrl(id, 'preview')).not.toContain('%2F');
  });

  it('trace_image_url_uses_gtmedia_scheme', () => {
    expect(new TauriApi().traceImageUrl(3)).toBe('http://gtmedia.localhost/trace/picture?v=3');
    mockConvertFileSrc('linux');
    expect(new TauriApi().traceImageUrl(1)).toBe('gtmedia://localhost/trace/picture?v=1');
  });

  it('open_media_externally_sends_only_the_media_id', async () => {
    await new TauriApi().openMediaExternally('m1');
    expect(calls).toEqual([{ cmd: 'open_media_externally', args: { id: 'm1' } }]);
  });

  it('every_api_method_implemented', () => {
    const body = apiSource.slice(apiSource.indexOf('export interface Api {'), apiSource.indexOf('/** OS dialogs'));
    const surface = [...body.matchAll(/^ {2}(\w+)\(/gm)].map((m) => m[1]);
    expect(surface.length).toBeGreaterThan(30);
    const tauri = Object.getOwnPropertyNames(TauriApi.prototype).filter((n) => n !== 'constructor');
    const mock = Object.getOwnPropertyNames(MockApi.prototype);
    for (const name of surface) {
      expect(tauri, `TauriApi.${name}`).toContain(name);
      expect(mock, `MockApi.${name}`).toContain(name);
    }
    // And no stray method on the Tauri side that the interface does not know.
    expect(tauri.filter((n) => !surface.includes(n))).toEqual([]);
  });

  it('every_method_calls_its_snake_case_command', async () => {
    const api = new TauriApi() as unknown as Record<string, (...a: unknown[]) => unknown>;
    const skip = new Set(['constructor', 'mediaUrl', 'traceImageUrl', 'importGedcom']);
    for (const name of Object.getOwnPropertyNames(TauriApi.prototype).filter((n) => !skip.has(n))) {
      calls = [];
      await api[name]();
      expect(calls, name).toHaveLength(1);
      expect(calls[0].cmd).toBe(name.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`));
    }
  });

  // --- Agent access: Task 6 must add Rust commands with these names and argument names ---
  it('agent_access_commands_and_args', async () => {
    const api = new TauriApi();
    reply = (cmd) => (cmd === 'agent_connection_create' ? { connection: { id: 'c1', name: 'n', created: 1 }, key: 'gt_ab', snippet: 's' } : null);
    await api.agentAccessState();
    await api.agentAccessSet(true, 8765);
    await api.agentAccessRequireKey(true);
    await api.agentConnectionCreate('Laptop');
    await api.agentConnectionRevoke('c1');
    await api.agentRules();
    expect(calls).toEqual([
      { cmd: 'agent_access_state', args: {} },
      { cmd: 'agent_access_set', args: { enabled: true, port: 8765 } },
      { cmd: 'agent_access_require_key', args: { required: true } },
      { cmd: 'agent_connection_create', args: { name: 'Laptop' } },
      { cmd: 'agent_connection_revoke', args: { id: 'c1' } },
      { cmd: 'agent_rules', args: {} },
    ]);
  });
  it('default_api_picks_tauri_when_internals_present', () => {
    // mockIPC (beforeEach) installed window.__TAURI_INTERNALS__.
    const b = defaultApi();
    expect(b.api).toBeInstanceOf(TauriApi);
    expect(b.platform).toBeInstanceOf(TauriPlatform);
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    expect(defaultApi().api).toBeInstanceOf(MockApi);
  });
});

describe('TauriPlatform', () => {
  it('agent_event_maps_to_callback_until_unsubscribed', async () => {
    const heard = vi.fn();
    const off = new TauriPlatform().onTreeChangedByAgent(heard);
    const payload = { touched: ['p1', 'n1'], agent: 'Transcript reader' };
    await vi.waitFor(async () => {
      await emit('tree-changed-by-agent', payload);
      expect(heard).toHaveBeenCalledWith(payload);
    });
    off();
    heard.mockClear();
    await emit('tree-changed-by-agent', payload);
    expect(heard).not.toHaveBeenCalled();
  });

  it('drag_drop_events_map_to_callbacks', async () => {
    const hover = vi.fn();
    const dropped = vi.fn();
    const off = new TauriPlatform().onFilesDropped(dropped, hover);
    const pos = { x: 1, y: 2 };
    // The listener registers asynchronously; keep emitting until it is in place.
    await vi.waitFor(async () => {
      await emit('tauri://drag-over', { position: pos });
      expect(hover).toHaveBeenCalledWith(true);
    });
    await emit('tauri://drag-leave', {});
    expect(hover).toHaveBeenLastCalledWith(false);
    await emit('tauri://drag-over', { position: pos });
    await emit('tauri://drag-drop', { paths: ['C:/a.jpg', 'C:/b.png'], position: pos });
    expect(dropped).toHaveBeenCalledWith(['C:/a.jpg', 'C:/b.png']);
    expect(hover).toHaveBeenLastCalledWith(false);
    off();
    dropped.mockClear();
    await emit('tauri://drag-drop', { paths: ['C:/c.jpg'], position: pos });
    expect(dropped).not.toHaveBeenCalled();
  });
});
