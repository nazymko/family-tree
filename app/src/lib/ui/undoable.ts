import type { Api, UndoState } from '../api/api';
import { undoState } from '../state/undo.svelte';
import { undoIfMine } from '../state/ownUndo';
import { notices, type Notice } from './notices.svelte';

/**
 * A notice whose Undo only undoes the command that produced `state` (its undo label is on top of the stack).
 * The action disappears once another command changes the top label, and is re-checked at click time.
 * Pass `apply(cmd, { silent: true })` for the command, so the shell's default toast does not appear as well.
 */
export function undoNotice(api: Api, message: string, state: UndoState, after?: () => void): Notice {
  undoState.set(state);
  const label = state.undoLabel;
  return {
    message,
    actionLabel: 'Undo',
    actionValid: () => undoState.canUndo && undoState.undoLabel === label,
    onaction: () => {
      void undoIfMine(
        api,
        async () => {
          const now = await api.undoState();
          return now.canUndo && now.undoLabel === label;
        },
        () => notices.show({ message: 'That change is no longer the latest one, so it cannot be undone from here. Use Undo in the top bar.' }),
        () => after?.(),
      );
    },
  };
}
