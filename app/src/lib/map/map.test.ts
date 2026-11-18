// Pure parts of the Map view: projection, filters, grouping, heat and paths. Invented people and villages only.
import { describe, expect, it } from 'vitest';
import type { MapDataDto, MapEventDto } from '../api/types';
import { decades, heatPoints, heatStrength, rampColor } from './heat';
import { filterEvents, kindsIn, missingCoordinates, noFilter, yearRange } from './filters';
import { birthLinks, buildPaths, colorScheme } from './paths';
import { hitPoint, pickLabels, placePoints, radiusFor } from './places';
import { clampCamera, fitPoints, fromScreen, mercator, minZoom, toScreen, unmercator, zoomAt } from './projection';

const ev = (id: string, kind: string, year: number | null, place: string, person: string): MapEventDto => ({ id, kind, year, place, person });

const data: MapDataDto = {
  places: [
    { id: 'b', label: 'Berezivka, Zelena oblast', name: 'Berezivka', lat: 50.5, lon: 30.25 },
    { id: 'd', label: 'Dubky, Zelena oblast', name: 'Dubky', lat: null, lon: null },
    { id: 'k', label: 'Kyiv', name: 'Kyiv', lat: 50.45, lon: 30.52 },
  ],
  people: [
    { id: 'ivan', name: 'Ivan Koval', surname: 'Koval', generation: 0, living: false },
    { id: 'petro', name: 'Petro Koval', surname: 'Koval', generation: 1, living: false },
    { id: 'maria', name: 'Maria Hnat', surname: 'Hnat', generation: 1, living: false },
  ],
  events: [
    ev('1', 'birth', 1870, 'd', 'petro'),
    ev('2', 'birth', 1900, 'b', 'ivan'),
    ev('3', 'residence', 1925, 'k', 'ivan'),
    ev('4', 'residence', 1930, 'k', 'ivan'),
    ev('5', 'death', 1970, 'k', 'ivan'),
    ev('6', 'birth', 1875, 'k', 'maria'),
    ev('7', 'marriage', null, 'b', 'maria'),
  ],
  links: [
    { parent: 'petro', child: 'ivan' },
    { parent: 'maria', child: 'ivan' },
  ],
  eventsWithoutPlace: 2,
};

describe('projection', () => {
  it('round_trips_lat_lon', () => {
    const w = mercator(50.45, 30.52);
    const b = unmercator(w.x, w.y);
    expect(b.lat).toBeCloseTo(50.45, 9);
    expect(b.lon).toBeCloseTo(30.52, 9);
  });

  it('puts_the_equator_and_greenwich_in_the_middle', () => {
    const w = mercator(0, 0);
    expect(w.x).toBeCloseTo(0.5, 12);
    expect(w.y).toBeCloseTo(0.5, 12);
    expect(mercator(90, 0).y).toBeGreaterThan(0);
  });

  it('screen_and_world_are_inverse', () => {
    const cam = { cx: 0.55, cy: 0.33, zoom: 6 };
    const [sx, sy] = toScreen(cam, 800, 600, 0.551, 0.331);
    const w = fromScreen(cam, 800, 600, sx, sy);
    expect(w.x).toBeCloseTo(0.551, 12);
    expect(w.y).toBeCloseTo(0.331, 12);
  });

  it('zooming_keeps_the_point_under_the_cursor', () => {
    const cam = { cx: 0.55, cy: 0.33, zoom: 5 };
    const before = fromScreen(cam, 800, 600, 300, 200);
    const z = zoomAt(cam, 800, 600, 300, 200, 2);
    const after = fromScreen(z, 800, 600, 300, 200);
    expect(z.zoom).toBe(7);
    expect(after.x).toBeCloseTo(before.x, 9);
    expect(after.y).toBeCloseTo(before.y, 9);
  });

  it('zoom_stays_within_limits_and_the_view_inside_the_world', () => {
    const c = clampCamera({ cx: -3, cy: 4, zoom: 40 }, 800, 600);
    expect(c.zoom).toBe(13);
    expect(c.cx).toBeGreaterThan(0);
    expect(c.cy).toBeLessThan(1);
    const out = zoomAt({ cx: 0.5, cy: 0.5, zoom: 2 }, 800, 600, 400, 300, -10);
    expect(out.zoom).toBeCloseTo(minZoom(800, 600), 9);
  });

  it('fits_points_into_the_view', () => {
    const pts = [mercator(50.5, 30.25), mercator(51.2, 33.1)];
    const cam = fitPoints(pts, 800, 600, 40);
    for (const p of pts) {
      const [sx, sy] = toScreen(cam, 800, 600, p.x, p.y);
      expect(sx).toBeGreaterThanOrEqual(39);
      expect(sx).toBeLessThanOrEqual(761);
      expect(sy).toBeGreaterThanOrEqual(39);
      expect(sy).toBeLessThanOrEqual(561);
    }
    expect(fitPoints([], 800, 600).zoom).toBeCloseTo(minZoom(800, 600), 9);
  });
});

