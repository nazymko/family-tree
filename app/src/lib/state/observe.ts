import type { Api, EditResult } from '../api/api';
import { project } from './project.svelte';
import { toasts } from './toast.svelte';
import { undoIfMine } from './ownUndo';
import { undoState } from './undo.svelte';

const PAST: Record<string, string> = {
  Save: 'saved',
  Add: 'added',
  Delete: 'deleted',
  Edit: 'saved',
  Remove: 'removed',
  Attach: 'attached',
  Detach: 'detached',
  Set: 'set',
  Reorder: 'reordered',
  Link: 'linked',
  Unlink: 'unlinked',
};

/** "Save event" -> "Event saved". Unknown shapes fall back to "<label>: done". */
export function toastText(label: string): string {
  const m = /^([A-Z][a-z]+)\s+(.+)$/.exec(label);
  if (!m || !PAST[m[1]]) return `${label}: done`;
  return `${m[2].charAt(0).toUpperCase()}${m[2].slice(1)} ${PAST[m[1]]}`;
}

/**
 * Wraps an Api so every change anywhere in the app updates the "Saved" indicator, bumps `project.rev`
 * (views reload) and keeps the undo buttons current. It is the single default toaster: `apply(cmd, { silent: true })`
 * skips the toast for editors that show their own.
 */
export function observeApi(inner: Api): Api {
  // Counts every change that goes through this wrapper; a toast's Undo only works while its change is still the latest.
  let seq = 0;

  async function tracked<T>(work: () => Promise<T>): Promise<T> {
    project.save = 'saving';
    try {
      const r = await work();
      project.save = 'saved';
      project.rev++;
      return r;
    } catch (e) {
      project.save = 'failed';
      throw e;
    }
  }

  /** Older toasts lose their Undo button as soon as something newer changes the undo stack. */
  function supersede(): number {
    toasts.items = toasts.items.map((t) => (t.action ? { ...t, action: undefined } : t));
    return ++seq;
  }

  function undoAction(mine: number) {
    return {
      label: 'Undo',
      run: () => {
        void undoIfMine(
          wrapped as Api,
          () => mine === seq,
          () => toasts.push({ text: 'That change is no longer the latest one, so it cannot be undone from here.', kind: 'warn' }),
        );
      },
    };
  }

  const wrapped: Partial<Api> = {
    async apply(cmd, opts) {
      const r: EditResult = await tracked(() => inner.apply(cmd, opts));
      undoState.ingest(r);
      const mine = supersede();
      if (!opts?.silent) toasts.push({ text: toastText(r.label), action: undoAction(mine) });
      return r;
    },
    async undo() {
      const r = await tracked(() => inner.undo());
      undoState.ingest(r);
      supersede();
      if (r) toasts.push({ text: `Undone: ${r.label.replace(/^Undo: /, '')}` });
      return r;
    },
    async redo() {
      const r = await tracked(() => inner.redo());
      undoState.ingest(r);
      supersede();
      if (r) toasts.push({ text: `Redone: ${r.label.replace(/^Redo: /, '')}` });
      return r;
    },
    async attachFiles(target, paths, opts) {
      const r = await tracked(() => inner.attachFiles(target, paths, opts));
      await undoState.refresh(inner);
      if (r.attached.length > 0 && !opts?.silent) {
        const mine = supersede();
        const n = r.attached.length;
        toasts.push({ text: `${n} ${n === 1 ? 'file' : 'files'} attached`, action: undoAction(mine) });
      }
      return r;
    },
    relinkMedia: (folder) => tracked(() => inner.relinkMedia(folder)),
    discardImport: () => tracked(() => inner.discardImport()),
    setSettings: (patch) => tracked(() => inner.setSettings(patch)),
  };
  return new Proxy(inner, {
    get(target, prop) {
      if (prop in wrapped) return wrapped[prop as keyof Api];
      const v = Reflect.get(target, prop, target);
      return typeof v === 'function' ? v.bind(target) : v;
    },
  });
}
