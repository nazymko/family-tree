// The bundled world map: Natural Earth (public domain), simplified and quantized by scripts/build-basemap.mjs into one
// TopoJSON file. It is decoded once into flat arrays of Web Mercator world coordinates, so drawing never re-projects
// what is off screen.
import { feature } from 'topojson-client';
import type { GeometryCollection, Topology } from 'topojson-specification';
import { mercator } from './projection';

/** One polygon ring or line, as x,y pairs in world coordinates, with its bounding box. */
export interface Ring {
  pts: Float64Array;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** A shape of the map: its rings or lines, and the few properties the file keeps. */
export interface Shape {
  rings: Ring[];
  /** Name (English), Ukrainian name, first zoom at which to show, population (places), country code. */
  n?: string;
  u?: string;
  z: number;
  p?: number;
  a?: string;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

export interface MapPlaceLabel {
  x: number;
  y: number;
  n: string;
  u?: string;
  z: number;
  p: number;
}

export interface Basemap {
  land: Shape[];
  lakes: Shape[];
  rivers: Shape[];
  countries: Shape[];
  admin1: Shape[];
  places: MapPlaceLabel[];
}

type Geometry = { type: string; coordinates: unknown };
type Props = { n?: string; u?: string; z?: number; p?: number; a?: string } | null;

function ring(coords: number[][]): Ring {
  const pts = new Float64Array(coords.length * 2);
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -Infinity;
  let y1 = -Infinity;
  for (let i = 0; i < coords.length; i++) {
    const w = mercator(coords[i][1], coords[i][0]);
    pts[2 * i] = w.x;
    pts[2 * i + 1] = w.y;
    if (w.x < x0) x0 = w.x;
    if (w.x > x1) x1 = w.x;
    if (w.y < y0) y0 = w.y;
    if (w.y > y1) y1 = w.y;
  }
  return { pts, x0, y0, x1, y1 };
}

function ringsOf(g: Geometry): Ring[] {
  const c = g.coordinates as number[][] | number[][][] | number[][][][];
  switch (g.type) {
    case 'LineString':
      return [ring(c as number[][])];
    case 'MultiLineString':
    case 'Polygon':
      return (c as number[][][]).map(ring);
    case 'MultiPolygon':
      return (c as number[][][][]).flatMap((poly) => poly.map(ring));
    default:
      return [];
  }
}

function shapes(topo: Topology, name: string): Shape[] {
  const obj = topo.objects[name] as GeometryCollection | undefined;
  if (!obj) return [];
  const fc = feature(topo, obj) as unknown as { features: { properties: Props; geometry: Geometry | null }[] };
  const out: Shape[] = [];
  for (const f of fc.features) {
    if (!f.geometry) continue;
    const rings = ringsOf(f.geometry);
    if (rings.length === 0) continue;
    const p = f.properties ?? {};
    out.push({
      rings,
      n: p.n,
      u: p.u,
      z: p.z ?? 0,
      p: p.p,
      a: p.a,
      x0: Math.min(...rings.map((r) => r.x0)),
      y0: Math.min(...rings.map((r) => r.y0)),
      x1: Math.max(...rings.map((r) => r.x1)),
      y1: Math.max(...rings.map((r) => r.y1)),
    });
  }
  return out;
}

export function decodeBasemap(topo: Topology): Basemap {
  const places: MapPlaceLabel[] = [];
  const obj = topo.objects.places as GeometryCollection | undefined;
  if (obj) {
    const fc = feature(topo, obj) as unknown as { features: { properties: Props; geometry: { type: string; coordinates: number[] } | null }[] };
    for (const f of fc.features) {
      if (!f.geometry || f.geometry.type !== 'Point') continue;
      const w = mercator(f.geometry.coordinates[1], f.geometry.coordinates[0]);
      const p = f.properties ?? {};
      places.push({ x: w.x, y: w.y, n: p.n ?? '', u: p.u, z: p.z ?? 9, p: p.p ?? 0 });
    }
    // Biggest first, so a label that fits is the more important one.
    places.sort((a, b) => b.p - a.p);
  }
  return {
    land: shapes(topo, 'land'),
    lakes: shapes(topo, 'lakes'),
    rivers: shapes(topo, 'rivers'),
    countries: shapes(topo, 'countries'),
    admin1: shapes(topo, 'admin1'),
    places,
  };
}

let loading: Promise<Basemap> | null = null;

/** Loads the bundled map once. */
export function loadBasemap(): Promise<Basemap> {
  loading ??= (async () => {
    const url = (await import('./data/basemap.topo.json?url')).default;
    const res = await fetch(url);
    if (!res.ok) throw new Error(`basemap ${res.status}`);
    return decodeBasemap((await res.json()) as Topology);
  })();
  loading.catch(() => (loading = null));
  return loading;
}
