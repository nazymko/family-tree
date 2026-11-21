import { describe, expect, it } from 'vitest';
import type { ChartEdge, ChartNode } from '../api/types';
import type { Scene } from './picture';
import { PIECE_MIN_PEOPLE, cutPieces, nodeKeys, refTagAt, screenFrames, type Piece } from './sheets';
import { planSheets } from './sheetPlan';
import type { ChartDirection } from './direction';
import { FAMS, loadFam } from './testing/famScenes';

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

const always = () => true;

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

describe('cutPieces', () => {
  const upTo = (n: number) => (p: Piece) => p.full.size + p.refs.size <= n;

  /** Everyone not a repeat is in full on exactly one piece. */
  function expectFullOnce(scene: Scene, pieces: Piece[]) {
    const seen = new Map<number, number>();
    for (const p of pieces) for (const id of p.full) seen.set(id, (seen.get(id) ?? 0) + 1);
    const full = scene.nodes.filter((n) => n.dupOf === null).map((n) => n.id);
    expect([...seen.keys()].sort((a, b) => a - b)).toEqual(full.sort((a, b) => a - b));
    expect([...seen.values()].every((k) => k === 1)).toBe(true);
  }

  /** Every reference names a piece that holds that person (or the repeat's first occurrence) in full. */
  function expectRefsToHolders(scene: Scene, pieces: Piece[]) {
    const byId = new Map(scene.nodes.map((n) => [n.id, n]));
    for (const p of pieces) {
      for (const [id, k] of p.refs) {
        expect(k).not.toBe(p.index);
        const n = byId.get(id)!;
        const real = n.dupOf !== null && byId.has(n.dupOf) ? n.dupOf : id;
        expect(pieces[k].full.has(real), `ref ${id} on piece ${p.index} -> ${k}`).toBe(true);
      }
      for (const id of p.plain) expect(p.full.has(byId.get(id)!.dupOf!)).toBe(true);
    }
  }

  function grandTree() {
    const t = new Tree();
    const g = base(t);
    for (const gp of [g.ff, g.fm, g.mf, g.mm]) chain(t, gp, 3, 2);
    return { t, g };
  }

  it('whole_fits_is_one_piece', () => {
    const { t, g } = grandTree();
    const pieces = cutPieces(t.scene(), [g.c], { fits: always });
    expect(pieces.length).toBe(1);
    expect(pieces[0].full.size).toBe(t.nodes.length);
    expect(pieces[0].refs.size).toBe(0);
    expect(pieces[0].top).toBeNull();
    expect(pieces[0].unsplittable).toBe(false);
  });

  it('everyone_once', () => {
    const { t, g } = grandTree();
    const pieces = cutPieces(t.scene(), [g.c], { fits: upTo(15) });
    expect(pieces.length).toBeGreaterThan(1);
    expectFullOnce(t.scene(), pieces);
    expectRefsToHolders(t.scene(), pieces);
  });

  it('spine_stays_on_cover', () => {
    const { t, g } = grandTree();
    const pieces = cutPieces(t.scene(), [g.c], { fits: upTo(15) });
    for (const id of [g.c, g.f, g.m, g.ff, g.fm, g.mf, g.mm]) expect(pieces[0].full.has(id)).toBe(true);
  });

  /** Centre, parents, grandparents (and theirs); the father's parents have ten childless children besides him, two married. */
  function auntsColumn() {
    const t = new Tree();
    const g = base(t);
    for (const gp of [g.ff, g.fm, g.mf, g.mm]) chain(t, gp, 1);
    const aunts: number[] = [];
    for (let i = 0; i < 10; i++) {
      const a = t.child(g.ff, g.fm);
      aunts.push(a);
      if (i % 4 === 1) aunts.push(t.partner(a));
    }
    return { t, g, aunts };
  }

  it('childless_brothers_and_sisters_leave_as_one_piece', () => {
    const { t, g, aunts } = auntsColumn();
    // Every block of the hang tree here is one person or a couple: only the group can move.
    const pieces = cutPieces(t.scene(), [g.c], { fits: (p) => p.full.size <= 15 });
    expectFullOnce(t.scene(), pieces);
    expectRefsToHolders(t.scene(), pieces);
    const group = pieces.find((p) => p.siblingsOf !== null)!;
    expect(group, 'a brothers-and-sisters piece').toBeDefined();
    expect([...group.full].sort((a, b) => a - b)).toEqual([...aunts].sort((a, b) => a - b));
    expect(group.siblingsOf).toBe(g.f);
    expect(pieces[0].full.has(g.f)).toBe(true);
    expect(pieces[0].unsplittable).toBe(false);
  });

  it('a_group_needs_childless_members', () => {
    const { t, g, aunts } = auntsColumn();
    // One aunt gets a child: she is no longer part of the group, the rest still are.
    const kid = t.child(aunts[0]);
    const pieces = cutPieces(t.scene(), [g.c], { fits: (p) => p.full.size <= 15 });
    const group = pieces.find((p) => p.siblingsOf !== null)!;
    expect(group.full.has(aunts[0])).toBe(false);
    expect(group.full.has(kid)).toBe(false);
    expect(group.full.has(aunts[1])).toBe(true);
  });

  /** Centre, parents, grandparents; an aunt with a 13-person family and an uncle with four children. */
  function auntTree() {
    const t = new Tree();
    const g = base(t);
    const aunt = t.child(g.ff, g.fm);
    for (let i = 0; i < 3; i++) {
      const kid = t.child(aunt);
      const p = t.partner(kid);
      t.child(kid, p);
      t.child(kid, p);
    }
    const uncle = t.child(g.mf, g.mm);
    for (let i = 0; i < 4; i++) t.child(uncle);
    return { t, g, aunt, uncle };
  }

  it('largest_block_moves_first', () => {
    const { t, g, aunt, uncle } = auntTree();
    const pieces = cutPieces(t.scene(), [g.c], { fits: upTo(20) });
    expect(pieces.length).toBe(2);
    expect(pieces[1].top).toBe(aunt);
    expect(pieces[1].parent).toBe(0);
    expect(pieces[1].full.size).toBe(13);
    expect(pieces[0].full.has(uncle)).toBe(true);
    expectFullOnce(t.scene(), pieces);
  });

  it('collateral_head_leaves_reference', () => {
    const { t, g, aunt } = auntTree();
    const pieces = cutPieces(t.scene(), [g.c], { fits: upTo(20) });
    expect(pieces[0].refs.get(aunt)).toBe(1);
    expect(pieces[1].refs.get(g.ff)).toBe(0);
    expect(pieces[1].refs.get(g.fm)).toBe(0);
    expect(pieces[1].full.has(aunt)).toBe(true);
    expect(pieces[0].full.has(aunt)).toBe(false);
  });

  it('line_cut_is_two_way', () => {
    const t = new Tree();
    const g = base(t);
    chain(t, g.ff, 3, 4);
    const [ffF] = t.edges.filter((e) => e.kind === 'parent' && e.to === g.ff).map((e) => e.from);
    const pieces = cutPieces(t.scene(), [g.c], { fits: upTo(25) });
    expectFullOnce(t.scene(), pieces);
    const line = pieces.find((p) => p.top === ffF)!;
    expect(line).toBeDefined();
    expect(t.gen(ffF)).toBe(-3);
    expect(line.refs.get(g.ff)).toBe(0);
    expect(pieces[0].refs.get(ffF)).toBe(line.index);
    expectRefsToHolders(t.scene(), pieces);
  });

  it('small_blocks_never_move', () => {
    const { t, g } = grandTree();
    // A sister of the centre with one child: a block of two.
    const sis = t.child(g.f, g.m);
    t.child(sis);
    const pieces = cutPieces(t.scene(), [g.c], { fits: upTo(15) });
    expect(PIECE_MIN_PEOPLE).toBe(3);
    expect(pieces.some((p) => p.top === sis)).toBe(false);
    for (const p of pieces.slice(1)) expect(p.full.size).toBeGreaterThanOrEqual(3);
  });

  it('unsplittable_giant', () => {
    const t = new Tree();
    const c = t.add(0);
    const p = t.partner(c);
    for (let i = 0; i < 300; i++) t.child(c, p);
    const pieces = cutPieces(t.scene(), [c], { fits: upTo(50) });
    expect(pieces.length).toBe(1);
    expect(pieces[0].unsplittable).toBe(true);
  });

  it('pedigree_collapse', () => {
    const t = new Tree();
    const g = base(t);
    const ffLine = chain(t, g.ff, 3, 1);
    for (const gp of [g.fm, g.mf]) chain(t, gp, 3, 1);
    const mmLine = chain(t, g.mm, 3, 1);
    const x = ffLine.find((id) => t.gen(id) === -4)!;
    const top = mmLine.find((id) => t.gen(id) === -5)!;
    const dup = t.add(-6, x);
    t.edge(dup, top, 'parent');
    const scene = t.scene();
    const pieces = cutPieces(scene, [g.c], { fits: upTo(12) });
    expect(pieces.length).toBeGreaterThan(2);
    expectFullOnce(scene, pieces);
    expectRefsToHolders(scene, pieces);
    const holder = pieces.find((p) => p.full.has(x))!;
    const withDup = pieces.filter((p) => p.refs.has(dup) || p.plain.has(dup));
    expect(withDup.length).toBeGreaterThan(0);
    for (const p of withDup) {
      if (p.plain.has(dup)) expect(p.index).toBe(holder.index);
      else expect(p.refs.get(dup)).toBe(holder.index);
    }
    expect(pieces.some((p) => p.full.has(dup))).toBe(false);
  });

  it('several_centres_in_different_rows', () => {
    const t = new Tree();
    const g = base(t);
    for (const gp of [g.ff, g.fm, g.mf, g.mm]) chain(t, gp, 3);
    const b = t.add(-1);
    const [bf, bm] = t.parents(b);
    for (const gp of [...t.parents(bf), ...t.parents(bm)]) chain(t, gp, 3);
    const scene = t.scene();
    const pieces = cutPieces(scene, [g.c, b], { fits: upTo(12) });
    expect(pieces.length).toBeGreaterThan(2);
    expectFullOnce(scene, pieces);
    expectRefsToHolders(scene, pieces);
    expect(pieces[0].full.has(g.c)).toBe(true);
    expect(pieces[0].full.has(b)).toBe(true);
  });

  it('refs_point_to_holders', () => {
    const { t, g } = auntTree();
    chain(t, g.ff, 3, 3);
    chain(t, g.mm, 3, 3);
    const pieces = cutPieces(t.scene(), [g.c], { fits: upTo(20) });
    expect(pieces.length).toBeGreaterThan(2);
    expectRefsToHolders(t.scene(), pieces);
    for (const p of pieces.slice(1)) expect(pieces[p.parent!]).toBeDefined();
  });

  it('fam_fixtures_everyone_once', () => {
    for (const f of FAMS) {
      const { scene, root } = loadFam(f, 'left');
      const pieces = cutPieces(scene, [root], { fits: upTo(120) });
      expect(pieces.length, f).toBeGreaterThan(1);
      expectFullOnce(scene, pieces);
      expectRefsToHolders(scene, pieces);
      for (const p of pieces) if (!p.unsplittable) expect(upTo(120)(p), `${f} piece ${p.index}`).toBe(true);
    }
  });
});

