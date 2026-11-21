import { describe, expect, it } from 'vitest';
import type { Rect } from '../api/types';
import { NAME_PX, layoutA3, printedPt, targetScale } from './a3';
import type { ChartDirection } from './direction';
import { compactX, sceneBounds } from './picture';
import { fitsTarget, pickView, tagRect, viewsOf, type DirScene, type PieceView } from './pieceView';
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
  it('same_people_every_direction', () => {
    const piece = piecesA[1];
    const keys = new Set([...piece.full, ...piece.refs.keys(), ...piece.plain].map((id) => cutA.keys.get(id)!));
    const views = viewsOf(piece, cutA, famA.dirs, OPTS);
    expect(views.map((v) => v.direction)).toEqual(DIRS);
    for (const v of views) {
      const dk = famA.dirs.find((d) => d.direction === v.direction)!.keys;
      expect(new Set(v.scene.nodes.map((n) => dk.get(n.id)!))).toEqual(keys);
      expect(v.refs.size).toBe(piece.refs.size);
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
          const r = tagRect(byId.get(id)!, v.direction, OPTS.tagW, OPTS.tagH);
          expect(r.x >= b.x - 1e-6 && r.y >= b.y - 1e-6 && r.x + r.w <= b.x + b.w + 1e-6 && r.y + r.h <= b.y + b.h + 1e-6).toBe(true);
          for (const n of v.scene.nodes) if (n.id !== id && overlaps(r, n)) expect.fail(`piece ${piece.index} ${v.direction}: tag of ${id} on box ${n.id}`);
        }
      }
    }
  });
});

function view(direction: ChartDirection, scale: number, w: number, h: number): PieceView {
  const bounds = { x: 0, y: 0, w, h };
  return { direction, scene: { nodes: [], edges: [] }, refs: new Map(), bounds, standalone: { ...layoutA3(bounds), scale } };
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
