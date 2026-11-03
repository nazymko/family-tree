import type { Id } from '../api/types';

export type AddRelation = 'father' | 'mother' | 'partner' | 'child' | 'sibling';

/** Cross-view requests the Shell answers (dialogs it owns). Task 9 replaces the dialog behind `add`. */
class UiState {
  add = $state<{ open: boolean; relation?: AddRelation; anchor?: Id; newParentSet?: boolean }>({ open: false });

  requestAdd(relation?: AddRelation, anchor?: Id, newParentSet?: boolean): void {
    this.add = { open: true, relation, anchor, newParentSet };
  }

  closeAdd(): void {
    this.add = { open: false };
  }
}

export const ui = new UiState();
