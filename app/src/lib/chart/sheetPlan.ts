import type { ChartNode } from '../api/types';
import { NAME_PX, printedPt, targetScale, type A3Choice, type A3Orientation } from './a3';
import type { ChartDirection } from './direction';
import { packPages, slotFits, type PackItem } from './pack';
import { cardsFit, fitsTarget, pickView, viewsOf, type DirScene, type PieceView, type ViewOptions } from './pieceView';
import { cutPieces, type NodeKey, type Piece } from './sheets';

/**
 * The plan of a packed "sheets by branch" set (spec amendment 21.11.2026): one page when the whole chart reads at the
 * target size; otherwise the chart cut into pieces (`cutPieces`), each in its own direction (`pickView`), shelf-packed
 * onto A3 pages (`packPages`), the references resolved to page numbers.
 */

export interface PlanOptions {
  choice: A3Choice;
  header: boolean;
  /** The direction on the screen: the cut is made on its layout, and it wins ties. */
  screenDir: ChartDirection;
  /** The root and chosen people, as keys. */
  centres: NodeKey[];
  /** Width (layout px, at `CAPTION_PT`) of the caption of the piece cut at `top`. */
  capW: (top: ChartNode) => number;
  /** Reference tag size, world units at the target scale (see `ViewOptions`). */
  tagW: number;
  tagH: number;
}

export interface PlannedPiece {
  index: number;
  /** The node (of the screen direction's layout) the piece was cut at; null for the cover. */
  top: ChartNode | null;
  view: PieceView;
  /** 1-based page number. */
  page: number;
  /** Top-left of the drawing on the page, layout px; top of the caption band (null without a caption). */
  x: number;
  y: number;
  capY: number | null;
  /** Reference box node id (in `view.scene`) -> page holding that person in full. */
  refPages: Map<number, number>;
  unsplittable: boolean;
}

export interface PlannedPage {
  no: number;
  orientation: A3Orientation;
  scale: number;
  namePt: number;
  pieces: PlannedPiece[];
}

export interface SheetPlan {
  /** The whole chart on one page, as "compact" prints it (no caption, no page numbers). */
  single: boolean;
  pages: PlannedPage[];
}

export function planSheets(dirs: DirScene[], o: PlanOptions): SheetPlan {
  const cut = dirs.find((d) => d.direction === o.screenDir) ?? dirs[0];
  if (!cut || cut.scene.nodes.length === 0) return { single: true, pages: [] };
  const vo: ViewOptions = { choice: o.choice, header: o.header, tagW: o.tagW, tagH: o.tagH };
  const t = targetScale();
  const byId = new Map(cut.scene.nodes.map((n) => [n.id, n]));
  const idOf = new Map([...cut.keys].map(([id, k]) => [k, id]));
  const centres = o.centres.map((k) => idOf.get(k)).filter((id): id is number => id !== undefined);
  const live = (n: ChartNode) => n.dupOf !== null && byId.has(n.dupOf);

  // (1) The whole chart on one page when it reads there.
  const whole: Piece = {
    index: 0,
    top: null,
    parent: null,
    full: new Set(cut.scene.nodes.filter((n) => !live(n)).map((n) => n.id)),
    refs: new Map(),
    plain: new Set(cut.scene.nodes.filter(live).map((n) => n.id)),
    unsplittable: false,
  };
  const wholeViews = viewsOf(whole, cut, dirs, vo);
  if (fitsTarget(wholeViews)) {
    const v = pickView(wholeViews, o.screenDir, () => false);
    const lay = v.standalone;
    const x = (v.bounds.x - lay.cam.x) * lay.scale;
    const y = (v.bounds.y - lay.cam.y) * lay.scale;
    const piece: PlannedPiece = { index: 0, top: null, view: v, page: 1, x, y, capY: null, refPages: new Map(), unsplittable: false };
    return { single: true, pages: [{ no: 1, orientation: lay.orientation, scale: lay.scale, namePt: printedPt(NAME_PX, lay.scale), pieces: [piece] }] };
  }

  // (2) Pieces, cut on the screen direction's layout. A piece fits when some direction holds it, caption band
  // included, on one page at the target size.
  const capOf = (p: Piece) => (p.top === null ? 0 : o.capW(byId.get(p.top)!));
  const slotOk = (p: Piece) => (v: PieceView) => slotFits({ id: p.index, w: v.bounds.w, h: v.bounds.h, capW: capOf(p) }, t, o.choice, o.header);
  // A piece's view in one direction, made once (the cut asks again for the pieces it keeps).
  const made = new Map<string, PieceView>();
  const viewIn = (p: Piece, d: DirScene): PieceView => {
    const sig = `${d.direction}|${[...p.full].sort((a, b) => a - b).join(',')}|${[...p.refs].sort((a, b) => a[0] - b[0]).join(';')}|${[...p.plain].sort((a, b) => a - b).join(',')}`;
    let v = made.get(sig);
    if (!v) {
      v = viewsOf(p, cut, [d], vo)[0];
      made.set(sig, v);
    }
    return v;
  };
  const pieces = cutPieces(cut.scene, centres, {
    fits: (p) => cardsFit(p, cut, vo) && dirs.some((d) => slotOk(p)(viewIn(p, d))),
  });

  // (3) Each piece's direction.
  const views = pieces.map((p) => pickView(dirs.map((d) => viewIn(p, d)), o.screenDir, slotOk(p)));

  // (4) Packing.
  const items: PackItem[] = pieces.map((p, i) => ({ id: p.index, w: views[i].bounds.w, h: views[i].bounds.h, capW: capOf(p) }));
  const packed = packPages(items, { target: t, choice: o.choice, header: o.header });

  // (5) Page numbers and references.
  const pageOf = new Map<number, number>();
  packed.forEach((pg, i) => pg.slots.forEach((s) => pageOf.set(s.id, i + 1)));
  const pages: PlannedPage[] = packed.map((pg, i) => ({
    no: i + 1,
    orientation: pg.orientation,
    scale: pg.scale,
    namePt: printedPt(NAME_PX, pg.scale),
    pieces: pg.slots.map((s) => {
      const p = pieces[s.id];
      const view = views[s.id];
      const refPages = new Map([...view.refs].map(([id, k]) => [id, pageOf.get(k)!]));
      return { index: p.index, top: p.top === null ? null : byId.get(p.top)!, view, page: i + 1, x: s.x, y: s.y, capY: s.capY, refPages, unsplittable: p.unsplittable };
    }),
  }));
  return { single: false, pages };
}
