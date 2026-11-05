<script module lang="ts">
  import type { CitationDto, EventDto, Id, NameDto, NoteDto, OwnerRef } from '../api/types';

  /** The editor a change's record opens on this person's page; `owner` is the record holding it when that is not the person. */
  export type EditTarget =
    | { kind: 'note'; note: NoteDto; owner?: OwnerRef }
    | { kind: 'event'; event: EventDto; owner?: { kind: 'person' | 'family'; id: Id } }
    | { kind: 'name'; name: NameDto }
    | { kind: 'family'; family: Id }
    | { kind: 'citation'; citation: CitationDto; owner?: OwnerRef };
</script>

<script lang="ts">
  import { tick, untrack } from 'svelte';
  import type { FamilyDto, LogChangeDto, LogRowDto, PersonDetail } from '../api/types';
  import { getApi } from '../api/context';
  import { errMessage } from '../editors/errors';
  import { agentChanges } from '../state/agentChanges.svelte';
  import { project } from '../state/project.svelte';
  import { toasts } from '../state/toast.svelte';
  import Button from '../ui/Button.svelte';
  import { currentLocale, t } from '../i18n';
  import { agentLabel, editLabel, factText, recordKindLabel } from '../i18n/backend';
  import LogChanges, { type ChangeAction } from './LogChanges.svelte';
  import UndoDialog, { type Pending } from './UndoDialog.svelte';
  import { loadRows, ROW_PAGE } from './pagedRows';

  let {
    id,
    detail = null,
    onedit,
    onvacated,
    onundo,
  }: {
    id: Id;
    /** What the page shows now; a record that is not in it has no editor to open. */
    detail?: PersonDetail | null;
    onedit?: (target: EditTarget) => void;
    /** The last row has gone, so the focus it held has nowhere to be: the page decides. */
    onvacated?: () => void;
    /** An undo question is on screen (`open`) or has been answered and closed (`closed`), so the page can wait with what the undo changed. */
    onundo?: (state: 'open' | 'closed') => void;
  } = $props();

  const api = getApi();
  const uid = $props.id();
  let rows = $state<LogRowDto[]>([]);
  let total = $state(0);
  /** The edit buttons each change gets, found by `resolve`; a record with no entry has no editor to open. */
  let targets = $state<Record<string, ActionTarget[]>>({});
  let heading = $state<HTMLElement>();
  let section = $state<HTMLElement>();
  let pending = $state<Pending | null>(null);
  let seq = 0;

  /** Reloads the rows the owner has opened so far, as many pages as it takes. */
  async function load(person: Id) {
    const mine = ++seq;
    try {
      const r = await loadRows((o, l) => api.personAgentChanges(person, o, l), untrack(() => rows.length));
      if (mine === seq) {
        rows = r.items;
        total = r.total;
      }
    } catch {
      // The person page works without this list; the change log still has every entry.
      if (mine === seq) {
        rows = [];
        total = 0;
      }
    }
  }

  async function more() {
    // A reload begun before this lands would drop the rows appended here: this call supersedes it.
    seq++;
    try {
      const page = await api.personAgentChanges(id, rows.length, ROW_PAGE);
      rows = [...rows, ...page.items.filter((r) => !rows.some((x) => x.group === r.group))];
      total = page.total;
    } catch (e) {
      toasts.push({ text: errMessage(e), kind: 'error' });
    }
  }

  $effect(() => {
    const person = id;
    void project.rev;
    void agentChanges.unreviewed;
    untrack(() => void load(person));
  });

  async function review() {
    try {
      await api.markReviewed({ kind: 'person', id });
      await agentChanges.reload();
      // The button that had focus is gone now.
      await tick();
      heading?.focus();
    } catch (e) {
      toasts.push({ text: errMessage(e), kind: 'error' });
    }
  }

  /** A row that went (or a dialog that closed) must not leave the keyboard user on nothing. */
  async function settle() {
    await load(id);
    await tick();
    const a = document.activeElement;
    if (a && a !== document.body && a.isConnected) return;
    if (rows.length > 0) heading?.focus();
    else onvacated?.();
  }

  function ask() {
    onundo?.('open');
  }

  async function closeUndo() {
    pending = null;
    onundo?.('closed');
    await settle();
  }

  const who = (r: LogRowDto) => (r.actor.kind === 'agent' ? agentLabel(r.actor.id, r.actor.name) : '');
  const when = (r: LogRowDto) => new Date(r.ts).toLocaleString(currentLocale());
  const rowName = (r: LogRowDto) => [when(r), who(r), editLabel(r.label)].filter(Boolean).join(', ');

  interface ActionTarget {
    key: string;
    what: string;
    target: EditTarget;
  }

  let rseq = 0;
  /**
   * Finds the editor for each record the shown changes are about: from what the page holds, or by id when the
   * record lives elsewhere (a note on a family, an event of a family this person is a partner in). A record that
   * is gone, or of a kind with no editor, gets no entry.
   */
  async function resolve(list: LogRowDto[], d: PersonDetail | null) {
    const mine = ++rseq;
    const out: Record<string, ActionTarget[]> = {};
    if (d) {
      const families: Record<Id, Promise<FamilyDto | null>> = {};
      const family = (fid: Id) => {
        families[fid] ??= api.getFamily(fid).catch(() => null);
        return families[fid];
      };
      const home = (kind: 'note' | 'citation', cid: Id) => api.recordHome(kind, cid).catch(() => null);
      for (const c of list.flatMap((r) => r.changes)) {
        const key = `${c.kind}:${c.id}`;
        if (key in out) continue;
        const record = `${recordKindLabel(c.kind)}: ${factText(c.fact)}`.trim();
        const one = (target: EditTarget): ActionTarget[] => [{ key, what: record, target }];
        let found: ActionTarget[] = [];
        switch (c.kind) {
          case 'note': {
            const note = d.notes.find((n) => n.id === c.id);
            if (note) found = one({ kind: 'note', note });
            else {
              const h = await home('note', c.id);
              if (h?.note) found = one({ kind: 'note', note: h.note, owner: h.owner });
            }
            break;
          }
          case 'citation': {
            const citation = d.citations.find((x) => x.id === c.id);
            if (citation) found = one({ kind: 'citation', citation });
            else {
              const h = await home('citation', c.id);
              if (h?.citation) found = one({ kind: 'citation', citation: h.citation, owner: h.owner });
            }
            break;
          }
          case 'event': {
            const event = d.events.find((e) => e.id === c.id);
            if (event) found = one({ kind: 'event', event });
            else {
              for (const p of d.partnerships) {
                const ev = (await family(p.familyId))?.events.find((e) => e.id === c.id);
                if (ev) {
                  found = one({ kind: 'event', event: ev, owner: { kind: 'family', id: p.familyId } });
                  break;
                }
              }
            }
            break;
          }
          case 'family':
            if (d.partnerships.some((p) => p.familyId === c.id) || d.parentSets.some((p) => p.familyId === c.id)) found = one({ kind: 'family', family: c.id });
            break;
          case 'person':
            if (c.id === id) {
              for (const nameId of new Set(c.fields.flatMap((f) => (f.nameId ? [f.nameId] : [])))) {
                const name = d.names.find((n) => n.id === nameId);
                if (!name) continue;
                const text = [name.given, ...name.surnames].filter(Boolean).join(' ');
                found.push({ key: `name:${nameId}`, what: `${t('changeLog.editName')}: ${text}`.trim(), target: { kind: 'name', name } });
              }
            }
            break;
        }
        out[key] = found;
        if (mine !== rseq) return;
      }
    }
    if (mine === rseq) targets = out;
  }

  $effect(() => {
    const list = rows;
    const d = detail;
    void currentLocale();
    untrack(() => void resolve(list, d));
  });

  /** One edit action per record of the change that has an editor and is still there. */
  function actions(c: LogChangeDto): ChangeAction[] {
    if (!onedit) return [];
    return (targets[`${c.kind}:${c.id}`] ?? []).map((a) => ({ key: a.key, what: a.what, run: () => onedit(a.target) }));
  }
