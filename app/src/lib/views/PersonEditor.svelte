<script lang="ts">
  import type { Snippet } from 'svelte';
  import type {
    EditResult,
    EventDto,
    HistoryEntry,
    Id,
    NameDto,
    NoteDto,
    PersonDetail,
    PersonRef,
  } from '../api/types';
  import { getApi } from '../api/context';
  import { nav } from '../state/nav.svelte';
  import { project } from '../state/project.svelte';
  import { settings } from '../state/settings.svelte';
  import { undoState } from '../state/undo.svelte';
  import IndexCard from '../ui/IndexCard.svelte';
  import Ribbon from '../ui/Ribbon.svelte';
  import Button from '../ui/Button.svelte';
  import Select from '../ui/Select.svelte';
  import TextField from '../ui/TextField.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Banner from '../ui/Banner.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import NameEditor from '../editors/NameEditor.svelte';
  import NoteEditor from '../editors/NoteEditor.svelte';
  import QuickAdd from '../editors/QuickAdd.svelte';
  import EventEditor from '../editors/EventEditor.svelte';
  import FamilyEditor from '../editors/FamilyEditor.svelte';
  import { errMessage } from '../editors/errors';

  let {
    id,
    mediaSection,
    sourcesSection,
    onDeleted,
  }: {
    id: Id;
    /** Task 13 injects its media panel here: receives the loaded detail. */
    mediaSection?: Snippet<[PersonDetail]>;
    /** Task 12 injects its sources panel here. */
    sourcesSection?: Snippet<[PersonDetail]>;
    /** Called after the person was deleted; defaults to opening the people list. */
    onDeleted?: () => void;
  } = $props();

  const api = getApi();
  let detail = $state<PersonDetail | null>(null);
  let history = $state<HistoryEntry[]>([]);
  let loadError = $state('');
  let error = $state('');

  let sex = $state('unknown');
  let sexText = $state('');
  let living = $state('');
  let isPrivate = $state(false);

  let dialog = $state<
    | null
    | { kind: 'name'; name?: NameDto }
    | { kind: 'note'; note?: NoteDto }
    | { kind: 'event'; event?: EventDto }
    | { kind: 'family'; family?: Id }
    | { kind: 'quick'; relation: 'father' | 'mother' | 'partner' | 'child' | 'sibling' }
    | { kind: 'delete' }
  >(null);

  let loadSeq = 0;
  async function load() {
    const mine = ++loadSeq;
    try {
      const d = await api.getPerson(id);
      const h = await api.history(id, 20);
      if (mine !== loadSeq) return; // a newer load is in flight
      detail = d;
      sex = d.sex;
      sexText = d.sexText ?? '';
      living = d.livingOverride ?? '';
      isPrivate = d.private;
      history = h;
      loadError = '';
    } catch (e) {
      if (mine === loadSeq) loadError = errMessage(e);
    }
    if (!settings.loaded) void settings.load(api).catch(() => {});
  }

  $effect(() => {
    void id;
    void project.rev; // reload after any change made elsewhere (citations, media, families)
    void load();
  });

  /** The shell's toaster (observeApi) reports the change and offers Undo; this only refreshes the page. */
  async function afterEdit(r: EditResult) {
    undoState.ingest(r);
    await load();
  }

  async function commit(field: string, patch: { sex?: PersonDetail['sex']; sexText?: string; livingOverride?: 'living' | 'deceased' | null; private?: boolean }) {
    error = '';
    try {
      const r = await api.apply({ type: 'updatePerson', id, ...patch });
      await afterEdit(r);
    } catch (e) {
      error = errMessage(e);
      await load();
    }
  }

  async function remove(cmd: Parameters<typeof api.apply>[0]) {
    error = '';
    try {
      await afterEdit(await api.apply(cmd));
    } catch (e) {
      error = errMessage(e);
    }
  }

  async function toggleHome() {
    error = '';
    try {
      await settings.update(api, { homePerson: isHome ? null : id });
    } catch (e) {
      error = errMessage(e);
    }
  }

  const isHome = $derived(settings.value.homePerson === id);
  const familyCount = $derived(detail ? detail.partnerships.length + detail.parentSets.length : 0);
  const soleEvents = $derived(
    detail ? detail.events.filter((e) => e.participants.every((p) => p.person.id === id)).length : 0,
  );

  const SEXES = [
    ['male', 'Male'],
    ['female', 'Female'],
    ['intersex', 'Intersex'],
    ['unknown', 'Unknown'],
    ['other', 'Other'],
  ].map(([value, label]) => ({ value, label }));

  const livingOptions = $derived([
    { value: '', label: `Computed (${detail?.livingComputed ? 'living' : 'deceased'})` },
    { value: 'living', label: 'Treat as living' },
    { value: 'deceased', label: 'Treat as deceased' },
  ]);

  const consequence = $derived(
    `This removes ${detail?.ref.name ?? ''} from ${familyCount} ${familyCount === 1 ? 'family' : 'families'} and deletes ${soleEvents} ${soleEvents === 1 ? 'event' : 'events'} only they took part in.`,
  );

  const cardLines = $derived(
    detail
      ? (['birth', 'death'] as const).flatMap((k) => {
          const e = detail!.events.find((x) => x.kind === k);
          return e ? [{ label: k === 'birth' ? 'Born' : 'Died', text: eventLine(e) || 'No date or place' }] : [];
        })
      : [],
  );

  function eventLine(e: EventDto): string {
    return [e.date?.display, e.place?.full].filter(Boolean).join(', ');
  }
  function open(p: PersonRef) {
    nav.go({ name: 'person', id: p.id });
  }
  function closeDialog() {
    dialog = null;
  }
  /** "Event saved" or "Event deleted", from what the edit actually did. */
  async function saved(r: EditResult) {
    dialog = null;
    await afterEdit(r);
  }
  /** An immediate change inside an open dialog: refresh quietly, the dialog shows the result. */
  async function quiet(r: EditResult) {
    undoState.ingest(r);
    await load();
  }
  async function confirmDelete() {
    try {
      const r = await api.apply({ type: 'deletePerson', id });
      undoState.ingest(r);
      dialog = null;
      if (onDeleted) onDeleted();
      else nav.go({ name: 'people' });
    } catch (e) {
      error = errMessage(e);
      dialog = null;
    }
  }
  function when(ts: number): string {
    return new Date(ts).toLocaleString();
  }
