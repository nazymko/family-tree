import { describe, expect, it } from 'vitest';
import type { ChartEdge, ChartNode } from '../api/types';
import type { Scene } from './picture';
import { branchNames, refTagAt, screenFrames, splitSheets, type Sheet } from './sheets';
import { asPrintChart, bigAround } from './testing/syntheticAround';

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
    // Cover; the great-grandfather's sheet with the two small remainders above it merged in, then the four sheets cut
    // at gen -5 above it; the same for the great-grandmother.
    expect(sheets.length).toBe(11);
    expectEveryoneOnce(scene, sheets);
    for (const s of sheets.slice(1)) {
      expect(fits(s.scene)).toBe(true);
      expect(s.ids.size).toBeGreaterThanOrEqual(8);
      expect(t.gen(s.top!)).not.toBe(-4);
      // The foot of every line sheet points to the sheet that holds that person in full.
      const top = s.top!;
      const holder = sheetOf(sheets, top)!;
      expect(s.refs.get(top)).toBe(holder);
      expect(holder).toBeLessThan(s.no);
      expect(s.scene.nodes.some((n) => n.id === top)).toBe(true);
    }
    // The grandparent's sheet kept only 6 people after the split, so it merged into the cover: the great-grandparents'
    // sheets hang straight from the cover, father's side first, each followed by its own sheets (depth-first).
    const [ffF, ffM] = t.edges.filter((e) => e.kind === 'parent' && e.to === g.ff).map((e) => e.from);
    expect(sheets.some((s) => s.top === g.ff)).toBe(false);
    expect(sheets[1].top).toBe(ffF);
    expect(sheets[1].refs.get(ffF)).toBe(1);
    expect(sheets[1].ids.size).toBe(18);
    expect(sheets[6].top).toBe(ffM);
    expect(sheets[6].refs.get(ffM)).toBe(1);
    expect(sheets.slice(2, 6).map((s) => [t.gen(s.top!), s.refs.get(s.top!)])).toEqual(Array(4).fill([-5, 2]));
    expect(sheets.slice(7).map((s) => [t.gen(s.top!), s.refs.get(s.top!)])).toEqual(Array(4).fill([-5, 7]));
    expect(sheets[0].ids.has(ffF)).toBe(true);
  });

  it('small_remainder_stays_when_merging_would_not_fit', () => {
    const t = new Tree();
    const g = base(t);
    chain(t, g.ff, 6, 2);
    // Thirty children make the cover 39 boxes: with the grandparent's 6 left-over people it would not fit.
    for (let i = 0; i < 30; i++) t.child(g.c);
    const scene = t.scene();
    const fits = (s: Scene) => s.nodes.length <= 40;
    const sheets = splitSheets(scene, [g.c], { fits });
    expectEveryoneOnce(scene, sheets);
    expect(sheets.length).toBe(12);
    expect(sheets[0].ids.size).toBe(39);
    expect(sheets[1].top).toBe(g.ff);
    expect(sheets[1].ids.size).toBe(6);
    expect(sheets[1].refs.get(g.ff)).toBe(1);
    const [ffF] = t.edges.filter((e) => e.kind === 'parent' && e.to === g.ff).map((e) => e.from);
    expect(sheets[2].top).toBe(ffF);
    expect(sheets[2].refs.get(ffF)).toBe(2);
    for (const s of sheets) expect(fits(s.scene)).toBe(true);
  });

  it('centres_in_different_rows', () => {
    const t = new Tree();
    const g = base(t);
    for (const gp of [g.ff, g.fm, g.mf, g.mm]) chain(t, gp, 3);
    // A second chosen person one row higher, with ancestors of their own.
    const b = t.add(-1);
    const [bf, bm] = t.parents(b);
    const bgps = [...t.parents(bf), ...t.parents(bm)];
    const above = bgps.map((gp) => chain(t, gp, 3));
    const scene = t.scene();
    const sheets = splitSheets(scene, [g.c, b], { fits: always });
    expectEveryoneOnce(scene, sheets);
    expect(sheets.length).toBe(9);
    for (const id of [b, bf, bm, ...bgps]) expect(sheets[0].ids.has(id)).toBe(true);
    for (const line of above) expect(sheets[0].ids.has(line[0])).toBe(false);
    expect(sheets.slice(1).map((s) => s.top)).toEqual([g.ff, g.fm, g.mf, g.mm, ...bgps]);
    for (const s of sheets.slice(1)) expect(s.refs.get(s.top!)).toBe(1);
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

  it('dup_on_its_originals_sheet_is_no_ref', () => {
    const t = new Tree();
    const g = base(t);
    const ffLine = chain(t, g.ff, 3);
    for (const gp of [g.fm, g.mf, g.mm]) chain(t, gp, 3);
    // An ancestor at gen -4 of the father's father's line is reached again higher up on the same line.
    const x = ffLine.find((id) => t.gen(id) === -4)!;
    const top = ffLine.find((id) => t.gen(id) === -5)!;
    const dup = t.add(-6, x);
    t.edge(dup, top, 'parent');
    const scene = t.scene();
    const sheets = splitSheets(scene, [g.c], { fits: always });
    expectEveryoneOnce(scene, sheets);
    const ffSheet = sheets.find((s) => s.top === g.ff)!;
    expect(ffSheet.ids.has(x)).toBe(true);
    expect(ffSheet.refs.has(dup)).toBe(false);
    expect(ffSheet.scene.nodes.some((n) => n.id === dup)).toBe(true);
    for (const s of sheets) for (const v of s.refs.values()) expect(v).not.toBe(s.no);
  });

  it('default_fits_splits_a_wide_line', () => {
    const t = new Tree();
    const g = base(t);
    chain(t, g.ff, 6, 2);
    const sheets = splitSheets(t.scene(), [g.c]);
    expect(sheets.length).toBeGreaterThan(2);
  });
});

