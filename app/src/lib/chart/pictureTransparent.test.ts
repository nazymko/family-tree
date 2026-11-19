import { afterEach, describe, expect, it, vi } from 'vitest';
import { makeFakeContext } from '../../test-setup';
import { planPicture } from './picture';
import { renderPng, renderPngAdaptive, renderSvg, type BuiltPicture } from './pictureExport';

const PNG_HEAD = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
type Call = { op: string; args: unknown[] };

function built(over: { w?: number; h?: number } = {}): BuiltPicture {
  const scene = { nodes: [], edges: [] };
  const plan = planPicture('screen', scene, { cam: { x: 0, y: 0, zoom: 1 }, size: { w: over.w ?? 800, h: over.h ?? 600 }, dpr: 1 });
  return { plan, theme: { paper: '#fbfbf8', ink2: '#626a79', font: 'sans-serif' }, caption: 'Chart', paint: vi.fn(), transparent: true };
}

/** A canvas whose top-left pixel can be read: a fill over it makes it opaque, a clear makes it clear again. */
function pixelContext(works: boolean) {
  const base = makeFakeContext();
  const calls = (base as unknown as { calls: Call[] }).calls;
  let alpha = 0;
  const own: Record<string, unknown> = {
    fillRect: (x: number, y: number, w: number, h: number) => {
      calls.push({ op: 'fillRect', args: [x, y, w, h] });
      if (works && x <= 0 && y <= 0 && w >= 1 && h >= 1) alpha = 255;
    },
    clearRect: () => {
      if (works) alpha = 0;
    },
    getImageData: () => ({ data: new Uint8ClampedArray([0, 0, 0, alpha]) }),
  };
  const store = base as unknown as Record<string, unknown>;
  const ctx = new Proxy(store, {
    get: (t, p: string) => (p in own ? own[p] : t[p]),
    set: (t, p: string, v) => {
      t[p] = v;
      return true;
    },
  });
  return { ctx: ctx as unknown as CanvasRenderingContext2D, calls, alpha: () => alpha };
}

function stubPixels(works = true) {
  const made: ReturnType<typeof pixelContext>[] = [];
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(function () {
    const p = pixelContext(works);
    made.push(p);
    return p.ctx as never;
  } as never);
  vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(function (cb: BlobCallback) {
    const bytes = new Uint8Array([...PNG_HEAD, 9]);
    cb(Object.assign(new Blob([bytes], { type: 'image/png' }), { arrayBuffer: async () => bytes.buffer }));
  } as never);
  return made;
}

afterEach(() => vi.restoreAllMocks());

describe('transparent pictures', () => {
  it('leaves_an_empty_corner_clear_and_paints_no_background', async () => {
    const made = stubPixels();
    await renderPng(built());
    expect(made).toHaveLength(1);
    // No background fill over the whole picture, and the probe left the corner at alpha 0.
    expect(made[0].calls.some((c) => c.op === 'fillRect' && (c.args[2] as number) >= 800)).toBe(false);
    expect(made[0].alpha()).toBe(0);
  });

  it('does_not_retry_a_working_canvas_but_still_catches_one_that_stayed_empty', async () => {
    const good = stubPixels();
    await renderPngAdaptive(built());
    expect(good).toHaveLength(1);
    vi.restoreAllMocks();
    const dead = stubPixels(false);
    await expect(renderPng(built())).rejects.toThrow();
    expect(dead).toHaveLength(1);
  });

  it('keeps_drawn_content_in_the_corner', async () => {
    const made = stubPixels();
    const b = built();
    b.paint = vi.fn((ctx: CanvasRenderingContext2D) => ctx.fillRect(0, 0, 10, 10));
    await renderPng(b);
    expect(made[0].alpha()).toBe(255);
  });

  it('an_opaque_picture_still_gets_its_background', async () => {
    const made = stubPixels();
    await renderPng({ ...built(), transparent: false });
    expect(made[0].calls.some((c) => c.op === 'fillRect' && (c.args[2] as number) === 800)).toBe(true);
  });

  it('has_no_background_rect_in_the_svg', () => {
    const plan = planPicture('screen', { nodes: [], edges: [] }, { cam: { x: 0, y: 0, zoom: 1 }, size: { w: 200, h: 100 }, dpr: 1 }, [], { vector: true });
    const text = new TextDecoder().decode(renderSvg({ ...built(), plan }));
    expect(text).not.toContain('<rect width="200" height="130"');
    expect(text).not.toContain('#fbfbf8');
    expect(text).toContain('<svg x="0" y="0" width="200" height="100"');
    const opaque = new TextDecoder().decode(renderSvg({ ...built(), plan, transparent: false }));
    expect(opaque).toContain('<rect width="200" height="130" fill="#fbfbf8"/>');
  });

  it('a_picture_without_a_caption_has_no_strip', () => {
    const scene = { nodes: [], edges: [] };
    const screen = { cam: { x: 0, y: 0, zoom: 1 }, size: { w: 200, h: 100 }, dpr: 1 };
    expect(planPicture('screen', scene, screen, [], { vector: true, caption: false }).height).toBe(100);
    expect(planPicture('screen', scene, screen, [], { vector: true }).height).toBe(130);
  });
});
