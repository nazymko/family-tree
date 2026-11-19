// The old maps drawn once into a picture of their own (the plane) and reused while the view moves. Stretching a scan
// onto its four corners takes up to 512 clipped triangles; doing that for every map on every frame of a pan made the
// Map lag. While the view moves, the plane is only shifted and scaled (one drawImage); when the view rests, or the maps
// change, it is drawn again for the new view. The plane reaches a quarter of the view past each edge, so a short pan
// shows no empty border.
import { drawWarped, screenQuad, type Quad, type WarpCtx } from './layers';
import { fromScreen, toScreen, type Camera, type World } from './projection';

/** An old map ready to draw: its corners in world points, its picture, and how opaque (0 to 1). */
export interface PlaneLayer {
  world: World[];
  img: CanvasImageSource;
  iw: number;
  ih: number;
  opacity: number;
}

/** How far past each edge of the view the plane reaches, as a share of the view. */
export const PLANE_MARGIN = 0.25;
/** Past this change of zoom while moving, the plane is drawn again rather than stretched. */
const MAX_STRETCH = 2;

type Canvas2D = HTMLCanvasElement;

function canvas(w: number, h: number): Canvas2D | null {
  if (typeof document === 'undefined') return null;
  const c = document.createElement('canvas');
  c.width = w;
  c.height = h;
  return c;
}

const imgIds = new WeakMap<object, number>();
let nextImg = 1;
function imgId(img: CanvasImageSource): number {
  let id = imgIds.get(img as object);
  if (!id) {
    id = nextImg++;
    imgIds.set(img as object, id);
  }
  return id;
}

/** What the plane shows: the same pictures, corners and opacities give the same key. */
export function planeKey(layers: PlaneLayer[]): string {
  return layers.map((l) => `${imgId(l.img)}:${l.iw}x${l.ih}:${l.opacity.toFixed(3)}:${l.world.map((p) => `${p.x.toFixed(9)},${p.y.toFixed(9)}`).join(';')}`).join('|');
}

/** The bounding box of a quad clipped to a w x h area, in whole pixels, or null when it misses it. */
export function clipBox(q: Quad, w: number, h: number): { x: number; y: number; w: number; h: number } | null {
  const xs = q.map((p) => p[0]);
  const ys = q.map((p) => p[1]);
  const x0 = Math.max(0, Math.floor(Math.min(...xs)) - 1);
  const y0 = Math.max(0, Math.floor(Math.min(...ys)) - 1);
  const x1 = Math.min(w, Math.ceil(Math.max(...xs)) + 1);
  const y1 = Math.min(h, Math.ceil(Math.max(...ys)) + 1);
  return x1 > x0 && y1 > y0 ? { x: x0, y: y0, w: x1 - x0, h: y1 - y0 } : null;
}

let scratch: Canvas2D | null = null;

/**
 * Paints the layers onto `g` (a w x h area in CSS pixels at `scale`; `g`'s transform maps CSS pixels of the area to its
 * own), for the camera `cam` looking at a view of vw x vh whose top left lies at (ox, oy) of the area. Each map is stretched fully opaque onto a scratch canvas and
 * laid on with its opacity in one go, so the seams of its triangles never show; only the map's own box is cleared and
 * copied, not the whole area.
 */
export function paintLayers(g: CanvasRenderingContext2D, layers: PlaneLayer[], cam: Camera, vw: number, vh: number, ox: number, oy: number, w: number, h: number, scale: number): number {
  let drawn = 0;
  const pw = Math.max(1, Math.round(w * scale));
  const ph = Math.max(1, Math.round(h * scale));
  for (const l of layers) {
    if (l.opacity <= 0 || l.world.length !== 4) continue;
    const q = screenQuad(l.world, cam, vw, vh).map(([x, y]) => [x + ox, y + oy]) as Quad;
    const box = clipBox(
      q.map(([x, y]) => [x * scale, y * scale]) as Quad,
      pw,
      ph,
    );
    if (!box) continue;
    scratch ??= canvas(pw, ph);
    if (!scratch) return drawn;
    if (scratch.width < pw || scratch.height < ph) {
      scratch.width = Math.max(scratch.width, pw);
      scratch.height = Math.max(scratch.height, ph);
    }
    const s = scratch.getContext('2d');
    if (!s) return drawn;
    s.setTransform(1, 0, 0, 1, 0, 0);
    s.clearRect(box.x, box.y, box.w, box.h);
    s.save();
    s.beginPath();
    s.rect(box.x, box.y, box.w, box.h);
    s.clip();
    s.setTransform(scale, 0, 0, scale, 0, 0);
    s.imageSmoothingQuality = 'high';
    drawWarped(s as WarpCtx, l.img, l.iw, l.ih, q);
    s.restore();
    g.save();
    g.globalAlpha = l.opacity;
    g.drawImage(scratch, box.x, box.y, box.w, box.h, box.x / scale, box.y / scale, box.w / scale, box.h / scale);
    g.restore();
    drawn++;
  }
  return drawn;
}

