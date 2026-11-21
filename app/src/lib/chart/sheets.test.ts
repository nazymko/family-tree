import { describe, expect, it } from 'vitest';
import type { ChartEdge, ChartNode } from '../api/types';
import type { Scene } from './picture';
import { splitSheets, type Sheet } from './sheets';

const nav = { parent: null, child: null, prev: null, next: null };

/** A synthetic "built around" scene: parent edges leave the parent (father's edge first), as `gt-layout` draws them. */
class Tree {
  nodes: ChartNode[] = [];
  edges: ChartEdge[] = [];

  add(gen: number, dupOf: number | null = null): number {
    const id = this.nodes.length + 1;
    this.nodes.push({ id, person: null, x: id * 120, y: gen * 100, w: 100, h: 50, gen, ahnen: null, dupOf, moreUp: false, moreDown: false, nav, label: `Person ${id}` });
    if (dupOf !== null) this.edge(id, dupOf, 'dup');
    return id;
  }

  gen(id: number): number {
    return this.nodes[id - 1].gen;
  }

  edge(from: number, to: number, kind: ChartEdge['kind']): void {
    this.edges.push({ from, to, kind, pts: [from * 120, 0, to * 120, 100] });
  }

  /** A child of `f` (and `m`) one generation below them. */
  child(f: number, m: number | null = null): number {
    const c = this.add(this.gen(f) + 1);
    this.edge(f, c, 'parent');
    if (m !== null) this.edge(m, c, 'parent');
    return c;
  }

  /** A partner of `a` in the same generation. */
  partner(a: number): number {
    const p = this.add(this.gen(a));
    this.edge(a, p, 'partner');
    return p;
  }

  /** Father and mother of `c`, a couple. */
  parents(c: number): [number, number] {
    const f = this.add(this.gen(c) - 1);
    const m = this.add(this.gen(c) - 1);
    this.edge(f, m, 'partner');
    this.edge(f, c, 'parent');
    this.edge(m, c, 'parent');
    return [f, m];
  }

  scene(): Scene {
    return { nodes: this.nodes, edges: this.edges };
  }
}

/**
 * `depth` generations of ancestors above `id`, both parents every time, and `siblings` opened brothers and sisters of
 * every person that gets parents. Returns the ancestors, nearest first.
 */
function chain(t: Tree, id: number, depth: number, siblings = 0): number[] {
  if (depth <= 0) return [];
  const [f, m] = t.parents(id);
  for (let i = 0; i < siblings; i++) t.child(f, m);
  return [f, m, ...chain(t, f, depth - 1, siblings), ...chain(t, m, depth - 1, siblings)];
}

/** Centre (gen 0), parents, the four grandparents. */
function base(t: Tree) {
  const c = t.add(0);
  const [f, m] = t.parents(c);
  const [ff, fm] = t.parents(f);
  const [mf, mm] = t.parents(m);
  return { c, f, m, ff, fm, mf, mm };
}

const sheetOf = (sheets: Sheet[], id: number) => sheets.find((s) => s.ids.has(id))?.no;

function expectEveryoneOnce(scene: Scene, sheets: Sheet[]) {
  const seen = new Map<number, number>();
  for (const s of sheets) for (const id of s.ids) seen.set(id, (seen.get(id) ?? 0) + 1);
  const full = scene.nodes.filter((n) => n.dupOf === null).map((n) => n.id);
  expect([...seen.keys()].sort((a, b) => a - b)).toEqual(full.sort((a, b) => a - b));
  expect([...seen.values()].every((k) => k === 1)).toBe(true);
}

const always = () => true;

