<script lang="ts">
  import { tick, type Snippet } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { t } from '../i18n';
  import IconButton from '../ui/IconButton.svelte';
  import { DEFAULT_BOX, cardName, fitBox, type TraceBox, type TraceDraft } from './model';

  interface Props {
    imageUrl: string;
    imgW: number;
    imgH: number;
    draft: TraceDraft;
    selectedId: string | null;
    /** Set while the user is choosing the second card of a link. */
    linking: boolean;
    onmark: (box: TraceBox) => void;
    onselect: (id: string | null) => void;
    onpick: (id: string) => void;
    ondelete: (id: string) => void;
    /** The person form; it is placed next to the selected card. */
    popover?: Snippet;
    /** Lets keyboard users jump past the cards to the review panel. */
    onskip?: () => void;
  }
  let { imageUrl, imgW, imgH, draft = $bindable(), selectedId, linking, onmark, onselect, onpick, ondelete, popover, onskip }: Props = $props();

  const MIN_ZOOM = 0.05;
  const MAX_ZOOM = 8;

  let viewport = $state<HTMLElement>();
  let vw = $state(0);
  let vh = $state(0);
  let z = $state(1);
  let tx = $state(0);
  let ty = $state(0);
  let tool = $state<'mark' | 'pan'>('mark');
  let fitted = false;
  /** Rubber band while drawing, in picture pixels. */
  let band = $state<{ x0: number; y0: number; x1: number; y1: number } | null>(null);

  const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));

  export function fit() {
    if (!vw || !vh || !imgW || !imgH) {
      z = 1;
      tx = 0;
      ty = 0;
      return;
    }
    z = clamp(Math.min(vw / imgW, vh / imgH), MIN_ZOOM, MAX_ZOOM);
    tx = (vw - imgW * z) / 2;
    ty = (vh - imgH * z) / 2;
  }

  $effect(() => {
    if (!viewport || typeof ResizeObserver === 'undefined') return;
    const el = viewport;
    const measure = (w: number, h: number) => {
      vw = Math.round(w);
      vh = Math.round(h);
    };
    measure(el.clientWidth, el.clientHeight);
    const ro = new ResizeObserver((entries) => {
      const r = entries[0]?.contentRect;
      if (r) measure(r.width, r.height);
    });
    ro.observe(el);
    return () => ro.disconnect();
  });

  $effect(() => {
    // Fit once, as soon as the viewport has a size.
    if (!fitted && vw > 0 && vh > 0) {
      fitted = true;
      fit();
    }
  });

  function zoomAt(factor: number, cx = vw / 2, cy = vh / 2) {
    const nz = clamp(z * factor, MIN_ZOOM, MAX_ZOOM);
    tx = cx - ((cx - tx) * nz) / z;
    ty = cy - ((cy - ty) * nz) / z;
    z = nz;
  }

  $effect(() => {
    const el = viewport;
    if (!el) return;
    // Not passive: the wheel zooms the picture instead of scrolling the page.
    const onwheel = (e: WheelEvent) => {
      e.preventDefault();
      const r = el.getBoundingClientRect();
      zoomAt(e.deltaY < 0 ? 1.15 : 1 / 1.15, e.clientX - r.left, e.clientY - r.top);
    };
    el.addEventListener('wheel', onwheel, { passive: false });
    return () => el.removeEventListener('wheel', onwheel);
  });

  /** Screen point to picture pixels. */
  function toPicture(clientX: number, clientY: number) {
    const r = viewport?.getBoundingClientRect();
    return { x: (clientX - (r?.left ?? 0) - tx) / z, y: (clientY - (r?.top ?? 0) - ty) / z };
  }

  function track(onmove: (e: MouseEvent) => void, onend: (e: MouseEvent) => void) {
    const move = (e: MouseEvent) => onmove(e);
    const up = (e: MouseEvent) => {
      window.removeEventListener('mousemove', move);
      window.removeEventListener('mouseup', up);
      onend(e);
    };
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', up);
  }

  function backgroundDown(e: MouseEvent) {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest('.card, .pop, .bar')) return;
    e.preventDefault();
    viewport?.focus();
    if (linking) return;
    if (tool === 'pan') {
      let lx = e.clientX;
      let ly = e.clientY;
      track(
        (m) => {
          tx += m.clientX - lx;
          ty += m.clientY - ly;
          lx = m.clientX;
          ly = m.clientY;
        },
        () => {},
      );
      return;
    }
    const s = toPicture(e.clientX, e.clientY);
    band = { x0: s.x, y0: s.y, x1: s.x, y1: s.y };
    track(
      (m) => {
        const p = toPicture(m.clientX, m.clientY);
        band = band && { ...band, x1: p.x, y1: p.y };
      },
      (m) => {
        const p = toPicture(m.clientX, m.clientY);
        const b = band;
        band = null;
        if (!b) return;
        const dx = Math.abs(p.x - b.x0) * z;
        const dy = Math.abs(p.y - b.y0) * z;
        if (dx < 4 && dy < 4) {
          // A click places a marker with the usual size, centred on the click.
          onmark(fitBox({ x: p.x / imgW - DEFAULT_BOX.w / 2, y: p.y / imgH - DEFAULT_BOX.h / 2, ...DEFAULT_BOX }));
        } else {
          onmark(fitBox({ x: Math.min(b.x0, p.x) / imgW, y: Math.min(b.y0, p.y) / imgH, w: dx / z / imgW, h: dy / z / imgH }));
        }
      },
    );
  }

  /** The "Add a person" button: a marker in the middle of what is in view. */
  export function markCenter() {
    const c = toPicture((viewport?.getBoundingClientRect().left ?? 0) + vw / 2, (viewport?.getBoundingClientRect().top ?? 0) + vh / 2);
    onmark(fitBox({ x: c.x / imgW - DEFAULT_BOX.w / 2, y: c.y / imgH - DEFAULT_BOX.h / 2, ...DEFAULT_BOX }));
  }

  function cardDown(e: MouseEvent, id: string) {
    if (e.button !== 0) return;
    e.stopPropagation();
    if (linking) return; // the click handler picks it
    const c = draft.cards.find((x) => x.id === id);
    if (!c) return;
    onselect(id);
    const sx = e.clientX;
    const sy = e.clientY;
    const ox = c.x;
    const oy = c.y;
    track(
      (m) => {
        const next = fitBox({ x: ox + (m.clientX - sx) / z / imgW, y: oy + (m.clientY - sy) / z / imgH, w: c.w, h: c.h });
        c.x = next.x;
        c.y = next.y;
      },
      () => {},
    );
  }

  function cardKey(e: KeyboardEvent, id: string) {
    const c = draft.cards.find((x) => x.id === id);
    if (!c) return;
    const step = e.shiftKey ? 0.02 : 0.004;
    const move = (dx: number, dy: number) => {
      const next = fitBox({ x: c.x + dx, y: c.y + dy, w: c.w, h: c.h });
      c.x = next.x;
      c.y = next.y;
      e.preventDefault();
    };
    if (e.key === 'ArrowLeft') move(-step, 0);
    else if (e.key === 'ArrowRight') move(step, 0);
    else if (e.key === 'ArrowUp') move(0, -step);
    else if (e.key === 'ArrowDown') move(0, step);
    else if (e.key === 'Delete') {
      e.preventDefault();
      ondelete(id);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onselect(null);
    }
  }

  function viewportKey(e: KeyboardEvent) {
    if (e.target !== e.currentTarget) return;
    const pan = e.shiftKey ? 200 : 60;
    if (e.key === '+' || e.key === '=') zoomAt(1.25);
    else if (e.key === '-') zoomAt(1 / 1.25);
    else if (e.key === '0') fit();
    else if (e.key === 'ArrowLeft') tx += pan;
    else if (e.key === 'ArrowRight') tx -= pan;
    else if (e.key === 'ArrowUp') ty += pan;
    else if (e.key === 'ArrowDown') ty -= pan;
    else return;
    e.preventDefault();
  }

  let stage = $state<HTMLElement>();
  let shownFor: string | null = null;
  let lastIds: string[] = [];

  // A keyboard user must not have to Tab through every card to reach the form: when a card is selected,
  // focus moves to the form's first field; when the form closes, focus goes back to that card.
  $effect(() => {
    const id = selectedId;
    const ids = draft.cards.map((c) => c.id);
    const before = lastIds;
    lastIds = ids;
    if (id === shownFor) return;
    const was = shownFor;
    shownFor = id;
    void tick().then(() => {
      if (id) {
        const first = viewport?.querySelector<HTMLElement>('.pop input, .pop select, .pop textarea, .pop button');
        first?.focus();
      } else if (was) {
        // The form is gone, so focus fell back to the page: put it on the card.
        const active = document.activeElement;
        if (active && active !== document.body) return;
        const find = (card: string) => stage?.querySelector<HTMLElement>(`[data-card="${card}"]`);
        const kept = find(was);
        if (kept) {
          kept.focus();
          return;
        }
        // The card itself was deleted: go to the next card, else the one before it, else the picture.
        const at = before.indexOf(was);
        const next = [...before.slice(at + 1), ...before.slice(0, Math.max(at, 0)).reverse()].find((c) => ids.includes(c));
        const target = next ? find(next) : null;
        if (target) target.focus();
        else viewport?.focus();
      }
    });
  });

  const selected = $derived(draft.cards.find((c) => c.id === selectedId) ?? null);
  const popStyle = $derived.by(() => {
    if (!selected) return '';
    const right = tx + (selected.x + selected.w) * imgW * z + 12;
    const left = right + 300 > vw ? Math.max(8, tx + selected.x * imgW * z - 312) : right;
    const top = clamp(ty + selected.y * imgH * z, 8, Math.max(8, vh - 380));
    return `left:${left}px;top:${top}px`;
  });
  const center = (id: string) => {
    const c = draft.cards.find((x) => x.id === id);
    return c ? { x: (c.x + c.w / 2) * imgW, y: (c.y + c.h / 2) * imgH } : { x: 0, y: 0 };
  };
