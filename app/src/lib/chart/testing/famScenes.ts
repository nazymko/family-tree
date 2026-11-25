// Family-shaped synthetic "built around" charts for the sheets print tests (plan 2026-11-22-sheets-packing, Task 1).
// The layouts are written by `crates/gt-layout/tests/fam_fixtures.rs` (all kin opened, compact cards, folded) into
// `./fam/<name>-<direction>.json.gz`: keys and geometry only. The names here are synthetic, from fixed lists.
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import type { ChartEdge, ChartNode } from '../../api/types';
import type { ChartDirection } from '../direction';
import type { Scene } from '../picture';

export const FAMS = ['fam-a', 'fam-b', 'fam-c', 'fam-d'] as const;
export type FamName = (typeof FAMS)[number];

/** Sheets "sheets by branch" printed before the packed sheets (best direction), from the investigation of 21.11.2026. */
export const TODAY_SHEETS: Record<FamName, number> = { 'fam-a': 9, 'fam-b': 9, 'fam-c': 5, 'fam-d': 15 };

export const GIVEN = ['Orest', 'Bohdana', 'Taras', 'Oksana', 'Lev', 'Halyna', 'Yaroslav', 'Mariika', 'Stepan', 'Daryna', 'Ihor', 'Zoriana'];
export const SUR = ['Bondar', 'Hrytsenko', 'Melnyk', 'Tkach', 'Savchuk', 'Koval', 'Lysenko', 'Moroz'];

interface Raw {
  root: number;
  nodes: { id: number; key: number; x: number; y: number; w: number; h: number; gen: number; dupOf: number | null }[];
  edges: { from: number; to: number; kind: ChartEdge['kind']; pts: number[] }[];
}

/** The fixture folder (vitest runs from `app`; `import.meta.dirname` when the runner gives it). */
const FAM_DIR = (import.meta as { dirname?: string }).dirname ? path.join((import.meta as { dirname?: string }).dirname!, 'fam') : path.resolve('src/lib/chart/testing/fam');

const cache = new Map<string, { scene: Scene; root: number }>();

/** The synthetic name of a fixture person. */
export function famName(key: number): string {
  return `${GIVEN[(key * 7) % GIVEN.length]} ${SUR[(key * 13) % SUR.length]}`;
}

/** One fixture chart: nodes carry `person.id = 'k<key>'` and a synthetic name. A fresh copy every call. */
export function loadFam(name: FamName, dir: ChartDirection): { scene: Scene; root: number } {
  const id = `${name}-${dir}`;
  let hit = cache.get(id);
  if (!hit) {
    const file = path.join(FAM_DIR, `${id}.json.gz`);
    const raw = JSON.parse(zlib.gunzipSync(fs.readFileSync(file)).toString('utf8')) as Raw;
    const nav = { parent: null, child: null, prev: null, next: null };
    const nodes: ChartNode[] = raw.nodes.map((n) => {
      const label = famName(n.key);
      const born = 2000 + n.gen * 30 + (n.key % 9);
      return {
        id: n.id,
        person: { id: `k${n.key}`, name: label, lifespan: `${born} – ${born + 60}`, sex: n.key % 2 ? 'female' : 'male', living: false, private: false, siblingMarker: false },
        x: n.x,
        y: n.y,
        w: n.w,
        h: n.h,
        gen: n.gen,
        ahnen: null,
        dupOf: n.dupOf,
        moreUp: false,
        moreDown: false,
        nav,
        label,
      };
    });
    hit = { scene: { nodes, edges: raw.edges.map((e) => ({ ...e })) }, root: raw.root };
    cache.set(id, hit);
  }
  return {
    scene: { nodes: hit.scene.nodes.map((n) => ({ ...n })), edges: hit.scene.edges.map((e) => ({ ...e, pts: [...e.pts] })) },
    root: hit.root,
  };
}
