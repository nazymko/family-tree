<script lang="ts">
  import Button from '../ui/Button.svelte';
  import ContextMenu from '../ui/ContextMenu.svelte';
  import type { MenuItem } from '../ui/Menu.svelte';
  import { getApi } from '../api/context';
  import type { Id } from '../api/types';
  import { parentFacts, type ParentFacts } from '../family/addOptions';
  import { t } from '../i18n';
  import type { PictureScope } from './picture';
  import { relativeItems } from './relativeItems';

  /**
   * The "Save as picture" button for a chart's toolbar and the menu behind it. The chart also opens the same menu
   * at the pointer (right-click) or over the selected box (Shift+F10, ContextMenu key) with `openAt`.
   * When it is opened on a person it starts with the add-a-relative items and "Open person".
   */
  let {
    hasSelection,
    busy = false,
    onpick,
    restoreFocus,
    onOpenPerson,
  }: {
    hasSelection: boolean;
    busy?: boolean;
    onpick: (scope: PictureScope) => void;
    /** Gives focus back to the chart when the menu closes after being opened from the chart. */
    restoreFocus: () => void;
    /** Shows "Open person" in the person section when given. */
    onOpenPerson?: (id: Id) => void;
  } = $props();

  const api = getApi();
  let at = $state<{ x: number; y: number; origin: 'chart' | 'button'; person: Id | null; facts: ParentFacts | null } | null>(null);
  let wrap = $state<HTMLElement>();
  let seq = 0;

  /** Opens the menu at a point. With a person, the add items follow that person's parents; without, only the save items show. */
  export async function openAt(x: number, y: number, person: Id | null = null): Promise<void> {
    if (busy) return;
    const mine = ++seq;
    let facts: ParentFacts | null = null;
    if (person) {
      try {
        facts = parentFacts(await api.getFamilyView(person));
      } catch {
        // Without the facts only the save items are offered.
      }
      if (mine !== seq) return;
    }
    at = { x, y, origin: 'chart', person: facts ? person : null, facts };
  }

  function fromButton(): void {
    if (at) return close(true);
    const r = wrap?.querySelector('button')?.getBoundingClientRect();
    seq++;
    at = { x: r?.left ?? 0, y: (r?.bottom ?? 0) + 4, origin: 'button', person: null, facts: null };
  }

  function close(restore: boolean): void {
    const origin = at?.origin;
    seq++;
    at = null;
    if (!restore) return;
    if (origin === 'button') wrap?.querySelector('button')?.focus();
    else restoreFocus();
  }

  const personItems = $derived.by<MenuItem[]>(() => {
    const id = at?.person;
    if (!id || !at?.facts) return [];
    const list = relativeItems(at.facts, id);
    if (onOpenPerson) list.push({ label: t('chartAdd.openPerson'), onselect: () => onOpenPerson(id) });
    return list;
  });

  const saveItems = $derived<MenuItem[]>([
    { label: t('picture.whole'), onselect: () => onpick('whole') },
    {
      label: t('picture.branch'),
      onselect: () => onpick('branch'),
      disabled: !hasSelection,
      title: hasSelection ? undefined : t('picture.selectFirst'),
    },
    { label: t('picture.screen'), onselect: () => onpick('screen') },
  ]);

  const items = $derived<MenuItem[]>(personItems.length ? [...personItems, ...saveItems.map((i, k) => (k === 0 ? { ...i, separatorBefore: true } : i))] : saveItems);
</script>

<!-- A press on the button must not count as a press outside the open menu, or the button could never close it. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<span class="wrap" bind:this={wrap} onmousedown={(e) => e.stopPropagation()}>
  <Button disabled={busy} aria-haspopup="menu" aria-expanded={at !== null} onclick={fromButton}>{t('picture.save')}</Button>
</span>
{#if at}
  <ContextMenu label={personItems.length ? t('chartAdd.menuLabel') : t('picture.menuLabel')} {items} x={at.x} y={at.y} onclose={close} />
{/if}

<style>
  .wrap {
    display: inline-flex;
  }
</style>