</script>

{#if rows.length > 0}
  <section class="panel" aria-labelledby="{uid}-h" bind:this={section}>
    <div class="head">
      <h2 id="{uid}-h" tabindex="-1" bind:this={heading}>{t('changeLog.panelTitle')}</h2>
      {#if agentChanges.unreviewed.has(id)}
        <Button onclick={review}>{t('changeLog.reviewed')}</Button>
      {/if}
    </div>
    <ul>
      {#each rows as r (r.group)}
        <li>
          <p class="meta">
            <span>{when(r)}</span>
            <span>{who(r)}</span>
            <span>{editLabel(r.label)}</span>
          </p>
          <LogChanges changes={r.changes} {actions} />
          <div class="row-actions">
            <Button disabled={!r.canUndo} aria-label={t('changeLog.undoWhat', { what: rowName(r) })} onclick={() => { pending = { kind: 'group', id: r.group }; ask(); }}>
              {t('changeLog.undo')}
            </Button>
          </div>
        </li>
      {/each}
    </ul>
    {#if rows.length < total}
      <div class="more"><Button onclick={more}>{t('changeLog.showMore')}</Button></div>
    {/if}
  </section>
{/if}

<UndoDialog {pending} onclose={closeUndo} />

<style>
  .panel {
    display: grid;
    gap: var(--s-3);
    padding: var(--s-4);
    border: 1px solid var(--rule);
    border-radius: 4px;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-4);
  }
  h2 {
    margin: 0;
    font-size: var(--t-md, 1rem);
    outline: none;
  }
  ul {
    display: grid;
    gap: var(--s-4);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: grid;
    gap: var(--s-2);
    padding-block-end: var(--s-3);
    border-block-end: 1px solid var(--rule);
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-1) var(--s-3);
    margin: 0;
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  .more {
    display: flex;
    justify-content: center;
  }
  .row-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
</style>
