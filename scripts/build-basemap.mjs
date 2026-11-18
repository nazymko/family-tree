#!/usr/bin/env node
// Builds the Map view's bundled basemap from Natural Earth (public domain, https://www.naturalearthdata.com).
//
//   node scripts/build-basemap.mjs <folder with the six ne_10m_*.geojson files> [--min-weight N]
//
// Download the files once, e.g. from https://github.com/nvkelso/natural-earth-vector/tree/master/geojson:
//   ne_10m_land, ne_10m_lakes, ne_10m_rivers_lake_centerlines, ne_10m_admin_0_countries,
//   ne_10m_admin_1_states_provinces, ne_10m_populated_places
//
// Writes app/src/lib/map/data/basemap.topo.json: one quantized, simplified TopoJSON with the objects
// land, lakes, rivers, countries, admin1, places. Property keys are short on purpose (the file ships in the app):
//   n = name, u = Ukrainian name, z = first zoom level to show, p = population (places), a = country code (admin1).
// Needs the build-time dependencies topojson-server and topojson-simplify (ISC, in app/package.json).
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const require = createRequire(path.join(path.dirname(fileURLToPath(import.meta.url)), '../app/package.json'));
const { topology } = require('topojson-server');
const { presimplify, simplify } = require('topojson-simplify');
const { quantize } = require('topojson-client');

const args = process.argv.slice(2);
const dir = args.find((a) => !a.startsWith('--'));
if (!dir) {
  console.error('Usage: node scripts/build-basemap.mjs <folder with the ne_10m_*.geojson files> [--min-weight N]');
  process.exit(2);
}
const wi = args.indexOf('--min-weight');
// The smallest triangle (square degrees) a point must span to survive; 1e-6 is about a 100 m wiggle.
const minWeight = wi >= 0 ? Number(args[wi + 1]) : 1e-6;
const QUANT = 3e5;

const read = (name) => JSON.parse(fs.readFileSync(path.join(dir, `ne_10m_${name}.geojson`), 'utf8'));
const z = (v) => (typeof v === 'number' ? Math.round(v * 10) / 10 : 0);
const keep = (obj) => Object.fromEntries(Object.entries(obj).filter(([, v]) => v !== undefined && v !== null && v !== ''));

function collection(name, props) {
  const j = read(name);
  return {
    type: 'FeatureCollection',
    features: j.features
      .filter((f) => f.geometry)
      .map((f) => ({ type: 'Feature', properties: keep(props(f.properties)), geometry: f.geometry })),
  };
}

const objects = {
  land: collection('land', () => ({})),
  lakes: collection('lakes', (p) => ({ n: p.name_en ?? p.name, u: p.name_uk, z: z(p.min_zoom) })),
  rivers: collection('rivers_lake_centerlines', (p) => ({ n: p.name_en ?? p.name, z: z(p.min_zoom) })),
  countries: collection('admin_0_countries', (p) => ({ n: p.NAME_EN ?? p.NAME, u: p.NAME_UK, z: z(p.MIN_ZOOM), a: p.ISO_A2 })),
  admin1: collection('admin_1_states_provinces', (p) => ({ n: p.name_en ?? p.name, u: p.name_uk, z: z(p.min_zoom), a: p.iso_a2 })),
  places: collection('populated_places', (p) => ({ n: p.NAME_EN ?? p.NAME, u: p.NAME_UK, z: z(p.MIN_ZOOM), p: p.POP_MAX })),
};

let topo = topology(objects);
topo = presimplify(topo);
topo = simplify(topo, minWeight);
// presimplify and simplify work in degrees and leave the weights as a third coordinate; the file wants a plain
// quantized, delta-encoded topology.
topo.arcs = topo.arcs.map((arc) => arc.map((pt) => [pt[0], pt[1]]));
topo = quantize(topo, QUANT);

const out = path.join(path.dirname(fileURLToPath(import.meta.url)), '../app/src/lib/map/data/basemap.topo.json');
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, JSON.stringify(topo));
const mb = (fs.statSync(out).size / 1048576).toFixed(2);
const points = topo.arcs.reduce((n, a) => n + a.length, 0);
console.log(`wrote ${out}: ${mb} MB, ${topo.arcs.length} arcs, ${points} points, min weight ${minWeight}`);
