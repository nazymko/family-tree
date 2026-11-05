<script module lang="ts">
  import type { CitationDto, EventDto, Id, NameDto, NoteDto } from '../api/types';

  /** The editor a change's record opens on this person's page. */
  export type EditTarget =
    | { kind: 'note'; note: NoteDto }
    | { kind: 'event'; event: EventDto }
    | { kind: 'name'; name: NameDto }
    | { kind: 'family'; family: Id }
    | { kind: 'citation'; citation: CitationDto };
</script>

<script lang="ts">
  import { tick, untrack } from 'svelte';
  import type { LogChangeDto, LogRowDto, PersonDetail } from '../api/types';
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
  let heading = $state<HTMLElement>();
  let section = $state<HTMLElement>();
  let pending = $state<Pending | null>(null);
  let seq = 0;

  async function load(person: Id) {
    const mine = ++seq;
    try {
      const r = await api.personAgentChanges(person);
      if (mine === seq) rows = r;
    } catch {
      // The person page works without this list; the change log still has every entry.
      if (mine === seq) rows = [];
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

  /** One edit action per record of the change that has an editor and is still on this page. */
  function actions(c: LogChangeDto): ChangeAction[] {
    const d = detail;
    if (!d || !onedit) return [];
    const go = (what: string, target: EditTarget): ChangeAction[] => [{ key: `${c.kind}:${c.id}`, what, run: () => onedit(target) }];
    const record = `${recordKindLabel(c.kind)}: ${factText(c.fact)}`.trim();
    switch (c.kind) {
      case 'note': {
        const note = d.notes.find((n) => n.id === c.id);
        return note ? go(record, { kind: 'note', note }) : [];
      }
      case 'event': {
        const event = d.events.find((e) => e.id === c.id);
        return event ? go(record, { kind: 'event', event }) : [];
      }
      case 'citation': {
        const citation = d.citations.find((x) => x.id === c.id);
        return citation ? go(record, { kind: 'citation', citation }) : [];
      }
      case 'family':
        return d.partnerships.some((p) => p.familyId === c.id) || d.parentSets.some((p) => p.familyId === c.id)
          ? go(record, { kind: 'family', family: c.id })
          : [];
      case 'person': {
        if (c.id !== id) return [];
        const out: ChangeAction[] = [];
        for (const nameId of new Set(c.fields.flatMap((f) => (f.nameId ? [f.nameId] : [])))) {
          const name = d.names.find((n) => n.id === nameId);
          if (!name) continue;
          const text = [name.given, ...name.surnames].filter(Boolean).join(' ');
          out.push({ key: `name:${nameId}`, what: `${t('changeLog.editName')}: ${text}`.trim(), run: () => onedit({ kind: 'name', name }) });
        }
        return out;
      }
      default:
        return [];
    }
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
  .row-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
</style>