describe('filters', () => {
  it('lists_kinds_in_life_order_and_the_year_range', () => {
    expect(kindsIn(data.events)).toEqual(['birth', 'marriage', 'residence', 'death']);
    expect(yearRange(data.events)).toEqual([1870, 1970]);
    expect(yearRange([])).toBeNull();
  });

  it('filters_by_kind_and_years_and_keeps_undated_only_when_open', () => {
    expect(filterEvents(data.events, noFilter())).toHaveLength(7);
    expect(filterEvents(data.events, { kinds: new Set(['birth']), from: null, to: null }).map((e) => e.id)).toEqual(['1', '2', '6']);
    const years = filterEvents(data.events, { kinds: new Set(), from: 1900, to: 1930 });
    expect(years.map((e) => e.id)).toEqual(['2', '3', '4']);
  });

  it('counts_events_whose_place_has_no_point', () => {
    const m = missingCoordinates(data, data.events);
    expect(m.events).toBe(1);
    expect(m.places[0].place.name).toBe('Dubky');
    expect(m.noPlace).toBe(2);
    const none = missingCoordinates(data, filterEvents(data.events, { kinds: new Set(['death']), from: null, to: null }));
    expect(none.events).toBe(0);
  });
});

describe('places', () => {
  it('groups_events_by_place_and_skips_places_without_a_point', () => {
    const pts = placePoints(data, data.events);
    expect(pts.map((p) => [p.name, p.count]).sort()).toEqual([
      ['Berezivka', 2],
      ['Kyiv', 4],
    ]);
    const kyiv = pts.find((p) => p.name === 'Kyiv')!;
    expect([...kyiv.people].sort()).toEqual(['ivan', 'maria']);
    expect(kyiv.byKind.get('residence')).toBe(2);
  });

  it('sizes_dots_by_count_with_a_cap', () => {
    expect(radiusFor(1)).toBe(4);
    expect(radiusFor(10)).toBeGreaterThan(radiusFor(2));
    expect(radiusFor(1e6)).toBe(26);
  });

  it('finds_the_dot_under_the_pointer', () => {
    const pts = placePoints(data, data.events);
    const cam = fitPoints(pts, 800, 600);
    const k = pts.find((p) => p.name === 'Kyiv')!;
    const [sx, sy] = toScreen(cam, 800, 600, k.x, k.y);
    expect(hitPoint(pts, cam, 800, 600, sx + 2, sy)?.name).toBe('Kyiv');
    expect(hitPoint(pts, cam, 800, 600, sx + 300, sy + 300)).toBeNull();
  });

  it('labels_the_busiest_first_and_drops_overlaps', () => {
    const pts = placePoints(data, data.events);
    const near = { cx: pts[0].x, cy: pts[0].y, zoom: 4 };
    const labels = pickLabels(pts, near, 800, 600, (s) => s.length * 7);
    expect(labels[0].text).toBe('Kyiv');
    // At this zoom the two villages are a few px apart: only one label fits.
    expect(labels).toHaveLength(1);
    const far = { cx: pts[0].x, cy: pts[0].y, zoom: 11 };
    expect(pickLabels(pts, far, 800, 600, (s) => s.length * 7).length).toBeGreaterThan(1);
  });
});

