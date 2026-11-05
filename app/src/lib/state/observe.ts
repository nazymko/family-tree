import type { Api, EditResult, ErrorCode, UndoReport } from '../api/api';
import { project } from './project.svelte';
import { toasts } from './toast.svelte';
import { undoIfMine } from './ownUndo';
import { undoState } from './undo.svelte';
import { t, plural } from '../i18n';
import { editDoneText, editLabel } from '../i18n/backend';
import { errMessage } from '../editors/errors';

/** "Save event" -> "Event saved", in the active language. */
export function toastText(label: string): string {
  return editDoneText(label);
}

/** Errors the backend sends when it turned a change down before writing anything. */
const LOST_NOTHING: ReadonlySet<string> = new Set<ErrorCode>(['not_found', 'invalid', 'conflict', 'no_project', 'stale']);

/**
 * Wraps an Api so every change anywhere in the app updates the "Saved" indicator, bumps `project.rev`
 * (views reload) and keeps the undo buttons current. It is the single default toaster: `apply(cmd, { silent: true })`
 * skips the toast for editors that show their own.
 */
export function observeApi(inner: Api): Api {
  // Counts every change that goes through this wrapper; a toast's Undo only works while its change is still the latest.
  let seq = 0;

  async function tracked<T>(work: () => Promise<T>): Promise<T> {
    const before = project.save === 'saving' ? 'saved' : project.save;
    project.save = 'saving';
    try {
      const r = await work();
      project.save = 'saved';
      project.rev++;
      return r;
    } catch (e) {
      // A refusal left the tree as it was, so it says nothing about what is saved.
      project.save = LOST_NOTHING.has((e as { code?: string } | null)?.code ?? '') ? before : 'failed';
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
      label: t('shell.undo'),
      run: () => {
        void undoIfMine(
          wrapped as Api,
          () => mine === seq,
          () => toasts.push({ text: t('edit.notLatest'), kind: 'warn' }),
        );
      },
    };
  }

  /**
   * A failed undo or redo: says why, and puts the buttons right, since the backend may have dropped the step.
   * Resolves rather than rejects because the callers are key presses and toast buttons, with nobody to catch it.
   */
  async function refused(e: unknown): Promise<null> {
    supersede();
    toasts.push({ text: errMessage(e), kind: 'error' });
    await undoState.refresh(inner).catch(() => {});
    return null;
  }

  /** Undo from the change log is off the Ctrl+Z stacks, but it can leave a step there unavailable. */
  async function logUndo(work: () => Promise<UndoReport>): Promise<UndoReport> {
    try {
      const r = await tracked(work);
      supersede();
      return r;
    } finally {
      // Either way the backend may have dropped a step. A failed read must not replace the answer the caller is waiting for.
      await undoState.refresh(inner).catch(() => {});
    }
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
      let r: EditResult | null;
      try {
        r = await tracked(() => inner.undo());
      } catch (e) {
        return refused(e);
      }
      undoState.ingest(r);
      supersede();
      if (r) toasts.push({ text: t('edit.undone', { label: editLabel(r.label.replace(/^Undo: /, '')) }) });
      return r;
    },
    async redo() {
      let r: EditResult | null;
      try {
        r = await tracked(() => inner.redo());
      } catch (e) {
        return refused(e);
      }
      undoState.ingest(r);
      supersede();
      if (r) toasts.push({ text: t('edit.redone', { label: editLabel(r.label.replace(/^Redo: /, '')) }) });
      return r;
    },
    async attachFiles(target, paths, opts) {
      const r = await tracked(() => inner.attachFiles(target, paths, opts));
      await undoState.refresh(inner);
      if (r.attached.length > 0) {
        // Silent or not, the attach is now the latest step; an older toast's Undo must not undo it.
        const mine = supersede();
        const n = r.attached.length;
        if (!opts?.silent) toasts.push({ text: plural('edit.filesAttached', n), action: undoAction(mine) });
      }
      return r;
    },
    relinkMedia: (folder) => tracked(() => inner.relinkMedia(folder)),
    discardImport: () => tracked(() => inner.discardImport()),
    setSettings: (patch) => tracked(() => inner.setSettings(patch)),
    undoChangeGroup: (group) => logUndo(() => inner.undoChangeGroup(group)),
    undoChangeBatch: (batch) => logUndo(() => inner.undoChangeBatch(batch)),
  };
  return new Proxy(inner, {
    get(target, prop) {
      if (prop in wrapped) return wrapped[prop as keyof Api];
      const v = Reflect.get(target, prop, target);
      return typeof v === 'function' ? v.bind(target) : v;
    },
  });
}
