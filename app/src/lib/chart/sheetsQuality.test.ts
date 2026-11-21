// Quality gates of the packed "sheets by branch" set on synthetic family-shaped charts (plan 2026-11-22, Task 5).
import { describe, expect, it } from 'vitest';
import type { ChartEdge, ChartNode } from '../api/types';
import { A3_MIN_PT, SHEET_TARGET_PT, a3Area, mmToPx, targetScale } from './a3';
import type { ChartDirection } from './direction';
import { CAPTION_MAX_MM, CAPTION_MM } from './pack';
import type { DirScene } from './pieceView';
import type { Scene } from './picture';
import { planSheets, type PlanOptions, type PlannedPiece, type SheetPlan } from './sheetPlan';
import { nodeKeys } from './sheets';
import { FAMS, TODAY_SHEETS, famName, loadFam, type FamName } from './testing/famScenes';

const DIRS: ChartDirection[] = ['top', 'bottom', 'left', 'right'];
const CAP_W = 200;

function famDirs(name: FamName, graft?: (s: Scene, d: ChartDirection) => void): { dirs: DirScene[]; root: number } {
  const dirs = DIRS.map((d) => {
    const { scene } = loadFam(name, d);
    graft?.(scene, d);
    return { direction: d, scene, keys: nodeKeys(scene) };
  });
  return { dirs, root: loadFam(name, 'top').root };
}

const opts = (dirs: DirScene[], root: number): PlanOptions => ({
  choice: 'auto',
  header: true,
  screenDir: 'top',
  centres: [dirs[0].keys.get(root)!],
  capW: () => CAP_W,
  tagW: 60,
  tagH: 14,
});

const keysOf = (dirs: DirScene[], p: PlannedPiece) => dirs.find((d) => d.direction === p.view.direction)!.keys;
const fullOf = (p: PlannedPiece) => p.view.scene.nodes.filter((n) => n.dupOf === null && !p.view.refs.has(n.id));

/** The slot area (layout px²) of a planned piece at its page's scale. */
function slotArea(p: PlannedPiece, scale: number): number {
  const band = p.capY === null ? 0 : mmToPx(CAPTION_MM);
  const cap = p.capY === null ? 0 : Math.min(CAP_W, mmToPx(CAPTION_MAX_MM));
  return Math.max(p.view.bounds.w * scale, cap) * (p.view.bounds.h * scale + band);
}

interface Run {
  dirs: DirScene[];
  plan: SheetPlan;
}
const runs = new Map<FamName, Run>();
function run(f: FamName): Run {
  let r = runs.get(f);
  if (!r) {
    const { dirs, root } = famDirs(f);
    r = { dirs, plan: planSheets(dirs, opts(dirs, root)) };
    runs.set(f, r);
  }
  return r;
}

function measures(f: FamName) {
  const { plan } = run(f);
  const t = targetScale();
  const all = plan.pages.flatMap((pg) => pg.pieces.map((p) => ({ pg, p })));
  const people = all.reduce((a, x) => a + fullOf(x.p).length, 0);
  const at = (pt: number) => all.filter((x) => x.pg.namePt >= pt - 1e-9).reduce((a, x) => a + fullOf(x.p).length, 0) / people;
  const fills = plan.pages.map((pg) => {
    const area = a3Area(pg.orientation, true);
    return pg.pieces.reduce((a, p) => a + slotArea(p, pg.scale), 0) / (area.w * area.h);
  });
  const land = a3Area('landscape', true);
  const lb = Math.ceil(all.reduce((a, x) => a + slotArea(x.p, t), 0) / (land.w * land.h));
  const head = fills.length > 1 ? fills.slice(0, -1) : fills;
  const cardFills = all.map((x) => pieceFill(x.p)).sort((a, b) => a - b);
  const cover = plan.pages.find((pg) => pg.pieces.some((p) => p.index === 0))!;
  return {
    pages: plan.pages.length,
    people,
    lb,
    at6: at(A3_MIN_PT),
    at8: at(SHEET_TARGET_PT),
    fill: head.reduce((a, b) => a + b, 0) / head.length,
    pieces: all.length,
    pieceFill: cardFills[Math.floor(cardFills.length / 2)],
    coverPt: cover.namePt,
  };
}