describe('refTagAt', () => {
  const box = { x: 100, y: 200, w: 140, h: 60 };

  it('puts_the_tag_on_the_side_away_from_the_ancestors_in_every_direction', () => {
    // Ancestors above: the tag goes under the box.
    expect(refTagAt(box, 'top', 3)).toEqual({ x: 170, y: 263, align: 'center', baseline: 'top' });
    // Ancestors below: the connector leaves the bottom edge, so the tag goes over the box.
    expect(refTagAt(box, 'bottom', 3)).toEqual({ x: 170, y: 197, align: 'center', baseline: 'bottom' });
    // Ancestors on the left: the tag goes to the right of the box, and the other way round.
    expect(refTagAt(box, 'left', 3)).toEqual({ x: 243, y: 230, align: 'left', baseline: 'middle' });
    expect(refTagAt(box, 'right', 3)).toEqual({ x: 97, y: 230, align: 'right', baseline: 'middle' });
  });

  it('never_lands_inside_the_box', () => {
    for (const d of ['top', 'bottom', 'left', 'right'] as const) {
      const a = refTagAt(box, d, 2);
      const inside = a.x > box.x && a.x < box.x + box.w && a.y > box.y && a.y < box.y + box.h;
      expect(inside, d).toBe(false);
    }
  });
});

describe('screenFrames', () => {
  it('frames_each_line_sheet_of_the_print_around_the_same_people_on_screen_with_the_printed_number', () => {
    const screen = bigAround();
    const print = asPrintChart(screen);
    const printScene: Scene = { nodes: print.nodes, edges: print.edges };
    // Small sheets, so the print cuts every grandparent's line.
    const sheets = splitSheets(printScene, [print.root], { fits: (sc) => sc.nodes.length <= 20 });
    expect(sheets.length).toBeGreaterThan(2);
    const frames = screenFrames(printScene, sheets, { nodes: screen.nodes, edges: screen.edges });
    // Every line sheet, in the printed numbering; the cover is what lies outside the frames.
    expect(frames.map((f) => f.no)).toEqual(sheets.filter((sh) => sh.top !== null).map((sh) => sh.no));
    const personOf = new Map(print.nodes.map((n) => [n.id, n.person!.id]));
    for (const f of frames) {
      const people = new Set([...sheets[f.no - 1].ids].map((id) => personOf.get(id)));
      const mine = screen.nodes.filter((n) => people.has(n.person!.id));
      expect(mine.length).toBe(people.size);
      expect(f.x).toBe(Math.min(...mine.map((n) => n.x)));
      expect(f.x + f.w).toBe(Math.max(...mine.map((n) => n.x + n.w)));
      expect(f.y).toBe(Math.min(...mine.map((n) => n.y)));
      expect(f.y + f.h).toBe(Math.max(...mine.map((n) => n.y + n.h)));
    }
  });

  it('frames_nothing_when_the_print_is_one_sheet', () => {
    const screen = bigAround();
    const scene: Scene = { nodes: screen.nodes, edges: screen.edges };
    const cover = splitSheets(scene, [screen.root], { fits: always }).slice(0, 1);
    expect(screenFrames(scene, cover, scene)).toEqual([]);
  });
});

describe('branchNames', () => {
  it('keeps_distinct_surnames_plain', () => {
    expect(branchNames([{ surname: 'Bondar', given: 'Taras' }, { surname: 'Melnyk', given: 'Oksana' }])).toEqual(['Bondar', 'Melnyk']);
  });

  it('adds_the_given_name_to_a_shared_surname_and_a_number_when_that_is_shared_too', () => {
    expect(
      branchNames([
        { surname: 'Bondar', given: 'Taras' },
        { surname: 'Bondar', given: 'Orest' },
        { surname: 'Tkach', given: 'Lev' },
        { surname: 'Tkach', given: 'Lev' },
        { surname: 'Melnyk', given: '' },
        { surname: 'Melnyk', given: '' },
      ]),
    ).toEqual(['Bondar, Taras', 'Bondar, Orest', 'Tkach, Lev', 'Tkach, Lev (2)', 'Melnyk', 'Melnyk (2)']);
  });
});
