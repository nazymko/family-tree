<script lang="ts">
  import { getApi } from '../api/context';
  import type { Id, WholeMeta } from '../api/types';
  import WholeCanvas from '../chart/WholeCanvas.svelte';
  import { nav } from '../state/nav.svelte';
  import { settings } from '../state/settings.svelte';
  import Button from '../ui/Button.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import { plural, t } from '../i18n';

  let { onOpenPerson }: { onOpenPerson?: (id: Id) => void } = $props();

  const api = getApi();
  let home = $state<Id | null>(null);
  let ready = $state(false);
  let meta = $state<WholeMeta | null>(null);

  /** The chosen people when this page shows a chart of some people instead of everyone (part of the route, so Back and Forward work). */
  const people = $derived(nav.route.name === 'whole' ? (nav.route.people ?? []) : []);
  const peopleKey = $derived(people.join(','));
  const inSelection = $derived(people.length > 0);
  let descendants = $state(false);
  let names = $state<string[]>([]);

  $effect(() => {
    api
      .getSettings()
      .then((s) => (home = s.homePerson))
      .catch(() => {})
      .finally(() => (ready = true));
  });

  // The all-descendants box is off whenever a new choice of people is made from the whole tree.
  $effect(() => {
    if (!inSelection) descendants = false;
  });

  // Who was chosen, by name, for the heading.
  $effect(() => {
    const ids = peopleKey === '' ? [] : peopleKey.split(',');
    let stale = false;
    void Promise.all(ids.map((id) => api.getPerson(id).then((d) => d.ref.name, () => ''))).then((all) => {
      if (!stale) names = all.filter((n) => n !== '');
    });
    return () => {
      stale = true;
    };
  });

  const openPerson = (id: Id) => (onOpenPerson ? onOpenPerson(id) : nav.go({ name: 'person', id }));
  function inPedigree(id: Id): void {
    nav.focus(id);
    nav.go({ name: 'pedigree' });
  }
  const showSelection = (ids: Id[]) => nav.go({ name: 'whole', people: ids });
  const allPeople = () => nav.go({ name: 'whole' });
  // Everybody chosen was deleted: the chart of some people no longer exists, so it leaves no history entry behind.
  const selectionGone = () => nav.replace({ name: 'whole' });
  const empty = $derived(meta !== null && meta.meta.nodeCount === 0 && !inSelection);
  const shownNames = $derived(names.slice(0, 3).join(', ') + (names.length > 3 ? ` ${t('whole.andMore', { n: names.length - 3 })}` : ''));
</script>

<section class="view" aria-labelledby="whole-title">
  <header>
    <h1 id="whole-title">{inSelection ? t('whole.selectionTitle') : t('whole.title')}</h1>
    {#if meta && !empty}
      <p class="count">{t('whole.count', { people: plural('common.people', meta.meta.nodeCount), groups: plural('whole.nGroupsIn', meta.blocks.length) })}</p>
    {/if}
    {#if inSelection}
      <p class="chosen" data-testid="chosen">
        {plural('whole.nMarked', meta?.chosen ?? people.length)}{#if shownNames}: {shownNames}{/if}
      </p>
      <Checkbox label={t('whole.showDescendants')} bind:checked={descendants} />
      <Button onclick={allPeople}>{t('whole.backToAll')}</Button>
    {/if}
  </header>
  {#if empty}
    <EmptyState title={t('whole.emptyTitle')}>{t('whole.emptyBody')}</EmptyState>
  {/if}
  {#if ready}
    <div class="body" hidden={empty}>
      <WholeCanvas
        homeId={home}
        focusId={nav.focusPerson}
        onOpenPerson={openPerson}
        onOpenInPedigree={inPedigree}
        onOpened={(m) => (meta = m)}
        reversed={settings.value.treeDirection === 'reversed'}
        {people}
        {descendants}
        onShowSelection={showSelection}
        onSelectionGone={selectionGone}
      />
    </div>
  {/if}
</section>

<style>
  .view {
    display: flex;
    flex-direction: column;
    gap: var(--s-4);
    height: 100%;
    min-height: 0;
  }
  header {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--s-6);
  }
  h1 {
    margin: 0;
  }
  .count,
  .chosen {
    margin: 0;
    color: var(--ink-2);
  }
  .body {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
  .body[hidden] {
    display: none;
  }
</style>
