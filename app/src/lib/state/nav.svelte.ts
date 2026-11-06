import type { Id } from '../api/types';

export type Route =
  | { name: 'welcome' }
  | { name: 'importReport' }
  | { name: 'family' }
  | { name: 'people' }
  /** `people`: the whole-tree page showing only these people, their ancestors and partners (Back returns to everyone). */
  | { name: 'whole'; people?: Id[] }
  | { name: 'pedigree' }
  | { name: 'descendants' }
  | { name: 'relatives' }
  | { name: 'person'; id: Id }
  | { name: 'sources' }
  | { name: 'media' }
  | { name: 'log' }
  | { name: 'trace' }
  | { name: 'settings' };

function same(a: Route, b: Route): boolean {
  if (a.name !== b.name) return false;
  if (a.name === 'whole' && b.name === 'whole') return (a.people ?? []).join() === (b.people ?? []).join();
  return a.name === 'person' && b.name === 'person' ? a.id === b.id : true;
}

class NavState {
  route = $state<Route>({ name: 'welcome' });
  /** The person the family view, charts and editors are centred on. */
  focusPerson = $state<Id | null>(null);
  /** Visited routes, oldest first; `index` points at the current one. */
  history = $state<Route[]>([{ name: 'welcome' }]);
  index = $state(0);
  /** The focus person at each history entry, so Back and Forward restore who was in view. */
  foci = $state<(Id | null)[]>([null]);

  canBack = $derived(this.index > 0);
  canForward = $derived(this.index < this.history.length - 1);

  go(r: Route): void {
    if (same(this.route, r)) {
      this.#enter(r);
      return;
    }
    this.history = [...this.history.slice(0, this.index + 1), r];
    this.foci = [...this.foci.slice(0, this.index + 1), this.focusPerson];
    this.index = this.history.length - 1;
    this.#enter(r);
  }

  /** Swaps the current history entry for another route, so Back does not return to a page that no longer works. */
  replace(r: Route): void {
    this.history = this.history.map((h, i) => (i === this.index ? r : h));
    this.#enter(r);
  }

  back(): void {
    if (this.index === 0) return;
    this.index -= 1;
    this.focusPerson = this.foci[this.index] ?? this.focusPerson;
    this.#enter(this.history[this.index]);
  }

  forward(): void {
    if (this.index >= this.history.length - 1) return;
    this.index += 1;
    this.focusPerson = this.foci[this.index] ?? this.focusPerson;
    this.#enter(this.history[this.index]);
  }

  /** Re-centre on a person without changing the route (family view clicks). */
  focus(id: Id | null): void {
    this.focusPerson = id;
    this.foci[this.index] = id;
  }

  /** Re-centre on another person AND add a history entry, so Back returns to the previous person. */
  visit(id: Id): void {
    if (id === this.focusPerson) return;
    this.history = [...this.history.slice(0, this.index + 1), this.route];
    this.foci = [...this.foci.slice(0, this.index + 1), id];
    this.index = this.history.length - 1;
    this.focusPerson = id;
  }

  reset(): void {
    this.route = { name: 'welcome' };
    this.focusPerson = null;
    this.history = [{ name: 'welcome' }];
    this.foci = [null];
    this.index = 0;
  }

  #enter(r: Route): void {
    this.route = r;
    if (r.name === 'person') this.focusPerson = r.id;
    this.foci[this.index] = this.focusPerson;
  }
}

export const nav = new NavState();
