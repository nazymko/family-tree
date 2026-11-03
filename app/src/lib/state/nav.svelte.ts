import type { Id } from '../api/types';

export type Route =
  | { name: 'welcome' }
  | { name: 'importReport' }
  | { name: 'family' }
  | { name: 'people' }
  | { name: 'pedigree' }
  | { name: 'descendants' }
  | { name: 'person'; id: Id }
  | { name: 'sources' }
  | { name: 'media' }
  | { name: 'settings' };

function same(a: Route, b: Route): boolean {
  if (a.name !== b.name) return false;
  return a.name === 'person' && b.name === 'person' ? a.id === b.id : true;
}

class NavState {
  route = $state<Route>({ name: 'welcome' });
  /** The person the family view, charts and editors are centred on. */
  focusPerson = $state<Id | null>(null);
  /** Visited routes, oldest first; `index` points at the current one. */
  history = $state<Route[]>([{ name: 'welcome' }]);
  index = $state(0);

  canBack = $derived(this.index > 0);
  canForward = $derived(this.index < this.history.length - 1);

  go(r: Route): void {
    if (same(this.route, r)) {
      this.#enter(r);
      return;
    }
    this.history = [...this.history.slice(0, this.index + 1), r];
    this.index = this.history.length - 1;
    this.#enter(r);
  }

  back(): void {
    if (this.index === 0) return;
    this.index -= 1;
    this.#enter(this.history[this.index]);
  }

  forward(): void {
    if (this.index >= this.history.length - 1) return;
    this.index += 1;
    this.#enter(this.history[this.index]);
  }

  /** Re-centre on a person without changing the route (family view clicks). */
  focus(id: Id | null): void {
    this.focusPerson = id;
  }

  reset(): void {
    this.route = { name: 'welcome' };
    this.focusPerson = null;
    this.history = [{ name: 'welcome' }];
    this.index = 0;
  }

  #enter(r: Route): void {
    this.route = r;
    if (r.name === 'person') this.focusPerson = r.id;
  }
}

export const nav = new NavState();
