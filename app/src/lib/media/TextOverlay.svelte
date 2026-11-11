<script lang="ts">
  import { tick } from 'svelte';
  import type { Id, TextAreaInput, TextRegionDto } from '../api/api';
  import { t } from '../i18n';
  import {
    KEY_STEP,
    areaBetween,
    cornerShownAt,
    moveArea,
    moveOnPicture,
    newPieceId,
    nextRotation,
    pointOnPicture,
    resizeArea,
    shownTurn,
    unturn,
    type Corner,
    type Rotation,
  } from './transcription';

  let {
    regions,
    transform,
    place = 'left: 0%; top: 0%; width: 100%; height: 100%;',
    turn = 0,
    editing = false,
    drawing = false,
    selected = null,
    onselect,
    onchange,
    ondrawn,
    onleave,
  }: {
    regions: TextRegionDto[];
    /** The CSS transform of the picture (zoom, pan and turn), so the boxes stay on the writing. */
    transform: string;
    /** Where the picture upright by EXIF sits in its frame, before the transform: the same as the picture's. */
    place?: string;
    /** Degrees clockwise the picture is shown turned. The pieces stay stored on the picture upright by EXIF. */
    turn?: Rotation;
    editing?: boolean;
    /** A drag across the picture marks a new piece. */
    drawing?: boolean;
    selected?: Id | null;
    onselect: (id: Id | null) => void;
    /** The whole new list; the viewer saves it as one change. */
    onchange: (regions: TextRegionDto[]) => void;
    /** A piece was marked: the viewer ends drawing and puts the cursor in its text. */
    ondrawn?: (id: Id) => void;
    /** The focused box went away: focus goes back to the picture. */
    onleave?: () => void;
  } = $props();

  const CORNERS: Corner[] = ['nw', 'ne', 'sw', 'se'];

  let layer = $state<HTMLElement>();
  let tipFor = $state<Id | null>(null);
  let tipAt = $state({ left: 0, top: 0 });
  /** The area of the piece being dragged, shown before it is saved on release. */
  let live = $state<{ id: Id; area: TextAreaInput } | null>(null);
  let drag = $state<{
    kind: 'move' | 'resize' | 'draw';
    id: Id;
    corner?: Corner;
    x: number;
    y: number;
    from: TextAreaInput;
    moved: boolean;
  } | null>(null);

  const placed = $derived(
    regions.map((r, i) => ({ r, i })).filter((p): p is { r: TextRegionDto & { area: TextAreaInput }; i: number } => !!p.r.area),
  );
  const tip = $derived(tipFor ? regions.find((r) => r.id === tipFor) : undefined);

  const pct = (v: number) => `${v * 100}%`;
  function areaOf(r: { id: Id; area: TextAreaInput }) {
    return live?.id === r.id ? live.area : r.area;
  }

  /** The turned picture's box on screen (with the zoom), to turn a pointer move into fractions of the picture. */
  function size() {
    const b = layer?.getBoundingClientRect();
    return { left: b?.left ?? 0, top: b?.top ?? 0, w: b?.width || 1, h: b?.height || 1 };
  }

  /** The corner of a box that shows at the top right of the screen, where its turn badge goes. */
  const badgeAt = $derived(cornerShownAt(turn, 'ne'));

  function showTip(id: Id, el: HTMLElement) {
    tipFor = id;
    const frame = layer?.parentElement?.getBoundingClientRect();
    const box = el.getBoundingClientRect();
    tipAt = { left: Math.max(0, box.left - (frame?.left ?? 0)), top: box.bottom - (frame?.top ?? 0) + 4 };
  }
  function hideTip(id: Id) {
    if (tipFor === id) tipFor = null;
  }

  function replaceArea(id: Id, area: TextAreaInput) {
    onchange(regions.map((r) => (r.id === id ? { ...r, area } : r)));
  }

  function start(e: PointerEvent, kind: 'move' | 'resize', r: { id: Id; area: TextAreaInput }, corner?: Corner) {
    if (!editing || e.button > 0) return;
    e.stopPropagation();
    // No text selection or native drag; that also keeps the browser from focusing the box, so it is done here:
    // after a click the arrow keys must move the box, not change the picture.
    e.preventDefault();
    focusBox(r.id);
    onselect(r.id);
    drag = { kind, id: r.id, corner, x: e.clientX, y: e.clientY, from: r.area, moved: false };
    layer?.setPointerCapture?.(e.pointerId);
  }

  function onlayerdown(e: PointerEvent) {
    if (!editing || !drawing || e.button > 0) return;
    e.stopPropagation();
    e.preventDefault();
    const { x, y } = pointOnPicture(turn, size(), e.clientX, e.clientY);
    drag = { kind: 'draw', id: newPieceId(), x: e.clientX, y: e.clientY, from: { x, y, w: 0, h: 0 }, moved: false };
    layer?.setPointerCapture?.(e.pointerId);
  }

  function onlayermove(e: PointerEvent) {
    if (!drag) return;
    if (e.clientX === drag.x && e.clientY === drag.y) return;
    drag.moved = true;
    const s = size();
    // The move on screen, turned back onto the picture as it is stored.
    const { dx, dy } = moveOnPicture(turn, s, e.clientX - drag.x, e.clientY - drag.y);
    const f = drag.from;
    const area =
      drag.kind === 'move'
        ? moveArea(f, dx, dy)
        : drag.kind === 'resize'
          ? resizeArea(f, drag.corner ?? 'se', dx, dy)
          : areaBetween(f.x, f.y, f.x + dx, f.y + dy);
    live = { id: drag.id, area };
  }

  function focusBox(id: Id) {
    layer?.querySelector<HTMLElement>(`[data-piece="${id}"]`)?.focus({ preventScroll: true });
  }

  /** The pointer was taken away (a touch turned into a scroll, the window lost it): nothing is saved. */
  function onlayercancel() {
    drag = null;
    live = null;
  }

  function onlayerup() {
    const d = drag;
    const area = live?.area;
    drag = null;
    live = null;
    if (!d || !d.moved || !area) return;
    if (d.kind === 'draw') {
      onchange([...regions, { id: d.id, area, rotation: 0, text: '' }]);
      onselect(d.id);
      ondrawn?.(d.id);
    } else {
      replaceArea(d.id, area);
    }
  }

  async function onboxkey(e: KeyboardEvent, r: { id: Id; area: TextAreaInput; rotation: TextRegionDto['rotation'] }) {
    if (!editing || e.ctrlKey || e.metaKey || e.altKey) return;
    const step = KEY_STEP;
    const arrows: Record<string, [number, number]> = {
      ArrowLeft: [-step, 0],
      ArrowRight: [step, 0],
      ArrowUp: [0, -step],
      ArrowDown: [0, step],
    };
    const d = arrows[e.key];
    if (d) {
      // Arrows go the way they point on screen, whatever the picture's turn; Shift moves the corner shown bottom right.
      const [dx, dy] = unturn(turn, d[0], d[1]);
      replaceArea(r.id, e.shiftKey ? resizeArea(r.area, cornerShownAt(turn, 'se'), dx, dy) : moveArea(r.area, dx, dy));
    } else if (e.key === 'r' || e.key === 'R') {
      onchange(regions.map((x) => (x.id === r.id ? { ...x, rotation: nextRotation(x.rotation) } : x)));
    } else if (e.key === 'Delete' || e.key === 'Backspace') {
      const at = placed.findIndex((p) => p.r.id === r.id);
      const next = placed[at + 1]?.r.id ?? placed[at - 1]?.r.id ?? null;
      tipFor = null;
      onchange(regions.filter((x) => x.id !== r.id));
      onselect(next);
      await tick();
      const el = next ? layer?.querySelector<HTMLElement>(`[data-piece="${next}"]`) : null;
      if (el) el.focus();
      else onleave?.();
    } else {
      return;
    }
    // Handled here: the viewer's own keys (arrows change the picture) must not see it.
    e.preventDefault();
    e.stopPropagation();
  }
