import type { Api, EditResult, UndoState } from '../api/api';

const EMPTY: UndoState = { canUndo: false, canRedo: false, undoLabel: null, redoLabel: null };

class UndoStore {
  canUndo = $state(false);
  canRedo = $state(false);
  undoLabel = $state<string | null>(null);
  redoLabel = $state<string | null>(null);

  set(s: UndoState): void {
    this.canUndo = s.canUndo;
    this.canRedo = s.canRedo;
    this.undoLabel = s.undoLabel;
    this.redoLabel = s.redoLabel;
  }

  /** Call with every EditResult (from apply, undo and redo) so the buttons stay current. */
  ingest(result: EditResult | null): void {
    if (result) this.set(result.undo);
  }

  async refresh(api: Api): Promise<void> {
    this.set(await api.undoState());
  }

  reset(): void {
    this.set(EMPTY);
  }
}

export const undoState = new UndoStore();
