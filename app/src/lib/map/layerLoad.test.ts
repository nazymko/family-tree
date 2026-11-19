// Which old-map pictures are loaded and which are let go: hidden and off-screen maps ask for nothing, at most a few
// originals, and pictures not wanted for a while are dropped. Invented sheets only.
import { describe, expect, it } from 'vitest';
import type { Id, MapLayerDto } from '../api/types';
import { KEEP_FULL_MS, KEEP_PREVIEW_MS, MAX_FULL, planLoads, type Held } from './layerLoad';
import { mercator, type Camera } from './projection';

/** A sheet 0.5 degree square with its top left at (lat, lon). */
function sheet(id: string, lat: number, lon: number, visible = true, placed = true): MapLayerDto {
  const c = (la: number, lo: number) => ({ latE6: Math.round(la * 1e6), lonE6: Math.round(lo * 1e6) });
  return { media: id, title: id, series: 'S', year: null, source: null, corners: placed ? [c(lat, lon), c(lat, lon + 0.5), c(lat - 0.5, lon + 0.5), c(lat - 0.5, lon)] : null, opacity: 70, visible, pos: 0, width: 10000, height: 9000 };
}

function camAt(lat: number, lon: number, zoom: number): Camera {
  const w = mercator(lat, lon);
  return { cx: w.x, cy: w.y, zoom };
}

const W = 1000;
const H = 700;
const held = (entries: [Id, Partial<Held>][]) => new Map<Id, Held>(entries.map(([id, h]) => [id, { preview: true, previewSide: 1600, full: false, seen: 0, fullSeen: 0, ...h }]));

describe('old map pictures', () => {
  it('asks_only_for_shown_placed_maps_near_the_view', () => {
    const layers = [sheet('shown', 50.5, 30.0), sheet('hidden', 50.5, 30.5, false), sheet('unplaced', 0, 0, true, false), sheet('far', 10, 10)];
    const p = planLoads({ layers, placing: null, cam: camAt(50.25, 30.5, 8), w: W, h: H, scale: 1, held: new Map(), now: 0 });
    expect([...p.wanted]).toEqual(['shown']);
    expect(p.preview).toEqual(['shown']);
    expect(p.full).toEqual([]);
  });

  it('a_map_being_placed_is_loaded_even_when_hidden', () => {
    const l = sheet('hidden', 50.5, 30.0, false);
    const p = planLoads({ layers: [l], placing: { media: 'hidden', corners: l.corners! }, cam: camAt(50.25, 30.25, 8), w: W, h: H, scale: 1, held: new Map(), now: 0 });
    expect(p.preview).toEqual(['hidden']);
  });

  it('a_map_just_past_the_edge_counts_as_near_so_a_short_pan_finds_it_loaded', () => {
    // At zoom 8 half a degree of longitude is about 180 px; the sheet starts 250 px right of the view's edge.
    const cam = camAt(50.25, 30.0, 8);
    const right = (W / 2 + 250) / (256 * 2 ** 8);
    const lon = 30.0 + right * 360;
    const p = planLoads({ layers: [sheet('near', 50.5, lon), sheet('away', 50.5, lon + 3)], placing: null, cam, w: W, h: H, scale: 1, held: new Map(), now: 0 });
    expect([...p.wanted]).toEqual(['near']);
  });

  it('loads_originals_only_for_the_largest_few_on_screen_and_never_while_moving', () => {
    // Zoomed in where six sheets meet the view: all want more than the preview.
    const layers = [0, 1, 2].flatMap((i) => [sheet(`a${i}`, 50.5, 30 + i * 0.05), sheet(`b${i}`, 50.0, 30 + i * 0.05)]);
    const h = held(layers.map((l) => [l.media, {}]));
    const p = planLoads({ layers, placing: null, cam: camAt(50.25, 30.1, 13), w: W, h: H, scale: 1, held: h, now: 0 });
    expect(p.wantedFull.size).toBe(MAX_FULL);
    expect(p.full).toHaveLength(MAX_FULL);
    expect(planLoads({ layers, placing: null, cam: camAt(50.25, 30.1, 13), w: W, h: H, scale: 1, held: h, now: 0, moving: true }).full).toEqual([]);
    // Zoomed out the preview is enough.
    expect(planLoads({ layers, placing: null, cam: camAt(50.25, 30.1, 6), w: W, h: H, scale: 1, held: h, now: 0 }).wantedFull.size).toBe(0);
  });

  it('lets_go_of_pictures_not_wanted_for_a_while_and_keeps_originals_within_the_cap', () => {
    const layers = [sheet('on', 50.5, 30.0), sheet('gone', 50.5, 30.5, false)];
    const cam = camAt(50.25, 30.25, 7);
    const recent = held([
      ['on', { seen: 1000 }],
      ['gone', { seen: 1000, full: true, fullSeen: 1000 }],
    ]);
    let p = planLoads({ layers, placing: null, cam, w: W, h: H, scale: 1, held: recent, now: 1000 + 1000 });
    expect(p.drop).toEqual([]);
    expect(p.dropFull).toEqual([]);
    p = planLoads({ layers, placing: null, cam, w: W, h: H, scale: 1, held: recent, now: 1000 + KEEP_FULL_MS + 1 });
    expect(p.dropFull).toEqual(['gone']);
    p = planLoads({ layers, placing: null, cam, w: W, h: H, scale: 1, held: recent, now: 1000 + KEEP_PREVIEW_MS + 1 });
    expect(p.drop).toEqual(['gone']);
    // A shown map near the view is never let go.
    expect(p.drop).not.toContain('on');

    // Originals held past the cap (just unwanted) go first, oldest first, before new ones arrive.
    const zoomed = [0, 1, 2].map((i) => sheet(`z${i}`, 50.5, 30 + i * 0.02));
    const many = held([
      ['old1', { full: true, fullSeen: 10, seen: 10 }],
      ['old2', { full: true, fullSeen: 20, seen: 20 }],
      ...zoomed.map((l): [Id, Partial<Held>] => [l.media, { seen: 30 }]),
    ]);
    p = planLoads({ layers: zoomed, placing: null, cam: camAt(50.25, 30.02, 13), w: W, h: H, scale: 1, held: many, now: 40 });
    expect(p.full).toHaveLength(MAX_FULL);
    expect(p.dropFull).toEqual(['old1', 'old2']);
  });
});
