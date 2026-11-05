import type { Api, AgentChangeEvent, Platform } from '../api/api';
import { project } from './project.svelte';
import { undoState } from './undo.svelte';

/** Context key: lets an editor dialog ask the page that opened it to reopen it on fresh data. Otherwise Reload closes it. */
export const EDITOR_RELOAD = Symbol('gene-tree.editor-reload');

/** Who carries the "changed by an agent" badge, and what the agents have touched while the window has been open. */
class AgentChanges {
  /** People an agent changed that nobody has reviewed yet. Replaced, never mutated, so readers re-run. */
  unreviewed = $state<ReadonlySet<string>>(new Set());
  lastTouched = $state<string[]>([]);
  lastAgent = $state('');
  /** Counts agent events; an editor remembers the value it opened at. */
  seq = $state(0);

  #touchedAt = new Map<string, number>();
  #api: Api | null = null;
  #load = 0;

  /** Loads the badges and follows the agents' writes until the returned function is called. */
  start(api: Api, platform: Platform): () => void {
    this.#api = api;
    void this.reload();
    const stop = platform.onTreeChangedByAgent((e) => this.#heard(e));
    return () => {
      stop();
      if (this.#api === api) this.#api = null;
    };
  }

  #heard(e: AgentChangeEvent): void {
    this.seq++;
    for (const id of e.touched) this.#touchedAt.set(id, this.seq);
    this.lastTouched = e.touched;
    this.lastAgent = e.agent;
    project.rev++;
    void this.reload();
    // An agent's write can make the next Ctrl+Z step unavailable.
    if (this.#api) void undoState.refresh(this.#api).catch(() => {});
  }

  /** Did an agent touch any of `ids` after event number `since`? */
  touchedSince(ids: readonly (string | null | undefined)[], since: number): boolean {
    return ids.some((id) => id != null && (this.#touchedAt.get(id) ?? 0) > since);
  }

  async reload(): Promise<void> {
    const api = this.#api;
    if (!api) return;
    const mine = ++this.#load;
    try {
      const ids = await api.unreviewedPeople();
      if (mine === this.#load) this.unreviewed = new Set(ids);
    } catch {
      // The badges are a convenience: the last known set stays until a reload works.
    }
  }

  reset(): void {
    this.#load++;
    this.#touchedAt.clear();
    this.unreviewed = new Set();
    this.lastTouched = [];
    this.lastAgent = '';
    this.seq = 0;
  }
}

export const agentChanges = new AgentChanges();
