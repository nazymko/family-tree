import type { Api, EditResult } from '../api/api';
import { undoState } from './undo.svelte';

/**
 * The one "undo only my own command" routine, shared by the shell's default toaster (`observeApi`) and the
 * views that show their own notice (`ui/undoable.ts`). `stillMine` says whether the change that produced the
 * toast is still the latest one; if not, `onStale` runs instead of undoing somebody else's change.
 */
export async function undoIfMine(
  api: Pick<Api, 'undo'>,
  stillMine: () => boolean | Promise<boolean>,
  onStale: () => void,
  after?: (r: EditResult | null) => void,
): Promise<void> {
  const go = (mine: boolean): Promise<void> => {
    if (!mine) {
      onStale();
      return Promise.resolve();
    }
    return api.undo().then((r) => {
      undoState.ingest(r);
      after?.(r);
    });
  };
  const ok = stillMine();
  return typeof ok === 'boolean' ? go(ok) : ok.then(go);
}
