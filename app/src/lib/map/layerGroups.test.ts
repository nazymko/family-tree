// Choosing which old maps are shown: series groups, the three states of a group, one map alone, the filter and the maps
// under a point. Invented sheets only.
import { describe, expect, it } from 'vitest';
import type { MapLayerDto } from '../api/types';
import {
  filterLayers,
  groupByPlace,
  groupLayers,
  isNoChange,
  layersUnder,
  mapsByPlace,
  matches,
  noFilters,
  onlyThese,
  onlyThis,
  placeOptions,
  seriesKey,
  setAll,
  toggleGroup,
  triState,
  yearMatches,
  yearSpan,
} from './layerGroups';

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

describe('old map years and places', () => {
  const y = (year: string | null, title = 'Sheet') => yearSpan({ ...layer(null, true, title), year });

  it('reads_a_year_a_range_and_a_century', () => {
    expect(y('1648')).toEqual({ from: 1648, to: 1648 });
    expect(y('1826–1840')).toEqual({ from: 1826, to: 1840 });
    expect(y('1826-1840')).toEqual({ from: 1826, to: 1840 });
    expect(y('1826—40')).toEqual({ from: 1826, to: 1840 });
    expect(y('XIX ст.')).toEqual({ from: 1801, to: 1900 });
    expect(y('XIX c.')).toEqual({ from: 1801, to: 1900 });
    expect(y('19th c.')).toEqual({ from: 1801, to: 1900 });
    expect(y('ХІХ ст.')).toEqual({ from: 1801, to: 1900 });
    expect(y('XVII ст.')).toEqual({ from: 1601, to: 1700 });
  });

  it('falls_back_to_the_title_when_there_is_no_year', () => {
    expect(y(null, 'Plan A 1648, sheet 1-1')).toEqual({ from: 1648, to: 1648 });
    expect(y(null, 'Rizzi, sq. 16 (1772, approx.)')).toEqual({ from: 1772, to: 1772 });
    expect(y(null, 'Schubert 10-verst, sheet 51 (1826–1840)')).toEqual({ from: 1826, to: 1840 });
    expect(y(null, 'Three-verst 22-5 (XIX ст.)')).toEqual({ from: 1801, to: 1900 });
    expect(y(null, 'Sheet 22-5')).toBeNull();
    expect(y('')).toBeNull();
  });

  it('matches_years_by_overlap_and_keeps_or_drops_the_maps_without_a_year', () => {
    const noYear = (title: string) => ({ ...layer(null, true, title), year: null });
    const [a, b, c] = [noYear('A 1700'), noYear('B (1750–1850)'), noYear('C')];
    const f = { ...noFilters, from: 1800, to: 1900 };
    expect([a, b, c].map((l) => yearMatches(l, f))).toEqual([false, true, true]);
    expect(yearMatches(c, { ...f, noYear: false })).toBe(false);
    expect(yearMatches(a, { ...noFilters, to: 1700 })).toBe(true);
    expect(yearMatches(a, { ...noFilters, from: 1701 })).toBe(false);
  });

  const place = (id: string, name: string, lat: number, lon: number, count = 1) => ({ id, name, label: name, lat, lon, count });
  const places = [place('p1', 'Zelene', 50, 30, 5), place('p2', 'Bila', 50.5, 30.5, 9), place('p3', 'Daleke', 10, 10), { id: 'p4', name: 'Nowhere', label: 'Nowhere', lat: null, lon: null, count: 3 }];

  it('indexes_maps_by_place_and_groups_them_by_place_with_the_rest_last', () => {
    const wide = layer('S', true, 'Wide', [49, 29, 51, 31]);
    const small = layer('S', false, 'Small', [49.9, 29.9, 50.1, 30.1]);
    const none = layer('S', false, 'Elsewhere', [0, 0, 1, 1]);
    const unplaced = layer('S', false, 'Unplaced');
    const top = [wide, small, none, unplaced];
    const idx = mapsByPlace(top, places);
    expect(idx.get('p1')).toEqual([wide, small]);
    expect(idx.get('p2')).toEqual([wide]);
    expect(idx.has('p3')).toBe(false);
    expect(placeOptions(idx, places).map((o) => [o.id, o.maps])).toEqual([
      ['p2', 1],
      ['p1', 2],
    ]);
    const g = groupByPlace(top, places, idx, noFilters);
    // Most family events first, the maps covering no place last.
    expect(g.map((x) => x.key)).toEqual(['p:p2', 'p:p1', 'other']);
    expect(g.map((x) => x.layers.length)).toEqual([1, 2, 2]);
    expect(g[2].layers).toEqual([none, unplaced]);
    expect(g[1].shown).toBe(1);
    // One place chosen: only that place's group.
    expect(groupByPlace(top, places, idx, { ...noFilters, place: 'p1' }).map((x) => x.key)).toEqual(['p:p1']);
    // Text filter narrows the maps inside the groups.
    expect(groupByPlace(top, places, idx, { ...noFilters, text: 'small' }).map((x) => [x.key, x.layers.length])).toEqual([['p:p1', 1]]);
  });

  it('combines_text_years_and_place_and_shows_only_these', () => {
    const a = { ...layer('S', true, 'Old', [49, 29, 51, 31]), year: '1700' };
    const b = { ...layer('S', false, 'New', [49, 29, 51, 31]), year: '1850' };
    const idx = mapsByPlace([a, b], places);
    expect(filterLayers([a, b], idx, { ...noFilters, from: 1800 })).toEqual([b]);
    expect(filterLayers([a, b], idx, { ...noFilters, from: 1800, text: 'old' })).toEqual([]);
    expect(filterLayers([a, b], idx, { ...noFilters, place: 'p3' })).toEqual([]);
    expect(onlyThese([a, b], [b])).toEqual({ shown: [b.media], hidden: [a.media] });
  });
});