/** Share of a piece's drawing taken by its cards. */
function pieceFill(p: PlannedPiece): number {
  const cards = p.view.scene.nodes.reduce((a, n) => a + n.w * n.h, 0);
  return cards / (p.view.bounds.w * p.view.bounds.h);
}

describe('packed sheets on family-shaped charts', { timeout: 30_000 }, () => {
  it('everyone_once', () => {
    for (const f of FAMS) {
      const { dirs, plan } = run(f);
      const want = [...dirs[0].keys].filter(([id]) => dirs[0].scene.nodes.find((n) => n.id === id)!.dupOf === null).map(([, k]) => k);
      const got = plan.pages.flatMap((pg) => pg.pieces.flatMap((p) => fullOf(p).map((n) => keysOf(dirs, p).get(n.id)!)));
      expect(got.length, f).toBe(new Set(got).size);
      expect(new Set(got), f).toEqual(new Set(want));
    }
  });

  it('refs_point_to_the_right_page', () => {
    for (const f of FAMS) {
      const { dirs, plan } = run(f);
      const hold = new Map<string, number>();
      for (const pg of plan.pages) for (const p of pg.pieces) for (const n of fullOf(p)) hold.set(n.person!.id, pg.no);
      for (const pg of plan.pages)
        for (const p of pg.pieces) {
          const byId = new Map(p.view.scene.nodes.map((n) => [n.id, n]));
          for (const [id, page] of p.refPages) {
            expect(page, `${f} ${keysOf(dirs, p).get(id)}`).toBe(hold.get(byId.get(id)!.person!.id));
          }
        }
    }
  });

  it('at_least_95_percent_at_6pt', () => {
    for (const f of FAMS) expect(measures(f).at6, f).toBeGreaterThanOrEqual(0.95);
  });

  it('shared_pages_at_target', () => {
    for (const f of FAMS) {
      for (const pg of run(f).plan.pages) {
        if (pg.pieces.length >= 2) expect(pg.namePt, `${f} page ${pg.no}`).toBeGreaterThanOrEqual(SHEET_TARGET_PT - 1e-9);
        if (pg.namePt < A3_MIN_PT) {
          expect(pg.pieces.length, `${f} page ${pg.no}`).toBe(1);
          expect(pg.pieces[0].unsplittable, `${f} page ${pg.no}`).toBe(true);
        }
      }
    }
  });

  it('fill', () => {
    for (const f of FAMS) expect(measures(f).fill, f).toBeGreaterThanOrEqual(0.5);
  });

  it('page_count_near_bound', () => {
    for (const f of FAMS) {
      const m = measures(f);
      expect(m.pages, f).toBeLessThanOrEqual(Math.ceil(1.5 * m.lb) + 1);
    }
  });

  it('giant_page_warned_others_at_target', () => {
    // A grandparent's brother or sister gets a child with a partner and 300 children, in a row in every direction.
    const top = loadFam('fam-b', 'top').scene;
    const sib = top.nodes.find((n) => n.gen === -2 && n.dupOf === null && !top.edges.some((e) => e.kind === 'parent' && e.from === n.id))!;
    expect(sib).toBeDefined();
    const sibKey = sib.person!.id;
    const nav = { parent: null, child: null, prev: null, next: null };
    const graft = (s: Scene, d: ChartDirection) => {
      const anchor = s.nodes.find((n) => n.person?.id === sibKey)!;
      let next = Math.max(...s.nodes.map((n) => n.id)) + 1;
      const across = d === 'top' || d === 'bottom';
      const step = (k: number) => (across ? { x: anchor.x + k * 140, y: anchor.y + (d === 'top' ? 1 : -1) * 88 } : { x: anchor.x + (d === 'left' ? 1 : -1) * 150, y: anchor.y + k * 66 });
      const add = (key: string, at: { x: number; y: number }, gen: number): ChartNode => {
        const n: ChartNode = {
          id: next++,
          person: { id: key, name: famName(next), lifespan: '', sex: 'male', living: false, private: false, siblingMarker: false },
          x: at.x,
          y: at.y,
          w: anchor.w,
          h: anchor.h,
          gen,
          ahnen: null,
          dupOf: null,
          moreUp: false,
          moreDown: false,
          nav,
          label: famName(next),
        };
        s.nodes.push(n);
        return n;
      };
      const line = (a: ChartNode, b: ChartNode, kind: ChartEdge['kind']) => s.edges.push({ from: a.id, to: b.id, kind, pts: [a.x + a.w / 2, a.y + a.h / 2, b.x + b.w / 2, b.y + b.h / 2] });
      const child = add('kg-child', step(0), anchor.gen + 1);
      line(anchor, child, 'parent');
      const far = (k: number) => (across ? { x: step(k).x, y: step(k).y + (d === 'top' ? 88 : -88) } : { x: step(k).x + (d === 'left' ? 150 : -150), y: step(k).y });
      const partner = add('kg-partner', step(1), anchor.gen + 1);
      line(child, partner, 'partner');
      for (let i = 0; i < 300; i++) {
        const kid = add(`kg-${i}`, far(i), anchor.gen + 2);
        line(child, kid, 'parent');
        line(partner, kid, 'parent');
      }
    };
    const { dirs, root } = famDirs('fam-b', graft);
    const plan = planSheets(dirs, opts(dirs, root));
    const giant = plan.pages.find((pg) => pg.pieces.some((p) => fullOf(p).some((n) => n.person!.id === 'kg-0')))!;
    expect(giant.pieces.length).toBe(1);
    expect(giant.pieces[0].unsplittable).toBe(true);
    expect(giant.namePt).toBeLessThan(SHEET_TARGET_PT);
    // No other page drops below the target because of it: fam-b alone already has pieces that cannot be split further
    // and print a little below 8 pt on pages of their own; with the giant there is exactly one such page more.
    const below = (p: SheetPlan) => p.pages.filter((pg) => pg.namePt < SHEET_TARGET_PT - 1e-9);
    expect(below(plan).length).toBe(below(run('fam-b').plan).length + 1);
    for (const pg of below(plan)) {
      expect(pg.pieces.length, `page ${pg.no}`).toBe(1);
      expect(pg.pieces[0].unsplittable, `page ${pg.no}`).toBe(true);
    }
  });

  it('deterministic', () => {
    const { dirs, root } = famDirs('fam-c');
    const a = planSheets(dirs, opts(dirs, root));
    const b = planSheets(dirs, opts(dirs, root));
    const shape = (p: SheetPlan) => p.pages.map((pg) => [pg.orientation, pg.scale, pg.pieces.map((x) => [x.index, x.view.direction, x.x, x.y, [...x.refPages]])]);
    expect(shape(a)).toEqual(shape(b));
  });

  it('perf_1500_people_four_directions', () => {
    const { dirs, root } = famDirs('fam-d');
    expect(dirs[0].scene.nodes.length).toBeGreaterThanOrEqual(1500);
    planSheets(dirs, opts(dirs, root));
    const t0 = performance.now();
    planSheets(dirs, opts(dirs, root));
    const ms = performance.now() - t0;
    console.log(`planSheets fam-d (4 directions): ${Math.round(ms)} ms`);
    expect(ms).toBeLessThan(2000);
  });

  it('table', () => {
    const rows = FAMS.map((f) => {
      const m = measures(f);
      const pts = run(f).plan.pages.map((pg) => Math.round(pg.namePt * 10) / 10);
      return { tree: f, people: m.people, today: TODAY_SHEETS[f], pages: m.pages, pieces: m.pieces, LB: m.lb, at6: Math.round(m.at6 * 1000) / 10, at8: Math.round(m.at8 * 1000) / 10, fill: Math.round(m.fill * 100) / 100, pieceFill: Math.round(m.pieceFill * 100) / 100, coverPt: Math.round(m.coverPt * 10) / 10, minPt: Math.min(...pts), maxPt: Math.max(...pts) };
    });
    console.table(rows);
    expect(rows.length).toBe(4);
  });
});
