<script lang="ts">
  import { getApi } from '../api/context';
  import { settings } from '../state/settings.svelte';
  import { t } from '../i18n';
  import Icon from './Icon.svelte';
  import { DIRECTION_LABEL, choicesFor, directionOf, type ChartDirection, type DirectionKey } from '../chart/direction';

  /**
   * The small four-way choice of the side the ancestors are drawn on, for one kind of chart. It is a native radio group:
   * the arrow keys move between the choices, a screen reader says "1 of 4", and Tab leaves the group.
   * The family view only has top and bottom.
   */
  let { chart }: { chart: DirectionKey } = $props();

  const api = getApi();
  const uid = $props.id();
  const choices = $derived(choicesFor(chart));
  const current = $derived(directionOf(settings.value.chartDirections, chart));

  // The icon is the tree with its ancestors on the choice's side: ancestors on top, turned a quarter or half.
  const turn: Record<ChartDirection, number> = { top: 0, bottom: 180, left: -90, right: 90 };

  /** Remembers the choice for this kind of chart; every view of it follows. */
  async function choose(d: ChartDirection): Promise<void> {
    if (d === current) return;
    try {
      await settings.update(api, { chartDirections: { ...settings.value.chartDirections, [chart]: d } });
    } catch {
      // The setting stays as it was; the group keeps showing it.
    }
  }
</script>

<div class="dir" role="radiogroup" aria-label={t('tree.direction')} data-testid="direction-control">
  {#each choices as d (d)}
    <label class:on={current === d} title={t(DIRECTION_LABEL[d])}>
      <input
        type="radio"
        name="{uid}-dir"
        value={d}
        checked={current === d}
        aria-label={t(DIRECTION_LABEL[d])}
        data-direction={d}
        onchange={() => void choose(d)}
      />
      <span class="ic" style:transform="rotate({turn[d]}deg)"><Icon name="tree-normal" /></span>
    </label>
  {/each}
</div>

<style>
  .dir {
    display: inline-flex;
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    overflow: hidden;
    vertical-align: middle;
  }
  label {
    position: relative;
    display: inline-grid;
    place-items: center;
    width: 2.25rem;
    height: calc(2.25rem - 2px);
    background: var(--paper-1);
    color: var(--ink-1);
    cursor: pointer;
  }
  label + label {
    border-inline-start: 1px solid var(--rule);
  }
  label:hover:not(.on) {
    background: var(--paper-2);
    color: var(--ink-0);
  }
  label.on {
    background: var(--ribbon-wash, var(--paper-2));
    color: var(--ribbon, var(--ink-0));
    box-shadow: inset 0 -2px 0 var(--ribbon, var(--ink-2));
  }
  .ic {
    display: inline-grid;
    place-items: center;
  }
  input {
    position: absolute;
    inset: 0;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  label:has(input:focus-visible) {
    outline: 2px solid var(--ribbon, var(--ink-1));
    outline-offset: -2px;
  }
</style>