/** The plane of one Map view. */
export class LayerPlane {
  private canvas: Canvas2D | null = null;
  private ref: { cam: Camera; w: number; h: number; scale: number; key: string } | null = null;
  /** How many times the plane was drawn again (for tests and the frame-time measure). */
  renders = 0;

  /** Lets go of the plane's memory (the Map is closed, or no map is shown). */
  release(): void {
    if (this.canvas) {
      this.canvas.width = 0;
      this.canvas.height = 0;
    }
    this.canvas = null;
    this.ref = null;
  }

  /**
   * Lays the layers on `main` (a vw x vh view in CSS pixels, at `scale`). While `moving`, the plane drawn for an earlier
   * view of the same maps is shifted and scaled; otherwise it is drawn again when the view or the maps changed.
   */
  draw(main: CanvasRenderingContext2D, layers: PlaneLayer[], cam: Camera, vw: number, vh: number, scale: number, moving: boolean): void {
    if (layers.length === 0) {
      this.release();
      return;
    }
    const key = planeKey(layers);
    const ref = this.ref;
    const same = !!ref && ref.key === key && ref.w === vw && ref.h === vh && ref.scale === scale;
    const still = same && ref.cam.cx === cam.cx && ref.cam.cy === cam.cy && ref.cam.zoom === cam.zoom;
    const stretch = ref ? 2 ** (cam.zoom - ref.cam.zoom) : 1;
    const reuse = same && (still || (moving && stretch <= MAX_STRETCH && stretch >= 1 / MAX_STRETCH));
    if (!reuse || !this.canvas || !ref) this.render(layers, cam, vw, vh, scale, key);
    const r = this.ref;
    const c = this.canvas;
    if (!r || !c) return;
    const mx = r.w * PLANE_MARGIN;
    const my = r.h * PLANE_MARGIN;
    // Where the plane's top left lies in the current view, and how big it is now.
    const corner = fromScreen(r.cam, r.w, r.h, -mx, -my);
    const [sx, sy] = toScreen(cam, vw, vh, corner.x, corner.y);
    const k = 2 ** (cam.zoom - r.cam.zoom);
    main.save();
    main.globalAlpha = 1;
    main.drawImage(c, sx, sy, (r.w + 2 * mx) * k, (r.h + 2 * my) * k);
    main.restore();
  }

  private render(layers: PlaneLayer[], cam: Camera, vw: number, vh: number, scale: number, key: string): void {
    const mx = vw * PLANE_MARGIN;
    const my = vh * PLANE_MARGIN;
    const w = vw + 2 * mx;
    const h = vh + 2 * my;
    const pw = Math.max(1, Math.round(w * scale));
    const ph = Math.max(1, Math.round(h * scale));
    this.canvas ??= canvas(pw, ph);
    const c = this.canvas;
    if (!c) return;
    if (c.width !== pw || c.height !== ph) {
      c.width = pw;
      c.height = ph;
    }
    const g = c.getContext('2d');
    if (!g) return;
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.clearRect(0, 0, pw, ph);
    g.setTransform(scale, 0, 0, scale, 0, 0);
    paintLayers(g, layers, cam, vw, vh, mx, my, w, h, scale);
    this.ref = { cam: { ...cam }, w: vw, h: vh, scale, key };
    this.renders++;
  }
}