</script>

<div class="canvas">
  <div class="bar" role="toolbar" aria-label={t('trace.pictureTools')}>
    <div class="group" role="group" aria-label={t('trace.tool')}>
      <button type="button" class="tool" aria-pressed={tool === 'mark'} onclick={() => (tool = 'mark')}>{t('trace.toolMark')}</button>
      <button type="button" class="tool" aria-pressed={tool === 'pan'} onclick={() => (tool = 'pan')}>{t('trace.toolPan')}</button>
    </div>
    <button type="button" class="tool add" onclick={() => markCenter()}>
      <Icon name="user-plus" /> {t('trace.addPerson')}
    </button>
    <div class="group" role="group" aria-label={t('trace.zoom')}>
      <IconButton label={t('chartCanvas.zoomOut')} icon="minus" onclick={() => zoomAt(1 / 1.25)} />
      <span class="pct tnum" aria-live="polite">{Math.round(z * 100)}%</span>
      <IconButton label={t('chartCanvas.zoomIn')} icon="plus" onclick={() => zoomAt(1.25)} />
      <button type="button" class="tool" onclick={() => fit()}>{t('chartCanvas.fit')}</button>
    </div>
  </div>

  <button type="button" class="skip" onclick={() => onskip?.()}>{t('trace.skip')}</button>

  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    bind:this={viewport}
    class="viewport"
    class:pan={tool === 'pan'}
    class:linking
    role="application"
    tabindex="0"
    aria-label={t('trace.canvasLabel')}
    onmousedown={backgroundDown}
    onkeydown={viewportKey}
  >
    <div class="stage" bind:this={stage} style="width:{imgW}px;height:{imgH}px;transform:translate({tx}px,{ty}px) scale({z})">
      <img src={imageUrl} alt={t('trace.imageAlt')} width={imgW} height={imgH} draggable="false" />

      <svg class="links" width={imgW} height={imgH} viewBox="0 0 {imgW} {imgH}" aria-hidden="true">
        <defs>
          <marker id="trace-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
            <path class="head" d="M0 0 10 5 0 10z" />
          </marker>
        </defs>
        {#each draft.links as l (l.id)}
          {@const a = center(l.from)}
          {@const b = center(l.to)}
          <line
            class="link {l.kind}"
            class:suggested={l.suggested}
            x1={a.x}
            y1={a.y}
            x2={b.x}
            y2={b.y}
            marker-end={l.kind === 'parent' ? 'url(#trace-arrow)' : undefined}
            vector-effect="non-scaling-stroke"
          />
        {/each}
      </svg>

      {#each draft.cards as c (c.id)}
        <button
          type="button"
          class="card"
          data-card={c.id}
          class:selected={c.id === selectedId}
          class:suggested={c.suggested}
          class:matched={!!c.existing}
          style="left:{c.x * imgW}px;top:{c.y * imgH}px;width:{c.w * imgW}px;height:{c.h * imgH}px"
          aria-label="{cardName(c) || t('trace.unnamedPerson')}{c.suggested ? t('trace.cardSuggestion') : ''}{c.existing ? t('trace.cardInTree') : ''}"
          aria-pressed={c.id === selectedId}
          onmousedown={(e) => cardDown(e, c.id)}
          onclick={() => (linking ? onpick(c.id) : onselect(c.id))}
          onkeydown={(e) => cardKey(e, c.id)}
        >
          <span class="name">{cardName(c) || t('trace.newPerson')}</span>
        </button>
      {/each}

      {#if band}
        <div
          class="band"
          style="left:{Math.min(band.x0, band.x1)}px;top:{Math.min(band.y0, band.y1)}px;width:{Math.abs(band.x1 - band.x0)}px;height:{Math.abs(band.y1 - band.y0)}px"
        ></div>
      {/if}
    </div>

    {#if selected && popover}
      <div class="pop" style={popStyle}>{@render popover()}</div>
    {/if}
  </div>
</div>

<style>
  .canvas {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background: var(--paper-2);
    overflow: hidden;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-4);
    padding: var(--s-3) var(--s-4);
    border-block-end: 1px solid var(--rule);
    background: var(--paper-1);
  }
  .group {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
  }
  .tool {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    min-height: 2rem;
    padding: 0 var(--s-4);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background: var(--paper-1);
    color: var(--ink-0);
    cursor: pointer;
  }
  .tool[aria-pressed='true'] {
    background: var(--ribbon);
    border-color: var(--ribbon);
    color: var(--paper-0);
  }
  .canvas {
    position: relative;
  }
  .skip {
    position: absolute;
    inset-inline-start: var(--s-3);
    inset-block-start: var(--s-3);
    z-index: 5;
    padding: var(--s-2) var(--s-4);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background: var(--paper-0);
    color: var(--ink-0);
    font: inherit;
    clip-path: inset(50%);
    width: 1px;
    height: 1px;
    overflow: hidden;
    white-space: nowrap;
  }
  .skip:focus {
    clip-path: none;
    width: auto;
    height: auto;
    outline: 2px solid var(--focus, var(--ribbon));
    outline-offset: 2px;
  }
  .tool:focus-visible,
  .viewport:focus-visible,
  .card:focus-visible {
    outline: 2px solid var(--focus, var(--ribbon));
    outline-offset: 2px;
  }
  .pct {
    min-width: 3.5rem;
    text-align: center;
  }
  .viewport {
    position: relative;
    overflow: hidden;
    cursor: crosshair;
    user-select: none;
    min-height: 0;
  }
  .viewport.pan {
    cursor: grab;
  }
  .viewport.linking {
    cursor: pointer;
  }
  .stage {
    position: absolute;
    left: 0;
    top: 0;
    transform-origin: 0 0;
  }
  .stage img {
    display: block;
    max-width: none;
    pointer-events: none;
  }
  .links {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .link {
    stroke-width: 3;
    stroke: var(--ink-1);
    fill: none;
  }
  .head {
    fill: var(--ink-1);
  }
  .link.partner {
    stroke: var(--ribbon);
    stroke-width: 4;
  }
  .link.suggested {
    stroke-dasharray: 8 6;
  }
  .card {
    position: absolute;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: 2px solid var(--ribbon);
    border-radius: 3px;
    background: color-mix(in srgb, var(--paper-1) 72%, transparent);
    color: var(--ink-0);
    font: inherit;
    cursor: move;
    overflow: hidden;
  }
  .card .name {
    font-size: max(10px, 0.9em);
    line-height: 1.1;
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    padding: 0 2px;
  }
  .card.matched {
    border-style: double;
    border-width: 4px;
  }
  .card.suggested {
    border-style: dashed;
    background: color-mix(in srgb, var(--paper-1) 45%, transparent);
  }
  .card.selected {
    box-shadow: 0 0 0 3px var(--paper-0), 0 0 0 5px var(--ribbon);
  }
  .viewport.linking .card {
    cursor: pointer;
    outline: 2px dotted var(--ink-0);
  }
  .band {
    position: absolute;
    border: 2px dashed var(--ribbon);
    background: color-mix(in srgb, var(--ribbon) 15%, transparent);
    pointer-events: none;
  }
  .pop {
    position: absolute;
    z-index: 5;
    width: 18.5rem;
    max-height: calc(100% - 16px);
    overflow: auto;
    padding: var(--s-4);
    border: 1px solid var(--ink-2);
    border-radius: var(--r-1);
    background: var(--paper-1);
    box-shadow: 0 6px 24px rgb(0 0 0 / 0.18);
    cursor: default;
  }
</style>
