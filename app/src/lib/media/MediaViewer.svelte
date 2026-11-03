<script lang="ts">
  import Dialog from '../ui/Dialog.svelte';
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import Icon from '../ui/Icon.svelte';
  import TextField from '../ui/TextField.svelte';
  import TextArea from '../ui/TextArea.svelte';
  import { getApi, getPlatform } from '../api/context';
  import { notices } from '../ui/notices.svelte';
  import { isPdf, mediaLabel } from './MediaGrid.svelte';
  import type { Id, MediaLinkDto, OwnerRef } from '../api/api';

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
  const platform = getPlatform();
  const MAX_ZOOM = 8;

  // svelte-ignore state_referenced_locally
  let at = $state(Math.min(Math.max(index, 0), Math.max(items.length - 1, 0)));
  let zoom = $state(1);
  let panX = $state(0);
  let panY = $state(0);
  let edits = $state<Record<Id, { title: string; description: string }>>({});
  let error = $state('');
  let primaryIds = $state(new Set<Id>());
  let drag: { x: number; y: number; px: number; py: number } | null = null;

  const link = $derived(items[at]);
  const m = $derived(link?.media);
  const title = $derived(m ? (edits[m.id]?.title ?? m.title ?? '') : '');
  const description = $derived(m ? (edits[m.id]?.description ?? m.description ?? '') : '');
  const primary = $derived(!!link?.primary || (m ? primaryIds.has(m.id) : false));

  function go(to: number) {
    const next = Math.min(Math.max(to, 0), items.length - 1);
    if (next === at) return;
    at = next;
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
    if (!m || target?.kind !== 'person') return;
    error = '';
    try {
      await api.apply({ type: 'setPrimaryMedia', person: target.id, media: m.id });
      primaryIds = new Set([...primaryIds, m.id]);
      notices.show({ message: 'Primary photo set' });
      onChanged?.();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
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

<Dialog open title={m ? mediaLabel({ ...m, title: title || m.title }) : 'Photo'} onclose={onClose}>
  {#if m}
    <div class="viewer">
      {#if error}<Banner kind="error">{error}</Banner>{/if}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <div
        class="stage"
        role="group"
        aria-label="Preview, {at + 1} of {items.length}. Plus and minus zoom, zero fits, arrow keys move, P sets primary."
        tabindex="0"
        {onkeydown}
        {onwheel}
        {onpointerdown}
        {onpointermove}
        {onpointerup}
        data-zoom={zoom}
      >
        {#if !m.resolved}
          <p class="note"><Icon name="file" size="2rem" /> File missing</p>
        {:else if isPdf(m)}
          <p class="note"><Icon name="file" size="2rem" /> This is a document. Open the original to read it.</p>
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
        <span aria-live="polite">Zoom {Math.round(zoom * 100)}%</span>
        <span>{at + 1} of {items.length}</span>
        {#if primary}<span>Primary photo</span>{/if}
        <Button disabled={at === 0} onclick={() => go(at - 1)}>Previous</Button>
        <Button disabled={at >= items.length - 1} onclick={() => go(at + 1)}>Next</Button>
        {#if target?.kind === 'person' && !isPdf(m)}
          <Button onclick={setPrimary}>Set as primary photo</Button>
        {/if}
        <Button onclick={() => platform.openExternally(api.mediaUrl(m.id, 'original'))}>Open original</Button>
      </div>
      {#key m.id}
        <TextField label="Title" value={title} onchange={(e) => saveField('title', e.currentTarget.value)} />
        <TextArea label="Description" value={description} rows={2} onchange={(e) => saveField('description', e.currentTarget.value)} />
      {/key}
    </div>
  {/if}
</Dialog>

<style>
  .viewer {
    display: grid;
    gap: var(--s-4);
    max-height: calc(100vh - 11rem);
    overflow: auto;
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
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-3);
    font-size: var(--t-sm);
    color: var(--ink-1);
  }
</style>