describe('screenFrames', () => {
  const DIRS: ChartDirection[] = ['top', 'bottom', 'left', 'right'];
  const plan = (name: 'fam-b') => {
    const dirs = DIRS.map((d) => {
      const { scene } = loadFam(name, d);
      return { direction: d, scene, keys: nodeKeys(scene) };
    });
    const root = loadFam(name, 'top').root;
    const p = planSheets(dirs, { choice: 'auto', header: true, screenDir: 'top', centres: [dirs[0].keys.get(root)!], capW: () => 200, tagW: 60, tagH: 14 });
    const keysOf = (id: number, d: ChartDirection) => dirs.find((x) => x.direction === d)!.keys.get(id);
    return { p, keysOf };
  };

  it('one_frame_per_piece_with_its_page', () => {
    const { p, keysOf } = plan('fam-b');
    // The chart on screen: the same people, other ids and places (a screen layout of its own).
    const screen = loadFam('fam-b', 'top').scene;
    const moved: Scene = { nodes: screen.nodes.map((n) => ({ ...n, id: n.id + 5000, x: n.x * 1.4 + 7, y: n.y * 1.6 - 3, dupOf: n.dupOf === null ? null : n.dupOf + 5000 })), edges: [] };
    const frames = screenFrames(p, keysOf, moved);
    const pieces = p.pages.flatMap((pg) => pg.pieces.map((pc) => ({ pg, pc })));
    expect(frames.length).toBe(pieces.length);
    expect(frames.map((f) => f.no)).toEqual(pieces.map((x) => x.pg.no));
    // The cover is framed, on page 1.
    expect(frames[0].no).toBe(1);
    expect(pieces[0].pc.index).toBe(0);
    // Each frame holds exactly the screen boxes of its piece's people in full.
    pieces.forEach(({ pc }, i) => {
      const people = new Set(pc.view.scene.nodes.filter((n) => n.dupOf === null && !pc.view.refs.has(n.id)).map((n) => n.person!.id));
      const mine = moved.nodes.filter((n) => n.dupOf === null && people.has(n.person!.id));
      expect(mine.length).toBe(people.size);
      const f = frames[i];
      expect(f.x).toBe(Math.min(...mine.map((n) => n.x)));
      expect(f.y).toBe(Math.min(...mine.map((n) => n.y)));
      expect(f.x + f.w).toBeCloseTo(Math.max(...mine.map((n) => n.x + n.w)), 6);
      expect(f.y + f.h).toBeCloseTo(Math.max(...mine.map((n) => n.y + n.h)), 6);
    });
  });

  it('frames_nothing_when_the_set_is_one_page', () => {
    const { p, keysOf } = plan('fam-b');
    const one = { single: false, pages: p.pages.slice(0, 1) };
    expect(screenFrames(one, keysOf, loadFam('fam-b', 'top').scene)).toEqual([]);
    expect(screenFrames({ single: true, pages: [] }, keysOf, loadFam('fam-b', 'top').scene)).toEqual([]);
  });
});
