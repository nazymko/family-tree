import type { Id } from '../api/types';

/**
 * The person who was just added from a dialog. The charts watch it: after the edit lands they refresh and select
 * this person. `n` counts the adds, so adding the same id twice still notifies.
 */
class AddedPerson {
  id = $state<Id | null>(null);
  n = $state(0);
  at = 0;

  note(id: Id): void {
    this.id = id;
    this.at = Date.now();
    this.n++;
  }
}

export const added = new AddedPerson();
