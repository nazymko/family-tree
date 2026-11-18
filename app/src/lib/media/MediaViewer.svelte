<script lang="ts">
  import { errMessage } from '../editors/errors';
  import { tick } from 'svelte';
  import Dialog from '../ui/Dialog.svelte';
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import Icon from '../ui/Icon.svelte';
  import TextField from '../ui/TextField.svelte';
  import TextArea from '../ui/TextArea.svelte';
  import PlacePicker from '../editors/PlacePicker.svelte';
  import { getApi, getPlatform } from '../api/context';
  import { notices } from '../ui/notices.svelte';
  import { isPdf, mediaLabel, openOriginal } from './MediaGrid.svelte';
  import TextOverlay from './TextOverlay.svelte';
  import FullTextPanel from './FullTextPanel.svelte';
  import VersionPicker from './VersionPicker.svelte';
  import { bestPrimary, groupCommand, linkedMember, sizeText, versionsOf, type ShownLink } from './versions';
  import { toInput, turnedBy, type Rotation } from './transcription';
  import type { EditCommand, Id, MediaDto, MediaLinkDto, OwnerRef, TextRegionDto } from '../api/api';
  import { t } from '../i18n';

  let {
    items,
    index,
    target,
    onClose,
    onChanged,
  }: {
    /** One link per version group (see `collapseVersions`): the group's primary, with its versions. */
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
  let confirming = $state(false);
  let busy = $state(false);
  let stage = $state<HTMLElement>();
  let drag: { x: number; y: number; px: number; py: number } | null = null;

  // The list shrinks when a picture goes: the next one takes its place, or the one before when it was the last.
  $effect(() => {
    if (at >= items.length) at = Math.max(items.length - 1, 0);
  });

  const link = $derived(items[at] as ShownLink | undefined);
  /** The page as listed: the group's primary, with every version of it. */
  const head = $derived(link?.media);
  /** Every file of the page, primary first; empty when it is kept in one file. */
  const versions = $derived(head ? versionsOf(head) : []);
  /** Which version of each page is on screen, by the id of the page's primary; the primary until another is chosen. */
  let viewing = $state<Record<Id, Id>>({});
  /** The file on screen: the primary, or the version chosen in the switcher. Text, turn and fields are this file's. */
  const m = $derived(head ? (versions.find((v) => v.id === viewing[head.id]) ?? head) : undefined);
  /** The file this record links to, for Remove and primary photo: the one on screen when it is linked. */
  const linkedId = $derived(link && m ? linkedMember(link, m.id) : undefined);
  /** Another version has a transcription while the one on screen has none. */
  const textElsewhere = $derived(
    m && !m.transcription.some((r) => r.text.trim()) ? versions.find((v) => v.id !== m.id && v.transcription.some((r) => r.text.trim())) : undefined,
  );
  let picking = $state(false);

  function showVersion(id: Id) {
    if (!head || m?.id === id) return;
    panel?.flush();
    viewing = { ...viewing, [head.id]: id };
    selected = null;
    drawing = false;
    fit();
  }

  /** Saves a change to the group and tells the parent; the viewer stays on the same page. */
  async function applyGroup(cmd: EditCommand) {
    error = '';
    try {
      await api.apply(cmd);
      onChanged?.();
    } catch (e) {
      error = errMessage(e, t('edit.wentWrong'));
    }
  }

  async function groupWith(picked: MediaDto) {
    picking = false;
    void keepFocusInside();
    if (!head) return;
    const cmd = groupCommand(head, picked);
    if (cmd) await applyGroup(cmd);
  }

  function makePrimary() {
    if (m && head && m.id !== head.id) void applyGroup({ type: 'groupMediaVersions', primary: m.id, others: [] });
  }

  function ungroupShown() {
    if (!m || !head) return;
    // The primary leaves: the largest of the rest is shown first from now on.
    const rest = versions.filter((v) => v.id !== m.id);
    const next = m.id === head.id ? bestPrimary(rest)?.id : undefined;
    void applyGroup({ type: 'ungroupMediaVersion', media: m.id, ...(next ? { newPrimary: next } : {}) });
  }
  const title = $derived(m ? (edits[m.id]?.title ?? m.title ?? '') : '');
  const description = $derived(m ? (edits[m.id]?.description ?? m.description ?? '') : '');
  /** Web pages this picture came from (http and https only), each with the host its button names. */
  const webPages = $derived(
    (m?.webLinks ?? []).flatMap((url) => {
      if (!/^https?:\/\//i.test(url)) return [];
      try {
        return [{ url, host: new URL(url).hostname.replace(/^www\./i, '') }];
      } catch {
        return [];
      }
    }),
  );
  async function openWebPage(url: string) {
    try {
      await platform.openUrl?.(url);
    } catch (e) {
      error = errMessage(e, t('edit.wentWrong'));
    }
  }
  const primary = $derived(!!link?.primary || (linkedId ? primaryIds.has(linkedId) : false));

  // The transcription: boxes on the picture and the full text beside it.
  /** Outlines of every piece are drawn; off, the pieces are still there to hover and tap, but not drawn. */
  let showText = $state(false);
  let editing = $state(false);
  let drawing = $state(false);
  let selected = $state<Id | null>(null);
  /** Pieces as shown: what was edited here, saved or on its way, until the parent hands over fresh items. */
  let saved = $state<Record<Id, TextRegionDto[]>>({});
  /** Pieces as the tree last stored them, as far as this viewer knows: what a failed save goes back to. */
  let stored: Record<Id, TextRegionDto[]> = {};
  /** Saves go one after another; a failure starts a new generation and drops the saves built on it. */
  let queue: Promise<void> = Promise.resolve();
  let generation = 0;
  let panel = $state<ReturnType<typeof FullTextPanel>>();
  /** How each picture is turned here: shown at once, until the parent hands over fresh items. */
  let turns = $state<Record<Id, Rotation>>({});
  $effect(() => {
    void items;
    saved = {};
    stored = {};
    turns = {};
  });
  const regions = $derived(m ? (saved[m.id] ?? m.transcription ?? []) : []);
  const picture = $derived(!!m && m.resolved && !isPdf(m));
  const placedAny = $derived(regions.some((r) => r.area));
  /** Degrees clockwise the picture is shown turned, on top of its EXIF orientation. */
  const turn = $derived<Rotation>(m ? (turns[m.id] ?? m.rotation ?? 0) : 0);
  const across = $derived(turn % 180 !== 0);
  /** Picture and boxes turn together, about the middle of the frame; panning is on screen, so it comes last. */
  const transform = $derived(`translate(${panX}px, ${panY}px) scale(${zoom}) rotate(${turn}deg)`);
  /** The box the picture fills, so the boxes on it line up whatever the stage's shape. */
  let naturalRatio = $state(4 / 3);
  const ratio = $derived(m?.width && m?.height ? m.width / m.height : naturalRatio);
  /** The frame takes the turned picture's shape, so a turned picture fits the stage too. */
  const frameRatio = $derived(across ? 1 / ratio : ratio);
  /**
   * Where the picture upright by EXIF sits in the frame before it is turned: centred, its width the frame's height
   * across a quarter turn. The pieces' boxes are placed in % of this same box.
   */
  const place = $derived(
    across
      ? `left: ${(100 - 100 * ratio) / 2}%; top: ${(100 - 100 / ratio) / 2}%; width: ${100 * ratio}%; height: ${100 / ratio}%;`
      : 'left: 0%; top: 0%; width: 100%; height: 100%;',
  );

  /**
   * Shows `next` at once and saves it after any save still on its way. It says which pieces it knew of, so a piece
   * an agent added meanwhile stays. When a save fails, the pieces go back to what was last stored, and edits made
   * on top of the failed one are not sent.
   */
  function saveRegions(next: TextRegionDto[]): Promise<void> {
    if (!m) return queue;
    const id = m.id;
    if (!(id in stored)) stored[id] = m.transcription ?? [];
    saved[id] = next;
    error = '';
    const gen = generation;
    queue = queue.then(async () => {
      if (gen !== generation) return;
      try {
        await api.apply({ type: 'setMediaTranscription', id, regions: toInput(next), known: stored[id].map((r) => r.id) });
        stored[id] = next;
        onChanged?.();
      } catch (e) {
        generation++;
        saved[id] = stored[id];
        error = errMessage(e, t('edit.wentWrong'));
      }
    });
    return queue;
  }

  /** Text typed and not yet saved is saved before the picture changes or the viewer closes. */
  function close() {
    panel?.flush();
    onClose();
  }

  function toggleEditing() {
    editing = !editing;
    if (!editing) drawing = false;
  }

  function go(to: number) {
    const next = Math.min(Math.max(to, 0), items.length - 1);
    if (next === at) return;
    panel?.flush();
    at = next;
    confirming = false;
    selected = null;
    drawing = false;
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

  /**
   * Turns the shown picture a quarter and saves it as an ordinary edit (undoable, in the history). The turned picture
   * is fitted to the stage. When the save fails, the turn goes back to what was last stored.
   */
  function turnPicture(by: 90 | -90): Promise<void> {
    if (!m || !picture) return queue;
    const id = m.id;
    const before = turn;
    const next = turnedBy(before, by);
    turns[id] = next;
    fit();
    error = '';
    queue = queue.then(async () => {
      try {
        await api.apply({ type: 'setMediaRotation', id, rotation: next });
        onChanged?.();
      } catch (e) {
        if (turns[id] === next) turns[id] = before;
        error = errMessage(e, t('edit.wentWrong'));
      }
    });
    return queue;
  }

  async function setPrimary() {
    if (!m || !linkedId || !m.resolved || isPdf(m) || target?.kind !== 'person') return;
    error = '';
    try {
      await api.apply({ type: 'setPrimaryMedia', person: target.id, media: linkedId }, { silent: true });
      primaryIds = new Set([...primaryIds, linkedId]);
      notices.show({ message: t('media.primarySet') });
      onChanged?.();
    } catch (e) {
      error = errMessage(e, t('edit.wentWrong'));
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
      if (wasOnly) close();
      else closeConfirm();
    } catch (e) {
      error = errMessage(e, t('edit.wentWrong'));
    } finally {
      busy = false;
    }
  }

  // The place picked or typed but not saved yet: typing alone must not make places.
  let pendingPlace = $state<{ placeId?: Id; placeText?: string } | null | undefined>(undefined);
  $effect(() => {
    void m?.id;
    pendingPlace = undefined;
  });

  async function savePlace() {
    if (!m || pendingPlace === undefined) return;
    const v = pendingPlace;
    error = '';
    try {
      const cmd: EditCommand = { type: 'updateMedia', id: m.id, ...(v?.placeId ? { placeId: v.placeId } : { placeText: v?.placeText ?? '' }) };
      await api.apply(cmd);
      pendingPlace = undefined;
      onChanged?.();
    } catch (e) {
      error = errMessage(e, t('edit.wentWrong'));
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
      error = errMessage(e, t('edit.wentWrong'));
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (picking) return;
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
      // Brackets, not R: R turns a piece of the transcription, and the arrows change the picture.
      case '[':
        void turnPicture(-90);
        break;
      case ']':
        void turnPicture(90);
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

<Dialog open title={m ? mediaLabel({ ...m, title: title || m.title }) : t('media.photo')} onclose={close}>
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
          <div class="frame" style="--ratio: {frameRatio}" data-turn={turn}>
            <img
              alt={mediaLabel(m)}
              src={api.mediaUrl(m.id, 'preview')}
              draggable="false"
              style="{place} transform: {transform}"
              onload={(e) => {
                const i = e.currentTarget as HTMLImageElement;
                if (i.naturalWidth && i.naturalHeight) naturalRatio = i.naturalWidth / i.naturalHeight;
              }}
            />
            {#if placedAny || (editing && drawing)}
              <TextOverlay
                {regions}
                {transform}
                {place}
                {turn}
                {editing}
                {drawing}
                outlines={showText || editing}
                {selected}
                onselect={(id) => (selected = id)}
                onchange={saveRegions}
                ondrawn={(id) => {
                  drawing = false;
                  void panel?.focusText(id);
                }}
                onleave={() => stage?.focus()}
              />
            {/if}
          </div>
        {/if}
      </div>
      {#if versions.length > 1}
        <div class="bar versions" role="group" aria-label={t('media.quality')} data-testid="version-switcher">
          <span>{t('media.quality')}</span>
          {#each versions as v, i (v.id)}
            {@const size = sizeText(v) ?? t('media.versionUnknownSize')}
            <Button aria-pressed={v.id === m.id} title={mediaLabel(v)} onclick={() => showVersion(v.id)}
              >{t(i === 0 ? 'media.versionPrimary' : 'media.versionOther', { size })}</Button
            >
          {/each}
        </div>
      {/if}
      {#if textElsewhere}
        <p class="hint" data-testid="version-text-hint">
          {t('media.versionOtherHasText')}
          <Button onclick={() => textElsewhere && showVersion(textElsewhere.id)}>{t('media.versionShowIt')}</Button>
        </p>
      {/if}
      {#if m.resolved && !(m.mime ?? '').startsWith('audio/')}
        <div class="bar" role="group" aria-label={t('media.fullText')}>
          {#if picture && placedAny}
            <Button aria-pressed={showText} onclick={() => (showText = !showText)}>{t('media.showText')}</Button>
          {/if}
          <Button aria-pressed={editing} onclick={toggleEditing}>{t('media.editText')}</Button>
          {#if picture}
            <Button
              aria-pressed={drawing}
              onclick={() => {
                drawing = !drawing;
                if (drawing) editing = true;
              }}>{t('media.drawPiece')}</Button
            >
            {#if drawing}<span>{t('media.drawHelp')}</span>{/if}
          {/if}
        </div>
      {/if}
      <div class="bar">
        <span aria-live="polite">{t('media.zoom', { pct: Math.round(zoom * 100) })}</span>
        <span>{t('media.position', { i: at + 1, n: items.length })}</span>
        {#if primary}<span>{t('media.primaryPhoto')}</span>{/if}
        <Button disabled={at === 0} onclick={() => go(at - 1)}>{t('media.previous')}</Button>
        <Button disabled={at >= items.length - 1} onclick={() => go(at + 1)}>{t('media.next')}</Button>
        {#if picture}
          <Button aria-keyshortcuts="[" title="{t('media.turnLeft')} ([)" onclick={() => turnPicture(-90)}>{t('media.turnLeft')}</Button>
          <Button aria-keyshortcuts="]" title="{t('media.turnRight')} (])" onclick={() => turnPicture(90)}>{t('media.turnRight')}</Button>
        {/if}
        {#if target?.kind === 'person' && m.resolved && !isPdf(m)}
          <Button onclick={setPrimary}>{t('media.setPrimary')}</Button>
        {/if}
        <Button onclick={() => openOriginal(api, m.id)}>{t('media.openOriginal')}</Button>
        {#if platform.openUrl}
          {#each webPages as page (page.url)}
            <Button title={page.url} onclick={() => openWebPage(page.url)}><Icon name="external-link" /> {t('media.openWebPage', { host: page.host })}</Button>
          {/each}
        {/if}
        <Button onclick={() => (picking = true)}>{t('media.groupVersions')}</Button>
        {#if versions.length > 1 && head && m.id !== head.id}
          <Button onclick={makePrimary}>{t('media.makePrimaryVersion')}</Button>
        {/if}
        {#if versions.length > 1}
          <Button onclick={ungroupShown}>{t('media.ungroupVersion')}</Button>
        {/if}
        {#if target}
          <Button disabled={busy} onclick={() => target && remove({ type: 'detachMedia', target, media: linkedId ?? m.id })}>{t(target.kind === 'source' ? 'media.unlinkSource' : 'media.unlink')}</Button>
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
      {#if m.resolved && (editing || regions.some((r) => r.text.trim()))}
        <FullTextPanel
          bind:this={panel}
          {regions}
          {editing}
          {turn}
          {selected}
          onselect={(id) => (selected = id)}
          onchange={saveRegions}
          onleave={() => stage?.focus()}
        />
      {/if}
      {#if picking}
        <VersionPicker
          exclude={versions.length > 1 ? versions.map((v) => v.id) : [m.id]}
          onpick={groupWith}
          onclose={() => {
            picking = false;
            void keepFocusInside();
          }}
        />
      {/if}
      {#key m.id}
        <TextField label={t('media.title')} value={title} onchange={(e) => saveField('title', e.currentTarget.value)} />
        <TextArea label={t('media.description')} value={description} rows={2} onchange={(e) => saveField('description', e.currentTarget.value)} />
        <div class="place">
          <PlacePicker label={t('media.place')} value={m.place ?? null} text="" onChange={(v) => (pendingPlace = v)} />
          {#if pendingPlace !== undefined}<Button onclick={savePlace}>{t('media.savePlace')}</Button>{/if}
        </div>
      {/key}
    </div>
  {/if}
</Dialog>

<style>
  .place {
    display: grid;
    gap: var(--s-2);
    justify-items: start;
  }
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
  .stage {
    container-type: size;
  }
  /* Exactly the picture's box, as large as the stage allows: boxes are placed in % of it. */
  .frame {
    position: relative;
    width: min(100cqw, calc(100cqh * var(--ratio)));
    aspect-ratio: var(--ratio);
  }
  /* Placed by `place`: the picture upright by EXIF, turned about the frame's middle. */
  img {
    display: block;
    position: absolute;
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
  .hint {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-3);
    margin: 0;
    font-size: var(--t-sm);
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
