import type { Api, EditResult } from '../api/api';
import { project } from './project.svelte';
import { toasts } from './toast.svelte';
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
 * Wraps an Api so every edit anywhere in the app keeps the undo buttons current, updates the
 * "Saved" indicator and raises a toast with Undo. Views just call `getApi().apply(...)`.
 */
export function observeApi(inner: Api): Api {
  const wrapped: Partial<Api> = {
    async apply(cmd) {
      project.save = 'saving';
      let r: EditResult;
      try {
        r = await inner.apply(cmd);
      } catch (e) {
        project.save = 'failed';
        throw e;
      }
      project.save = 'saved';
      project.rev++;
      undoState.ingest(r);
      toasts.push({
        text: toastText(r.label),
        action: {
          label: 'Undo',
          run: () => {
            void wrapped.undo!();
          },
        },
      });
      return r;
    },
    async undo() {
      const r = await inner.undo();
      undoState.ingest(r);
      project.rev++;
      if (r) toasts.push({ text: `Undone: ${r.label.replace(/^Undo: /, '')}` });
      return r;
    },
    async redo() {
      const r = await inner.redo();
      undoState.ingest(r);
      project.rev++;
      if (r) toasts.push({ text: `Redone: ${r.label.replace(/^Redo: /, '')}` });
      return r;
    },
  };
  return new Proxy(inner, {
    get(target, prop) {
      if (prop in wrapped) return wrapped[prop as keyof Api];
      const v = Reflect.get(target, prop, target);
      return typeof v === 'function' ? v.bind(target) : v;
    },
  });
}