describe('heat', () => {
  it('weights_points_by_event_count', () => {
    const h = heatPoints(placePoints(data, data.events));
    expect(h.map((p) => p.w).sort()).toEqual([2, 4]);
    expect(heatStrength(4, 4)).toBe(1);
    expect(heatStrength(1, 4)).toBeCloseTo(0.5, 9);
    expect(heatStrength(1, 0)).toBe(0);
  });

  it('steps_by_decade', () => {
    expect(decades(1873, 1921)).toEqual([1870, 1880, 1890, 1900, 1910, 1920]);
  });

  it('runs_from_transparent_to_red', () => {
    expect(rampColor(0)[3]).toBe(0);
    const hot = rampColor(1);
    expect(hot[0]).toBeGreaterThan(200);
    expect(hot[2]).toBeLessThan(80);
  });
});

describe('paths', () => {
  it('orders_a_life_and_merges_consecutive_events_at_one_place', () => {
    const paths = buildPaths(data, data.events);
    const ivan = paths.find((p) => p.person.id === 'ivan')!;
    expect(ivan.stops.map((s) => s.name)).toEqual(['Berezivka', 'Kyiv']);
    expect(ivan.stops[1]).toMatchObject({ from: 1925, to: 1970, kinds: ['residence', 'death'] });
    // Petro's only event is at a place without a point: no path for him.
    expect(paths.find((p) => p.person.id === 'petro')).toBeUndefined();
  });

  it('puts_undated_birth_first_and_undated_death_last', () => {
    const d: MapDataDto = {
      ...data,
      events: [ev('a', 'death', null, 'k', 'ivan'), ev('b', 'residence', 1920, 'b', 'ivan'), ev('c', 'birth', null, 'k', 'ivan')],
    };
    const p = buildPaths(d, d.events)[0];
    expect(p.stops.map((s) => s.kinds[0])).toEqual(['birth', 'residence', 'death']);
  });

  it('links_birth_places_of_parent_and_child_only_when_both_are_known_and_differ', () => {
    const links = birthLinks(data, data.events);
    // Petro was born at a place without a point: only Maria (Kyiv) to Ivan (Berezivka).
    expect(links).toHaveLength(1);
    expect(links[0]).toMatchObject({ parent: 'maria', child: 'ivan' });
    const same = birthLinks({ ...data, events: [ev('x', 'birth', 1, 'k', 'ivan'), ev('y', 'birth', 1, 'k', 'maria')] }, [ev('x', 'birth', 1, 'k', 'ivan'), ev('y', 'birth', 1, 'k', 'maria')]);
    expect(same).toHaveLength(0);
  });

  it('colours_by_generation_and_by_surname_with_a_legend', () => {
    const g = colorScheme(data.people, 'generation');
    expect(g.legend.map((l) => l.label)).toEqual(['0', '1']);
    expect(g.color(data.people[1])).toBe(g.color(data.people[2]));
    expect(g.color(data.people[0])).not.toBe(g.color(data.people[1]));
    const s = colorScheme(data.people, 'surname');
    expect(s.legend.map((l) => l.label)).toEqual(['Koval', 'Hnat']);
    expect(s.color(data.people[0])).toBe(s.color(data.people[1]));
    expect(s.color(data.people[2])).not.toBe(s.color(data.people[0]));
  });
});

describe('ten thousand events', () => {
  it('group_filter_and_path_quickly', () => {
    const places = Array.from({ length: 300 }, (_, i) => ({ id: `p${i}`, label: `V${i}`, name: `V${i}`, lat: 49 + (i % 20) * 0.1, lon: 30 + Math.floor(i / 20) * 0.1 }));
    const people = Array.from({ length: 10000 }, (_, i) => ({ id: `q${i}`, name: `P${i}`, surname: `S${i % 40}`, generation: i % 8, living: false }));
    const events = people.map((p, i) => ev(`e${i}`, i % 3 ? 'birth' : 'death', 1800 + (i % 200), `p${i % 300}`, p.id));
    const big: MapDataDto = { places, people, events, links: [], eventsWithoutPlace: 0 };
    const t = performance.now();
    const f = filterEvents(events, { kinds: new Set(['birth']), from: 1850, to: 1950 });
    const pts = placePoints(big, f);
    const paths = buildPaths(big, f);
    heatPoints(pts);
    const took = performance.now() - t;
    expect(f.length).toBeGreaterThan(1000);
    expect(pts.length).toBeGreaterThan(100);
    expect(paths.length).toBe(f.length);
    expect(took).toBeLessThan(500);
  });
});
