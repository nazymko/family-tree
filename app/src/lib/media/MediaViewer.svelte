<script lang="ts">
  import { tick } from 'svelte';
  import Dialog from '../ui/Dialog.svelte';
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import Icon from '../ui/Icon.svelte';
  import TextField from '../ui/TextField.svelte';
  import TextArea from '../ui/TextArea.svelte';
  import { getApi } from '../api/context';
  import { notices } from '../ui/notices.svelte';
  import { isPdf, mediaLabel, openOriginal } from './MediaGrid.svelte';
  import type { EditCommand, Id, MediaLinkDto, OwnerRef } from '../api/api';
  import { t } from '../i18n';

  let {
    items,
    index,
    target,
    onClose,
    onChanged,
  }: {
    items: MediaLinkDto[];
    index: number;
    /** When it is a person, P sets the shown photo as that person's primary photo. */
    target?: OwnerRef;
    onClose: () => void;
    onChanged?: () => void;
  } = $props();

  const api = getApi();
  const MAX_ZOOM = 8;

  // svelte-ignore state_referenced_locally
  let at = $state(Math.min(Math.max(index, 0), Math.max(items.length - 1, 0)));
  let zoom = $state(1);
  let panX = $state(0);
  let panY = $state(0);
  let edits = $state<Record<Id, { title: string; description: string }>>({});
  let error = $state('');
  let primaryIds = $state(new Set<Id>());
  let confirming = $state(false);
  let busy = $state(false);
  let stage = $state<HTMLElement>();
  let drag: { x: number; y: number; px: number; py: number } | null = null;

  // The list shrinks when a picture goes: the next one takes its place, or the one before when it was the last.
  $effect(() => {
    if (at >= items.length) at = Math.max(items.length - 1, 0);
  });

  const link = $derived(items[at]);
  const m = $derived(link?.media);
  const title = $derived(m ? (edits[m.id]?.title ?? m.title ?? '') : '');
  const description = $derived(m ? (edits[m.id]?.description ?? m.description ?? '') : '');
  const primary = $derived(!!link?.primary || (m ? primaryIds.has(m.id) : false));

  function go(to: number) {
    const next = Math.min(Math.max(to, 0), items.length - 1);
    if (next === at) return;
    at = next;
    confirming = false;
    fit();
  }
  function fit() {
    zoom = 1;
    panX = 0;
    panY = 0;
  }
  function setZoom(z: number) {
    zoom = Math.min(MAX_ZOOM, Math.max(1, z));
    if (zoom === 1) {
      panX = 0;
      panY = 0;
    }
  }

  async function setPrimary() {
    if (!m || !m.resolved || isPdf(m) || target?.kind !== 'person') return;
    error = '';
    try {
      await api.apply({ type: 'setPrimaryMedia', person: target.id, media: m.id }, { silent: true });
      primaryIds = new Set([...primaryIds, m.id]);
      notices.show({ message: t('media.primarySet') });
      onChanged?.();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  /**
   * The dialog listens for Escape and Tab on its panel. When the focused button disappears, focus falls to the
   * body and those keys stop working, so it comes back to the picture.
   */
  async function keepFocusInside() {
    await tick();
    if (!document.activeElement || document.activeElement === document.body) stage?.focus();
  }

  function closeConfirm() {
    confirming = false;
    void keepFocusInside();
  }

  /** Unlinks or deletes the shown picture; the viewer closes when that was the only one. */
  async function remove(command: EditCommand) {
    error = '';
    busy = true;
    const wasOnly = items.length <= 1;
    try {
      await api.apply(command);
      fit();
      onChanged?.();
      if (wasOnly) onClose();
      else closeConfirm();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function saveField(field: 'title' | 'description', value: string) {
    if (!m) return;
    const prev = edits[m.id] ?? { title: m.title ?? '', description: m.description ?? '' };
    edits[m.id] = { ...prev, [field]: value };
    error = '';
    try {
      await api.apply({ type: 'updateMedia', id: m.id, [field]: value });
      onChanged?.();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function onkeydown(e: KeyboardEvent) {
    const t = e.target as HTMLElement;
    if (t.matches('input, textarea, select') || e.ctrlKey || e.metaKey || e.altKey) return;
    switch (e.key) {
      case '+':
      case '=':
        setZoom(zoom * 2);
        break;
      case '-':
        setZoom(zoom / 2);
        break;
      case '0':
        fit();
        break;
      case 'ArrowLeft':
        go(at - 1);
        break;
      case 'ArrowRight':
        go(at + 1);
        break;
      case 'p':
      case 'P':
        void setPrimary();
        break;
      default:
        return;
    }
    e.preventDefault();
  }

  function onwheel(e: WheelEvent) {
    e.preventDefault();
    setZoom(e.deltaY < 0 ? zoom * 1.25 : zoom / 1.25);
  }
  function onpointerdown(e: PointerEvent) {
    if (zoom <= 1) return;
    drag = { x: e.clientX, y: e.clientY, px: panX, py: panY };
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
  }
  function onpointermove(e: PointerEvent) {
    if (!drag) return;
    panX = drag.px + e.clientX - drag.x;
    panY = drag.py + e.clientY - drag.y;
  }
  function onpointerup() {
    drag = null;
  }
</script>

<svelte:window {onkeydown} />

<Dialog open title={m ? mediaLabel({ ...m, title: title || m.title }) : t('media.photo')} onclose={onClose}>
  {#if m}
    <div class="viewer">
      {#if error}<Banner kind="error">{error}</Banner>{/if}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div
        bind:this={stage}
        class="stage"
        role="group"
        aria-label={t('media.previewHelp', { i: at + 1, n: items.length })}
        tabindex="0"
        {onwheel}
        {onpointerdown}
        {onpointermove}
        {onpointerup}
        data-zoom={zoom}
      >
        {#if !m.resolved}
          <p class="note"><Icon name="file" size="2rem" /> {t('media.fileMissing')}</p>
        {:else if isPdf(m)}
          <p class="note"><Icon name="file" size="2rem" /> {t('media.isDocument')}</p>
        {:else}
          <img
            alt={mediaLabel(m)}
            src={api.mediaUrl(m.id, 'preview')}
            draggable="false"
            style="transform: translate({panX}px, {panY}px) scale({zoom})"
          />
        {/if}
      </div>
      <div class="bar">
        <span aria-live="polite">{t('media.zoom', { pct: Math.round(zoom * 100) })}</span>
        <span>{t('media.position', { i: at + 1, n: items.length })}</span>
        {#if primary}<span>{t('media.primaryPhoto')}</span>{/if}
        <Button disabled={at === 0} onclick={() => go(at - 1)}>{t('media.previous')}</Button>
        <Button disabled={at >= items.length - 1} onclick={() => go(at + 1)}>{t('media.next')}</Button>
        {#if target?.kind === 'person' && m.resolved && !isPdf(m)}
          <Button onclick={setPrimary}>{t('media.setPrimary')}</Button>
        {/if}
        <Button onclick={() => openOriginal(api, m.id)}>{t('media.openOriginal')}</Button>
        {#if target}
          <Button disabled={busy} onclick={() => target && remove({ type: 'detachMedia', target, media: m.id })}>{t('media.unlink')}</Button>
        {/if}
        <Button variant="danger" disabled={busy} onclick={() => (confirming = true)}>{t('media.deleteFromTree')}</Button>
      </div>
      {#if confirming}
        <div class="confirm" role="alert">
          <p>{t('media.deleteConfirm', { name: mediaLabel(m) })}</p>
          <div class="choices">
            <Button onclick={closeConfirm}>{t('media.keep')}</Button>
            <Button variant="danger" disabled={busy} onclick={() => remove({ type: 'deleteMedia', id: m.id })}>{t('media.deleteFromTree')}</Button>
          </div>
        </div>
      {/if}
      {#key m.id}
        <TextField label={t('media.title')} value={title} onchange={(e) => saveField('title', e.currentTarget.value)} />
        <TextArea label={t('media.description')} value={description} rows={2} onchange={(e) => saveField('description', e.currentTarget.value)} />
      {/key}
    </div>
  {/if}
</Dialog>

<style>
  .viewer {
    display: grid;
    gap: var(--s-4);
  }
  .stage {
    height: min(40vh, 22rem);
    flex: none;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: var(--paper-2);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    touch-action: none;
    cursor: grab;
  }
  img {
    max-width: 100%;
    max-height: 100%;
    transform-origin: center;
    user-select: none;
  }
  .note {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    color: var(--ink-1);
  }
  .confirm {
    display: grid;
    gap: var(--s-3);
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background: var(--paper-2);
  }
  .confirm p {
    margin: 0;
  }
  .choices {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--s-3);
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-3);
    font-size: var(--t-sm);
    color: var(--ink-1);
  }
</style>
