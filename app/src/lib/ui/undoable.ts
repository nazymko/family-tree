import type { Api, UndoState } from '../api/api';
import { undoState } from '../state/undo.svelte';
import { notices, type Notice } from './notices.svelte';

/**
 * A toast whose Undo only undoes the command that produced `state` (its undo label is on top of the
 * stack). The action disappears once another command changes the top label (the shell ingests every
 * EditResult into `undoState`), and is re-checked against the Api at click time.
 */
export function undoNotice(api: Api, message: string, state: UndoState, after?: () => void): Notice {
  undoState.set(state);
  const label = state.undoLabel;
  return {
    message,
    actionLabel: 'Undo',
    actionValid: () => undoState.canUndo && undoState.undoLabel === label,
    onaction: () => {
      void (async () => {
        const now = await api.undoState();
        if (!now.canUndo || now.undoLabel !== label) {
          notices.show({ message: 'That change is no longer the latest one, so it cannot be undone from here. Use Undo in the Edit menu.' });
          return;
        }
        undoState.ingest(await api.undo());
        after?.();
      })();
    },
  };
}
