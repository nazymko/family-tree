import type { Api, AgentChangeEvent, Platform } from '../api/api';
import { project } from './project.svelte';
import { undoState } from './undo.svelte';
import { t } from '../i18n';

/** Context key for the page that opened an editor from data it read itself. */
export const EDITOR_HOST = Symbol('gene-tree.editor-host');

export interface EditorHost {
  /** The agent event count when the data the page holds was read; an editor built from that data counts from here. */
  loadedAt(): number;
  /**
   * Reopens the page's open editor on what the tree says now, if the read works and nothing overtakes it; if not, the
   * editor stays as it is. False when the open editor is not one of this page's own: that one closes itself instead.
   */
  reopen(): Promise<boolean>;
}

/** A burst of agent writes refreshes the open views at most once per this many milliseconds, with one last time at its end. */
export const REFRESH_MS = 250;

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
  #timer: ReturnType<typeof setTimeout> | null = null;
  #waiting = false;

  /** Loads the badges and follows the agents' writes until the returned function is called. */
  start(api: Api, platform: Platform): () => void {
    this.#api = api;
    void this.reload();
    const stop = platform.onTreeChangedByAgent((e) => this.#heard(e));
    return () => {
      stop();
      this.#cancelRefresh();
      if (this.#api === api) this.#api = null;
    };
  }

  #heard(e: AgentChangeEvent): void {
    this.seq++;
    for (const id of e.touched) this.#touchedAt.set(id, this.seq);
    this.lastTouched = e.touched;
    this.lastAgent = e.agent;
    this.#refreshSoon();
  }

  #refreshSoon(): void {
    if (this.#timer !== null) {
      this.#waiting = true;
      return;
    }
    project.rev++;
    void this.reload();
    // An agent's write can make the next Ctrl+Z step unavailable.
    if (this.#api) void undoState.refresh(this.#api).catch(() => {});
    this.#timer = setTimeout(() => {
      this.#timer = null;
      if (!this.#waiting) return;
      this.#waiting = false;
      this.#refreshSoon();
    }, REFRESH_MS);
  }

  #cancelRefresh(): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
    this.#waiting = false;
  }

  /** `label` for a screen reader, which cannot see the dot on a chart: says when an agent's change is not reviewed yet. */
  spoken(label: string, person: string | null | undefined): string {
    return person != null && this.unreviewed.has(person) ? `${label}, ${t('changeLog.badgeLabel')}` : label;
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
    this.#cancelRefresh();
    this.#load++;
    this.#touchedAt.clear();
    this.unreviewed = new Set();
    this.lastTouched = [];
    this.lastAgent = '';
    this.seq = 0;
  }
}

export const agentChanges = new AgentChanges();