</script>

<!-- The geometry is the picture's, so it never mirrors in a right-to-left language. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:this={layer}
  class="layer"
  class:drawing={editing && drawing}
  class:across={turn % 180 !== 0}
  dir="ltr"
  style="{place} transform: {transform}"
  onpointerdown={onlayerdown}
  onpointermove={onlayermove}
  onpointerup={onlayerup}
  onpointercancel={onlayercancel}
>
  {#each placed as { r, i } (r.id)}
    {@const a = areaOf(r)}
    {@const shown = shownTurn(r.rotation, turn)}
    <button
      type="button"
      class="box"
      class:selected={selected === r.id}
      data-piece={r.id}
      aria-label={t('media.piece', { i: i + 1 })}
      aria-pressed={selected === r.id}
      aria-describedby={[tipFor === r.id ? `tip-${r.id}` : '', editing ? 'box-help' : ''].filter(Boolean).join(' ') || undefined}
      style="left: {pct(a.x)}; top: {pct(a.y)}; width: {pct(a.w)}; height: {pct(a.h)}"
      onclick={() => {
        focusBox(r.id);
        onselect(r.id);
      }}
      onpointerenter={(e) => showTip(r.id, e.currentTarget)}
      onpointerleave={() => hideTip(r.id)}
      onfocus={(e) => showTip(r.id, e.currentTarget)}
      onblur={() => hideTip(r.id)}
      onpointerdown={(e) => start(e, 'move', r)}
      onkeydown={(e) => onboxkey(e, r)}
    >
      {#if shown}<span class="turn {badgeAt}" style="transform: rotate({-turn}deg) translateX(-100%)" aria-hidden="true">↻{shown}°</span>{/if}
      {#if editing && selected === r.id}
        {#each CORNERS as c (c)}
          <span class="handle {c}" aria-hidden="true" onpointerdown={(e) => start(e, 'resize', r, c)}></span>
        {/each}
      {/if}
    </button>
  {/each}
  {#if live && drag?.kind === 'draw'}
    <div class="box drawn" style="left: {pct(live.area.x)}; top: {pct(live.area.y)}; width: {pct(live.area.w)}; height: {pct(live.area.h)}"></div>
  {/if}
</div>
{#if editing}<p id="box-help" class="sr-only">{t('media.boxHelp')}</p>{/if}
{#if tip && !live}
  <div id="tip-{tip.id}" class="tip" role="tooltip" dir="auto" style="left: {tipAt.left}px; top: {tipAt.top}px">
    {tip.text.trim() || t('media.pieceEmpty')}
    {#if shownTurn(tip.rotation, turn)}<small>{t('media.turned', { deg: shownTurn(tip.rotation, turn) })}</small>{/if}
  </div>
{/if}

<style>
  .layer {
    position: absolute;
    transform-origin: center;
  }
  .layer.drawing {
    cursor: crosshair;
  }
  .box {
    position: absolute;
    margin: 0;
    padding: 0;
    border: 2px solid var(--focus);
    border-radius: 2px;
    background: color-mix(in srgb, var(--focus) 12%, transparent);
    cursor: pointer;
    min-width: 0;
    min-height: 0;
  }
  .box:hover,
  .box:focus-visible {
    background: color-mix(in srgb, var(--focus) 24%, transparent);
    outline: 2px solid var(--paper-0);
    outline-offset: 0;
  }
  .box.selected {
    border-style: dashed;
    border-color: var(--danger);
    background: color-mix(in srgb, var(--danger) 14%, transparent);
  }
  .box.drawn {
    pointer-events: none;
    border-style: dotted;
  }
  .turn {
    position: absolute;
    padding: 0 2px;
    font-size: 10px;
    line-height: 1.2;
    color: var(--paper-0);
    background: var(--focus);
  }
  /* The badge hangs from the box corner that shows top right, inside the box, and reads upright: its own top left is
     pinned to that corner, turned back by the picture's turn, then shifted its width to the left. */
  .turn {
    transform-origin: 0 0;
    white-space: nowrap;
  }
  .turn.ne {
    left: 100%;
    top: 0;
  }
  .turn.nw {
    left: 0;
    top: 0;
  }
  .turn.se {
    left: 100%;
    top: 100%;
  }
  .turn.sw {
    left: 0;
    top: 100%;
  }
  .handle {
    position: absolute;
    width: 10px;
    height: 10px;
    background: var(--paper-0);
    border: 2px solid var(--danger);
    touch-action: none;
  }
  .nw {
    left: -6px;
    top: -6px;
    cursor: nwse-resize;
  }
  .ne {
    right: -6px;
    top: -6px;
    cursor: nesw-resize;
  }
  .sw {
    left: -6px;
    bottom: -6px;
    cursor: nesw-resize;
  }
  .se {
    right: -6px;
    bottom: -6px;
    cursor: nwse-resize;
  }
  /* A quarter turn swaps the diagonals the corners drag along. */
  .across .nw,
  .across .se {
    cursor: nesw-resize;
  }
  .across .ne,
  .across .sw {
    cursor: nwse-resize;
  }
  .tip {
    position: absolute;
    z-index: 2;
    max-width: min(28rem, 90%);
    padding: var(--s-2) var(--s-3);
    white-space: pre-wrap;
    font-size: var(--t-sm);
    color: var(--ink-0);
    background: var(--paper-0);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    box-shadow: var(--shadow-pop);
    pointer-events: none;
  }
  .tip small {
    display: block;
    color: var(--ink-1);
  }
</style>
