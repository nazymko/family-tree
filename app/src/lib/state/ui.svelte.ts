import type { Id, ParentRelId } from '../api/types';

export type AddRelation = 'father' | 'mother' | 'partner' | 'child' | 'sibling';

/** Cross-view requests the Shell answers (dialogs it owns). Task 9 replaces the dialog behind `add`. */
class UiState {
  add = $state<{ open: boolean; relation?: AddRelation; anchor?: Id; newParentSet?: boolean; parentRel?: ParentRelId }>({ open: false });

  requestAdd(relation?: AddRelation, anchor?: Id, newParentSet?: boolean, parentRel?: ParentRelId): void {
    this.add = { open: true, relation, anchor, newParentSet, parentRel };
  }

  closeAdd(): void {
    this.add = { open: false };
  }
}

export const ui = new UiState();
