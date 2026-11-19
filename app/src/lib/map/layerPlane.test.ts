// The plane of old maps: drawn once, only moved while the view moves, drawn again when the view rests or the maps
// change. jsdom has no canvas, so a recording stand-in counts what is drawn. Invented corners only.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LayerPlane, clipBox, planeKey, type PlaneLayer } from './layerPlane';
import { cornersToWorld } from './layers';
import { mercator, type Camera } from './projection';

let draws = 0;
function fakeContext(): CanvasRenderingContext2D {
  const noop = () => {};
  return new Proxy({} as Record<string, unknown>, {
    get(target, prop) {
      if (prop === 'drawImage') return () => draws++;
      if (prop in target) return target[prop as string];
      return noop;
    },
    set(target, prop, value) {
      target[prop as string] = value;
      return true;
    },
  }) as unknown as CanvasRenderingContext2D;
}

beforeEach(() => {
  draws = 0;
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(() => fakeContext() as never);
});
afterEach(() => vi.restoreAllMocks());

const img = document.createElement('canvas');
function layer(lat: number, lon: number, opacity = 0.7): PlaneLayer {
  const c = (la: number, lo: number) => ({ latE6: la * 1e6, lonE6: lo * 1e6 });
  return { world: cornersToWorld([c(lat, lon), c(lat, lon + 1), c(lat - 1, lon + 1), c(lat - 1, lon)]), img, iw: 100, ih: 80, opacity };
}
const at = (zoom: number, dx = 0): Camera => {
  const w = mercator(50, 30);
  return { cx: w.x + dx, cy: w.y, zoom };
};

describe('old map plane', () => {
  it('is_moved_not_drawn_again_while_the_view_moves_and_drawn_again_once_it_rests', () => {
    const plane = new LayerPlane();
    const main = fakeContext();
    const layers = [layer(50.5, 29.5), layer(50.5, 30.5)];
    plane.draw(main, layers, at(7), 800, 600, 1, false);
    expect(plane.renders).toBe(1);
    for (let i = 1; i <= 20; i++) plane.draw(main, layers, at(7, i * 1e-5), 800, 600, 1, true);
    expect(plane.renders).toBe(1);
    // Resting at the new place: drawn again, once.
    plane.draw(main, layers, at(7, 20e-5), 800, 600, 1, false);
    plane.draw(main, layers, at(7, 20e-5), 800, 600, 1, false);
    expect(plane.renders).toBe(2);
  });

  it('is_drawn_again_when_the_maps_change_or_the_zoom_moves_too_far', () => {
    const plane = new LayerPlane();
    const main = fakeContext();
    const layers = [layer(50.5, 29.5)];
    plane.draw(main, layers, at(7), 800, 600, 1, false);
    // Another opacity is another plane, even while moving.
    plane.draw(main, [layer(50.5, 29.5, 0.4)], at(7), 800, 600, 1, true);
    expect(plane.renders).toBe(2);
    plane.draw(main, [layer(50.5, 29.5, 0.4)], at(7.5), 800, 600, 1, true);
    expect(plane.renders).toBe(2);
    plane.draw(main, [layer(50.5, 29.5, 0.4)], at(8.5), 800, 600, 1, true);
    expect(plane.renders).toBe(3);
  });

  it('paints_only_maps_that_reach_the_plane', () => {
    const plane = new LayerPlane();
    const main = fakeContext();
    // One map in view, one on the other side of the world: the far one is never stretched.
    plane.draw(main, [layer(50.5, 29.5), layer(-30, -120)], at(7), 800, 600, 1, false);
    // One drawImage stretches the near map (a parallelogram needs one), one lays it on the plane, one lays the plane on the view.
    expect(draws).toBe(3);
    expect(planeKey([layer(50, 30)])).toBe(planeKey([layer(50, 30)]));
    expect(planeKey([layer(50, 30)])).not.toBe(planeKey([layer(50, 30.001)]));
    expect(clipBox([[-50, -50], [-10, -50], [-10, -10], [-50, -10]], 100, 100)).toBeNull();
    expect(clipBox([[10.2, 20.7], [50, 20], [50, 60], [10, 60]], 40, 100)).toEqual({ x: 9, y: 19, w: 31, h: 42 });
  });
});
