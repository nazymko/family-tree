<script lang="ts">
  import { tick } from 'svelte';
  import type { EditResult, EventDto, Id, ParentRelId, PersonRef, UnionKindId } from '../api/types';
  import { getApi } from '../api/context';
  import Select from '../ui/Select.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Button from '../ui/Button.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import PersonPicker from './PersonPicker.svelte';
  import EventEditor from './EventEditor.svelte';
  import QuickAdd from './QuickAdd.svelte';
  import EditorDialog from './EditorDialog.svelte';
  import SubEditors from './SubEditors.svelte';
  import { errMessage } from './errors';
  import { t } from '../i18n';
  import { eventKindLabel } from '../i18n/backend';

  let {
    family,
    anchor,
    seedPartner,
    onDone,
    onCancel,
    onChanged,
  }: {
    /** Edit this family; omit to create one. */
    family?: Id;
    /** Optional fallback: a person in the family, used only if the Api cannot read a family by id. */
    anchor?: Id;
    /** Create mode: pre-fill the first partner slot. */
    seedPartner?: Id;
    onDone: (r: EditResult) => void;
    onCancel: () => void;
    /** Called after each immediate change (children, relationships, union events) so the caller can refresh. */
    onChanged?: (r: EditResult) => void;
  } = $props();

  interface ChildRow {
    person: PersonRef;
    rel: [ParentRelId, ParentRelId];
    preferred: boolean;
  }

  const api = getApi();
  const uid = $props.id();
  const UNION = $derived<{ value: UnionKindId; label: string }[]>([
    { value: 'married', label: t('famEd.unionMarried') },
    { value: 'civil_union', label: t('famEd.unionCivil') },
    { value: 'unmarried', label: t('famEd.unionUnmarried') },
    { value: 'unknown', label: t('famEd.unionUnknown') },
    { value: 'other', label: t('famEd.unionOther') },
  ]);
  const RELS = $derived<{ value: ParentRelId; label: string }[]>([
    { value: 'birth', label: t('famEd.relBirth') },
    { value: 'adopted', label: t('famEd.relAdopted') },
    { value: 'step', label: t('famEd.relStep') },
    { value: 'foster', label: t('famEd.relFoster') },
    { value: 'guardian', label: t('famEd.relGuardian') },
    { value: 'sealed', label: t('famEd.relSealed') },
    { value: 'unknown', label: t('famEd.relUnknown') },
    { value: 'other', label: t('famEd.relOther') },
  ]);

  // svelte-ignore state_referenced_locally
  const editing = family !== undefined;
  let slot1 = $state<PersonRef | null>(null);
  let slot2 = $state<PersonRef | null>(null);
  let kind = $state<string>('married');
  let events = $state<EventDto[]>([]);
  let children = $state<ChildRow[]>([]);
  let init = $state({ s1: '', s2: '', kind: 'married' });
  let loading = $state(true);
  let saving = $state(false);
  let error = $state('');
  let childPick = $state(0);
  let dragFrom = $state<number | null>(null);
  let sub = $state<null | { kind: 'event'; event?: EventDto } | { kind: 'child' } | { kind: 'delete' }>(null);

  async function readFamily() {
    if (!family) return null;
    try {
      const f = await api.getFamily(family);
      return {
        first: f.partners[0],
        second: f.partners[1],
        kind: f.kind,
        events: f.events,
        rows: f.children.map((c) => ({ person: c.person, rel: c.rel, preferred: c.preferred }) satisfies ChildRow),
      };
    } catch (e) {
      // Fallback for an Api without getFamily: find the family through a person who belongs to it.
      const start = anchor ?? seedPartner;
      if (!start || (e as { code?: string }).code === 'not_found') throw e;
      return readViaAnchor(start);
    }
  }

  async function readViaAnchor(start: Id) {
    const a = await api.getPerson(start);
    let own = a.partnerships.find((p) => p.familyId === family);
    let first: PersonRef | null = null;
    if (own) first = a.ref;
    else {
      const set = a.parentSets.find((s) => s.familyId === family);
      for (const par of set?.parents ?? []) {
        if (!par) continue;
        const d = await api.getPerson(par.id);
        own = d.partnerships.find((p) => p.familyId === family);
        if (own) {
          first = d.ref;
          break;
        }
      }
    }
    if (!own) throw { code: 'not_found', message: t('famEd.gone') };
    const rows = await Promise.all(
      own.children.map(async (c) => {
        const d = await api.getPerson(c.person.id);
        const ps = d.parentSets.find((s) => s.familyId === family);
        return { person: c.person, rel: c.rel, preferred: ps?.preferred ?? false } satisfies ChildRow;
      }),
    );
    return { first, second: own.partner, kind: own.kind, events: own.events, rows };
  }

  async function reload(initial: boolean) {
    try {
      if (!editing) {
        if (initial && seedPartner) slot1 = (await api.getPerson(seedPartner)).ref;
      } else {
        const f = await readFamily();
        if (f) {
          events = f.events;
          children = f.rows;
          if (initial) {
            slot1 = f.first;
            slot2 = f.second;
            kind = f.kind;
          }
        }
      }
      if (initial) init = { s1: slot1?.id ?? '', s2: slot2?.id ?? '', kind };
    } catch (e) {
      error = errMessage(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void reload(true);
  });

  const dirty = $derived((slot1?.id ?? '') !== init.s1 || (slot2?.id ?? '') !== init.s2 || kind !== init.kind);
  const childIds = $derived(children.map((c) => c.person.id));
  const ownParent = $derived(
    [slot1, slot2].find((p) => p && childIds.includes(p.id)) ?? null,
  );
  const sameTwice = $derived(!!slot1 && !!slot2 && slot1.id === slot2.id);
  const canSave = $derived(!loading && !ownParent && !sameTwice);
  const childAnchor = $derived(slot1 ?? slot2);

  async function save() {
    saving = true;
    error = '';
    try {
      onDone(
        await api.apply({
          type: 'saveFamily',
          ...(family ? { id: family } : {}),
          partner1: slot1?.id ?? null,
          partner2: slot2?.id ?? null,
          kind: kind as UnionKindId,
        }),
      );
    } catch (e) {
      error = errMessage(e);
    } finally {
      saving = false;
    }
  }

  async function immediate(cmd: Parameters<typeof api.apply>[0]) {
    error = '';
    try {
      const r = await api.apply(cmd);
      onChanged?.(r);
      await reload(false);
      return r;
    } catch (e) {
      error = errMessage(e);
      await reload(false);
      return null;
    }
  }

  function setRel(row: ChildRow, idx: 0 | 1, v: string) {
    const rel: [ParentRelId, ParentRelId] = [...row.rel];
    rel[idx] = v as ParentRelId;
    row.rel = rel;
    void immediate({ type: 'updateChildLink', family: family!, child: row.person.id, rel, preferred: row.preferred });
  }

  function setPreferred(row: ChildRow, v: boolean) {
    row.preferred = v;
    void immediate({ type: 'updateChildLink', family: family!, child: row.person.id, rel: row.rel, preferred: v });
  }

  const rowId = (id: Id) => `${uid}-child-${id}`;

  let reordering = false;
  async function moveTo(from: number, to: number, keepFocus: Id | null) {
    if (reordering || to < 0 || to >= children.length || from === to) return;
    reordering = true;
    try {
      await doMove(from, to, keepFocus);
    } finally {
      reordering = false;
    }
  }

  async function doMove(from: number, to: number, keepFocus: Id | null) {
    const next = [...children];
    const [m] = next.splice(from, 1);
    next.splice(to, 0, m);
    children = next;
    await immediate({ type: 'reorderChildren', family: family!, order: next.map((c) => c.person.id) });
    if (keepFocus) {
      await tick();
      document.getElementById(rowId(keepFocus))?.focus();
    }
  }

  function onRowKey(e: KeyboardEvent, i: number) {
    if (!e.altKey || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) return;
    if ((e.target as HTMLElement).closest('select, input, textarea')) return; // keep the control's own keys
    e.preventDefault();
    void moveTo(i, i + (e.key === 'ArrowUp' ? -1 : 1), children[i].person.id);
  }

  async function addExisting(p: PersonRef | null) {
    if (!p) return;
    const d = await api.getPerson(p.id).catch(() => null);
    await immediate({
      type: 'linkChild',
      family: family!,
      child: p.id,
      rel: ['birth', 'birth'],
      preferred: !d || d.parentSets.length === 0,
    });
    childPick += 1;
  }

  async function removeFamily() {
    try {
      const r = await api.apply({ type: 'deleteFamily', id: family! });
      sub = null;
      onDone(r);
    } catch (e) {
      error = errMessage(e);
      sub = null;
    }
  }

  const parentName = (p: PersonRef | null, n: 1 | 2) => p?.name ?? t('famEd.parentUnknown', { n });
</script>

<EditorDialog
  title={editing ? t('famEd.editFamily') : t('famEd.addFamily')}
  saveLabel={t('edit.saveFamily')}
  watch={[family, anchor]}
  {dirty}
  {saving}
  {error}
  {canSave}
  onsave={save}
  oncancel={onCancel}
>
  <div class="partners">
    <PersonPicker
      label={t('famEd.partner1')}
      value={slot1}
      exclude={slot2 ? [slot2.id] : []}
      onPick={(p) => (slot1 = p)}
      error={ownParent && ownParent.id === slot1?.id ? t('famEd.ownParent') : undefined}
    />
    <PersonPicker
      label={t('famEd.partner2')}
      value={slot2}
      exclude={slot1 ? [slot1.id] : []}
      onPick={(p) => (slot2 = p)}
      hint={t('famEd.partnerHint')}
      error={ownParent && ownParent.id === slot2?.id ? t('famEd.ownParent') : undefined}
    />
  </div>
  <Select label={t('famEd.typeOfUnion')} options={UNION} bind:value={kind} />

  {#if editing}
    <section aria-labelledby="{uid}-ev">
      <div class="sec-head">
        <h3 id="{uid}-ev">{t('famEd.unionEvents')}</h3>
        <Button onclick={() => (sub = { kind: 'event' })}>{t('editor.addEvent')}</Button>
      </div>
      {#if events.length === 0}
        <p class="empty">{t('famEd.noUnionEvents')}</p>
      {:else}
        <ul class="rows">
          {#each events as e (e.id)}
            <li>
              <span class="kind">{eventKindLabel(e.kind, e.label)}</span>
              <span class="grow">{[e.date?.display, e.place?.full].filter(Boolean).join(', ') || t('person.noDateOrPlace')}</span>
              <Button onclick={() => (sub = { kind: 'event', event: e })} aria-label={t('person.editNamed', { label: eventKindLabel(e.kind, e.label) })}>{t('common.edit')}</Button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section aria-labelledby="{uid}-ch">
      <h3 id="{uid}-ch">{t('family.children')}</h3>
      {#if children.length === 0}
        <p class="empty">{t('famEd.noChildren')}</p>
      {:else}
        <p class="hint">{t('famEd.reorderHint')}</p>
        <ol class="children">
          {#each children as c, i (c.person.id)}
            <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
            <li
              id={rowId(c.person.id)}
              tabindex="0"
              role="group"
              aria-label={t('famEd.childOf', { name: c.person.name, i: i + 1, n: children.length })}
              draggable={dragFrom !== null || undefined}
              onkeydown={(e) => onRowKey(e, i)}
              ondragover={(e) => dragFrom !== null && e.preventDefault()}
              ondrop={(e) => {
                e.preventDefault();
                if (dragFrom !== null) void moveTo(dragFrom, i, null);
                dragFrom = null;
              }}
              ondragend={() => (dragFrom = null)}
            >
              <span
                class="grip"
                aria-hidden="true"
                role="presentation"
                onmousedown={() => (dragFrom = i)}
                onmouseup={() => (dragFrom = null)}>⋮⋮</span
              >
              <div class="who">
                <strong><bdi dir="auto">{c.person.name}</bdi></strong>
                <span class="meta">{c.person.lifespan}</span>
              </div>
              <div class="rels">
                <Select
                  label={t('famEd.relationshipTo', { parent: parentName(slot1, 1) })}
                  options={RELS}
                  value={c.rel[0]}
                  onchange={(v) => setRel(c, 0, v)}
                />
                <Select
                  label={t('famEd.relationshipTo', { parent: parentName(slot2, 2) })}
                  options={RELS}
                  value={c.rel[1]}
                  onchange={(v) => setRel(c, 1, v)}
                />
              </div>
              <div class="foot">
                <Checkbox
                  label={t('famEd.preferredParents')}
                  checked={c.preferred}
                  onchange={(v) => setPreferred(c, v)}
                />
                <Button
                  variant="quiet"
                  aria-label={t('famEd.removeFromFamily', { name: c.person.name })}
                  onclick={() => immediate({ type: 'unlinkChild', family: family!, child: c.person.id })}
                  >{t('common.remove')}</Button
                >
              </div>
            </li>
          {/each}
        </ol>
      {/if}
      {#key childPick}
        <PersonPicker
          label={t('famEd.addExisting')}
          value={null}
          exclude={[...childIds, ...(slot1 ? [slot1.id] : []), ...(slot2 ? [slot2.id] : [])]}
          onPick={addExisting}
        />
      {/key}
      <div>
        <Button disabled={!childAnchor} onclick={() => (sub = { kind: 'child' })}>{t('famEd.addNewChild')}</Button>
      </div>
    </section>
  {/if}

  {#snippet extra(stale: boolean)}
    {#if editing}<Button variant="danger" disabled={stale} onclick={() => (sub = { kind: 'delete' })}>{t('edit.deleteFamily')}</Button>{/if}
  {/snippet}
</EditorDialog>

{#if sub?.kind === 'event' && family}
  <SubEditors>
    <EventEditor
      owner={{ kind: 'family', id: family }}
      event={sub.event}
      onCancel={() => (sub = null)}
      onSaved={(r) => {
        sub = null;
        onChanged?.(r);
        void reload(false);
      }}
    />
  </SubEditors>
{:else if sub?.kind === 'child' && childAnchor && family}
  <SubEditors>
    <QuickAdd
      anchor={childAnchor}
      relation="child"
      {family}
      onCancel={() => (sub = null)}
      onDone={(r) => {
        sub = null;
        onChanged?.(r);
        void reload(false);
      }}
    />
  </SubEditors>
{:else if sub?.kind === 'delete'}
  <Dialog open={true} title={t('famEd.deleteTitle')} onclose={() => (sub = null)}>
    <p>{t('famEd.childrenKept')}</p>
    {#snippet footer()}
      <Button onclick={() => (sub = null)}>{t('famEd.keepFamily')}</Button>
      <Button variant="danger" onclick={removeFamily}>{t('edit.deleteFamily')}</Button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .partners {
    display: grid;
    gap: var(--s-4);
  }
  section {
    display: grid;
    gap: var(--s-4);
    padding-top: var(--s-4);
    border-top: 1px solid var(--rule);
  }
  .sec-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .rows,
  .children {
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
    min-width: 7rem;
    font-weight: 500;
  }
  .empty,
  .hint,
  .meta {
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  .children li {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--s-3) var(--s-4);
    padding: var(--s-4);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    margin-bottom: var(--s-3);
    background: var(--paper-1);
  }
  .children li:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .who {
    display: flex;
    gap: var(--s-4);
    align-items: baseline;
  }
  .grip {
    color: var(--ink-2);
    cursor: grab;
    letter-spacing: -0.2em;
    user-select: none;
  }
  .rels {
    grid-column: 1 / 3;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--s-4);
  }
  .foot {
    grid-column: 1 / 3;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-4);
  }
</style>
