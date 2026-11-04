<script lang="ts">
  import Button from '../ui/Button.svelte';
  import ContextMenu from '../ui/ContextMenu.svelte';
  import type { MenuItem } from '../ui/Menu.svelte';
  import { t } from '../i18n';
  import type { PictureScope } from './picture';

  /**
   * The "Save as picture" button for a chart's toolbar and the menu behind it. The chart also opens the same menu
   * at the pointer (right-click) or over the selected box (Shift+F10, ContextMenu key) with `openAt`.
   */
  let {
    hasSelection,
    busy = false,
    onpick,
    restoreFocus,
  }: {
    hasSelection: boolean;
    busy?: boolean;
    onpick: (scope: PictureScope) => void;
    /** Gives focus back to the chart when the menu closes after being opened from the chart. */
    restoreFocus: () => void;
  } = $props();

  let at = $state<{ x: number; y: number; origin: 'chart' | 'button' } | null>(null);
  let wrap = $state<HTMLElement>();

  export function openAt(x: number, y: number): void {
    if (!busy) at = { x, y, origin: 'chart' };
  }

  function fromButton(): void {
    if (at) return close(true);
    const r = wrap?.querySelector('button')?.getBoundingClientRect();
    at = { x: r?.left ?? 0, y: (r?.bottom ?? 0) + 4, origin: 'button' };
  }

  function close(restore: boolean): void {
    const origin = at?.origin;
    at = null;
    if (!restore) return;
    if (origin === 'button') wrap?.querySelector('button')?.focus();
    else restoreFocus();
  }

  const items = $derived<MenuItem[]>([
    { label: t('picture.whole'), onselect: () => onpick('whole') },
    {
      label: t('picture.branch'),
      onselect: () => onpick('branch'),
      disabled: !hasSelection,
      title: hasSelection ? undefined : t('picture.selectFirst'),
    },
    { label: t('picture.screen'), onselect: () => onpick('screen') },
  ]);
</script>

<!-- A press on the button must not count as a press outside the open menu, or the button could never close it. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<span class="wrap" bind:this={wrap} onmousedown={(e) => e.stopPropagation()}>
  <Button disabled={busy} aria-haspopup="menu" aria-expanded={at !== null} onclick={fromButton}>{t('picture.save')}</Button>
</span>
{#if at}
  <ContextMenu label={t('picture.menuLabel')} {items} x={at.x} y={at.y} onclose={close} />
{/if}

<style>
  .wrap {
    display: inline-flex;
  }
</style>
