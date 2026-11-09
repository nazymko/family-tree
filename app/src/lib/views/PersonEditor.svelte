<script lang="ts">
  import { setContext, tick, untrack, type Snippet } from 'svelte';
  import type {
    EditResult,
    EventDto,
    FamilyViewDto,
    Id,
    LogRowDto,
    CitationDto,
    NameDto,
    NoteDto,
    OwnerRef,
    PersonDetail,
    PersonRef,
  } from '../api/types';
  import { getApi } from '../api/context';
  import { nav } from '../state/nav.svelte';
  import { project } from '../state/project.svelte';
  import { settings } from '../state/settings.svelte';
  import { undoState } from '../state/undo.svelte';
  import { ui } from '../state/ui.svelte';
  import { agentChanges, EDITOR_HOST, type EditorHost } from '../state/agentChanges.svelte';
  import FamilyTree from './FamilyTree.svelte';
  import AgentChangesPanel, { type EditTarget } from './AgentChangesPanel.svelte';
  import { loadRows, ROW_PAGE } from './pagedRows';
  import LogChanges from './LogChanges.svelte';
  import Badge from '../ui/Badge.svelte';
  import AgentBadge from '../ui/AgentBadge.svelte';
  import IndexCard from '../ui/IndexCard.svelte';
  import Button from '../ui/Button.svelte';
  import Menu, { type MenuItem } from '../ui/Menu.svelte';
  import EventSummary from '../editors/EventSummary.svelte';
  import Select from '../ui/Select.svelte';
  import TextField from '../ui/TextField.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Banner from '../ui/Banner.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import NameEditor from '../editors/NameEditor.svelte';
  import NoteEditor from '../editors/NoteEditor.svelte';
  import VoiceSamples from '../media/VoiceSamples.svelte';
  import QuickAdd from '../editors/QuickAdd.svelte';
  import EventEditor from '../editors/EventEditor.svelte';
  import SurnamesEditor from '../editors/SurnamesEditor.svelte';
  import FamilyEditor from '../editors/FamilyEditor.svelte';
  import CitationEditor from '../editors/CitationEditor.svelte';
  import { errMessage } from '../editors/errors';
  import { t, plural, currentLocale, type MessageKey } from '../i18n';
  import { agentLabel, editLabel, eventKindLabel } from '../i18n/backend';
  import { withSexIcon } from '../ui/sex';
  import { birthSurnameText } from '../ui/birthSurname';

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
  /** Every change group that reached this person, newest first, as far as the owner has paged. */
  let history = $state<LogRowDto[]>([]);
  let historyTotal = $state(0);
  /** The tree holds changes from before authors were tracked: some notes and events may not show in the history. */
  let olderChanges = $state(false);
  let title = $state<HTMLElement>();
  /** An undo is being asked or answered; an undo that removes this person waits for the answer to be read. */
  let undoing = false;
  let gone = false;
  let loadError = $state('');
  let error = $state('');

  let sex = $state('unknown');
  let sexText = $state('');
  let living = $state('');
  let isPrivate = $state(false);

  let dialog = $state<
    | null
    | { kind: 'name'; name?: NameDto }
    | { kind: 'note'; note?: NoteDto; owner?: OwnerRef }
    | { kind: 'event'; event?: EventDto; newKind?: string; owner?: { kind: 'person' | 'family'; id: Id } }
    | { kind: 'surnames' }
    | { kind: 'family'; family?: Id }
    | { kind: 'citation'; citation: CitationDto; owner?: OwnerRef }
    | { kind: 'quick'; relation: 'father' | 'mother' | 'partner' | 'child' | 'sibling' }
    | { kind: 'delete' }
  >(null);

  /** The closest relatives as a small tree; the page still works when this fails to load. */
  let familyView = $state<FamilyViewDto | null>(null);

  let loadSeq = 0;
  /** The agent event count when `detail` was read; an editor built from `detail` counts from here. */
  let detailAt = 0;
  /** Notes on this person's own families: they belong to the family, and are read here too. */
  /** Notes on the person's own families, then on the families of their parents (`parents` names them). */
  let familyNotes = $state<{ familyId: string; partner: string | null; role?: 'husband' | 'wife'; parents?: string; notes: NoteDto[] }[]>([]);
  let basics = $state<HTMLElement>();
  /** What was last written into the "describe" field, so anything else there is text not yet committed. */
  let sexTextShown = '';
  let committing = 0;

  const holdsFocus = (field: string) => !!basics?.querySelector(`[data-field="${field}"]`)?.contains(document.activeElement);

  /** Brings the basics in line with the tree, except where the owner is working. `force` is for a commit that failed. */
  function syncBasics(d: PersonDetail, force = false) {
    const free = (field: string) => force || !holdsFocus(field);
    // The sex decides whether the "describe" field exists at all, so typing there holds it too.
    const typing = sexText !== sexTextShown || !free('sexText');
    if (!typing) {
      sexText = d.sexText ?? '';
      sexTextShown = sexText;
      if (free('sex')) sex = d.sex;
    }
    if (free('living')) living = d.livingOverride ?? '';
    if (free('private')) isPrivate = d.private;
  }

  /** Focus moved on: a field held back while it had focus catches up. A commit under way reloads by itself. */
  async function basicsBlurred() {
    await tick();
    if (detail && committing === 0) syncBasics(detail);
  }

  /** True when what it read is what the page now holds: not overtaken by a newer load, and not failed. */
  async function load(): Promise<boolean> {
    const mine = ++loadSeq;
    const at = untrack(() => agentChanges.seq);
    let landed = false;
    try {
      const d = await api.getPerson(id);
      const h = await loadRows((o, l) => api.personHistory(id, o, l), untrack(() => history.length));
      const fv = await api.getFamilyView(id).catch(() => null);
      const fn: typeof familyNotes = await Promise.all(
        d.partnerships.map(async (p) => ({
          familyId: p.familyId,
          partner: p.partner?.name ?? null,
          // In a marriage the partner is the husband or the wife, not just "partner".
          role:
            p.kind === 'married' && p.partner?.sex === 'male'
              ? ('husband' as const)
              : p.kind === 'married' && p.partner?.sex === 'female'
                ? ('wife' as const)
                : undefined,
          notes: (await api.getFamily(p.familyId).catch(() => null))?.notes ?? [],
        })),
      );
      // The parents' family notes too: the history of this page lists changes to them.
      const fp = await Promise.all(
        d.parentSets.map(async (ps) => ({
          familyId: ps.familyId,
          partner: null,
          parents: ps.parents.flatMap((x) => (x ? [x.name] : [])).join(', '),
          notes: (await api.getFamily(ps.familyId).catch(() => null))?.notes ?? [],
        })),
      );
      fn.push(...fp.filter((f) => !fn.some((o) => o.familyId === f.familyId)));
      if (mine !== loadSeq) return false; // a newer load is in flight
      detail = d;
      familyNotes = fn.filter((f) => f.notes.length > 0);
      detailAt = at;
      familyView = fv;
      syncBasics(d);
      history = h.items;
      historyTotal = h.total;
      loadError = '';
      landed = true;
    } catch (e) {
      if (mine === loadSeq) {
        // The person this page is about is gone (an undo took it back, or it was deleted): leave it, no error screen.
        // While an undo question is open the answer is read first, and the page leaves when it is closed.
        if (detail && (e as { code?: string } | null)?.code === 'not_found') {
          if (undoing) gone = true;
          else leave();
        } else loadError = errMessage(e);
      }
    }
    if (!settings.loaded) void settings.load(api).catch(() => {});
    return landed;
  }

  function leave() {
    detail = null;
    if (onDeleted) onDeleted();
    else nav.go({ name: 'people' });
  }

  $effect(() => {
    void id;
    void project.rev;
    void api
      .hasUntrackedChanges()
      .then((v) => (olderChanges = v))
      .catch(() => (olderChanges = false)); // reload after any change made elsewhere (citations, media, families)
    void load();
  });

  /** The shell's toaster (observeApi) reports the change and offers Undo; this only refreshes the page. */
  async function afterEdit(r: EditResult) {
    undoState.ingest(r);
    await load();
  }

  async function commit(field: string, patch: { sex?: PersonDetail['sex']; sexText?: string; livingOverride?: 'living' | 'deceased' | null; private?: boolean }) {
    error = '';
    if (patch.sexText !== undefined) sexTextShown = patch.sexText;
    committing++;
    try {
      const r = await api.apply({ type: 'updatePerson', id, ...patch });
      await afterEdit(r);
    } catch (e) {
      error = errMessage(e);
      await load();
      if (detail) syncBasics(detail, true);
    } finally {
      committing--;
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
  let historyBox = $state<HTMLDetailsElement>();
  const parentNames = (s: { parents: ({ name: string } | null)[] }) => s.parents.flatMap((x) => (x ? [x.name] : [])).join(', ');

  /** Charts read their root from the focus person; the "around" screen carries it in the route. */
  function openChart(kind: 'around' | 'pedigree' | 'descendants' | 'relatives') {
    nav.focus(id);
    nav.go(kind === 'around' ? { name: 'around', ids: [id] } : { name: kind });
  }

  /** Scrolls to a section and moves focus to its heading (the history folds open first). */
  async function jump(headingId: string) {
    if (headingId === 'h-history' && historyBox) historyBox.open = true;
    await tick();
    const h = document.getElementById(headingId);
    h?.scrollIntoView?.({ block: 'start' });
    h?.focus({ preventScroll: true });
  }

  const moreItems = $derived<MenuItem[]>([
    { label: t('surnames.button'), onselect: () => (dialog = { kind: 'surnames' }) },
    { label: isHome ? t('common.homePerson') : t('person.makeHome'), onselect: () => void toggleHome() },
  ]);

  const addItems = $derived<MenuItem[]>([
    { label: t('person.menu_father'), onselect: () => (dialog = { kind: 'quick', relation: 'father' }) },
    { label: t('person.menu_mother'), onselect: () => (dialog = { kind: 'quick', relation: 'mother' }) },
    { label: t('person.menu_sibling'), onselect: () => (dialog = { kind: 'quick', relation: 'sibling' }) },
    { label: t('person.menu_partner'), onselect: () => (dialog = { kind: 'quick', relation: 'partner' }) },
    { label: t('person.menu_child'), onselect: () => (dialog = { kind: 'quick', relation: 'child' }) },
    { label: t('person.menu_note'), onselect: () => (dialog = { kind: 'note' }) },
    { label: t('person.menu_event'), onselect: () => (dialog = { kind: 'event' }) },
    {
      label: t('person.menu_media'),
      onselect: () => {
        nav.focus(id);
        nav.go({ name: 'media' });
      },
    },
    {
      label: t('person.menu_source'),
      onselect: async () => {
        await jump('h-sources');
        document.querySelector<HTMLElement>('[aria-labelledby="h-sources"] [data-add-citation]')?.click();
      },
    },
  ]);

  const familyCount = $derived(detail ? detail.partnerships.length + detail.parentSets.length : 0);
  const soleEvents = $derived(
    detail ? detail.events.filter((e) => e.participants.every((p) => p.person.id === id)).length : 0,
  );

  const SEXES = $derived(
    (['male', 'female', 'intersex', 'unknown', 'other'] as const).map((value) => ({ value, label: withSexIcon(value, t(`sexOption.${value}`)) })),
  );

  const livingOptions = $derived([
    { value: '', label: detail?.livingComputed ? t('person.computedLiving') : t('person.computedDeceased') },
    { value: 'living', label: t('person.treatLiving') },
    { value: 'deceased', label: t('person.treatDeceased') },
  ]);

  const consequence = $derived(
    t('person.consequence', {
      name: detail?.ref.name ?? '',
      families: plural('person.familiesFrom', familyCount),
      events: plural('person.eventsOnly', soleEvents),
    }),
  );

  const birthEvent = $derived(detail?.events.find((x) => x.kind === 'birth'));
  const deathEvent = $derived(detail?.events.find((x) => x.kind === 'death'));
  const maiden = $derived(birthSurnameText(detail?.ref));
  const preferredName = $derived(detail ? (detail.names.find((n) => n.preferred) ?? detail.names[0]) : undefined);
  /** Shown on the card only when the surname changed with marriage. */
  const surnameLines = $derived.by(() => {
    const married = detail?.names.find((n) => n.kind === 'married' && n.surnames.length > 0);
    const birth = detail?.names.find((n) => n.kind === 'birth' && n.surnames.length > 0);
    if (!married) return [];
    // The surname prefix belongs to the surname (van Dijk), the same way the line under the name reads it.
    const full = (n: NameDto) => [n.surnamePrefix?.trim(), n.surnames.join(', ')].filter(Boolean).join(' ');
    return [
      ...(birth ? [{ label: t('surnames.atBirth'), text: full(birth) }] : []),
      { label: t('surnames.afterMarriage'), text: full(married) },
    ];
  });
  const cardLines = $derived(
    detail
      ? [
          ...surnameLines,
          ...(['birth', 'death'] as const).flatMap((k) => {
            const e = k === 'birth' ? birthEvent : deathEvent;
            return e ? [{ label: k === 'birth' ? t('person.born') : t('person.died'), text: eventLine(e) || t('person.noDateOrPlace') }] : [];
          }),
        ]
      : [],
  );

  function eventLine(e: EventDto): string {
    return [e.date?.display, e.place?.full].filter(Boolean).join(', ');
  }
  const nameKind = (k: string) => t(`nameKind.${k}` as MessageKey);
  function open(p: PersonRef) {
    nav.go({ name: 'person', id: p.id });
  }
  function closeDialog() {
    dialog = null;
  }
  /** An agent changed what the open dialog shows: reopen it on what the tree says now, or close it if that is gone. */
  let dialogEpoch = $state(0);
  async function reopenDialog(): Promise<boolean> {
    if (!dialog) return false;
    if (!(await load())) return true;
    const d = dialog;
    // Only a record that is really gone closes the dialog. Any other failure keeps it open, with what was typed.
    const unlessGone = <T,>(read: Promise<T>) =>
      read.catch((e: unknown) => {
        if ((e as { code?: string } | null)?.code === 'not_found') return null;
        throw e;
      });
    try {
    if (d?.kind === 'name' && d.name) {
      const id = d.name.id;
      const name = detail?.names.find((n) => n.id === id);
      dialog = name ? { kind: 'name', name } : null;
    } else if (d?.kind === 'event' && d.event) {
      const id = d.event.id;
      let event: EventDto | undefined;
      if (!d.owner || (d.owner.kind === 'person' && d.owner.id === detail?.ref.id)) event = detail?.events.find((e) => e.id === id);
      else if (d.owner.kind === 'family') event = (await unlessGone(api.getFamily(d.owner.id)))?.events.find((e) => e.id === id);
      dialog = event ? { ...d, event } : null;
    } else if (d?.kind === 'note' && d.note) {
      const id = d.note.id;
      if (!d.owner) {
        const note = detail?.notes.find((n) => n.id === id);
        dialog = note ? { kind: 'note', note } : null;
      } else {
        const home = await unlessGone(api.recordHome('note', id));
        dialog = home?.note ? { kind: 'note', note: home.note, owner: home.owner } : null;
      }
    } else if (d?.kind === 'citation') {
      const home = await unlessGone(api.recordHome('citation', d.citation.id));
      dialog = home?.citation ? { kind: 'citation', citation: home.citation, owner: home.owner } : null;
    }
    } catch {
      return true;
    }
    dialogEpoch++;
    return true;
  }
  setContext<EditorHost>(EDITOR_HOST, { loadedAt: () => detailAt, reopen: reopenDialog });
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
  function onUndoAsked(s: 'open' | 'closed') {
    undoing = s === 'open';
    if (s === 'closed' && gone) {
      gone = false;
      leave();
    }
  }
  async function moreHistory() {
    try {
      const page = await api.personHistory(id, history.length, ROW_PAGE);
      history = [...history, ...page.items.filter((r) => !history.some((h) => h.group === r.group))];
      historyTotal = page.total;
    } catch (e) {
      error = errMessage(e);
    }
  }
  /** The owner edits what an agent changed, in the same editor as anywhere on this page. */
  function openEdit(target: EditTarget) {
    if (target.kind === 'note') dialog = { kind: 'note', note: target.note, owner: target.owner };
    else if (target.kind === 'event') dialog = { kind: 'event', event: target.event, owner: target.owner };
    else if (target.kind === 'name') dialog = { kind: 'name', name: target.name };
    else if (target.kind === 'family') dialog = { kind: 'family', family: target.family };
    else dialog = { kind: 'citation', citation: target.citation, owner: target.owner };
  }
  const whoWrote = (r: LogRowDto) => (r.actor.kind === 'user' ? t('changeLog.you') : r.actor.kind === 'agent' ? agentLabel(r.actor.id, r.actor.name) : t('changeLog.beforeTracking'));
  function when(ts: number): string {
    return new Date(ts).toLocaleString(currentLocale());
  }
</script>

<div class="page">
  {#if loadError}
    <Banner kind="error">{loadError}</Banner>
  {:else if !detail}
    <p class="loading">{t('person.loading')}</p>
  {:else}
    <div class="card">
    <IndexCard as="section" ribbon={isHome}>
      <div class="head">
        <h1 tabindex="-1" bind:this={title}><bdi dir="auto">{detail.ref.name}</bdi>{#if agentChanges.unreviewed.has(id)} <AgentBadge />{/if}</h1>
        {#if maiden}<p class="maiden">{maiden}</p>{/if}
        <p class="life">{detail.ref.lifespan || t('person.noDates')}</p>
        <div class="head-actions" role="group" aria-label={t('person.quickEdit')}>
          <Button
            variant="primary"
            onclick={() => (dialog = preferredName ? { kind: 'name', name: preferredName } : { kind: 'name' })}>{t('edit.editName')}</Button
          >
          <Button onclick={() => (dialog = { kind: 'event', event: birthEvent, newKind: 'birth' })}>
            {birthEvent ? t('person.editBirth') : t('person.addBirth')}
          </Button>
          <Button onclick={() => (dialog = { kind: 'event', event: deathEvent, newKind: 'death' })}>
            {deathEvent ? t('person.editDeath') : t('person.addDeath')}
          </Button>
          <Menu label={t('person.more')} text={t('person.more')} items={moreItems} />
        </div>
      </div>
      {#snippet body()}
        {#each cardLines as l (l.label)}
          <p class="card-line"><span class="card-label" class:wide={surnameLines.length > 0}>{l.label}</span> {l.text}</p>
        {/each}
      {/snippet}
    </IndexCard>
    </div>
    <div class="quick">
      <div class="qrow">
        <span class="qlabel" id="goto-label">{t('person.goto')}</span>
        <nav class="qbtns" aria-labelledby="goto-label">
          <Button onclick={() => openChart('around')}>{t('person.gotoAround')}</Button>
          <Button onclick={() => openChart('pedigree')}>{t('person.gotoPedigree')}</Button>
          <Button onclick={() => openChart('descendants')}>{t('person.gotoDescendants')}</Button>
          <Button onclick={() => openChart('relatives')}>{t('person.gotoRelatives')}</Button>
          {#each detail.partnerships as ps (ps.familyId)}
            <Button onclick={() => nav.go({ name: 'familyPage', id: ps.familyId })}
              >{ps.partner ? t('person.gotoFamilyWith', { name: ps.partner.name }) : t('person.gotoFamilyUnknown')}</Button
            >
          {/each}
          {#each detail.parentSets as s (s.familyId)}
            <Button onclick={() => nav.go({ name: 'familyPage', id: s.familyId })}
              >{detail.parentSets.length > 1 ? t('person.gotoParentsFamilyNamed', { names: parentNames(s) }) : t('person.gotoParentsFamily')}</Button
            >
          {/each}
          <Button onclick={() => jump('h-notes')}>{t('person.gotoNotes', { n: detail.notes.length })}</Button>
          <Button onclick={() => jump('h-media')}>{t('person.gotoMedia', { n: detail.media.length })}</Button>
          <Button onclick={() => jump('h-sources')}>{t('person.gotoSources', { n: detail.citations.length })}</Button>
          <Button onclick={() => jump('h-history')}>{t('person.history')}</Button>
        </nav>
      </div>
      <div class="qrow">
        <span class="qlabel">{t('person.addMenu')}</span>
        <div class="qbtns"><Menu label={t('person.addMenu')} text={t('person.addMenu')} items={addItems} /></div>
      </div>
    </div>
    {#if error}<Banner kind="error">{error}</Banner>{/if}

    {#if familyView}
      <section aria-labelledby="h-tree" class="tree-sec">
        <h2 id="h-tree">{t('person.familyTree')}</h2>
        <FamilyTree
          view={familyView}
          homeId={project.info?.homePerson ?? settings.value.homePerson ?? null}
          onopen={(pid) => nav.go({ name: 'person', id: pid })}
          onadd={(...args) => ui.requestAdd(...args)}
        />
      </section>
    {/if}

    <section aria-labelledby="h-names">
      <div class="sec-head">
        <h2 id="h-names">{t('person.names')}</h2>
        <Button onclick={() => (dialog = { kind: 'name' })}>{t('editor.addName')}</Button>
      </div>
      <ul class="rows">
        {#each detail.names as n (n.id)}
          <li>
            <span class="grow"><bdi dir="auto">{n.display}</bdi></span>
            <span class="meta">{n.preferred ? t('person.preferredSuffix', { kind: nameKind(n.kind) }) : nameKind(n.kind)}</span>
            <Button onclick={() => (dialog = { kind: 'name', name: n })} aria-label={t('person.editNameNamed', { name: n.display })}>{t('common.edit')}</Button>
            <Button
              variant="quiet"
              disabled={detail.names.length < 2}
              aria-label={t('person.deleteNameNamed', { name: n.display })}
              onclick={() => remove({ type: 'deleteName', person: id, name: n.id })}>{t('common.delete')}</Button
            >
          </li>
        {/each}
      </ul>
    </section>

    <section aria-labelledby="h-basics">
      <h2 id="h-basics">{t('person.basics')}</h2>
      <div class="grid" bind:this={basics} onfocusout={basicsBlurred}>
        <div data-field="sex">
          <Select
            label={t('newPerson.sex')}
            options={SEXES}
            bind:value={sex}
            onchange={(v) => commit('Sex', { sex: v as PersonDetail['sex'] })}
          />
        </div>
        {#if sex === 'other'}
          <div data-field="sexText">
            <TextField
              label={t('person.describeSex')}
              bind:value={sexText}
              onchange={() => commit('Sex', { sex: 'other', sexText })}
            />
          </div>
        {/if}
        <div data-field="living">
          <Select
            label={t('person.livingStatus')}
            options={livingOptions}
            bind:value={living}
            onchange={(v) => commit('Living status', { livingOverride: v === '' ? null : (v as 'living' | 'deceased') })}
          />
        </div>
        <div data-field="private">
          <Checkbox
            label={t('common.private')}
            hint={t('person.privateHint')}
            bind:checked={isPrivate}
            onchange={(c) => commit('Privacy', { private: c })}
          />
        </div>
      </div>
    </section>

    <section aria-labelledby="h-events">
      <div class="sec-head">
        <h2 id="h-events">{t('person.events')}</h2>
        <Button onclick={() => (dialog = { kind: 'event' })}>{t('editor.addEvent')}</Button>
      </div>
      {#if detail.events.length === 0}
        <p class="empty">{t('person.noEvents')}</p>
      {:else}
        <ul class="rows events">
          {#each detail.events as e (e.id)}
            <li>
              <span class="kind">{eventKindLabel(e.kind, e.label)}</span>
              <EventSummary event={e} />
              <Button onclick={() => (dialog = { kind: 'event', event: e })} aria-label={t('person.editNamed', { label: eventKindLabel(e.kind, e.label) })}>{t('common.edit')}</Button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section aria-labelledby="h-rel">
      <h2 id="h-rel">{t('person.relationships')}</h2>
      <h3>{t('family.parents')}</h3>
      {#each detail.parentSets as s (s.familyId)}
        <p class="line">
          {#each s.parents as p, i (i)}
            {#if p}<Button variant="quiet" onclick={() => open(p)}><bdi dir="auto">{p.name}</bdi></Button>{/if}
          {/each}
        </p>
      {:else}
        <p class="empty">{t('person.noParents')}</p>
      {/each}
      <div class="btns">
        <Button onclick={() => (dialog = { kind: 'quick', relation: 'father' })}>{t('edit.addFather')}</Button>
        <Button onclick={() => (dialog = { kind: 'quick', relation: 'mother' })}>{t('edit.addMother')}</Button>
        <Button onclick={() => (dialog = { kind: 'quick', relation: 'sibling' })}>{t('edit.addSibling')}</Button>
      </div>
      <h3>{t('person.partnersAndChildren')}</h3>
      {#each detail.partnerships as ps (ps.familyId)}
        <div class="partnership">
          <p class="line">
            {#if ps.partner}
              <Button variant="quiet" onclick={() => open(ps.partner!)}><bdi dir="auto">{ps.partner.name}</bdi></Button>
            {:else}
              <span>{t('person.unknownPartner')}</span>
            {/if}
            {#if ps.summary}<span class="meta">{ps.summary}</span>{/if}
            <Button onclick={() => (dialog = { kind: 'family', family: ps.familyId })} aria-label={t('person.editFamilyWith', { name: ps.partner?.name ?? t('person.unknownPartnerLower') })}
              >{t('family.editFamily')}</Button
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
        <p class="empty">{t('person.noPartners')}</p>
      {/each}
      <div class="btns">
        <Button onclick={() => (dialog = { kind: 'quick', relation: 'partner' })}>{t('edit.addPartner')}</Button>
        <Button onclick={() => (dialog = { kind: 'quick', relation: 'child' })}>{t('edit.addChild')}</Button>
        <Button onclick={() => (dialog = { kind: 'family' })}>{t('person.addFamily')}</Button>
      </div>
    </section>

    <section aria-labelledby="h-notes">
      <div class="sec-head">
        <h2 id="h-notes" tabindex="-1">{t('person.notes')}</h2>
        <Button onclick={() => (dialog = { kind: 'note' })}>{t('editor.addNote')}</Button>
      </div>
      {#each detail.notes as n (n.id)}
        <article class="note">
          <p class="note-kind">{t(`noteKind.${n.kind}`)}</p>
          <p class="note-text">{n.text}</p>
          <VoiceSamples items={n.media} />
          <div class="btns">
            <Button onclick={() => (dialog = { kind: 'note', note: n })} aria-label={t('person.editNote', { kind: t(`noteKind.${n.kind}`) })}>{t('common.edit')}</Button>
            <Button
              variant="quiet"
              aria-label={t('person.deleteNote', { kind: t(`noteKind.${n.kind}`) })}
              onclick={() => remove({ type: 'deleteNote', target: { kind: 'person', id }, note: n.id })}
              >{t('common.delete')}</Button
            >
          </div>
        </article>
      {:else}
        {#if familyNotes.length === 0}<p class="empty">{t('person.noNotes')}</p>{/if}
      {/each}
      {#each familyNotes as fam (fam.familyId)}
        <div class="sec-head">
          <h3>{fam.parents !== undefined ? t('person.parentsFamilyNotes', { names: fam.parents }) : fam.partner && fam.role ? t(fam.role === 'husband' ? 'person.familyNotesHusband' : 'person.familyNotesWife', { name: fam.partner }) : fam.partner ? t('person.familyNotesWith', { name: fam.partner }) : t('person.familyNotes')}</h3>
          <Button onclick={() => (dialog = { kind: 'family', family: fam.familyId })}>{t('person.openFamily')}</Button>
        </div>
        {#each fam.notes as n (n.id)}
          <article class="note">
            <p class="note-kind">{t(`noteKind.${n.kind}`)}</p>
            <p class="note-text">{n.text}</p>
            <VoiceSamples items={n.media} />
            <div class="btns">
              <Button
                onclick={() => (dialog = { kind: 'note', note: n, owner: { kind: 'family', id: fam.familyId } })}
                aria-label={t('person.editNote', { kind: t(`noteKind.${n.kind}`) })}>{t('common.edit')}</Button
              >
              <Button
                variant="quiet"
                aria-label={t('person.deleteNote', { kind: t(`noteKind.${n.kind}`) })}
                onclick={() => remove({ type: 'deleteNote', target: { kind: 'family', id: fam.familyId }, note: n.id })}
                >{t('common.delete')}</Button
              >
            </div>
          </article>
        {/each}
      {/each}
    </section>

    <section aria-labelledby="h-media">
      <h2 id="h-media" tabindex="-1">{t('nav.media')}</h2>
      {#if mediaSection}{@render mediaSection(detail)}{:else}<p class="empty">{t('person.attachedCount', { n: detail.media.length })}</p>{/if}
    </section>

    <section aria-labelledby="h-sources">
      <h2 id="h-sources" tabindex="-1">{t('sources.title')}</h2>
      {#if sourcesSection}{@render sourcesSection(detail)}{:else}<p class="empty">{t('person.citedCount', { n: detail.citations.length })}</p>{/if}
    </section>

    <div class="danger-zone">
      <Button variant="danger" onclick={() => (dialog = { kind: 'delete' })}>{t('people.deleteTitle')}</Button>
    </div>

    <section aria-labelledby="h-history">
      <details bind:this={historyBox}>
        <summary><h2 id="h-history" tabindex="-1">{t('person.history')}</h2></summary>
        {#if history.length === 0}
          <p class="empty">{t('person.noHistory')}</p>
        {:else}
          <ul class="rows">
            {#each history as h (h.group)}
              <li>
                <span class="grow">{editLabel(h.label)}</span>
                {#if h.reverted}<Badge>{t('changeLog.undone')}</Badge>{/if}
                <span class="meta">{whoWrote(h)}</span><span class="meta">{when(h.ts)}</span>
                <div class="changed"><LogChanges changes={h.changes} /></div>
              </li>
            {/each}
          </ul>
          {#if history.length < historyTotal}
            <div class="more"><Button onclick={moreHistory}>{t('changeLog.showMore')}</Button></div>
          {/if}
        {/if}
        {#if olderChanges}<p class="empty">{t('person.olderChanges')}</p>{/if}
      </details>
    </section>

    <AgentChangesPanel {id} {detail} onedit={openEdit} onvacated={() => title?.focus()} onundo={onUndoAsked} />
  {/if}
</div>

{#key dialogEpoch}
{#if detail && dialog?.kind === 'name'}
  <NameEditor person={id} name={dialog.name} onCancel={closeDialog} onSaved={(r) => saved(r)} />
{:else if detail && dialog?.kind === 'event'}
  <EventEditor
    owner={dialog.owner ?? { kind: 'person', id }}
    event={dialog.event}
    newKind={dialog.newKind}
    onCancel={closeDialog}
    onSaved={(r) => saved(r)}
  />
{:else if detail && dialog?.kind === 'surnames'}
  <SurnamesEditor person={id} names={detail.names} onCancel={closeDialog} onSaved={(r) => saved(r)} />
{:else if detail && dialog?.kind === 'family'}
  <FamilyEditor
    family={dialog.family}
    anchor={id}
    seedPartner={id}
    onCancel={closeDialog}
    onChanged={quiet}
    onDone={(r) => saved(r)}
  />
{:else if detail && dialog?.kind === 'citation'}
  <CitationEditor
    target={dialog.owner ?? { kind: 'person', id }}
    citation={dialog.citation}
    onCancel={closeDialog}
    onDone={(r) => saved(r)}
  />
{:else if detail && dialog?.kind === 'note'}
  <NoteEditor
    target={dialog.owner ?? { kind: 'person', id }}
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
  <Dialog open={true} title={t('person.deleteTitle', { name: detail.ref.name })} onclose={closeDialog}>
    <p>{consequence}</p>
    <p>{t('person.undoAfter')}</p>
    {#snippet footer()}
      <Button onclick={closeDialog}>{t('person.keepPerson')}</Button>
      <Button variant="danger" onclick={confirmDelete}>{t('people.deleteTitle')}</Button>
    {/snippet}
  </Dialog>
{/if}
{/key}

<style>
  .page {
    display: grid;
    gap: var(--s-6);
    padding: var(--s-6);
  }
  /* The text sections keep a readable width; the family tree may use the whole page. */
  .page > :not(.tree-sec) {
    max-width: 70rem;
  }
  .tree-sec {
    min-width: 0;
    max-width: none;
  }
  .card {
    max-width: var(--measure);
  }
  [data-field] {
    display: contents;
  }
  .head {
    display: grid;
    gap: 0;
  }
  .head h1 {
    font-size: var(--t-2xl);
    line-height: var(--t-2xl-lh);
  }
  .maiden,
  .life {
    color: var(--ink-1);
    font-variant-numeric: tabular-nums;
  }
  .head-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-3);
    padding-top: var(--s-4);
  }
  .quick {
    display: grid;
    gap: var(--s-3);
    max-width: var(--measure);
  }
  .qrow {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-3);
  }
  .qlabel {
    min-width: 6rem;
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  .qbtns {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-3);
    min-width: 0;
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
  /* With the surname lines the labels are longer; keep one aligned column. */
  .card-label.wide {
    min-width: 12rem;
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
  .rows li:has(.changed) {
    flex-wrap: wrap;
  }
  .changed {
    flex-basis: 100%;
    padding-bottom: var(--s-2);
  }
  .events li {
    align-items: baseline;
  }
  .grow {
    flex: 1;
  }
  .more {
    padding-top: var(--s-2);
  }
  .head h1:focus {
    outline: none;
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