</script>

<div class="page">
  {#if loadError}
    <Banner kind="error">{loadError}</Banner>
  {:else if !detail}
    <p class="loading">Loading person</p>
  {:else}
    <div class="card">
    <IndexCard as="section" ribbon={isHome}>
      <div class="head">
        <h1><bdi dir="auto">{detail.ref.name}</bdi></h1>
        <p class="life">{detail.ref.lifespan || 'No dates recorded'}</p>
        <div class="head-actions">
          <Button onclick={toggleHome} aria-pressed={isHome}>
            <Ribbon title="Home person" size="1rem" />
            {isHome ? 'Home person' : 'Make home person'}
          </Button>
        </div>
      </div>
      {#snippet body()}
        {#each cardLines as l (l.label)}
          <p class="card-line"><span class="card-label">{l.label}</span> {l.text}</p>
        {/each}
      {/snippet}
    </IndexCard>
    </div>

    {#if error}<Banner kind="error">{error}</Banner>{/if}

    <section aria-labelledby="h-names">
      <div class="sec-head">
        <h2 id="h-names">Names</h2>
        <Button onclick={() => (dialog = { kind: 'name' })}>Add name</Button>
      </div>
      <ul class="rows">
        {#each detail.names as n (n.id)}
          <li>
            <span class="grow"><bdi dir="auto">{n.display}</bdi></span>
            <span class="meta">{n.kind}{n.preferred ? ', preferred' : ''}</span>
            <Button onclick={() => (dialog = { kind: 'name', name: n })} aria-label="Edit name {n.display}">Edit</Button>
            <Button
              variant="quiet"
              disabled={detail.names.length < 2}
              aria-label="Delete name {n.display}"
              onclick={() => remove({ type: 'deleteName', person: id, name: n.id })}>Delete</Button
            >
          </li>
        {/each}
      </ul>
    </section>

    <section aria-labelledby="h-basics">
      <h2 id="h-basics">Basics</h2>
      <div class="grid">
        <Select
          label="Sex"
          options={SEXES}
          bind:value={sex}
          onchange={(v) => commit('Sex', { sex: v as PersonDetail['sex'] })}
        />
        {#if sex === 'other'}
          <TextField
            label="Describe sex"
            bind:value={sexText}
            onchange={() => commit('Sex', { sex: 'other', sexText })}
          />
        {/if}
        <Select
          label="Living status"
          options={livingOptions}
          bind:value={living}
          onchange={(v) => commit('Living status', { livingOverride: v === '' ? null : (v as 'living' | 'deceased') })}
        />
        <Checkbox
          label="Private"
          hint="Private people are left out of exports unless you include them"
          bind:checked={isPrivate}
          onchange={(c) => commit('Privacy', { private: c })}
        />
      </div>
    </section>

    <section aria-labelledby="h-events">
      <div class="sec-head">
        <h2 id="h-events">Events and facts</h2>
        <Button onclick={() => (dialog = { kind: 'event' })}>Add event</Button>
      </div>
      {#if detail.events.length === 0}
        <p class="empty">No events or facts recorded yet.</p>
      {:else}
        <ul class="rows">
          {#each detail.events as e (e.id)}
            <li>
              <span class="kind">{e.label}</span>
              <span class="grow">{eventLine(e) || 'No date or place'}</span>
              <Button onclick={() => (dialog = { kind: 'event', event: e })} aria-label="Edit {e.label}">Edit</Button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section aria-labelledby="h-rel">
      <h2 id="h-rel">Relationships</h2>
      <h3>Parents</h3>
      {#each detail.parentSets as s (s.familyId)}
        <p class="line">
          {#each s.parents as p, i (i)}
            {#if p}<Button variant="quiet" onclick={() => open(p)}><bdi dir="auto">{p.name}</bdi></Button>{/if}
          {/each}
        </p>
      {:else}
        <p class="empty">No parents recorded.</p>
      {/each}
      <div class="btns">
        <Button onclick={() => (dialog = { kind: 'quick', relation: 'father' })}>Add father</Button>
        <Button onclick={() => (dialog = { kind: 'quick', relation: 'mother' })}>Add mother</Button>
        <Button onclick={() => (dialog = { kind: 'quick', relation: 'sibling' })}>Add sibling</Button>
      </div>
      <h3>Partners and children</h3>
      {#each detail.partnerships as ps (ps.familyId)}
        <div class="partnership">
          <p class="line">
            {#if ps.partner}
              <Button variant="quiet" onclick={() => open(ps.partner!)}><bdi dir="auto">{ps.partner.name}</bdi></Button>
            {:else}
              <span>Unknown partner</span>
            {/if}
            {#if ps.summary}<span class="meta">{ps.summary}</span>{/if}
            <Button onclick={() => (dialog = { kind: 'family', family: ps.familyId })} aria-label="Edit family with {ps.partner?.name ?? 'unknown partner'}"
              >Edit family</Button
            >
          </p>
          {#each ps.children as c (c.person.id)}
            <p class="line child">
              <Button variant="quiet" onclick={() => open(c.person)}><bdi dir="auto">{c.person.name}</bdi></Button>
              <span class="meta">{c.person.lifespan}</span>
            </p>
          {/each}
        </div>
      {:else}
        <p class="empty">No partners or children recorded.</p>
      {/each}
      <div class="btns">
        <Button onclick={() => (dialog = { kind: 'quick', relation: 'partner' })}>Add partner</Button>
        <Button onclick={() => (dialog = { kind: 'quick', relation: 'child' })}>Add child</Button>
        <Button onclick={() => (dialog = { kind: 'family' })}>Add family</Button>
      </div>
    </section>

    <section aria-labelledby="h-notes">
      <div class="sec-head">
        <h2 id="h-notes">Notes</h2>
        <Button onclick={() => (dialog = { kind: 'note' })}>Add note</Button>
      </div>
      {#each detail.notes as n (n.id)}
        <article class="note">
          <p class="note-kind">{n.kind}</p>
          <p class="note-text">{n.text}</p>
          <div class="btns">
            <Button onclick={() => (dialog = { kind: 'note', note: n })} aria-label="Edit {n.kind} note">Edit</Button>
            <Button
              variant="quiet"
              aria-label="Delete {n.kind} note"
              onclick={() => remove({ type: 'deleteNote', target: { kind: 'person', id }, note: n.id })}
              >Delete</Button
            >
          </div>
        </article>
      {:else}
        <p class="empty">No notes yet.</p>
      {/each}
    </section>

    <section aria-labelledby="h-media">
      <h2 id="h-media">Media</h2>
      {#if mediaSection}{@render mediaSection(detail)}{:else}<p class="empty">{detail.media.length} attached.</p>{/if}
    </section>

    <section aria-labelledby="h-sources">
      <h2 id="h-sources">Sources</h2>
      {#if sourcesSection}{@render sourcesSection(detail)}{:else}<p class="empty">{detail.citations.length} cited.</p>{/if}
    </section>

    <section aria-labelledby="h-history">
      <details>
        <summary><h2 id="h-history">History</h2></summary>
        {#if history.length === 0}
          <p class="empty">No changes recorded yet.</p>
        {:else}
          <ul class="rows">
            {#each history as h, i (i)}
              <li><span class="grow">{h.label}</span><span class="meta">{when(h.ts)}</span></li>
            {/each}
          </ul>
        {/if}
      </details>
    </section>

    <div class="danger-zone">
      <Button variant="danger" onclick={() => (dialog = { kind: 'delete' })}>Delete person</Button>
    </div>
  {/if}
</div>

{#if detail && dialog?.kind === 'name'}
  <NameEditor person={id} name={dialog.name} onCancel={closeDialog} onSaved={(r) => saved(r)} />
{:else if detail && dialog?.kind === 'event'}
  <EventEditor
    owner={{ kind: 'person', id }}
    event={dialog.event}
    onCancel={closeDialog}
    onSaved={(r) => saved(r)}
  />
{:else if detail && dialog?.kind === 'family'}
  <FamilyEditor
    family={dialog.family}
    anchor={id}
    seedPartner={id}
    onCancel={closeDialog}
    onChanged={quiet}
    onDone={(r) => saved(r)}
  />
{:else if detail && dialog?.kind === 'note'}
  <NoteEditor
    target={{ kind: 'person', id }}
    note={dialog.note}
    onCancel={closeDialog}
    onSaved={(r) => saved(r)}
  />
{:else if detail && dialog?.kind === 'quick'}
  <QuickAdd
    anchor={detail.ref}
    relation={dialog.relation}
    onCancel={closeDialog}
    onDone={(r) => saved(r)}
  />
{:else if detail && dialog?.kind === 'delete'}
  <Dialog open={true} title="Delete {detail.ref.name}?" onclose={closeDialog}>
    <p>{consequence}</p>
    <p>You can undo this straight afterwards.</p>
    {#snippet footer()}
      <Button onclick={closeDialog}>Keep person</Button>
      <Button variant="danger" onclick={confirmDelete}>Delete person</Button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .page {
    display: grid;
    gap: var(--s-6);
    max-width: 70rem;
    padding: var(--s-6);
  }
  .card {
    max-width: var(--measure);
  }
  .head {
    display: grid;
    gap: 0;
  }
  .head h1 {
    font-size: var(--t-2xl);
    line-height: var(--t-2xl-lh);
  }
  .life {
    color: var(--ink-1);
    font-variant-numeric: tabular-nums;
  }
  .head-actions {
    padding-top: var(--s-3);
  }
  .head-actions :global(.btn) {
    margin-inline-start: calc(-1 * var(--s-5)); /* the ribbon glyph aligns with the name */
  }
  .card-line {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .card-label {
    display: inline-block;
    min-width: 4rem;
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  section {
    display: grid;
    gap: var(--s-4);
    padding-bottom: var(--s-5);
    border-bottom: 1px solid var(--rule);
    max-width: var(--measure);
  }
  .sec-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .rows li {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    min-height: 2.5rem;
    border-top: 1px solid var(--rule);
  }
  .grow {
    flex: 1;
  }
  .kind {
    min-width: 8rem;
    font-weight: 500;
  }
  .meta {
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  .empty,
  .loading {
    color: var(--ink-1);
  }
  .grid {
    display: grid;
    gap: var(--s-4);
  }
  .btns {
    display: flex;
    gap: var(--s-3);
    flex-wrap: wrap;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--s-3);
  }
  .line.child {
    padding-inline-start: var(--s-6);
  }
  .note {
    display: grid;
    gap: var(--s-2);
  }
  .note-kind {
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  .note-text {
    white-space: pre-wrap;
    font-family: var(--font-serif);
    font-size: var(--t-md);
    line-height: var(--rule-step);
    background-image: repeating-linear-gradient(to bottom, transparent 0 27px, var(--rule) 27px 28px);
  }
  summary {
    cursor: pointer;
  }
  summary h2 {
    display: inline;
  }
  .danger-zone {
    padding-bottom: var(--s-8);
  }
</style>