describe('splitSheets', () => {
  it('everyone_once', () => {
    const t = new Tree();
    const g = base(t);
    t.child(g.c);
    t.partner(g.c);
    for (const gp of [g.ff, g.fm, g.mf, g.mm]) chain(t, gp, 3, 1);
    const aunt = t.child(g.ff, g.fm);
    t.child(aunt);
    const scene = t.scene();
    const sheets = splitSheets(scene, [g.c], { fits: always });
    expect(sheets.length).toBe(5);
    expectEveryoneOnce(scene, sheets);
    expect(sheets.map((s) => s.no)).toEqual([1, 2, 3, 4, 5]);
  });

  it('cover_holds_three_generations', () => {
    const t = new Tree();
    const g = base(t);
    const kids = [t.child(g.c), t.child(g.c)];
    const wife = t.partner(g.c);
    const aunt = t.child(g.ff, g.fm);
    const auntKids = [t.child(aunt), t.child(aunt)];
    for (const gp of [g.ff, g.fm, g.mf, g.mm]) chain(t, gp, 3);
    // A brother of a grandparent (a child of the grandparent's parents) is on the cover too.
    const [ffF, ffM] = t.edges.filter((e) => e.kind === 'parent' && e.to === g.ff).map((e) => e.from);
    const greatUncle = t.child(ffF, ffM);
    const sheets = splitSheets(t.scene(), [g.c], { fits: always });
    const cover = sheets[0];
    expect(cover.no).toBe(1);
    expect(cover.top).toBeNull();
    for (const id of [g.c, g.f, g.m, g.ff, g.fm, g.mf, g.mm, aunt, ...auntKids, ...kids, wife, greatUncle]) expect(cover.ids.has(id)).toBe(true);
    expect(cover.ids.has(ffF)).toBe(false);
  });

  it('line_sheet_per_grandparent', () => {
    const t = new Tree();
    const g = base(t);
    const gps = [g.ff, g.fm, g.mf, g.mm];
    for (const gp of gps) {
      const [father] = chain(t, gp, 3);
      // Ten opened brothers and sisters of the grandparent's father.
      const [gf, gm] = t.edges.filter((e) => e.kind === 'parent' && e.to === father).map((e) => e.from);
      for (let i = 0; i < 10; i++) t.child(gf, gm);
    }
    const scene = t.scene();
    const sheets = splitSheets(scene, [g.c], { fits: always });
    expect(sheets.length).toBe(5);
    expect(sheets.slice(1).map((s) => s.top)).toEqual(gps);
    for (const s of sheets.slice(1)) {
      expect(s.refs.get(s.top!)).toBe(1);
      expect(s.ids.size).toBe(24);
      expect(s.scene.nodes.length).toBe(25);
    }
    expectEveryoneOnce(scene, sheets);
  });

  it('tiny_line_stays_on_cover', () => {
    const t = new Tree();
    const g = base(t);
    for (const gp of [g.ff, g.fm, g.mf]) chain(t, gp, 3);
    const lone = t.add(t.gen(g.mm) - 1);
    t.edge(lone, g.mm, 'parent');
    const sheets = splitSheets(t.scene(), [g.c], { fits: always });
    expect(sheets.length).toBe(4);
    expect(sheets.some((s) => s.top === g.mm)).toBe(false);
    expect(sheets[0].ids.has(lone)).toBe(true);
    expect(sheets[0].refs.size).toBe(0);
  });

  it('splits_when_too_big', () => {
    const t = new Tree();
    const g = base(t);
    // About 250 people above one grandparent: six generations, two opened siblings of everyone with parents.
    chain(t, g.ff, 6, 2);
    const scene = t.scene();
    const line = scene.nodes.filter((n) => n.gen <= -3).length;
    expect(line).toBeGreaterThanOrEqual(200);
    const fits = (s: Scene) => s.nodes.length <= 40;
    const sheets = splitSheets(scene, [g.c], { fits });
    expect(sheets.length).toBeGreaterThan(2);
    expectEveryoneOnce(scene, sheets);
    for (const s of sheets.slice(1)) {
      expect(fits(s.scene)).toBe(true);
      // The foot of every line sheet points to the sheet that holds that person in full.
      const top = s.top!;
      const holder = sheetOf(sheets, top)!;
      expect(s.refs.get(top)).toBe(holder);
      expect(holder).toBeLessThan(s.no);
      expect(s.scene.nodes.some((n) => n.id === top)).toBe(true);
    }
    // Split at the great-grandparents first: their own sheets follow the grandparent's sheet, father's side first.
    const [ffF, ffM] = t.edges.filter((e) => e.kind === 'parent' && e.to === g.ff).map((e) => e.from);
    expect(sheets[1].top).toBe(g.ff);
    expect(sheets[2].top).toBe(ffF);
    const ffMSheet = sheets.find((s) => s.top === ffM)!;
    expect(ffMSheet.refs.get(ffM)).toBe(2);
    // Depth-first: every sheet of the father's side comes before the mother's.
    const fatherSide = sheets.filter((s) => s.no > 2 && s.no < ffMSheet.no);
    expect(fatherSide.length).toBeGreaterThan(1);
  });

  it('several_centres_or_no_ancestors', () => {
    const t = new Tree();
    const a = t.add(0);
    const [f, m] = t.parents(a);
    const b = t.child(f, m);
    t.child(a);
    t.partner(b);
    const scene = t.scene();
    const sheets = splitSheets(scene, [a, b]);
    expect(sheets.length).toBe(1);
    expect(sheets[0].ids.size).toBe(scene.nodes.length);
    expect(sheets[0].scene.nodes.length).toBe(scene.nodes.length);
  });

  it('pedigree_collapse', () => {
    const t = new Tree();
    const g = base(t);
    const ffLine = chain(t, g.ff, 3);
    for (const gp of [g.fm, g.mf]) chain(t, gp, 3);
    const mmLine = chain(t, g.mm, 3);
    // An ancestor on the father's father's line (gen -4) is also an ancestor on the mother's mother's line.
    const x = ffLine.find((id) => t.gen(id) === -4)!;
    const top = mmLine.find((id) => t.gen(id) === -5)!;
    const dup = t.add(-6, x);
    t.edge(dup, top, 'parent');
    const scene = t.scene();
    const sheets = splitSheets(scene, [g.c], { fits: always });
    expectEveryoneOnce(scene, sheets);
    const xSheet = sheetOf(sheets, x)!;
    expect(sheets[xSheet - 1].top).toBe(g.ff);
    const mmSheet = sheets.find((s) => s.top === g.mm)!;
    expect(mmSheet.refs.get(dup)).toBe(xSheet);
    expect(mmSheet.scene.nodes.some((n) => n.id === dup)).toBe(true);
    expect(sheets.some((s) => s.ids.has(dup))).toBe(false);
  });

  it('default_fits_splits_a_wide_line', () => {
    const t = new Tree();
    const g = base(t);
    chain(t, g.ff, 6, 2);
    const sheets = splitSheets(t.scene(), [g.c]);
    expect(sheets.length).toBeGreaterThan(2);
  });
});
