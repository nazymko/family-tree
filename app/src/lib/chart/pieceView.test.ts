import { describe, expect, it } from 'vitest';
import type { Rect } from '../api/types';
import { NAME_PX, layoutA3, printedPt, targetScale } from './a3';
import type { ChartDirection } from './direction';
import { compactX, sceneBounds } from './picture';
import { fitsTarget, pickView, tagDir, tagRect, viewsOf, type DirScene, type PieceView } from './pieceView';
import { cutPieces, nodeKeys, type Piece } from './sheets';
import { loadFam } from './testing/famScenes';

const DIRS: ChartDirection[] = ['top', 'bottom', 'left', 'right'];
const OPTS = { choice: 'auto' as const, header: true, tagW: 60, tagH: 14 };
const upTo = (n: number) => (p: Piece) => p.full.size + p.refs.size <= n;

function famDirs(name: 'fam-a' | 'fam-b'): { dirs: DirScene[]; root: number } {
  const dirs = DIRS.map((d) => {
    const { scene } = loadFam(name, d);
    return { direction: d, scene, keys: nodeKeys(scene) };
  });
  return { dirs, root: loadFam(name, 'top').root };
}

const famA = famDirs('fam-a');
const cutA = famA.dirs[0];
const piecesA = cutPieces(cutA.scene, [famA.root], { fits: upTo(120) });

const overlaps = (a: Rect, b: Rect) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

describe('viewsOf', () => {
  it('children_on_other_pieces_share_one_box', () => {
    let groups = 0;
    for (const piece of piecesA) {
      // In the cut scene: children (references here) of the same parents shown here, with their pieces.
      const parentsOf = new Map<number, number[]>();
      for (const e of cutA.scene.edges)
        if (e.kind === 'parent' && piece.refs.has(e.to) && !piece.refs.has(e.from) && (piece.full.has(e.from) || piece.plain.has(e.from)))
          parentsOf.set(e.to, [...(parentsOf.get(e.to) ?? []), e.from]);
      const fams = new Map<string, number[]>();
      for (const [c, ps] of parentsOf) fams.set([...ps].sort().join(','), [...(fams.get([...ps].sort().join(',')) ?? []), c]);
      for (const v of viewsOf(piece, cutA, famA.dirs, OPTS)) {
        const dk = famA.dirs.find((d) => d.direction === v.direction)!.keys;
        const shown = new Set(v.scene.nodes.map((n) => dk.get(n.id)!));
        const idOf = new Map(v.scene.nodes.map((n) => [dk.get(n.id)!, n.id]));
        for (const kids of fams.values()) {
          if (kids.length < 2) continue;
          const here = kids.filter((c) => shown.has(cutA.keys.get(c)!));
          expect(here.length, `piece ${piece.index} ${v.direction}`).toBe(1);
          const rep = idOf.get(cutA.keys.get(here[0])!)!;
          expect(v.groups.get(rep)).toEqual([...new Set(kids.map((c) => piece.refs.get(c)!))].sort((x, y) => x - y));
          groups++;
        }
      }
    }
    expect(groups).toBeGreaterThan(0);
  });

  it('same_people_every_direction', () => {
    const piece = piecesA[1];
    const keys = new Set([...piece.full, ...piece.refs.keys(), ...piece.plain].map((id) => cutA.keys.get(id)!));
    const views = viewsOf(piece, cutA, famA.dirs, OPTS);
    expect(views.map((v) => v.direction)).toEqual(DIRS);
    for (const v of views) {
      const dk = famA.dirs.find((d) => d.direction === v.direction)!.keys;
      const shown = new Set(v.scene.nodes.map((n) => dk.get(n.id)!));
      // Everyone in full and every plain repeat; references, except children sharing their family's one box.
      for (const k of shown) expect(keys.has(k)).toBe(true);
      for (const id of [...piece.full, ...piece.plain]) expect(shown.has(cutA.keys.get(id)!)).toBe(true);
      const shared = [...v.groups.values()].length;
      expect(v.refs.size).toBeGreaterThanOrEqual(shared);
      expect(v.refs.size).toBeLessThanOrEqual(piece.refs.size);
    }
  });

  it('compacted_both_axes', () => {
    const line = piecesA.filter((p) => p.top !== null && cutA.scene.nodes.find((n) => n.id === p.top)!.gen <= -3).sort((a, b) => b.full.size - a.full.size)[0];
    expect(line).toBeDefined();
    const left = famA.dirs.find((d) => d.direction === 'left')!;
    const view = viewsOf(line, cutA, [left], OPTS)[0];
    const keys = new Set(view.scene.nodes.map((n) => left.keys.get(n.id)!));
    const keep = new Set(left.scene.nodes.filter((n) => keys.has(left.keys.get(n.id)!)).map((n) => n.id));
    const xOnly = sceneBounds(compactX({ nodes: left.scene.nodes.filter((n) => keep.has(n.id)), edges: left.scene.edges.filter((e) => keep.has(e.from) && keep.has(e.to)) }));
    expect(view.bounds.h).toBeLessThan(xOnly.h);
    expect(view.bounds.w).toBeLessThanOrEqual(xOnly.w + OPTS.tagW + 2);
  });

  it('tags_inside_bounds_and_clear', () => {
    for (const piece of piecesA) {
      for (const v of viewsOf(piece, cutA, famA.dirs, OPTS)) {
        const byId = new Map(v.scene.nodes.map((n) => [n.id, n]));
        const b = v.bounds;
        for (const id of v.refs.keys()) {
          if (v.groups.has(id)) continue; // names its sheets inside, no tag
          const r = tagRect(byId.get(id)!, tagDir(v, id), OPTS.tagW, OPTS.tagH);
          expect(r.x >= b.x - 1e-6 && r.y >= b.y - 1e-6 && r.x + r.w <= b.x + b.w + 1e-6 && r.y + r.h <= b.y + b.h + 1e-6).toBe(true);
          for (const n of v.scene.nodes) if (n.id !== id && overlaps(r, n)) expect.fail(`piece ${piece.index} ${v.direction}: tag of ${id} on box ${n.id}`);
        }
      }
    }
  });
});

