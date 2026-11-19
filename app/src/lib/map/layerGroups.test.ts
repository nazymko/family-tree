// Choosing which old maps are shown: series groups, the three states of a group, one map alone, the filter and the maps
// under a point. Invented sheets only.
import { describe, expect, it } from 'vitest';
import type { MapLayerDto } from '../api/types';
import { groupLayers, isNoChange, layersUnder, matches, onlyThis, seriesKey, setAll, toggleGroup, triState } from './layerGroups';

let n = 0;
function layer(series: string | null, visible: boolean, title = `Sheet ${++n}`, corners: [number, number, number, number] | null = null): MapLayerDto {
  const c = corners && [
    { latE6: corners[0] * 1e6, lonE6: corners[1] * 1e6 },
    { latE6: corners[0] * 1e6, lonE6: corners[3] * 1e6 },
    { latE6: corners[2] * 1e6, lonE6: corners[3] * 1e6 },
    { latE6: corners[2] * 1e6, lonE6: corners[1] * 1e6 },
  ];
  return { media: `m${n}-${title}`, title, series, year: '1865', source: null, corners: c, opacity: 70, visible, pos: n, width: 100, height: 80 };
}

describe('old map groups', () => {
  it('groups_by_series_in_the_order_of_their_top_map_and_puts_maps_without_a_series_last', () => {
    const top = [layer('Three-verst map', true), layer(null, false), layer('Schubert', false), layer(' three-verst  MAP ', false), layer('Schubert', true)];
    const g = groupLayers(top);
    expect(g.map((x) => x.series)).toEqual(['Three-verst map', 'Schubert', null]);
    expect(g.map((x) => x.layers.length)).toEqual([2, 2, 1]);
    expect(g.map((x) => x.shown)).toEqual([1, 1, 0]);
    // Each group keeps the top map first.
    expect(g[0].layers[0]).toBe(top[0]);
    expect(g[0].layers[1]).toBe(top[3]);
    expect(seriesKey('  Two-verst   map ')).toBe('two-verst map');
  });

  it('a_group_is_all_shown_none_or_partly_and_a_click_shows_a_partial_group_in_full', () => {
    const some = [layer('S', true), layer('S', false), layer('S', false)];
    expect(triState(some)).toBe('some');
    const c = toggleGroup(some);
    expect(c).toEqual({ shown: [some[1].media, some[2].media], hidden: [] });
    const all = some.map((l) => ({ ...l, visible: true }));
    expect(triState(all)).toBe('all');
    expect(toggleGroup(all)).toEqual({ shown: [], hidden: all.map((l) => l.media) });
    expect(triState(all.map((l) => ({ ...l, visible: false })))).toBe('none');
    // Maps already so are not asked for again.
    expect(isNoChange(setAll(all, true))).toBe(true);
  });

  it('only_this_shows_the_map_and_hides_every_other_shown_one_in_one_change', () => {
    const top = [layer('A', true), layer('B', true), layer('B', false), layer(null, true)];
    expect(onlyThis(top, top[2].media)).toEqual({ shown: [top[2].media], hidden: [top[0].media, top[1].media, top[3].media] });
    // Already shown: only the others are hidden.
    expect(onlyThis(top, top[0].media)).toEqual({ shown: [], hidden: [top[1].media, top[3].media] });
  });

  it('the_filter_finds_maps_by_title_series_year_or_sheet_number_in_any_case', () => {
    const a = layer('Three-verst map', true, 'Sheet 30-13');
    const b = layer('Schubert', true, 'Kyiv governorate');
    expect(matches(a, '30-13')).toBe(true);
    expect(matches(a, 'three sheet')).toBe(true);
    expect(matches(b, '30-13')).toBe(false);
    expect(matches(b, '1865 KYIV')).toBe(true);
    expect(groupLayers([a, b], 'kyiv').map((g) => g.series)).toEqual(['Schubert']);
    expect(groupLayers([a, b], 'nowhere')).toEqual([]);
  });

  it('lists_the_placed_maps_under_a_point', () => {
    const here = layer('A', false, 'Here', [50.6, 30.0, 49.9, 31.2]);
    const there = layer('A', true, 'There', [48.0, 24.0, 47.0, 25.0]);
    const unplaced = layer('A', true, 'Unplaced');
    expect(layersUnder([here, there, unplaced], 50.2, 30.5).map((l) => l.title)).toEqual(['Here']);
    expect(layersUnder([here, there, unplaced], 10, 10)).toEqual([]);
  });
});