describe('tags and lines', () => {
  it('tags_clear_of_their_own_lines', () => {
    let flipped = 0;
    for (const piece of piecesA) {
      for (const v of viewsOf(piece, cutA, famA.dirs, OPTS)) {
        flipped += v.flip.size;
        const byId = new Map(v.scene.nodes.map((n) => [n.id, n]));
        for (const id of v.refs.keys()) {
          if (v.groups.has(id)) continue; // names its sheets inside, no tag
          const r = tagRect(byId.get(id)!, tagDir(v, id), OPTS.tagW, OPTS.tagH);
          for (const e of v.scene.edges) {
            if (e.from !== id && e.to !== id) continue;
            for (let i = 0; i + 3 < e.pts.length; i += 2) {
              // Connectors are orthogonal: a segment's box is the segment.
              const seg = { x: Math.min(e.pts[i], e.pts[i + 2]), y: Math.min(e.pts[i + 1], e.pts[i + 3]), w: Math.abs(e.pts[i + 2] - e.pts[i]), h: Math.abs(e.pts[i + 3] - e.pts[i + 1]) };
              const hit = seg.x < r.x + r.w && r.x < seg.x + seg.w && seg.y < r.y + r.h && r.y < seg.y + seg.h;
              if (hit) expect.fail(`piece ${piece.index} ${v.direction}: tag of ${id} on its line ${e.kind}`);
            }
          }
        }
      }
    }
    expect(flipped).toBeGreaterThan(0);
  });
});

function view(direction: ChartDirection, scale: number, w: number, h: number): PieceView {
  const bounds = { x: 0, y: 0, w, h };
  return { direction, scene: { nodes: [], edges: [] }, refs: new Map(), flip: new Set(), side: new Map(), groups: new Map(), bounds, standalone: { ...layoutA3(bounds), scale } };
}

describe('pickView', () => {
  const t = targetScale();

  it('picks_smallest_area_that_fits', () => {
    const v = pickView([view('top', t * 1.2, 1000, 900), view('left', t * 1.1, 800, 700), view('right', t * 0.9, 100, 100)], 'top');
    expect(v.direction).toBe('left');
    expect(fitsTarget([view('top', t * 0.9, 1, 1), view('left', t, 1, 1)])).toBe(true);
    expect(fitsTarget([view('top', t * 0.9, 1, 1)])).toBe(false);
  });

  it('picks_largest_when_none_fits', () => {
    const v = pickView([view('top', t * 0.5, 100, 100), view('bottom', t * 0.7, 900, 900), view('left', t * 0.6, 10, 10)], 'top');
    expect(v.direction).toBe('bottom');
  });

  it('tie_keeps_screen_direction', () => {
    expect(pickView([view('top', t * 1.2, 1000, 1000), view('left', t * 1.2, 1000, 1000.5)], 'left').direction).toBe('left');
    expect(pickView([view('top', t * 1.2, 1000, 1000), view('left', t * 1.2, 1000, 1000)], 'bottom').direction).toBe('top');
    expect(pickView([view('right', t * 0.5, 1, 1), view('bottom', t * 0.5, 1, 1)], 'left').direction).toBe('bottom');
  });
});

describe('targetScale', () => {
  it('target_scale_is_8pt', () => {
    expect(Math.abs(printedPt(NAME_PX, targetScale()) - 8)).toBeLessThan(1e-9);
  });
});
